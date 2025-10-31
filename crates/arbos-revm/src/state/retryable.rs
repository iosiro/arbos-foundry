use revm::primitives::{B256, U256};

use crate::{
    ArbitrumContextTr,
    state::types::{
        StorageBackedAddress, StorageBackedBytes, StorageBackedQueue, StorageBackedU64,
        StorageBackedU256, map_address, substorage,
    },
};

const ARBOS_STATE_RETRYABLE_TIMEOUT_QUEUE_KEY: &[u8] = &[0];
const ARBOS_STATE_RETRYABLE_CALLDATA_KEY: &[u8] = &[1];

pub struct RetryableState<'a, CTX>(&'a mut CTX, B256)
where
    CTX: ArbitrumContextTr;

impl<'a, CTX: ArbitrumContextTr> RetryableState<'a, CTX> {
    pub fn new(context: &'a mut CTX, subkey: B256) -> Self {
        Self(context, subkey)
    }

    pub fn timeout_queue(&mut self) -> StorageBackedQueue<'_, CTX> {
        let slot = substorage(&self.1, ARBOS_STATE_RETRYABLE_TIMEOUT_QUEUE_KEY);
        StorageBackedQueue::new(self.0, slot)
    }

    pub fn retryable(&mut self, id: B256) -> Retryable<'_, CTX> {
        let slot = substorage(&self.1, id.as_slice());
        Retryable::new(self.0, slot)
    }
}

pub struct Retryable<'a, CTX>(&'a mut CTX, B256)
where
    CTX: ArbitrumContextTr;

impl<'a, CTX: ArbitrumContextTr> Retryable<'a, CTX> {
    pub fn new(context: &'a mut CTX, subkey: B256) -> Self {
        Self(context, subkey)
    }

    pub fn num_tries(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = map_address(&self.1, &B256::from(U256::from(0u64)));
        StorageBackedU64::new(self.0, slot)
    }

    pub fn from(&mut self) -> StorageBackedAddress<'_, CTX> {
        let slot = map_address(&self.1, &B256::from(U256::from(1u64)));
        StorageBackedAddress::new(self.0, slot)
    }

    pub fn to(&mut self) -> StorageBackedAddress<'_, CTX> {
        let slot = map_address(&self.1, &B256::from(U256::from(2u64)));
        StorageBackedAddress::new(self.0, slot)
    }

    pub fn callvalue(&mut self) -> StorageBackedU256<'_, CTX> {
        let slot = map_address(&self.1, &B256::from(U256::from(3u64)));
        StorageBackedU256::new(self.0, slot)
    }

    pub fn beneficiary(&mut self) -> StorageBackedAddress<'_, CTX> {
        let slot = map_address(&self.1, &B256::from(U256::from(4u64)));
        StorageBackedAddress::new(self.0, slot)
    }

    pub fn calldata(&mut self) -> StorageBackedBytes<'_, CTX> {
        let slot = substorage(&self.1, ARBOS_STATE_RETRYABLE_CALLDATA_KEY);
        StorageBackedBytes::new(self.0, slot)
    }

    pub fn timeout(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = map_address(&self.1, &B256::from(U256::from(5u64)));
        StorageBackedU64::new(self.0, slot)
    }

    pub fn timeout_windows_left(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = map_address(&self.1, &B256::from(U256::from(6u64)));
        StorageBackedU64::new(self.0, slot)
    }
}
