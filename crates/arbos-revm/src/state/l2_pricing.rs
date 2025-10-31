use revm::primitives::{B256, U256};

use crate::{
    ArbitrumContextTr,
    state::types::{StorageBackedU64, StorageBackedU256, map_address},
};

pub struct L2Pricing<'a, CTX>(&'a mut CTX, B256)
where
    CTX: ArbitrumContextTr;

impl<'a, CTX: ArbitrumContextTr> L2Pricing<'a, CTX> {
    pub fn new(context: &'a mut CTX, subkey: B256) -> Self {
        Self(context, subkey)
    }

    pub fn speed_limit_per_second(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = map_address(&self.1, &B256::from(U256::from(0u64)));
        StorageBackedU64::new(self.0, slot)
    }

    pub fn per_block_gas_limit(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = map_address(&self.1, &B256::from(U256::from(1u64)));
        StorageBackedU64::new(self.0, slot)
    }

    pub fn base_fee_wei(&mut self) -> StorageBackedU256<'_, CTX> {
        let slot = map_address(&self.1, &B256::from(U256::from(2u64)));
        StorageBackedU256::new(self.0, slot)
    }

    pub fn min_base_fee_wei(&mut self) -> StorageBackedU256<'_, CTX> {
        let slot = map_address(&self.1, &B256::from(U256::from(3u64)));
        StorageBackedU256::new(self.0, slot)
    }

    pub fn gas_backlog(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = map_address(&self.1, &B256::from(U256::from(4u64)));
        StorageBackedU64::new(self.0, slot)
    }

    pub fn pricing_inertia(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = map_address(&self.1, &B256::from(U256::from(5u64)));
        StorageBackedU64::new(self.0, slot)
    }

    pub fn backlog_tolerance(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = map_address(&self.1, &B256::from(U256::from(6u64)));
        StorageBackedU64::new(self.0, slot)
    }

    pub fn per_tx_gas_limit(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = map_address(&self.1, &B256::from(U256::from(7u64)));
        StorageBackedU64::new(self.0, slot)
    }
}
