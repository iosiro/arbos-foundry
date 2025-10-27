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