//! Contains trait [`DefaultOp`] used to create a default context.
use revm::{
    context::{BlockEnv, CfgEnv, TxEnv},
    database_interface::EmptyDB,
    Context, Journal, MainContext,
};

use crate::chain_config::ArbitrumChainInfo;

/// Type alias for the default context type of the ArbitrumEvm.
pub type ArbitrumContext<DB> = Context<BlockEnv, TxEnv, CfgEnv, DB, Journal<DB>, ArbitrumChainInfo>;

/// Trait that allows for a default context to be created.
pub trait DefaultArbitrum {
    /// Create a default context.
    fn arbitrum() -> ArbitrumContext<EmptyDB>;
}

impl DefaultArbitrum for ArbitrumContext<EmptyDB> {
    fn arbitrum() -> Self {
        Context::mainnet().with_chain(ArbitrumChainInfo::default())
    }
}
