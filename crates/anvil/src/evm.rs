use std::fmt::Debug;

use foundry_evm_core::{
    evm::{EthEvm, EthEvmContext},
    precompiles::{DynPrecompile, PrecompilesMap},
};
use revm::{precompile::PrecompileWithAddress, Database, Inspector};

/// Object-safe trait that enables injecting extra precompiles when using
/// `anvil` as a library.
pub trait PrecompileFactory: Send + Sync + Unpin + Debug {
    /// Returns a set of precompiles to extend the EVM with.
    fn precompiles(&self) -> Vec<PrecompileWithAddress>;
}

/// Inject precompiles into the EVM dynamically.
pub fn inject_precompiles<DB, I>(
    evm: &mut EthEvm<DB, I, PrecompilesMap>,
    precompiles: Vec<PrecompileWithAddress>,
) where
    DB: Database,
    I: Inspector<EthEvmContext<DB>>,
{
    for p in precompiles {
        evm.precompiles
            .apply_precompile(p.address(), |_| Some(DynPrecompile::from(*p.precompile())));
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{address, Address, Bytes, TxKind};
    use arbos_revm::{ArbitrumCfgEnv, ArbitrumLocalContext as LocalContext, ArbitrumTransaction, ArbitrumSpecId as SpecId};
    use foundry_evm::EvmEnv;
    use foundry_evm_core::precompiles::PrecompilesMap;
    use itertools::Itertools;
    use revm::{
        context::{JournalTr, TxEnv},
        database::{EmptyDB, EmptyDBTyped},
        handler::{instructions::EthInstructions, EthPrecompiles},
        inspector::NoOpInspector,
        interpreter::interpreter::EthInterpreter,
        precompile::{
            PrecompileOutput, PrecompileResult, PrecompileSpecId, PrecompileWithAddress,
            Precompiles,
        },
        ExecuteEvm, Journal,
    };
    use std::convert::Infallible;

    use crate::{
        evm::{EthEvm, EthEvmContext},
        inject_precompiles, PrecompileFactory,
    };

    // A precompile activated in the `Prague` spec.
    const ETH_PRAGUE_PRECOMPILE: Address = address!("0x0000000000000000000000000000000000000011");

    // A custom precompile address and payload for testing.
    const PRECOMPILE_ADDR: Address = address!("0x0000000000000000000000000000000000000071");
    const PAYLOAD: &[u8] = &[0xde, 0xad, 0xbe, 0xef];

    #[derive(Debug)]
    struct CustomPrecompileFactory;

    impl PrecompileFactory for CustomPrecompileFactory {
        fn precompiles(&self) -> Vec<PrecompileWithAddress> {
            vec![PrecompileWithAddress::from((
                PRECOMPILE_ADDR,
                custom_echo_precompile as fn(&[u8], u64) -> PrecompileResult,
            ))]
        }
    }

    /// Custom precompile that echoes the input data.
    /// In this example it uses `0xdeadbeef` as the input data, returning it as output.
    fn custom_echo_precompile(input: &[u8], _gas_limit: u64) -> PrecompileResult {
        Ok(PrecompileOutput { bytes: Bytes::copy_from_slice(input), gas_used: 0 })
    }

    /// Creates a new EVM instance with the custom precompile factory.
    fn create_eth_evm(
        spec: SpecId,
    ) -> (foundry_evm::Env, EthEvm<EmptyDBTyped<Infallible>, NoOpInspector, PrecompilesMap>) {
        let eth_env = foundry_evm::Env {
            evm_env: EvmEnv {
                block_env: Default::default(),
                cfg_env: ArbitrumCfgEnv::new_with_spec(spec),
            },
            tx: ArbitrumTransaction {
                base: TxEnv {
                    kind: TxKind::Call(PRECOMPILE_ADDR),
                    data: PAYLOAD.into(),
                    ..Default::default()
                },
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
            precompiles: Precompiles::new(PrecompileSpecId::from_spec_id(spec.into_eth_spec())),
            spec: spec.into_eth_spec(),
        }
        .precompiles;
        let eth_evm = EthEvm::new_with_inspector(
            eth_evm_context,
            NoOpInspector,
            EthInstructions::<EthInterpreter, EthEvmContext<EmptyDB>>::default(),
            PrecompilesMap::from_static(eth_precompiles),
        );

        (eth_env, eth_evm)
    }

    #[test]
    fn build_eth_evm_with_extra_precompiles_default_spec() {
        let (env, mut evm) = create_eth_evm(SpecId::default());

        // Check that the Prague precompile IS present when using the default spec.
        assert!(evm.precompiles.addresses().contains(&ETH_PRAGUE_PRECOMPILE));

        assert!(!evm.precompiles.addresses().contains(&PRECOMPILE_ADDR));

        inject_precompiles(&mut evm, CustomPrecompileFactory.precompiles());

        assert!(evm.precompiles.addresses().contains(&PRECOMPILE_ADDR));

        let result = evm.transact(env.tx).unwrap();

        assert!(result.result.is_success());
        assert_eq!(result.result.output(), Some(&PAYLOAD.into()));
    }

    #[test]
    fn build_eth_evm_with_extra_precompiles_london_spec() {
        let (env, mut evm) = create_eth_evm(SpecId::ArbosStylusChargingFixes);

        // Check that the Prague precompile IS NOT present when using the London spec.
        assert!(!evm.precompiles.addresses().contains(&ETH_PRAGUE_PRECOMPILE));

        assert!(!evm.precompiles.addresses().contains(&PRECOMPILE_ADDR));

        inject_precompiles(&mut evm, CustomPrecompileFactory.precompiles());

        assert!(evm.precompiles.addresses().contains(&PRECOMPILE_ADDR));

        let result = evm.transact(env.tx).unwrap();

        assert!(result.result.is_success());
        assert_eq!(result.result.output(), Some(&PAYLOAD.into()));
    }
}
