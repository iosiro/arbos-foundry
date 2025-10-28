//! Configuration types for EVM environment.

use alloy_primitives::U256;
use revm::context::Cfg;

use crate::BlockSetter;

/// Container type that holds both the configuration and block environment for EVM execution.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EvmEnv<BLOCK, CONFIG> {
    /// The configuration environment with handler settings
    pub cfg_env: CONFIG,
    /// The block environment containing block-specific data
    pub block_env: BLOCK,
}

impl<BLOCK, CONFIG> EvmEnv<BLOCK, CONFIG>
where
    BLOCK: BlockSetter,
    CONFIG: Cfg,
{
    /// Create a new `EvmEnv` from its components.
    ///
    /// # Arguments
    ///
    /// * `cfg_env_with_handler_cfg` - The configuration environment with handler settings
    /// * `block` - The block environment containing block-specific data
    pub const fn new(cfg_env: CONFIG, block_env: BLOCK) -> Self {
        Self { cfg_env, block_env }
    }

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

    /// Overrides the configured block number
    pub fn with_block_number(mut self, number: U256) -> Self {
        self.block_env.set_number(number);
        self
    }

    /// Convenience function that overrides the configured block number with the given
    /// `Some(number)`.
    ///
    /// This is intended for block overrides.
    pub fn with_block_number_opt(mut self, number: Option<U256>) -> Self {
        if let Some(number) = number {
            self.block_env.set_number(number);
        }
        self
    }

    /// Sets the block number if provided.
    pub fn set_block_number_opt(&mut self, number: Option<U256>) -> &mut Self {
        if let Some(number) = number {
            self.block_env.set_number(number);
        }
        self
    }

    /// Overrides the configured block timestamp.
    pub fn with_timestamp(mut self, timestamp: U256) -> Self {
        self.block_env.set_timestamp(timestamp);
        self
    }

    /// Convenience function that overrides the configured block timestamp with the given
    /// `Some(timestamp)`.
    ///
    /// This is intended for block overrides.
    pub fn with_timestamp_opt(mut self, timestamp: Option<U256>) -> Self {
        if let Some(timestamp) = timestamp {
            self.block_env.set_timestamp(timestamp);
        }
        self
    }

    /// Sets the block timestamp if provided.
    pub fn set_timestamp_opt(&mut self, timestamp: Option<U256>) -> &mut Self {
        if let Some(timestamp) = timestamp {
            self.block_env.set_timestamp(timestamp);
        }
        self
    }

    /// Overrides the configured block base fee.
    pub fn with_base_fee(mut self, base_fee: u64) -> Self {
        self.block_env.set_basefee(base_fee);
        self
    }

    /// Convenience function that overrides the configured block base fee with the given
    /// `Some(base_fee)`.
    ///
    /// This is intended for block overrides.
    pub fn with_base_fee_opt(mut self, base_fee: Option<u64>) -> Self {
        if let Some(base_fee) = base_fee {
            self.block_env.set_basefee(base_fee);
        }
        self
    }

    /// Sets the block base fee if provided.
    pub fn set_base_fee_opt(&mut self, base_fee: Option<u64>) -> &mut Self {
        if let Some(base_fee) = base_fee {
            self.block_env.set_basefee(base_fee);
        }
        self
    }
}

impl<BLOCK, CONFIG> From<(CONFIG, BLOCK)> for EvmEnv<BLOCK, CONFIG> {
    fn from((cfg_env, block_env): (CONFIG, BLOCK)) -> Self {
        Self { cfg_env, block_env }
    }
}
