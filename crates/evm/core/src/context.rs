//! Foundry EVM context type aliases.
//!
//! This module provides type aliases for EVM context types, allowing for future
//! flexibility in customizing block, transaction, and configuration environments.

use arbos_revm::{
    ArbitrumContext, config::ArbitrumConfig, local_context::ArbitrumLocalContext,
    transaction::ArbitrumTransaction,
};
use revm::{context::BlockEnv, primitives::hardfork::SpecId};

/// Foundry's block environment type.
pub type FoundryBlockEnv = BlockEnv;

/// Foundry's transaction environment type.
pub type FoundryTxEnv = ArbitrumTransaction;

/// Foundry's configuration environment type.
pub type FoundryCfgEnv<Spec = SpecId> = ArbitrumConfig<Spec>;

/// Foundry's local context type.
pub type FoundryLocalContext = ArbitrumLocalContext;

/// Foundry's EVM context type.
///
/// This uses `ArbitrumContext` from arbos-revm which extends the standard revm
/// Context with Arbitrum-specific functionality.
pub type FoundryContext<DB> = ArbitrumContext<DB>;
