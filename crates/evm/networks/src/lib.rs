//! # foundry-evm-networks
//!
//! Foundry EVM network configuration.

use alloy_primitives::{Address, map::AddressHashMap};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// Re-export precompile types from foundry-evm-core when available, but define a minimal
// trait here to avoid circular dependencies.

/// A dynamic precompile that can be used with network configurations.
///
/// This is a type alias that matches `foundry_evm_core::precompiles::DynPrecompile`.
/// The actual type is defined in foundry-evm-core to avoid circular dependencies.
pub type DynPrecompile = std::sync::Arc<dyn DynPrecompileTrait>;

/// Minimal trait for dynamic precompiles used by network configurations.
///
/// This trait is implemented by `foundry_evm_core::precompiles::DynPrecompile`.
pub trait DynPrecompileTrait: Send + Sync + std::fmt::Debug {}

// Implement for all types that satisfy the bounds
impl<T: Send + Sync + std::fmt::Debug + ?Sized> DynPrecompileTrait for T {}

/// Trait for precompile providers that can be extended with dynamic precompiles.
pub trait ExtendablePrecompiles {
    /// The type of dynamic precompile used by this provider.
    type Precompile;

    /// Extends the precompiles with the given iterator of (address, precompile) pairs.
    fn extend<I>(&mut self, precompiles: I)
    where
        I: IntoIterator<Item = (Address, Self::Precompile)>;

    /// Inserts a precompile at the given address.
    fn insert_precompile(&mut self, address: Address, precompile: Self::Precompile);
}

#[derive(Clone, Debug, Default, Parser, Copy, Serialize, Deserialize, PartialEq)]
pub struct NetworkConfigs {}

impl NetworkConfigs {
    pub fn with_chain_id(self, _chain_id: u64) -> Self {
        self
    }

    pub fn bypass_prevrandao(&self, _chain_id: u64) -> bool {
        true
    }

    /// Inject precompiles for configured networks.
    pub fn inject_precompiles<P: ExtendablePrecompiles>(self, _precompiles: &mut P) {}

    /// Returns precompiles label for configured networks, to be used in traces.
    pub fn precompiles_label(self) -> AddressHashMap<String> {
        AddressHashMap::default()
    }

    /// Returns precompiles for configured networks.
    pub fn precompiles(self) -> BTreeMap<String, Address> {
        BTreeMap::new()
    }
}
