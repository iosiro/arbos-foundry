use revm::primitives::{Address, B256, U256};

use crate::{
    ArbitrumContextTr,
    state::types::{
        ArbosStateError, StorageBackedAddress, StorageBackedAddressSet, StorageBackedI256,
        StorageBackedTr, StorageBackedU64, StorageBackedU256, map_address, substorage,
    },
};

const ARBOS_L1_PRICING_BATCH_POSTER_TABLE_KEY: &[u8] = &[0];
const ARBOS_L1_PRICING_PAY_RECIPIENT_KEY: &[u8] = &[0];
const ARBOS_L1_PRICING_EQUILIBRATION_UNITS_KEY: &[u8] = &[1];
const ARBOS_L1_PRICING_INERTIA_KEY: &[u8] = &[2];
const ARBOS_L1_PRICING_PER_UNIT_REWARD_KEY: &[u8] = &[3];
const ARBOS_L1_PRICING_LAST_UPDATE_TIME_KEY: &[u8] = &[4];
const ARBOS_L1_PRICING_FUNDS_DUE_FOR_REWARDS_KEY: &[u8] = &[5];
const ARBOS_L1_PRICING_UNITS_SINCE_UPDATE_KEY: &[u8] = &[6];
const ARBOS_L1_PRICING_PRICE_PER_UNIT_KEY: &[u8] = &[7];
const ARBOS_L1_PRICING_LAST_SURPLUS_KEY: &[u8] = &[8];
const ARBOS_L1_PRICING_PER_BATCH_GAS_COST_KEY: &[u8] = &[9];
const ARBOS_L1_PRICING_AMORTIZED_COST_CAP_BIPS_KEY: &[u8] = &[10];
const ARBOS_L1_PRICING_L1_FEES_AVAILABLE_KEY: &[u8] = &[11];
const ARBOS_L1_PRICING_GAS_FLOOR_PER_TOKEN_KEY: &[u8] = &[12];

pub struct L1Pricing<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    context: &'a mut CTX,
    gas: Option<&'a mut revm::interpreter::Gas>,
    is_static: bool,
    slot: B256,
}

impl<'a, CTX: ArbitrumContextTr> L1Pricing<'a, CTX> {
    pub fn new(
        context: &'a mut CTX,
        gas: Option<&'a mut revm::interpreter::Gas>,
        is_static: bool,
        slot: B256,
    ) -> Self {
        Self { context, gas, is_static, slot }
    }

    pub fn batch_poster_table(&mut self) -> BatchPosterTable<'_, CTX> {
        let slot = substorage(&self.slot, ARBOS_L1_PRICING_BATCH_POSTER_TABLE_KEY);
        BatchPosterTable::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
    }
    pub fn reward_recipient(&mut self) -> StorageBackedAddress<'_, CTX> {
        let slot = substorage(&self.slot, ARBOS_L1_PRICING_PAY_RECIPIENT_KEY);
        StorageBackedAddress::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
    }
    pub fn equilibration_units(&mut self) -> StorageBackedU256<'_, CTX> {
        let slot = substorage(&self.slot, ARBOS_L1_PRICING_EQUILIBRATION_UNITS_KEY);
        StorageBackedU256::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
    }
    pub fn inertia(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = substorage(&self.slot, ARBOS_L1_PRICING_INERTIA_KEY);
        StorageBackedU64::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
    }
    pub fn per_unit_reward(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = substorage(&self.slot, ARBOS_L1_PRICING_PER_UNIT_REWARD_KEY);
        StorageBackedU64::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
    }
    pub fn last_update_time(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = substorage(&self.slot, ARBOS_L1_PRICING_LAST_UPDATE_TIME_KEY);
        StorageBackedU64::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
    }
    pub fn funds_due_for_rewards(&mut self) -> StorageBackedI256<'_, CTX> {
        let slot = substorage(&self.slot, ARBOS_L1_PRICING_FUNDS_DUE_FOR_REWARDS_KEY);
        StorageBackedI256::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
    }
    pub fn units_since_update(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = substorage(&self.slot, ARBOS_L1_PRICING_UNITS_SINCE_UPDATE_KEY);
        StorageBackedU64::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
    }
    pub fn price_per_unit(&mut self) -> StorageBackedU256<'_, CTX> {
        let slot = substorage(&self.slot, ARBOS_L1_PRICING_PRICE_PER_UNIT_KEY);
        StorageBackedU256::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
    }
    pub fn last_surplus(&mut self) -> StorageBackedI256<'_, CTX> {
        let slot = substorage(&self.slot, ARBOS_L1_PRICING_LAST_SURPLUS_KEY);
        StorageBackedI256::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
    }
    pub fn per_batch_gas_cost(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = substorage(&self.slot, ARBOS_L1_PRICING_PER_BATCH_GAS_COST_KEY);
        StorageBackedU64::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
    }
    pub fn amortized_cost_cap_bips(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = substorage(&self.slot, ARBOS_L1_PRICING_AMORTIZED_COST_CAP_BIPS_KEY);
        StorageBackedU64::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
    }
    pub fn l1_fees_available(&mut self) -> StorageBackedU256<'_, CTX> {
        let slot = substorage(&self.slot, ARBOS_L1_PRICING_L1_FEES_AVAILABLE_KEY);
        StorageBackedU256::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
    }
    pub fn gas_floor_per_token(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = substorage(&self.slot, ARBOS_L1_PRICING_GAS_FLOOR_PER_TOKEN_KEY);
        StorageBackedU64::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
    }
}

const ARBOS_BATCH_POSTER_ADDRS_KEY: &[u8] = &[0];
const ARBOS_BATCH_POSTER_INFO_KEY: &[u8] = &[1];

pub struct BatchPosterTable<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    context: &'a mut CTX,
    gas: Option<&'a mut revm::interpreter::Gas>,
    is_static: bool,
    slot: B256,
}

impl<'a, CTX: ArbitrumContextTr> BatchPosterTable<'a, CTX> {
    pub fn new(
        context: &'a mut CTX,
        gas: Option<&'a mut revm::interpreter::Gas>,
        is_static: bool,
        slot: B256,
    ) -> Self {
        Self { context, gas, is_static, slot }
    }

    fn posters_address_set(&mut self) -> StorageBackedAddressSet<'_, CTX> {
        let slot = substorage(&self.slot, ARBOS_BATCH_POSTER_ADDRS_KEY);
        StorageBackedAddressSet::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
    }

    pub fn all(&mut self) -> Result<Vec<Address>, ArbosStateError> {
        self.posters_address_set().all()
    }

    pub fn get(&mut self, batch_poster: Address) -> BatchPosterState<'_, CTX> {
        let poster_info = substorage(&self.slot, ARBOS_BATCH_POSTER_INFO_KEY);
        let bp_storage = substorage(&poster_info, batch_poster.as_slice());
        BatchPosterState::new(self.context, self.gas.as_deref_mut(), self.is_static, bp_storage)
    }

    pub fn contains(&mut self, batch_poster: Address) -> Result<bool, ArbosStateError> {
        Ok(self.all()?.contains(&batch_poster))
    }

    pub fn add_if_missing(
        &mut self,
        batch_poster: Address,
        pay_recipient: Address,
    ) -> Result<bool, ArbosStateError> {
        if self.contains(batch_poster)? {
            return Ok(false);
        }

        self.add(batch_poster, pay_recipient)?;
        Ok(true)
    }

    pub fn add(
        &mut self,
        batch_poster: Address,
        pay_recipient: Address,
    ) -> Result<(), ArbosStateError> {
        self.posters_address_set().add(batch_poster)?;
        self.get(batch_poster).pay_recipient().set(pay_recipient)
    }

    pub fn total_funds_due(&mut self) -> StorageBackedI256<'_, CTX> {
        let slot = map_address(&self.slot, &B256::from(U256::ZERO));
        StorageBackedI256::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
    }

    pub fn fee_collector(&mut self, batch_poster: Address) -> Result<Address, ArbosStateError> {
        self.get(batch_poster).pay_recipient().get()
    }

    pub fn set_fee_collector(
        &mut self,
        batch_poster: Address,
        new_fee_collector: Address,
    ) -> Result<(), ArbosStateError> {
        self.get(batch_poster).pay_recipient().set(new_fee_collector)
    }
}

pub struct BatchPosterState<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    context: &'a mut CTX,
    gas: Option<&'a mut revm::interpreter::Gas>,
    is_static: bool,
    slot: B256,
}

impl<'a, CTX: ArbitrumContextTr> BatchPosterState<'a, CTX> {
    pub fn new(
        context: &'a mut CTX,
        gas: Option<&'a mut revm::interpreter::Gas>,
        is_static: bool,
        slot: B256,
    ) -> Self {
        Self { context, gas, is_static, slot }
    }

    pub fn funds_due(&mut self) -> StorageBackedU256<'_, CTX> {
        let slot = map_address(&self.slot, &B256::from(U256::ZERO));
        StorageBackedU256::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
    }

    pub fn pay_recipient(&mut self) -> StorageBackedAddress<'_, CTX> {
        let slot = map_address(&self.slot, &B256::from(U256::ONE));
        StorageBackedAddress::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
    }
}
