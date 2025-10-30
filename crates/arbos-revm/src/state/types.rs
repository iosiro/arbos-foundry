use revm::{
    context::JournalTr,
    primitives::{Address, B256, I256, U256, keccak256},
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

pub struct StorageBackedI256<'a, CTX>(&'a mut CTX, B256)
where
    CTX: ArbitrumContextTr;

impl<'a, CTX> StorageBackedI256<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    pub fn new(context: &'a mut CTX, slot: B256) -> Self {
        Self(context, slot)
    }

    pub fn get(&mut self) -> I256 {
        let v =
            self.0.journal_mut().sload(ARBOS_STATE_ADDRESS, self.1.into()).unwrap_or_default().data;
        I256::from_raw(v)
    }

    pub fn set(&mut self, value: I256) {
        let _ = self.0.sstore(ARBOS_STATE_ADDRESS, self.1.into(), U256::from(value));
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

pub struct StorageBackedBytes<'a, CTX>(&'a mut CTX, B256)
where
    CTX: ArbitrumContextTr;

impl<'a, CTX> StorageBackedBytes<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    pub fn new(context: &'a mut CTX, slot: B256) -> Self {
        Self(context, slot)
    }

    pub fn get(&mut self) -> Vec<u8> {
        let size_slot = map_address(&self.1, &B256::from(U256::from(0u64)));
        let size = self
            .0
            .journal_mut()
            .sload(ARBOS_STATE_ADDRESS, size_slot.into())
            .unwrap_or_default()
            .data
            .saturating_to::<u64>();
        let mut out = Vec::with_capacity(size as usize);
        let mut offset = 0u64;
        while offset < size {
            let chunk_slot = map_address(&self.1, &B256::from(U256::from(offset + 1)));
            let chunk = self
                .0
                .journal_mut()
                .sload(ARBOS_STATE_ADDRESS, chunk_slot.into())
                .unwrap_or_default()
                .data;
            let chunk_bytes = chunk.to_be_bytes_vec();
            let to_copy = std::cmp::min(size - offset, 32);
            out.extend_from_slice(&chunk_bytes[..to_copy as usize]);
            offset += to_copy;
        }
        out
    }

    pub fn set(&mut self, value: &[u8]) {
        let size_slot = map_address(&self.1, &B256::from(U256::from(0u64)));
        let _ =
            self.0.sstore(ARBOS_STATE_ADDRESS, size_slot.into(), U256::from(value.len() as u64));
        let mut offset = 0u64;
        while offset < value.len() as u64 {
            let chunk_slot = map_address(&self.1, &B256::from(U256::from(offset + 1)));
            let to_copy = std::cmp::min(value.len() as u64 - offset, 32);
            let mut chunk_bytes = [0u8; 32];
            chunk_bytes[..to_copy as usize]
                .copy_from_slice(&value[offset as usize..(offset + to_copy) as usize]);
            let chunk = B256::from_slice(&chunk_bytes);
            let _ = self.0.sstore(ARBOS_STATE_ADDRESS, chunk_slot.into(), chunk.into());
            offset += to_copy;
        }
    }
}

pub struct StorageBackedQueue<'a, CTX>(&'a mut CTX, B256)
where
    CTX: ArbitrumContextTr;

impl<'a, CTX> StorageBackedQueue<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    pub fn new(context: &'a mut CTX, slot: B256) -> Self {
        Self(context, slot)
    }

    fn head_slot(&self) -> B256 {
        map_address(&self.1, &B256::from(U256::from(0u64)))
    }

    fn tail_slot(&self) -> B256 {
        map_address(&self.1, &B256::from(U256::from(1u64)))
    }

    pub fn size(&mut self) -> u64 {
        let head = self
            .0
            .journal_mut()
            .sload(ARBOS_STATE_ADDRESS, self.head_slot().into())
            .unwrap_or_default()
            .data
            .saturating_to::<u64>();
        let tail = self
            .0
            .journal_mut()
            .sload(ARBOS_STATE_ADDRESS, self.tail_slot().into())
            .unwrap_or_default()
            .data
            .saturating_to::<u64>();
        tail.saturating_sub(head)
    }

    pub fn peek(&mut self) -> Option<U256> {
        let head_slot = { self.head_slot() };

        let tail_slot = { self.tail_slot() };

        let head = self
            .0
            .journal_mut()
            .sload(ARBOS_STATE_ADDRESS, head_slot.into())
            .unwrap_or_default()
            .data
            .saturating_to::<u64>();
        let tail = self
            .0
            .journal_mut()
            .sload(ARBOS_STATE_ADDRESS, tail_slot.into())
            .unwrap_or_default()
            .data
            .saturating_to::<u64>();
        if head >= tail {
            return None;
        }
        let elem_slot = map_address(&self.1, &B256::from(U256::from(head)));
        let v = self
            .0
            .journal_mut()
            .sload(ARBOS_STATE_ADDRESS, elem_slot.into())
            .unwrap_or_default()
            .data;
        Some(v)
    }

    pub fn pop(&mut self) -> Option<U256> {
        let head_slot = { self.head_slot() };

        let tail_slot = { self.tail_slot() };

        let head = self
            .0
            .journal_mut()
            .sload(ARBOS_STATE_ADDRESS, head_slot.into())
            .unwrap_or_default()
            .data
            .saturating_to::<u64>();
        let tail = self
            .0
            .journal_mut()
            .sload(ARBOS_STATE_ADDRESS, tail_slot.into())
            .unwrap_or_default()
            .data
            .saturating_to::<u64>();
        if head >= tail {
            return None;
        }
        let elem_slot = map_address(&self.1, &B256::from(U256::from(head)));
        let v = self
            .0
            .journal_mut()
            .sload(ARBOS_STATE_ADDRESS, elem_slot.into())
            .unwrap_or_default()
            .data;

        // increment head
        let new_head = head.saturating_add(1);
        let _ = self.0.sstore(ARBOS_STATE_ADDRESS, head_slot.into(), U256::from(new_head));

        Some(v)
    }

    pub fn push(&mut self, value: U256) {
        let tail_slot = { self.tail_slot() };

        let tail = self
            .0
            .journal_mut()
            .sload(ARBOS_STATE_ADDRESS, tail_slot.into())
            .unwrap_or_default()
            .data
            .saturating_to::<u64>();
        let elem_slot = map_address(&self.1, &B256::from(U256::from(tail)));
        let _ = self.0.sstore(ARBOS_STATE_ADDRESS, elem_slot.into(), value);

        // increment tail
        let new_tail = tail.saturating_add(1);
        let _ = self.0.sstore(ARBOS_STATE_ADDRESS, tail_slot.into(), U256::from(new_tail));
    }
}
