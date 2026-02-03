//! Foundry EVM context type aliases.
//!
//! This module provides type aliases for EVM context types, allowing for future
//! flexibility in customizing block, transaction, and configuration environments.

use revm::{
    Context,
    context::{BlockEnv, CfgEnv, TxEnv},
};

/// Foundry's block environment type.
pub type FoundryBlockEnv = BlockEnv;

/// Foundry's transaction environment type.
pub type FoundryTxEnv = TxEnv;

/// Foundry's configuration environment type.
pub type FoundryCfgEnv = CfgEnv;

/// Foundry's EVM context type.
///
/// This is equivalent to `alloy_evm::eth::EthEvmContext<DB>` but defined locally
/// to remove the dependency on `alloy-evm`.
pub type FoundryContext<DB> = Context<FoundryBlockEnv, FoundryTxEnv, FoundryCfgEnv, DB>;
