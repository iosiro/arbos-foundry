use std::io::Read;

use crate::ArbitrumContextTr;
use crate::constants::ARBOS_STATE_ADDRESS;
use crate::state::types::{map_address, substorage};
use alloy_rlp::{BufMut, Decodable, Encodable, Error, Header};
use revm::bytecode::bitvec::index;
use revm::context::JournalTr;
use revm::primitives::{Address, B256, Bytes, U256};

#[derive(Debug, Clone)]
enum RLPItem {
    Address(Address),
    Index(u64),
}

pub struct AddressTable<'a, CTX>(&'a mut CTX, B256)
where
    CTX: ArbitrumContextTr;

impl<'a, CTX> AddressTable<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    /// Open an AddressTable rooted at `slot`
    pub fn new(context: &'a mut CTX, slot: B256) -> Self {
        Self(context, slot)
    }

    fn backing_slot(&self) -> B256 {
        // the array (size at index 0, elements at 1..)
        self.1
    }

    fn by_address_substorage(&self) -> B256 {
        // substorage index 0 used for by-address mapping
        substorage(&self.1, &[])
    }

    fn size_slot(&self) -> B256 {
        // size stored under map(backing_slot, 0)
        map_address(&self.backing_slot(), &B256::from(U256::from(0u64)))
    }

    /// internal: read the stored 1-based index for `address` (0 means not present)
    fn get_stored_index(&mut self, address: &Address) -> U256 {
        let by_addr = self.by_address_substorage();
        let key = B256::left_padding_from(address.as_slice());
        let slot = map_address(&by_addr, &key);
        let v =
            self.0.journal_mut().sload(ARBOS_STATE_ADDRESS, slot.into()).unwrap_or_default().data;
        v
    }

    /// Register `address` if not present and return zero-based index.
    /// If already present, returns existing zero-based index.
    pub fn register(&mut self, address: &Address) -> u64 {
        // check by-address mapping
        let existing = self.get_stored_index(address);
        if !existing.is_zero() {
            // stored index is 1-based in storage
            return existing.saturating_to::<u64>() - 1;
        }

        // not present: increment size and append into backing_storage at new index (1-based)
        let size_slot = self.size_slot();
        let size_u256 = self
            .0
            .journal_mut()
            .sload(ARBOS_STATE_ADDRESS, size_slot.into())
            .unwrap_or_default()
            .data;

        let size = size_u256.saturating_to::<u64>();
        let new_num = size + 1;

        // store address into backing storage at element index new_num (map(backing, new_num))
        let elem_slot = map_address(&self.backing_slot(), &B256::from(U256::from(new_num)));
        let _ = self.0.sstore(
            ARBOS_STATE_ADDRESS,
            elem_slot.into(),
            B256::left_padding_from(address.as_slice()).into(),
        );

        // update size
        let _ = self.0.sstore(ARBOS_STATE_ADDRESS, size_slot.into(), U256::from(new_num));

        // record by-address -> new_num (1-based)
        let by_addr = self.by_address_substorage();
        let by_key = B256::left_padding_from(address.as_slice());
        let _ = self.0.sstore(
            ARBOS_STATE_ADDRESS,
            map_address(&by_addr, &by_key).into(),
            U256::from(new_num),
        );

        // return zero-based index
        new_num - 1
    }

    /// Look up an address; returns (zero_based_index, exists)
    pub fn lookup(&mut self, address: &Address) -> Option<u64> {
        let existing = self.get_stored_index(address);
        if existing.is_zero() { None } else { Some(existing.saturating_to::<u64>() - 1) }
    }

    /// true if address exists
    pub fn address_exists(&mut self, address: &Address) -> bool {
        self.lookup(address).is_some()
    }

    /// number of items (size)
    pub fn size(&mut self) -> u64 {
        let size_slot = self.size_slot();
        let v = self
            .0
            .journal_mut()
            .sload(ARBOS_STATE_ADDRESS, size_slot.into())
            .unwrap_or_default()
            .data;
        v.saturating_to::<u64>()
    }

    /// Lookup by zero-based index. Returns (address, exists)
    pub fn lookup_index(&mut self, index: u64) -> Option<Address> {
        let items = self.size();
        if index >= items {
            return None;
        }
        // stored at 1-based index
        let elem_slot = map_address(&self.backing_slot(), &B256::from(U256::from(index + 1)));
        let v = self
            .0
            .journal_mut()
            .sload(ARBOS_STATE_ADDRESS, elem_slot.into())
            .unwrap_or_default()
            .data;
        let addr = Address::from_slice(&v.to_be_bytes_vec()[12..32]);
        Some(addr)
    }

    pub fn compress(&mut self, address: &Address) -> Bytes {
        if let Some(index) = self.lookup(address) {
            // encode as index
            let item = RLPItem::Index(index); // stored as 1-based
            let mut out = Vec::new();
            item.encode(&mut out);
            return Bytes::from(out);
        } else {
            // encode as address
            let item = RLPItem::Address(*address);
            let mut out = Vec::new();
            item.encode(&mut out);
            return Bytes::from(out);
        }
    }

    pub fn decompress(&mut self, data: &[u8]) -> Result<(Address, u64), String> {
        let mut slice = data;
        let mut stream =
            alloy_rlp::Rlp::new(&mut slice).map_err(|e| format!("Invalid RLP: {:?}", e))?;
        stream.get_next::<RLPItem>().map_err(|e| format!("RLP decode error: {:?}", e)).and_then(
            |item| match item {
                Some(RLPItem::Address(addr)) => Ok((addr, (data.len() - slice.len()) as u64)),
                Some(RLPItem::Index(idx)) => {
                    let addr =
                        self.lookup_index(idx).ok_or_else(|| "invalid index in compressed address".to_string())?;
                    Ok((addr, (data.len() - slice.len()) as u64))
                }
                None => todo!("Implement RLP decoding for None"),
            },
        )
    }
}


impl Encodable for RLPItem {
    fn encode(&self, out: &mut dyn BufMut) {
        match self {
            RLPItem::Address(addr) => {
                out.put_slice(&addr.as_slice());
            }
            RLPItem::Index(idx) => {
                out.put_u64(*idx);
            }
        }
    }
}

impl Decodable for RLPItem {
    fn decode(data: &mut &[u8]) -> Result<Self, Error> {
        let mut payload = Header::decode_bytes(data, true)?;
        match u8::decode(&mut payload)? {
            0 => Ok(Self::Address(Address::decode(&mut payload)?)),
            1 => Ok(Self::Index(u64::decode(&mut payload)?)),
            _ => Err(Error::Custom("unknown type")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use revm::primitives::{hex::FromHex, Address, B256};
    use alloy_rlp::{Decodable, Encodable};

    #[test]
    fn encode_decode_address_roundtrip() {
        let addr = Address::from_hex("0xdeadbeef").expect("valid hex");
        let item = RLPItem::Address(addr);

        // Encode
        let mut out = Vec::new();
        item.encode(&mut out);

        // Decode back
        let mut slice: &[u8] = &out;
        let decoded = RLPItem::decode(&mut slice).expect("decode should succeed");

        match decoded {
            RLPItem::Address(decoded_addr) => assert_eq!(decoded_addr, addr),
            other => panic!("expected Address variant, got {:?}", other),
        }

        assert!(
            slice.is_empty(),
            "after decoding there should be no leftover bytes"
        );
    }

    #[test]
    fn encode_decode_index_roundtrip() {
        let idx: u64 = 42;
        let item = RLPItem::Index(idx);

        let mut out = Vec::new();
        item.encode(&mut out);

        let mut slice: &[u8] = &out;
        let decoded = RLPItem::decode(&mut slice).expect("decode should succeed");

        match decoded {
            RLPItem::Index(decoded_idx) => assert_eq!(decoded_idx, idx),
            other => panic!("expected Index variant, got {:?}", other),
        }

        assert!(
            slice.is_empty(),
            "after decoding there should be no leftover bytes"
        );
    }

    #[test]
    fn decode_invalid_data_fails() {
        // Random data not matching Address or Index encoding
        let bad_data = vec![0xff, 0x00, 0x11, 0x22];
        let mut slice: &[u8] = &bad_data;
        let res = RLPItem::decode(&mut slice);
        assert!(
            res.is_err(),
            "decoding invalid bytes should return an error"
        );
    }
}