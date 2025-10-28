use revm::{
    context::JournalTr,
    primitives::{Address, B256, U256, keccak256},
};

use crate::{ArbitrumContextTr, constants::ARBOS_STATE_ADDRESS};

// --- utility helpers moved to module scope ---
pub fn substorage(root: &B256, index: &[u8]) -> B256 {
    let mut subkey_bytes =
        if root.is_zero() { Vec::with_capacity(1) } else { root.as_slice().to_vec() };
    subkey_bytes.extend_from_slice(index);
    keccak256(subkey_bytes)
}

pub fn map_address(storage_key: &B256, key: &B256) -> B256 {
    let key_bytes = key.as_slice();
    let boundary = key_bytes.len() - 1;

    let mut to_hash = Vec::with_capacity(storage_key.len() + boundary);
    if !storage_key.is_zero() {
        to_hash.extend_from_slice(storage_key.as_slice());
    }
    to_hash.extend_from_slice(&key_bytes[..boundary]);

    let digest = keccak256(&to_hash);

    let mut mapped = digest[..boundary].to_vec();
    mapped.push(key_bytes[boundary]);
    B256::from_slice(&mapped)
}

// --- small portable storage wrappers ---

/// Generic wrapper for a storage-backed u64 value (stored as U256)
pub struct StorageBackedU64<'a, CTX>(&'a mut CTX, B256)
where
    CTX: ArbitrumContextTr;

impl<'a, CTX> StorageBackedU64<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    pub fn new(context: &'a mut CTX, slot: B256) -> Self {
        Self(context, slot)
    }

    pub fn get(&mut self) -> u64 {
        let v =
            self.0.journal_mut().sload(ARBOS_STATE_ADDRESS, self.1.into()).unwrap_or_default().data;
        println!("StorageBackedU64 get: slot={:?} value={}", self.1, v);
        v.saturating_to()
    }

    pub fn set(&mut self, value: u64) {
        let _ = self.0.sstore(ARBOS_STATE_ADDRESS, self.1.into(), U256::from(value));
    }
}

/// Storage-backed address set implemented as array-with-length at index 0. Values are left-padded
/// B256.
pub struct StorageBackedAddressSet<'a, CTX>(&'a mut CTX, B256)
where
    CTX: ArbitrumContextTr;

impl<'a, CTX> StorageBackedAddressSet<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    pub fn new(context: &'a mut CTX, slot: B256) -> Self {
        Self(context, slot)
    }

    fn size_slot(&self) -> B256 {
        map_address(&self.1, &B256::from(U256::from(0u64)))
    }

    pub fn len(&mut self) -> usize {
        let size_slot = self.size_slot();
        let v = self
            .0
            .journal_mut()
            .sload(ARBOS_STATE_ADDRESS, size_slot.into())
            .unwrap_or_default()
            .data;
        v.saturating_to::<usize>()
    }

    pub fn all(&mut self) -> Vec<Address> {
        let n = self.len();
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let slot = map_address(&self.1, &B256::from(U256::from(i as u64 + 1)));
            let v = self
                .0
                .journal_mut()
                .sload(ARBOS_STATE_ADDRESS, slot.into())
                .unwrap_or_default()
                .data;
            let addr = Address::from_slice(&v.to_be_bytes_vec()[12..32]);
            out.push(addr);
        }
        out
    }

    pub fn contains(&mut self, address: &Address) -> bool {
        let by_address = substorage(&self.1, &[0]);
        let slot = map_address(&by_address, &B256::left_padding_from(address.as_slice()));
        let v =
            self.0.journal_mut().sload(ARBOS_STATE_ADDRESS, slot.into()).unwrap_or_default().data;
        !v.is_zero()
    }

    pub fn add(&mut self, address: &Address) {
        // push to array
        let size_slot = self.size_slot();
        let mut size = self
            .0
            .journal_mut()
            .sload(ARBOS_STATE_ADDRESS, size_slot.into())
            .unwrap_or_default()
            .data
            .saturating_to::<u64>();
        let slot = map_address(&self.1, &B256::from(U256::from(size + 1)));
        let _ = self.0.sstore(
            ARBOS_STATE_ADDRESS,
            slot.into(),
            B256::left_padding_from(address.as_slice()).into(),
        );
        size += 1;
        let _ = self.0.sstore(ARBOS_STATE_ADDRESS, size_slot.into(), U256::from(size));

        // also set by-address index so contains() is O(1)
        let by_address = substorage(&self.1, &[0]);
        let _ = self.0.sstore(
            ARBOS_STATE_ADDRESS,
            map_address(&by_address, &B256::left_padding_from(address.as_slice())).into(),
            U256::from(1u64),
        );
    }

    pub fn remove(&mut self, address: &Address) {
        let by_address = substorage(&self.1, &[0]);
        let by_address_slot =
            map_address(&by_address, &B256::left_padding_from(address.as_slice()));
        let _ = self.0.sstore(ARBOS_STATE_ADDRESS, by_address_slot.into(), U256::from(0u64));
        // NOTE: we don't compact the array in storage to keep logic simple and predictable.
    }
}

/// Generic wrapper for a storage-backed u32 value (stored as U256)
pub struct StorageBackedU32<'a, CTX>(&'a mut CTX, B256)
where
    CTX: ArbitrumContextTr;

impl<'a, CTX> StorageBackedU32<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    pub fn new(context: &'a mut CTX, slot: B256) -> Self {
        Self(context, slot)
    }

    pub fn get(&mut self) -> u32 {
        let v =
            self.0.journal_mut().sload(ARBOS_STATE_ADDRESS, self.1.into()).unwrap_or_default().data;
        v.saturating_to()
    }

    pub fn set(&mut self, value: u32) {
        let _ = self.0.sstore(ARBOS_STATE_ADDRESS, self.1.into(), U256::from(value));
    }
}

pub struct StorageBackedU256<'a, CTX>(&'a mut CTX, B256)
where
    CTX: ArbitrumContextTr;

impl<'a, CTX> StorageBackedU256<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    pub fn new(context: &'a mut CTX, slot: B256) -> Self {
        Self(context, slot)
    }

    pub fn get(&mut self) -> U256 {
        let v =
            self.0.journal_mut().sload(ARBOS_STATE_ADDRESS, self.1.into()).unwrap_or_default().data;
        v.saturating_to()
    }

    pub fn set(&mut self, value: U256) {
        let _ = self.0.sstore(ARBOS_STATE_ADDRESS, self.1.into(), value);
    }
}

pub struct StorageBackedAddress<'a, CTX>(&'a mut CTX, B256)
where
    CTX: ArbitrumContextTr;

impl<'a, CTX> StorageBackedAddress<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    pub fn new(context: &'a mut CTX, slot: B256) -> Self {
        Self(context, slot)
    }

    pub fn get(&mut self) -> Address {
        let v =
            self.0.journal_mut().sload(ARBOS_STATE_ADDRESS, self.1.into()).unwrap_or_default().data;
        Address::from_slice(&v.to_be_bytes_vec()[12..32])
    }

    pub fn set(&mut self, value: &Address) {
        let _ = self.0.sstore(
            ARBOS_STATE_ADDRESS,
            self.1.into(),
            B256::left_padding_from(value.as_slice()).into(),
        );
    }
}
