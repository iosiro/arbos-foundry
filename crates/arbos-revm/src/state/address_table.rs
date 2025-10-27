use arbutil::crypto::keccak;
use revm::{context::JournalTr, precompile::bn254::add, primitives::{Address, B256, U256}};

use crate::{constants::{ARBOS_STATE_ADDRESS, ARBOS_STATE_ADDRESS_TABLE_KEY}, state::types::{map_address, substorage, StorageBackedAddress, StorageBackedAddressSet}, ArbitrumContextTr};


struct AddressTable<'a, CTX>(&'a mut CTX, B256)
where
    CTX: ArbitrumContextTr;

impl<'a, CTX> AddressTable<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    pub fn new(context: &'a mut CTX) -> Self {
        let root = B256::ZERO;
        let subkey = substorage(&root, ARBOS_STATE_ADDRESS_TABLE_KEY);
        Self(context, subkey)
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

    fn address_set(&mut self) -> StorageBackedAddressSet<'_, CTX> {
        let by_address = keccak(self.1);
        StorageBackedAddressSet::new(self.0, by_address.into())
    }

    pub fn register(&mut self, address: &Address) {
        let mut addr_set = self.address_set();

        addr_set.add(address);
    }
}

