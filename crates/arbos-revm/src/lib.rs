//! # arbos-revm
//!
//! This crate provides the Arbitrum EVM implementation
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
mod buffer;

// pub mod api;
pub mod config;
pub mod constants;
pub mod context;
pub mod evm;
pub mod handler;
pub mod inspector;
pub mod local_context;
// pub mod macros;
pub mod result;
pub mod state;
pub mod stylus_api;
pub mod stylus_executor;
pub mod transaction;

pub use evm::ArbitrumEvm;
pub use result::ArbitrumHaltReason;

//pub use precompiles::ArbitrumPrecompiles;
//pub use spec::*;
pub use context::{ArbitrumContext, ArbitrumContextTr};
use revm::primitives::hex;

pub trait Utf8OrHex {
    fn from_utf8_or_hex(data: impl Into<Vec<u8>>) -> String;
}

impl Utf8OrHex for String {
    fn from_utf8_or_hex(data: impl Into<Vec<u8>>) -> String {
        match Self::from_utf8(data.into()) {
            Ok(string) => string,
            Err(error) => hex::encode(error.as_bytes()),
        }
    }
}
