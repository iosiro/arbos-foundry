//! # foundry-evm-networks
//!
//! Foundry EVM network configuration.

use alloy_evm::precompiles::PrecompilesMap;
use alloy_primitives::{Address, map::AddressHashMap};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    pub fn inject_precompiles(self, _precompiles: &mut PrecompilesMap) {}

    /// Returns precompiles label for configured networks, to be used in traces.
    pub fn precompiles_label(self) -> AddressHashMap<String> {
        AddressHashMap::default()
    }

    /// Returns precompiles for configured networks.
    pub fn precompiles(self) -> BTreeMap<String, Address> {
        BTreeMap::new()
    }
}
