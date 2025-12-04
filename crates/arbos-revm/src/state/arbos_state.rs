use revm::{
    context::JournalTr,
    interpreter::{Gas, gas::COLD_ACCOUNT_ACCESS_COST},
    primitives::{Address, B256, U256},
};

use crate::{
    ArbitrumContextTr,
    constants::{
        ARBOS_CHAIN_CONFIG_KEY, ARBOS_CHAIN_OWNERS_KEY, ARBOS_STATE_ADDRESS,
        ARBOS_STATE_ADDRESS_TABLE_KEY, ARBOS_STATE_BLOCKHASHES_KEY, ARBOS_STATE_FEATURES_KEY,
        ARBOS_STATE_L1_PRICING_KEY, ARBOS_STATE_L2_PRICING_KEY, ARBOS_STATE_NATIVE_TOKEN_OWNER_KEY,
        ARBOS_STATE_PROGRAMS_KEY, ARBOS_STATE_RETRYABLES_KEY,
    },
    state::{
        address_table::AddressTable,
        block_hashes::BlockHashes,
        l1_pricing::L1Pricing,
        l2_pricing::L2Pricing,
        program::{DataPricerParams, Programs, StylusParams},
        retryable::{Retryable, RetryableState},
        types::{
            ArbosStateError, StorageBackedAddress, StorageBackedAddressSet, StorageBackedQueue,
            StorageBackedTr, StorageBackedU64, StorageBackedU256, map_address, substorage,
        },
    },
};

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
    fn is_chain_owner(&mut self, address: Address) -> Result<bool, ArbosStateError>;
    fn is_native_token_owner(&mut self, address: Address) -> Result<bool, ArbosStateError>;
    fn code_hash(&mut self, address: Address) -> Result<B256, ArbosStateError>;
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
    fn features(&mut self) -> StorageBackedU256<'_, CTX>;
    fn chain_config(&mut self) -> StorageBackedU256<'_, CTX>;
    fn blockhashes(&mut self) -> BlockHashes<'_, CTX>;
}

pub trait ArbState<'a, CTX: ArbitrumContextTr> {
    type ArbStateGetterType: ArbStateGetter<CTX>;
    fn arb_state(
        &'a mut self,
        gas: Option<&'a mut Gas>,
        is_static: bool,
    ) -> Self::ArbStateGetterType;
}

impl<'a, CTX: ArbitrumContextTr + 'a> ArbState<'a, CTX> for CTX {
    type ArbStateGetterType = ArbStateWrapper<'a, CTX>;
    fn arb_state(
        &'a mut self,
        gas: Option<&'a mut Gas>,
        is_static: bool,
    ) -> Self::ArbStateGetterType {
        ArbStateWrapper::new(self, gas, is_static)
    }
}

pub struct ArbStateWrapper<'a, CTX: ArbitrumContextTr> {
    context: &'a mut CTX,
    gas: Option<&'a mut Gas>,
    is_static: bool,
}

#[derive(Clone, Debug)]
pub struct ArbosStateParams {
    pub upgrade_version: u64,
    pub upgrade_timestamp: u64,
    pub network_fee_account: Address,
    pub infra_fee_account: Address,
    pub chain_id: U256,
    pub genesis_block_num: u64,
    pub brotli_compression_level: u64,
    pub native_token_enabled_time: u64,
    pub stylus_params: StylusParams,
    pub data_pricer_params: DataPricerParams,
}

impl Default for ArbosStateParams {
    fn default() -> Self {
        Self {
            upgrade_version: 0,
            upgrade_timestamp: 0,
            network_fee_account: Address::ZERO,
            infra_fee_account: Address::ZERO,
            chain_id: U256::ZERO,
            genesis_block_num: 0,
            brotli_compression_level: 1,
            native_token_enabled_time: 0,
            stylus_params: StylusParams::default(),
            data_pricer_params: DataPricerParams::default(),
        }
    }
}

impl<'a, CTX: ArbitrumContextTr> ArbStateWrapper<'a, CTX> {
    pub fn new(context: &'a mut CTX, mut gas: Option<&'a mut Gas>, is_static: bool) -> Self {
        if let Err(err) = context.journal_mut().load_account_info_skip_cold_load(
            ARBOS_STATE_ADDRESS,
            false,
            false,
        ) {
            // Consume all gas so downstream callers see a consistent failure state rather than a
            // panic.
            if let Some(gas) = gas.as_deref_mut() {
                gas.spend_all();
            }
            let _ = err;
        }
        Self { context, gas, is_static }
    }
}

impl<'a, CTX> ArbStateWrapper<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    pub fn initialize(&mut self, params: &ArbosStateParams) -> Result<(), ArbosStateError> {
        self.upgrade_version().set(params.upgrade_version)?;
        self.upgrade_timestamp().set(params.upgrade_timestamp)?;
        self.network_fee_account().set(params.network_fee_account)?;
        self.infra_fee_account().set(params.infra_fee_account)?;
        self.chain_id().set(params.chain_id)?;
        self.genesis_block_num().set(params.genesis_block_num)?;
        self.brotli_compression_level().set(params.brotli_compression_level)?;
        self.native_token_enabled_time().set(params.native_token_enabled_time)?;

        self.programs().initialize(&params.stylus_params, &params.data_pricer_params)?;

        Ok(())
    }

    pub fn get(&mut self) -> Result<ArbosStateParams, ArbosStateError> {
        Ok(ArbosStateParams {
            upgrade_version: self.upgrade_version().get()?,
            upgrade_timestamp: self.upgrade_timestamp().get()?,
            network_fee_account: self.network_fee_account().get()?,
            infra_fee_account: self.infra_fee_account().get()?,
            chain_id: self.chain_id().get()?,
            genesis_block_num: self.genesis_block_num().get()?,
            brotli_compression_level: self.brotli_compression_level().get()?,
            native_token_enabled_time: self.native_token_enabled_time().get()?,
            stylus_params: self.programs().stylus_params().get()?,
            data_pricer_params: self.programs().data_pricer().get()?,
        })
    }
}

impl<'a, CTX> ArbStateGetter<CTX> for ArbStateWrapper<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    fn programs(&mut self) -> Programs<'_, CTX> {
        Programs::new(
            self.context,
            self.gas.as_deref_mut(),
            self.is_static,
            state_subkey(ARBOS_STATE_PROGRAMS_KEY),
        )
    }

    fn brotli_compression_level(&mut self) -> StorageBackedU64<'_, CTX> {
        StorageBackedU64::new(
            self.context,
            self.gas.as_deref_mut(),
            self.is_static,
            state_slot(ARBOS_STATE_BROTLI_COMPRESSION_LEVEL_OFFSET),
        )
    }
    fn native_token_enabled_time(&mut self) -> StorageBackedU64<'_, CTX> {
        StorageBackedU64::new(
            self.context,
            self.gas.as_deref_mut(),
            self.is_static,
            state_slot(ARBOS_STATE_NATIVE_TOKEN_ENABLED_FROM_TIME_OFFSET),
        )
    }
    fn infra_fee_account(&mut self) -> StorageBackedAddress<'_, CTX> {
        StorageBackedAddress::new(
            self.context,
            self.gas.as_deref_mut(),
            self.is_static,
            state_slot(ARBOS_STATE_INFRA_FEE_ACCOUNT_OFFSET),
        )
    }
    fn genesis_block_num(&mut self) -> StorageBackedU64<'_, CTX> {
        StorageBackedU64::new(
            self.context,
            self.gas.as_deref_mut(),
            self.is_static,
            state_slot(ARBOS_STATE_GENESIS_BLOCK_NUM_OFFSET),
        )
    }
    fn upgrade_version(&mut self) -> StorageBackedU64<'_, CTX> {
        StorageBackedU64::new(
            self.context,
            self.gas.as_deref_mut(),
            self.is_static,
            state_slot(ARBOS_STATE_UPGRADE_VERSION_OFFSET),
        )
    }
    fn upgrade_timestamp(&mut self) -> StorageBackedU64<'_, CTX> {
        StorageBackedU64::new(
            self.context,
            self.gas.as_deref_mut(),
            self.is_static,
            state_slot(ARBOS_STATE_UPGRADE_TIMESTAMP_OFFSET),
        )
    }
    fn chain_id(&mut self) -> StorageBackedU256<'_, CTX> {
        StorageBackedU256::new(
            self.context,
            self.gas.as_deref_mut(),
            self.is_static,
            state_slot(ARBOS_STATE_CHAIN_ID_OFFSET),
        )
    }

    fn blockhashes(&mut self) -> BlockHashes<'_, CTX> {
        BlockHashes::new(
            self.context,
            self.gas.as_deref_mut(),
            self.is_static,
            state_subkey(ARBOS_STATE_BLOCKHASHES_KEY),
        )
    }

    fn chain_owners<'b>(&'b mut self) -> StorageBackedAddressSet<'b, CTX> {
        StorageBackedAddressSet::new(
            self.context,
            self.gas.as_deref_mut(),
            self.is_static,
            state_subkey(ARBOS_CHAIN_OWNERS_KEY),
        )
    }

    fn chain_config(&mut self) -> StorageBackedU256<'_, CTX> {
        let slot = map_address(&state_subkey(ARBOS_CHAIN_CONFIG_KEY), &B256::ZERO);
        StorageBackedU256::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
    }

    fn features(&mut self) -> StorageBackedU256<'_, CTX> {
        let slot = map_address(&state_subkey(ARBOS_STATE_FEATURES_KEY), &B256::ZERO);
        StorageBackedU256::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
    }

    fn native_token_owners<'b>(&'b mut self) -> StorageBackedAddressSet<'b, CTX> {
        StorageBackedAddressSet::new(
            self.context,
            self.gas.as_deref_mut(),
            self.is_static,
            state_subkey(ARBOS_STATE_NATIVE_TOKEN_OWNER_KEY),
        )
    }

    fn is_chain_owner(&mut self, address: Address) -> Result<bool, ArbosStateError> {
        self.chain_owners().contains(address)
    }

    fn is_native_token_owner(&mut self, address: Address) -> Result<bool, ArbosStateError> {
        self.native_token_owners().contains(address)
    }

    fn address_table(&mut self) -> AddressTable<'_, CTX> {
        AddressTable::new(
            self.context,
            self.gas.as_deref_mut(),
            self.is_static,
            state_subkey(ARBOS_STATE_ADDRESS_TABLE_KEY),
        )
    }

    fn l1_pricing(&mut self) -> L1Pricing<'_, CTX> {
        L1Pricing::new(
            self.context,
            self.gas.as_deref_mut(),
            self.is_static,
            state_subkey(ARBOS_STATE_L1_PRICING_KEY),
        )
    }

    fn l2_pricing(&mut self) -> L2Pricing<'_, CTX> {
        L2Pricing::new(
            self.context,
            self.gas.as_deref_mut(),
            self.is_static,
            state_subkey(ARBOS_STATE_L2_PRICING_KEY),
        )
    }

    fn retryable_state(&mut self) -> RetryableState<'_, CTX> {
        RetryableState::new(
            self.context,
            self.gas.as_deref_mut(),
            self.is_static,
            state_subkey(ARBOS_STATE_RETRYABLES_KEY),
        )
    }

    fn retryable<'b>(&'b mut self, id: B256) -> Retryable<'b, CTX> {
        let root = state_subkey(ARBOS_STATE_RETRYABLES_KEY);
        Retryable::new(
            self.context,
            self.gas.as_deref_mut(),
            self.is_static,
            substorage(&root, id.as_slice()),
        )
    }

    fn timeout_queue(&mut self) -> StorageBackedQueue<'_, CTX> {
        let root = state_subkey(ARBOS_STATE_RETRYABLES_KEY);
        let slot = substorage(&root, &[0]);
        StorageBackedQueue::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
    }

    fn code_hash(&mut self, address: Address) -> Result<B256, ArbosStateError> {
        let code_hash =
            self.context.load_account_code_hash(address).map(|s| s.data).unwrap_or_default();
        if let Some(gas) = self.gas.as_deref_mut()
            && !gas.record_cost(COLD_ACCOUNT_ACCESS_COST)
        {
            return Err(ArbosStateError::OutOfGas);
        }
        Ok(code_hash)
    }

    fn network_fee_account(&mut self) -> StorageBackedAddress<'_, CTX> {
        StorageBackedAddress::new(
            self.context,
            self.gas.as_deref_mut(),
            self.is_static,
            state_slot(ARBOS_STATE_NETWORK_FEE_ACCOUNT_OFFSET),
        )
    }
}
