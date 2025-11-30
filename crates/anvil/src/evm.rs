use alloy_evm::{
    Database,
    precompiles::{DynPrecompile, PrecompileInput},
};
use core::ops::{Deref, DerefMut};
use foundry_evm::core::evm::{EthEvm, EthEvmContext, PrecompilesMap, TxEnv};
use revm::{
    DatabaseCommit, ExecuteEvm, InspectEvm, Inspector,
    context::result::{EVMError, ExecutionResult, HaltReason, ResultAndState},
    handler::PrecompileProvider,
    interpreter::InterpreterResult,
    precompile::Precompile,
};
use std::fmt::Debug;

/// Object-safe trait that enables injecting extra precompiles when using
/// `anvil` as a library.
pub trait PrecompileFactory: Send + Sync + Unpin + Debug {
    /// Returns a set of precompiles to extend the EVM with.
    fn precompiles(&self) -> Vec<(Precompile, u64)>;
}

/// Inject custom precompiles into the EVM dynamically.
pub fn inject_custom_precompiles<DB, I>(
    evm: &mut AnvilEvm<DB, I, PrecompilesMap<DB>>,
    precompiles: Vec<(Precompile, u64)>,
) where
    DB: Database,
    I: Inspector<EthEvmContext<DB>>,
{
    for (precompile, gas) in precompiles {
        let addr = *precompile.address();
        let func = *precompile.precompile();
        evm.precompiles_mut().apply_precompile(&addr, move |_| {
            Some(DynPrecompile::from(move |input: PrecompileInput<'_>| func(input.data, gas)))
        });
    }
}

pub struct AnvilEvm<DB: Database, I, P> {
    inner: EthEvm<DB, I, P>,
    inspect: bool,
}

impl<DB, I, P> Deref for AnvilEvm<DB, I, P>
where
    DB: Database,
{
    type Target = EthEvm<DB, I, P>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<DB, I, P> DerefMut for AnvilEvm<DB, I, P>
where
    DB: Database,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<DB, I, P> AnvilEvm<DB, I, P>
where
    DB: Database,
{
    pub fn new(evm: EthEvm<DB, I, P>, inspect: bool) -> Self {
        Self { inner: evm, inspect }
    }
}

impl<DB, I, PRECOMPILE> AnvilEvm<DB, I, PRECOMPILE>
where
    DB: Database,
    I: Inspector<EthEvmContext<DB>>,
    PRECOMPILE: PrecompileProvider<EthEvmContext<DB>, Output = InterpreterResult>,
{
    pub fn precompiles(&self) -> &PRECOMPILE {
        &self.inner.precompiles
    }

    pub fn precompiles_mut(&mut self) -> &mut PRECOMPILE {
        &mut self.inner.precompiles
    }

    pub fn transact(
        &mut self,
        tx: TxEnv,
    ) -> Result<ResultAndState<HaltReason>, EVMError<DB::Error>> {
        if self.inspect { self.inner.inspect_tx(tx) } else { self.inner.transact(tx) }
    }

    pub fn inspector(&self) -> &I {
        &self.inner.inspector
    }

    pub fn inspector_mut(&mut self) -> &mut I {
        &mut self.inner.inspector
    }
}

impl<DB, I, PRECOMPILE> AnvilEvm<DB, I, PRECOMPILE>
where
    DB: Database + DatabaseCommit,
    I: Inspector<EthEvmContext<DB>>,
    PRECOMPILE: PrecompileProvider<EthEvmContext<DB>, Output = InterpreterResult>,
{
    /// Executes a transaction and commits the state changes to the underlying database.
    pub fn transact_commit(
        &mut self,
        tx: TxEnv,
    ) -> Result<ExecutionResult<HaltReason>, EVMError<DB::Error>> {
        let ResultAndState { result, state } = self.transact(tx)?;
        self.ctx.journaled_state.database.commit(state);

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, convert::Infallible};

    use crate::{PrecompileFactory, evm::AnvilEvm, inject_custom_precompiles};
    use alloy_primitives::{Address, Bytes, TxKind, address};
    use arbos_revm::precompiles::ArbitrumPrecompileProvider;
    use foundry_evm::{
        EvmEnv,
        core::evm::{CfgEnv, EthEvm, EthEvmContext, LocalContext, PrecompilesMap, TxEnv},
    };

    use revm::{
        Journal,
        context::JournalTr,
        database::{EmptyDB, EmptyDBTyped},
        handler::instructions::EthInstructions,
        inspector::NoOpInspector,
        interpreter::interpreter::EthInterpreter,
        precompile::{Precompile, PrecompileId, PrecompileOutput, PrecompileResult},
        primitives::hardfork::SpecId,
    };

    // A precompile activated in the `Prague` spec.
    const ETH_PRAGUE_PRECOMPILE: Address = address!("0x0000000000000000000000000000000000000011");

    // A custom precompile address and payload for testing.
    const PRECOMPILE_ADDR: Address = address!("0x0000000000000000000000000000000000000044");
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

    pub type TestEvm =
        AnvilEvm<EmptyDBTyped<Infallible>, NoOpInspector, PrecompilesMap<EmptyDBTyped<Infallible>>>;

    /// Creates a new EVM instance with the custom precompile factory.
    fn create_eth_evm(spec: SpecId) -> (foundry_evm::Env, TestEvm) {
        let eth_env = foundry_evm::Env {
            evm_env: EvmEnv { block_env: Default::default(), cfg_env: CfgEnv::new_with_spec(spec) },
            tx: TxEnv {
                kind: TxKind::Call(PRECOMPILE_ADDR),
                data: PAYLOAD.into(),
                ..Default::default()
            },
        };

        let eth_evm_context = EthEvmContext {
            journaled_state: Journal::new(EmptyDB::default()),
            block: eth_env.evm_env.block_env.clone(),
            cfg: eth_env.evm_env.cfg_env.clone(),
            tx: eth_env.tx.clone(),
            chain: (),
            local: LocalContext::default(),
            error: Ok(()),
        };

        let precompiles = ArbitrumPrecompileProvider::new(spec);

        let eth_evm = AnvilEvm::new(
            EthEvm::new_with_inspector(
                eth_evm_context,
                NoOpInspector,
                EthInstructions::<EthInterpreter, EthEvmContext<EmptyDB>>::default(),
                PrecompilesMap::new(precompiles),
            ),
            true,
        );

        (eth_env, eth_evm)
    }

    #[test]
    fn build_eth_evm_with_extra_precompiles_default_spec() {
        let (env, mut evm) = create_eth_evm(SpecId::default());

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
        let (env, mut evm) = create_eth_evm(SpecId::LONDON);

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
