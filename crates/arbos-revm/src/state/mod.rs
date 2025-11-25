use revm::{
    context::JournalTr,
    interpreter::Gas,
    primitives::{Address, B256, U256},
};

use crate::{
    ArbitrumContextTr,
    constants::{
        ARBOS_CHAIN_OWNERS_KEY, ARBOS_PROGRAMS_STATE_CACHE_MANAGERS_KEY, ARBOS_STATE_ADDRESS,
        ARBOS_STATE_ADDRESS_TABLE_KEY, ARBOS_STATE_L1_PRICING_KEY, ARBOS_STATE_L2_PRICING_KEY,
        ARBOS_STATE_NATIVE_TOKEN_OWNER_KEY, ARBOS_STATE_PROGRAMS_KEY, ARBOS_STATE_RETRYABLES_KEY,
    },
    state::{
        address_table::AddressTable,
        l1_pricing::L1Pricing,
        l2_pricing::L2Pricing,
        program::Programs,
        retryable::{Retryable, RetryableState},
        types::{
            ArbosStateError, StorageBackedAddress, StorageBackedAddressSet, StorageBackedQueue,
            StorageBackedTr, StorageBackedU64, StorageBackedU256, map_address, substorage,
        },
    },
};

pub mod address_table;
pub mod l1_pricing;
pub mod l2_pricing;
pub mod program;
pub mod retryable;
pub mod types;

const ARBOS_STATE_UPGRADE_VERSION_OFFSET: u8 = 1;
const ARBOS_STATE_UPGRADE_TIMESTAMP_OFFSET: u8 = 2;
const ARBOS_STATE_NETWORK_FEE_ACCOUNT_OFFSET: u8 = 3;
const ARBOS_STATE_CHAIN_ID_OFFSET: u8 = 4;
const ARBOS_STATE_GENESIS_BLOCK_NUM_OFFSET: u8 = 5;
const ARBOS_STATE_INFRA_FEE_ACCOUNT_OFFSET: u8 = 6;
const ARBOS_STATE_BROTLI_COMPRESSION_LEVEL_OFFSET: u8 = 7;
const ARBOS_STATE_NATIVE_TOKEN_ENABLED_FROM_TIME_OFFSET: u8 = 8;

fn state_slot(offset: u8) -> B256 {
    map_address(&B256::ZERO, &B256::from(U256::from(offset as u64)))
}

fn state_subkey(key: &[u8]) -> B256 {
    substorage(&B256::ZERO, key)
}

pub trait ArbStateGetter<CTX: ArbitrumContextTr> {
    fn programs(&mut self) -> Programs<'_, CTX>;
    fn chain_owners<'b>(&'b mut self) -> StorageBackedAddressSet<'b, CTX>;
    fn native_token_owners<'b>(&'b mut self) -> StorageBackedAddressSet<'b, CTX>;
    fn cache_managers<'b>(&'b mut self) -> StorageBackedAddressSet<'b, CTX>;
    fn is_chain_owner(&mut self, address: Address) -> Result<bool, ArbosStateError>;
    fn is_native_token_owner(&mut self, address: Address) -> Result<bool, ArbosStateError>;
    fn upgrade_timestamp(&mut self) -> StorageBackedU64<'_, CTX>;
    fn upgrade_version(&mut self) -> StorageBackedU64<'_, CTX>;
    fn network_fee_account(&mut self) -> StorageBackedAddress<'_, CTX>;
    fn infra_fee_account(&mut self) -> StorageBackedAddress<'_, CTX>;
    fn chain_id(&mut self) -> StorageBackedU256<'_, CTX>;
    fn genesis_block_num(&mut self) -> StorageBackedU64<'_, CTX>;
    fn brotli_compression_level(&mut self) -> StorageBackedU64<'_, CTX>;
    fn native_token_enabled_time(&mut self) -> StorageBackedU64<'_, CTX>;
    fn address_table(&mut self) -> AddressTable<'_, CTX>;
    fn l1_pricing(&mut self) -> L1Pricing<'_, CTX>;
    fn l2_pricing(&mut self) -> L2Pricing<'_, CTX>;
    fn retryable_state(&mut self) -> RetryableState<'_, CTX>;
    fn retryable<'b>(&'b mut self, id: B256) -> Retryable<'b, CTX>;
    fn timeout_queue(&mut self) -> StorageBackedQueue<'_, CTX>;
}

pub trait ArbState<'a, CTX: ArbitrumContextTr> {
    type ArbStateGetterType: ArbStateGetter<CTX>;
    fn arb_state(&'a mut self, gas: Option<&'a mut Gas>) -> Self::ArbStateGetterType;
}

impl<'a, CTX: ArbitrumContextTr + 'a> ArbState<'a, CTX> for CTX {
    type ArbStateGetterType = ArbStateWrapper<'a, CTX>;
    fn arb_state(&'a mut self, gas: Option<&'a mut Gas>) -> Self::ArbStateGetterType {
        ArbStateWrapper::new(self, gas)
    }
}

pub struct ArbStateWrapper<'a, CTX: ArbitrumContextTr> {
    context: &'a mut CTX,
    gas: Option<&'a mut Gas>,
}

impl<'a, CTX: ArbitrumContextTr> ArbStateWrapper<'a, CTX> {
    pub fn new(context: &'a mut CTX, gas: Option<&'a mut Gas>) -> Self {
        context.journal_mut().warm_account(ARBOS_STATE_ADDRESS).unwrap();
        Self { context, gas }
    }
}

impl<'a, CTX> ArbStateGetter<CTX> for ArbStateWrapper<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    fn programs(&mut self) -> Programs<'_, CTX> {
        Programs::new(self.context, self.gas.as_deref_mut(), state_subkey(ARBOS_STATE_PROGRAMS_KEY))
    }

    fn chain_owners<'b>(&'b mut self) -> StorageBackedAddressSet<'b, CTX> {
        StorageBackedAddressSet::new(
            self.context,
            self.gas.as_deref_mut(),
            state_subkey(ARBOS_CHAIN_OWNERS_KEY),
        )
    }

    fn native_token_owners<'b>(&'b mut self) -> StorageBackedAddressSet<'b, CTX> {
        StorageBackedAddressSet::new(
            self.context,
            self.gas.as_deref_mut(),
            state_subkey(ARBOS_STATE_NATIVE_TOKEN_OWNER_KEY),
        )
    }

    fn cache_managers<'b>(&'b mut self) -> StorageBackedAddressSet<'b, CTX> {
        let root = state_subkey(ARBOS_STATE_PROGRAMS_KEY);
        let slot = substorage(&root, ARBOS_PROGRAMS_STATE_CACHE_MANAGERS_KEY);
        StorageBackedAddressSet::new(self.context, self.gas.as_deref_mut(), slot)
    }

    fn is_chain_owner(&mut self, address: Address) -> Result<bool, ArbosStateError> {
        self.chain_owners().contains(address)
    }

    fn is_native_token_owner(&mut self, address: Address) -> Result<bool, ArbosStateError> {
        self.native_token_owners().contains(address)
    }

    fn upgrade_timestamp(&mut self) -> StorageBackedU64<'_, CTX> {
        StorageBackedU64::new(
            self.context,
            self.gas.as_deref_mut(),
            state_slot(ARBOS_STATE_UPGRADE_TIMESTAMP_OFFSET),
        )
    }

    fn upgrade_version(&mut self) -> StorageBackedU64<'_, CTX> {
        StorageBackedU64::new(
            self.context,
            self.gas.as_deref_mut(),
            state_slot(ARBOS_STATE_UPGRADE_VERSION_OFFSET),
        )
    }

    fn network_fee_account(&mut self) -> StorageBackedAddress<'_, CTX> {
        StorageBackedAddress::new(
            self.context,
            self.gas.as_deref_mut(),
            state_slot(ARBOS_STATE_NETWORK_FEE_ACCOUNT_OFFSET),
        )
    }

    fn infra_fee_account(&mut self) -> StorageBackedAddress<'_, CTX> {
        StorageBackedAddress::new(
            self.context,
            self.gas.as_deref_mut(),
            state_slot(ARBOS_STATE_INFRA_FEE_ACCOUNT_OFFSET),
        )
    }

    fn chain_id(&mut self) -> StorageBackedU256<'_, CTX> {
        StorageBackedU256::new(
            self.context,
            self.gas.as_deref_mut(),
            state_slot(ARBOS_STATE_CHAIN_ID_OFFSET),
        )
    }

    fn genesis_block_num(&mut self) -> StorageBackedU64<'_, CTX> {
        StorageBackedU64::new(
            self.context,
            self.gas.as_deref_mut(),
            state_slot(ARBOS_STATE_GENESIS_BLOCK_NUM_OFFSET),
        )
    }

    fn brotli_compression_level(&mut self) -> StorageBackedU64<'_, CTX> {
        StorageBackedU64::new(
            self.context,
            self.gas.as_deref_mut(),
            state_slot(ARBOS_STATE_BROTLI_COMPRESSION_LEVEL_OFFSET),
        )
    }

    fn native_token_enabled_time(&mut self) -> StorageBackedU64<'_, CTX> {
        StorageBackedU64::new(
            self.context,
            self.gas.as_deref_mut(),
            state_slot(ARBOS_STATE_NATIVE_TOKEN_ENABLED_FROM_TIME_OFFSET),
        )
    }

    fn address_table(&mut self) -> AddressTable<'_, CTX> {
        AddressTable::new(
            self.context,
            self.gas.as_deref_mut(),
            state_subkey(ARBOS_STATE_ADDRESS_TABLE_KEY),
        )
    }

    fn l1_pricing(&mut self) -> L1Pricing<'_, CTX> {
        L1Pricing::new(
            self.context,
            self.gas.as_deref_mut(),
            state_subkey(ARBOS_STATE_L1_PRICING_KEY),
        )
    }

    fn l2_pricing(&mut self) -> L2Pricing<'_, CTX> {
        L2Pricing::new(
            self.context,
            self.gas.as_deref_mut(),
            state_subkey(ARBOS_STATE_L2_PRICING_KEY),
        )
    }

    fn retryable_state(&mut self) -> RetryableState<'_, CTX> {
        RetryableState::new(
            self.context,
            self.gas.as_deref_mut(),
            state_subkey(ARBOS_STATE_RETRYABLES_KEY),
        )
    }

    fn retryable<'b>(&'b mut self, id: B256) -> Retryable<'b, CTX> {
        let root = state_subkey(ARBOS_STATE_RETRYABLES_KEY);
        Retryable::new(self.context, self.gas.as_deref_mut(), substorage(&root, id.as_slice()))
    }

    fn timeout_queue(&mut self) -> StorageBackedQueue<'_, CTX> {
        let root = state_subkey(ARBOS_STATE_RETRYABLES_KEY);
        let slot = substorage(&root, &[0]);
        StorageBackedQueue::new(self.context, self.gas.as_deref_mut(), slot)
    }
}
