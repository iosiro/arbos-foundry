#![cfg_attr(not(test), warn(unused_crate_dependencies))]
mod buffer;

pub mod api;
pub mod chain_config;
pub mod constants;
pub mod evm;
pub mod handler;
pub mod inspector;
pub mod precompiles;
pub mod result;
pub mod spec;
pub mod stylus;
pub mod stylus_api;
pub mod transaction;

pub use evm::ArbitrumEvm;
pub use result::ArbitrumHaltReason;

pub use precompiles::ArbitrumPrecompiles;
pub use spec::*;
pub use transaction::{ArbitrumTransaction, ArbitrumTransactionError};
