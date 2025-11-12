use alloy_evm::{Database, Evm, EvmEnv, precompiles::{DynPrecompile, PrecompileInput, PrecompilesMap}};
use alloy_primitives::{Address, Bytes};
use arbos_revm::{ArbitrumContext, ArbitrumEvm, config::ArbitrumConfig, precompiles::ArbitrumPrecompiles};
use revm::{ExecuteEvm, InspectEvm, Inspector, SystemCallEvm, context::{BlockEnv, TxEnv, result::{EVMError, HaltReason, ResultAndState}}, handler::PrecompileProvider, precompile::Precompile, primitives::hardfork::SpecId};
use std::fmt::Debug;

pub type AnvilEvmContext<DB> = ArbitrumContext<DB>;

pub struct AnvilEvm<DB: Database, I, P = PrecompilesMap<AnvilEvmContext<DB>, ArbitrumPrecompiles<AnvilEvmContext<DB>>>> {
    pub inner: ArbitrumEvm<AnvilEvmContext<DB>, I, P>,
    pub inspect: bool,
}

impl<DB, I, PRECOMPILE> Evm for AnvilEvm<DB, I, PRECOMPILE>
where
    DB: Database,
    I: Inspector<AnvilEvmContext<DB>>,
    PRECOMPILE: PrecompileProvider<AnvilEvmContext<DB>, Output = revm::interpreter::InterpreterResult>,
{
    type DB = DB;
    type Block = BlockEnv;
    type Config = ArbitrumConfig;
    type Tx = TxEnv;
    type Error = EVMError<DB::Error>;
    type HaltReason = HaltReason;
    type Spec = SpecId;
    type Precompiles = PRECOMPILE;
    type Inspector = I;

    fn block(&self) -> &Self::Block {
        &self.inner.0.block
    }

    fn chain_id(&self) -> u64 {
        self.inner.0.cfg.chain_id
    }

    fn transact_raw(
        &mut self,
        tx: Self::Tx,
    ) -> Result<ResultAndState<Self::HaltReason>, Self::Error> {
        if self.inspect {
            self.inner.0.inspect_tx(tx)
        } else {
            self.inner.0.transact(tx)
        }
    }

    fn transact_system_call(
        &mut self,
        caller: Address,
        contract: Address,
        data: Bytes,
    ) -> Result<ResultAndState<Self::HaltReason>, Self::Error> {
        self.inner.0.system_call_with_caller(caller, contract, data)
    }

    fn finish(self) -> (Self::DB, EvmEnv<Self::Block, Self::Config>) {
        let AnvilEvmContext { block: block_env, cfg: cfg_env, journaled_state, .. } = self.inner.0.ctx;

        (journaled_state.database, EvmEnv { block_env, cfg_env })
    }

    fn set_inspector_enabled(&mut self, enabled: bool) {
        self.inspect = enabled;
    }

    fn components(&self) -> (&Self::DB, &Self::Inspector, &Self::Precompiles) {
        (&self.inner.0.ctx.journaled_state.database, &self.inner.0.inspector, &self.inner.0.precompiles)
    }

    fn components_mut(&mut self) -> (&mut Self::DB, &mut Self::Inspector, &mut Self::Precompiles) {
        (
            &mut self.inner.0.ctx.journaled_state.database,
            &mut self.inner.0.inspector,
            &mut self.inner.0.precompiles,
        )
    }
}


/// Object-safe trait that enables injecting extra precompiles when using
/// `anvil` as a library.
pub trait PrecompileFactory: Send + Sync + Unpin + Debug {
    /// Returns a set of precompiles to extend the EVM with.
    fn precompiles(&self) -> Vec<(Precompile, u64)>;
}

/// Inject custom precompiles into the EVM dynamically.
pub fn inject_custom_precompiles<DB, I>(
    evm: &mut AnvilEvm<DB, I>,
    precompiles: Vec<(Precompile, u64)>,
) where
    DB: Database,
    I: Inspector<AnvilEvmContext<DB>>,
{
    for (precompile, gas) in precompiles {
        let addr = *precompile.address();
        let func = *precompile.precompile();
        evm.precompiles_mut().apply_precompile(&addr, move |_| {
            Some(DynPrecompile::from(move |input: PrecompileInput<'_>| func(input.data, gas)))
        });
    }
}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, convert::Infallible};

    use crate::{PrecompileFactory, evm::{AnvilEvm, AnvilEvmContext}, inject_custom_precompiles};
    use alloy_evm::{Evm, EvmEnv, precompiles::PrecompilesMap};
    use alloy_primitives::{Address, Bytes, TxKind, address};
    use arbos_revm::{ArbitrumEvm, config::ArbitrumConfig, local_context::ArbitrumLocalContext, precompiles::ArbitrumPrecompiles};
    use revm::{
        Journal,
        context::{JournalTr, TxEnv},
        database::{EmptyDB, EmptyDBTyped},
        handler::{instructions::EthInstructions},
        inspector::NoOpInspector,
        interpreter::interpreter::EthInterpreter,
        precompile::{
            Precompile, PrecompileId, PrecompileOutput, PrecompileResult,
        },
        primitives::hardfork::SpecId,
    };

    // A precompile activated in the `Prague` spec.
    const ETH_PRAGUE_PRECOMPILE: Address = address!("0x0000000000000000000000000000000000000011");

    // A custom precompile address and payload for testing.
    const PRECOMPILE_ADDR: Address = address!("0x0000000000000000000000000000000000000081");
    const PAYLOAD: &[u8] = &[0xde, 0xad, 0xbe, 0xef];

    #[derive(Debug)]
    struct CustomPrecompileFactory;

    impl PrecompileFactory for CustomPrecompileFactory {
        fn precompiles(&self) -> Vec<(Precompile, u64)> {
            vec![(
                Precompile::from((
                    PrecompileId::Custom(Cow::Borrowed("custom_echo")),
                    PRECOMPILE_ADDR,
                    custom_echo_precompile as fn(&[u8], u64) -> PrecompileResult,
                )),
                1000,
            )]
        }
    }

    /// Custom precompile that echoes the input data.
    /// In this example it uses `0xdeadbeef` as the input data, returning it as output.
    fn custom_echo_precompile(input: &[u8], _gas_limit: u64) -> PrecompileResult {
        Ok(PrecompileOutput { bytes: Bytes::copy_from_slice(input), gas_used: 0, reverted: false })
    }

    /// Creates a new EVM instance with the custom precompile factory.
    fn create_evm(
        spec: SpecId,
    ) -> (
        foundry_evm::Env,
        AnvilEvm<
            EmptyDBTyped<Infallible>,
            NoOpInspector,
            PrecompilesMap<AnvilEvmContext<EmptyDBTyped<Infallible>>, ArbitrumPrecompiles<AnvilEvmContext<EmptyDBTyped<Infallible>>>>,
        >,
    ) {
        let eth_env = foundry_evm::Env {
            evm_env: EvmEnv { block_env: Default::default(), cfg_env: ArbitrumConfig::default() },
            tx: TxEnv {
                kind: TxKind::Call(PRECOMPILE_ADDR),
                data: PAYLOAD.into(),
                ..Default::default()
            },
        };

        let eth_evm_context = AnvilEvmContext {
            journaled_state: Journal::new(EmptyDB::default()),
            block: eth_env.evm_env.block_env.clone(),
            cfg: eth_env.evm_env.cfg_env.clone(),
            tx: eth_env.tx.clone(),
            chain: (),
            local: ArbitrumLocalContext::default(),
            error: Ok(()),
        };

        let eth_evm = AnvilEvm {
            inner: ArbitrumEvm::new_with_inspector(
                eth_evm_context,
                NoOpInspector,
                EthInstructions::<EthInterpreter, AnvilEvmContext<EmptyDB>>::default(),
                PrecompilesMap::new(ArbitrumPrecompiles::default()),
            ),
            inspect: true,
        };

        (eth_env, eth_evm)
    }

    #[test]
    fn build_eth_evm_with_extra_precompiles_default_spec() {
        let (env, mut evm) = create_evm(SpecId::default());

        // Check that the Prague precompile IS present when using the default spec.
        assert!(evm.precompiles().contains(&ETH_PRAGUE_PRECOMPILE));

        assert!(!evm.precompiles().contains(&PRECOMPILE_ADDR));

        inject_custom_precompiles(&mut evm, CustomPrecompileFactory.precompiles());

        assert!(evm.precompiles().contains(&PRECOMPILE_ADDR));

        let result = evm.transact(env.tx).unwrap();

        assert!(result.result.is_success());
        assert_eq!(result.result.output(), Some(&PAYLOAD.into()));
    }

    #[test]
    fn build_eth_evm_with_extra_precompiles_london_spec() {
        let (env, mut evm) = create_evm(SpecId::LONDON);

        // Check that the Prague precompile IS NOT present when using the London spec.
        assert!(!evm.precompiles().contains(&ETH_PRAGUE_PRECOMPILE));

        assert!(!evm.precompiles().contains(&PRECOMPILE_ADDR));

        inject_custom_precompiles(&mut evm, CustomPrecompileFactory.precompiles());

        assert!(evm.precompiles().contains(&PRECOMPILE_ADDR));

        let result = evm.transact(env.tx).unwrap();

        assert!(result.result.is_success());
        assert_eq!(result.result.output(), Some(&PAYLOAD.into()));
    }
}
