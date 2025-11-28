use revm::primitives::{B256, U256};

use crate::{
    ArbitrumContextTr,
    state::types::{StorageBackedTr, StorageBackedU64, StorageBackedU256, map_address},
};

pub struct L2Pricing<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    context: &'a mut CTX,
    gas: Option<&'a mut revm::interpreter::Gas>,
    slot: B256,
}

impl<'a, CTX: ArbitrumContextTr> L2Pricing<'a, CTX> {
    pub fn new(
        context: &'a mut CTX,
        gas: Option<&'a mut revm::interpreter::Gas>,
        subkey: B256,
    ) -> Self {
        Self { context, gas, slot: subkey }
    }

    #[inline]
    fn slot(&self, offset: u64) -> B256 {
        map_address(&self.slot, &B256::from(U256::from(offset)))
    }

    pub fn speed_limit_per_second(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = self.slot(0);
        StorageBackedU64::new(self.context, self.gas.as_deref_mut(), slot)
    }

    pub fn per_block_gas_limit(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = self.slot(1);
        StorageBackedU64::new(self.context, self.gas.as_deref_mut(), slot)
    }

    pub fn base_fee_wei(&mut self) -> StorageBackedU256<'_, CTX> {
        let slot = self.slot(2);
        StorageBackedU256::new(self.context, self.gas.as_deref_mut(), slot)
    }

    pub fn min_base_fee_wei(&mut self) -> StorageBackedU256<'_, CTX> {
        let slot = self.slot(3);
        StorageBackedU256::new(self.context, self.gas.as_deref_mut(), slot)
    }

    pub fn gas_backlog(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = self.slot(4);
        StorageBackedU64::new(self.context, self.gas.as_deref_mut(), slot)
    }

    pub fn pricing_inertia(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = self.slot(5);
        StorageBackedU64::new(self.context, self.gas.as_deref_mut(), slot)
    }

    pub fn backlog_tolerance(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = self.slot(6);
        StorageBackedU64::new(self.context, self.gas.as_deref_mut(), slot)
    }

    pub fn per_tx_gas_limit(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = self.slot(7);
        StorageBackedU64::new(self.context, self.gas.as_deref_mut(), slot)
    }
}
