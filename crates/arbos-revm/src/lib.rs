//! # arbos-revm
//!
//! This crate provides the Arbitrum EVM implementation
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
mod buffer;

// pub mod api;
pub mod chain;
pub mod constants;
pub mod context;
pub mod evm;
pub mod handler;
pub mod inspector;
pub mod precompiles;
pub mod result;
//pub mod spec;
pub mod stylus_api;
pub mod stylus_executor;
pub mod stylus_state;
pub mod transaction;

pub use evm::ArbitrumEvm;
pub use result::ArbitrumHaltReason;

//pub use precompiles::ArbitrumPrecompiles;
//pub use spec::*;
pub use context::{ArbitrumContext, ArbitrumContextTr};
