//! Concrete executor construction for fresh and restored Chisel sessions.

#[cfg(feature = "monad")]
use foundry_evm::core::evm::MonadEvmNetwork;
#[cfg(feature = "optimism")]
use foundry_evm::core::evm::OpEvmNetwork;
use foundry_evm::{
    core::evm::{ArbitrumEvmNetwork, EthEvmNetwork, FoundryEvmNetwork, TempoEvmNetwork},
    executors::ExecutorBuilder,
    opts::EvmOpts,
};

/// Reconstructs tooling and execution policy after a session's configuration is resolved.
pub trait ChiselEvmNetwork: FoundryEvmNetwork {
    /// Builds this concrete family's executor tooling from the session's saved or CLI options.
    fn executor_builder(opts: &EvmOpts) -> ExecutorBuilder<Self>;
}

impl ChiselEvmNetwork for EthEvmNetwork {
    fn executor_builder(_opts: &EvmOpts) -> ExecutorBuilder<Self> {
        ExecutorBuilder::<Self>::new()
    }
}

impl ChiselEvmNetwork for ArbitrumEvmNetwork {
    fn executor_builder(opts: &EvmOpts) -> ExecutorBuilder<Self> {
        ExecutorBuilder::<Self>::new().stylus_config(opts.stylus_config.clone())
    }
}

impl ChiselEvmNetwork for TempoEvmNetwork {
    fn executor_builder(_opts: &EvmOpts) -> ExecutorBuilder<Self> {
        ExecutorBuilder::<Self>::new()
    }
}

#[cfg(feature = "optimism")]
impl ChiselEvmNetwork for OpEvmNetwork {
    fn executor_builder(_opts: &EvmOpts) -> ExecutorBuilder<Self> {
        ExecutorBuilder::<Self>::new()
    }
}

#[cfg(feature = "monad")]
impl ChiselEvmNetwork for MonadEvmNetwork {
    fn executor_builder(_opts: &EvmOpts) -> ExecutorBuilder<Self> {
        ExecutorBuilder::<Self>::new()
    }
}
