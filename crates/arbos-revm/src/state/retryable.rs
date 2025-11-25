use revm::primitives::{Address, B256, Bytes, U256};

use crate::{
    ArbitrumContextTr,
    state::types::{
        ArbosStateError, StorageBackedAddress, StorageBackedBytes, StorageBackedQueue,
        StorageBackedTr, StorageBackedU64, StorageBackedU256, map_address, substorage,
    },
};

const ARBOS_STATE_RETRYABLE_TIMEOUT_QUEUE_KEY: &[u8] = &[0];
const ARBOS_STATE_RETRYABLE_CALLDATA_KEY: &[u8] = &[1];

pub struct RetryableState<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    context: &'a mut CTX,
    gas: Option<&'a mut revm::interpreter::Gas>,
    slot: B256,
}

impl<'a, CTX: ArbitrumContextTr> RetryableState<'a, CTX> {
    pub fn new(
        context: &'a mut CTX,
        gas: Option<&'a mut revm::interpreter::Gas>,
        subkey: B256,
    ) -> Self {
        Self { context, gas, slot: subkey }
    }

    pub fn timeout_queue(&mut self) -> StorageBackedQueue<'_, CTX> {
        let slot = substorage(&self.slot, ARBOS_STATE_RETRYABLE_TIMEOUT_QUEUE_KEY);
        StorageBackedQueue::new(self.context, self.gas.as_deref_mut(), slot)
    }

    pub fn retryable(&mut self, id: B256) -> Retryable<'_, CTX> {
        let slot = substorage(&self.slot, id.as_slice());
        Retryable::new(self.context, self.gas.as_deref_mut(), slot)
    }
}

pub struct Retryable<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    context: &'a mut CTX,
    gas: Option<&'a mut revm::interpreter::Gas>,
    slot: B256,
}

impl<'a, CTX: ArbitrumContextTr> Retryable<'a, CTX> {
    pub fn new(
        context: &'a mut CTX,
        gas: Option<&'a mut revm::interpreter::Gas>,
        subkey: B256,
    ) -> Self {
        Self { context, gas, slot: subkey }
    }

    pub fn num_tries(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = map_address(&self.slot, &B256::from(U256::from(0u64)));
        StorageBackedU64::new(self.context, self.gas.as_deref_mut(), slot)
    }

    pub fn from(&mut self) -> StorageBackedAddress<'_, CTX> {
        let slot = map_address(&self.slot, &B256::from(U256::from(1u64)));
        StorageBackedAddress::new(self.context, self.gas.as_deref_mut(), slot)
    }

    pub fn to(&mut self) -> StorageBackedAddress<'_, CTX> {
        let slot = map_address(&self.slot, &B256::from(U256::from(2u64)));
        StorageBackedAddress::new(self.context, self.gas.as_deref_mut(), slot)
    }

    pub fn callvalue(&mut self) -> StorageBackedU256<'_, CTX> {
        let slot = map_address(&self.slot, &B256::from(U256::from(3u64)));
        StorageBackedU256::new(self.context, self.gas.as_deref_mut(), slot)
    }

    pub fn beneficiary(&mut self) -> StorageBackedAddress<'_, CTX> {
        let slot = map_address(&self.slot, &B256::from(U256::from(4u64)));
        StorageBackedAddress::new(self.context, self.gas.as_deref_mut(), slot)
    }

    pub fn calldata(&mut self) -> StorageBackedBytes<'_, CTX> {
        let slot = substorage(&self.slot, ARBOS_STATE_RETRYABLE_CALLDATA_KEY);
        StorageBackedBytes::new(self.context, self.gas.as_deref_mut(), slot)
    }

    pub fn timeout(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = map_address(&self.slot, &B256::from(U256::from(5u64)));
        StorageBackedU64::new(self.context, self.gas.as_deref_mut(), slot)
    }

    pub fn timeout_windows_left(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = map_address(&self.slot, &B256::from(U256::from(6u64)));
        StorageBackedU64::new(self.context, self.gas.as_deref_mut(), slot)
    }

    pub fn clear(&mut self) -> Result<(), ArbosStateError> {
        self.num_tries().set(0)?;
        self.timeout().set(0)?;
        self.callvalue().set(U256::ZERO)?;
        self.to().set(Address::ZERO)?;
        self.from().set(Address::ZERO)?;
        self.calldata().set(&Bytes::new())?;
        self.beneficiary().set(Address::ZERO)?;
        self.timeout_windows_left().set(0)
    }
}
