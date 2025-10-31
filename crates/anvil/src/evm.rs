use alloy_evm::{
    Database, Evm,
    eth::EthEvmContext,
    precompiles::{DynPrecompile, PrecompileInput},
};

use foundry_evm::core::either_evm::EitherEvm;
use revm::{Inspector, precompile::Precompile};
use std::fmt::Debug;

/// Object-safe trait that enables injecting extra precompiles when using
/// `anvil` as a library.
pub trait PrecompileFactory: Send + Sync + Unpin + Debug {
    /// Returns a set of precompiles to extend the EVM with.
    fn precompiles(&self) -> Vec<(Precompile, u64)>;
}

/// Inject custom precompiles into the EVM dynamically.
pub fn inject_custom_precompiles<DB, I>(
    evm: &mut EitherEvm<DB, I>,
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

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, convert::Infallible};

    use crate::{PrecompileFactory, inject_custom_precompiles};
    use alloy_evm::{EthEvm, Evm, EvmEnv, eth::EthEvmContext};
    use alloy_primitives::{Address, Bytes, TxKind, address};
    use foundry_evm::core::precompiles_map::PrecompilesMap;
    use revm::{
        Journal,
        context::{CfgEnv, Evm as RevmEvm, JournalTr, LocalContext, TxEnv},
        database::{EmptyDB, EmptyDBTyped},
        handler::{EthPrecompiles, instructions::EthInstructions},
        inspector::NoOpInspector,
        interpreter::interpreter::EthInterpreter,
        precompile::{
            Precompile, PrecompileId, PrecompileOutput, PrecompileResult, PrecompileSpecId,
            Precompiles,
        },
        primitives::hardfork::SpecId,
    };

    // A precompile activated in the `Prague` spec.
    const ETH_PRAGUE_PRECOMPILE: Address = address!("0x0000000000000000000000000000000000000011");

    // A custom precompile address and payload for testing.
    const PRECOMPILE_ADDR: Address = address!("0x0000000000000000000000000000000000000071");
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
    fn create_eth_evm(
        spec: SpecId,
    ) -> (
        foundry_evm::Env,
        EthEvm<
            EmptyDBTyped<Infallible>,
            NoOpInspector,
            PrecompilesMap<EthEvmContext<EmptyDBTyped<Infallible>>, EthPrecompiles>,
        >,
    ) {
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

        let eth_precompiles = EthPrecompiles {
            precompiles: Precompiles::new(PrecompileSpecId::from_spec_id(spec)),
            spec,
        };

        let eth_evm = EthEvm::new(
            RevmEvm::new_with_inspector(
                eth_evm_context,
                NoOpInspector,
                EthInstructions::<EthInterpreter, EthEvmContext<EmptyDB>>::default(),
                PrecompilesMap::new(eth_precompiles),
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
