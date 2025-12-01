use core::{error::Error, fmt::Debug};

use revm::context::{Block, Cfg};

#[cfg(feature = "overrides")]
pub mod overrides;
pub mod precompiles;
mod traits;
pub use traits::EvmInternals;

/// Container type that holds both the configuration and block environment for EVM execution.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EvmEnv<BLOCK, CONFIG>
where
    BLOCK: Block + Default,
    CONFIG: Cfg + Default,
{
    /// The configuration environment with handler settings
    pub cfg_env: CONFIG,
    /// The block environment containing block-specific data
    pub block_env: BLOCK,
}

impl<BLOCK, CONFIG> EvmEnv<BLOCK, CONFIG>
where
    BLOCK: Block + Default,
    CONFIG: Cfg + Default,
{
    /// Returns a reference to the block environment.
    pub const fn block_env(&self) -> &BLOCK {
        &self.block_env
    }

    /// Returns a reference to the configuration environment.
    pub const fn cfg_env(&self) -> &CONFIG {
        &self.cfg_env
    }

    /// Returns the chain ID of the environment.
    pub fn chainid(&self) -> u64 {
        self.cfg_env.chain_id()
    }

    /// Returns the spec id of the chain
    pub fn spec_id(&self) -> CONFIG::Spec {
        self.cfg_env.spec()
    }
}

/// Helper trait to bound [`revm::Database::Error`] with common requirements.
pub trait Database: revm::Database<Error: Error + Send + Sync + 'static> + Debug {}
impl<T> Database for T where T: revm::Database<Error: Error + Send + Sync + 'static> + Debug {}
