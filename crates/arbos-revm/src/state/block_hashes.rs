use crate::{
    ArbitrumContextTr,
    state::types::{
        ArbosStateError, StorageBackedB256, StorageBackedTr, StorageBackedU64, map_address,
    },
};

use revm::{
    interpreter::Gas,
    primitives::{B256, U256, keccak256},
};

pub struct BlockHashes<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    context: &'a mut CTX,
    gas: Option<&'a mut Gas>,
    slot: B256,
}

impl<'a, CTX> BlockHashes<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    pub fn new(context: &'a mut CTX, gas: Option<&'a mut Gas>, slot: B256) -> Self {
        Self { context, gas, slot }
    }

    pub fn l1_block_number(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = map_address(&self.slot, &B256::ZERO);
        StorageBackedU64::new(self.context, self.gas.as_deref_mut(), slot)
    }

    pub fn block_hash(&mut self, number: u64) -> Result<B256, ArbosStateError> {
        let current_number = self.l1_block_number().get()?;
        if number >= current_number || number + 256 < current_number {
            return Err(ArbosStateError::InvalidBlockNumberForBlockHash);
        }

        let slot = map_address(&self.slot, &B256::from(U256::from(1 + (number % 256))));
        StorageBackedB256::new(self.context, self.gas.as_deref_mut(), slot).get()
    }

    pub fn record_new_l1_block(
        &mut self,
        number: u64,
        block_hash: B256,
        arbos_version: u64,
    ) -> Result<(), ArbosStateError> {
        let mut next_number = self.l1_block_number().get()?;
        if number < next_number {
            // we already have a stored hash for the block, so just return
            return Ok(());
        }
        if next_number + 256 < number {
            next_number = number - 256; // no need to record hashes that we're just going to discard
        }
        while next_number + 1 < number {
            // fill in hashes for any "skipped over" blocks
            next_number += 1;
            let mut next_num_buf = [0u8; 8];
            if arbos_version >= 8 {
                next_num_buf.copy_from_slice(&next_number.to_le_bytes());
            }

            let mut data = block_hash.to_vec();
            data.extend_from_slice(&next_num_buf);
            let fill = keccak256(data);

            let slot = map_address(&self.slot, &B256::from(U256::from(1 + (next_number % 256))));
            StorageBackedB256::new(self.context, self.gas.as_deref_mut(), slot).set(fill)?;
        }

        let slot = map_address(&self.slot, &B256::from(U256::from(1 + (number % 256))));
        StorageBackedB256::new(self.context, self.gas.as_deref_mut(), slot).set(block_hash)?;

        self.l1_block_number().set(number + 1)
    }
}
