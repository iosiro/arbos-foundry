use revm::{context::{Host, JournalTr}, primitives::{B256, U256}};

use crate::{state::{program::Programs, types::{map_address, substorage, StorageBackedAddress, StorageBackedAddressSet, StorageBackedU256, StorageBackedU64}}, constants::{ARBOS_CHAIN_OWNERS_KEY, ARBOS_STATE_ADDRESS, ARBOS_STATE_NATIVE_TOKEN_OWNER_KEY}, ArbitrumContextTr};

pub mod address_table;
pub mod program;
mod types;

const ARBOS_STATE_VERSION_OFFSET: u8 = 0;
const ARBOS_STATE_UPGRADE_VERSION_OFFSET: u8 = 1;
const ARBOS_STATE_UPGRADE_TIMESTAMP_OFFSET: u8 = 2;
const ARBOS_STATE_NETWORK_FEE_ACCOUNT_OFFSET: u8 = 3;
const ARBOS_STATE_CHAIN_ID_OFFSET: u8 = 4;
const ARBOS_STATE_GENESIS_BLOCK_NUM_OFFSET: u8 = 5;
const ARBOS_STATE_INFRA_FEE_ACCOUNT_OFFSET: u8 = 6;
const ARBOS_STATE_BROTLI_COMPRESSION_LEVEL_OFFSET: u8 = 7;
const ARBOS_STATE_NATIVE_TOKEN_ENABLED_FROM_TIME_OFFSET: u8 = 8;

pub trait ArbStateGetter<CTX: ArbitrumContextTr> {
    fn programs(&mut self) -> Programs<'_, CTX>;
    fn chain_owners<'b>(&'b mut self) -> StorageBackedAddressSet<'b, CTX>;
    fn native_token_owners<'b>(&'b mut self) -> StorageBackedAddressSet<'b, CTX>;
    fn upgrade_timestamp(&mut self) ->  StorageBackedU64<'_, CTX>;
    fn upgrade_version(&mut self) ->  StorageBackedU64<'_, CTX>;
    fn network_fee_account(&mut self) ->  StorageBackedAddress<'_, CTX>;
    fn infra_fee_account(&mut self) ->  StorageBackedAddress<'_, CTX>;
    fn chain_id(&mut self) ->  StorageBackedU256<'_, CTX>;
    fn genesis_block_num(&mut self) ->  StorageBackedU64<'_, CTX>;
    fn brotli_compression_level(&mut self) ->  StorageBackedU64<'_, CTX>;
    fn native_token_enabled_time(&mut self) ->  StorageBackedU64<'_, CTX>;
    fn address_table(&mut self) -> 
}

pub trait ArbState<'a, CTX: ArbitrumContextTr> {
    type ArbStateGetterType: ArbStateGetter<CTX>;
    fn arb_state(&'a mut self) ->  Self::ArbStateGetterType;
}

impl<'a, CTX: ArbitrumContextTr + 'a> ArbState<'a, CTX> for CTX {
    type ArbStateGetterType = ArbStateWrapper<'a, CTX>;
    fn arb_state(&'a mut self) -> Self::ArbStateGetterType {
        ArbStateWrapper::new(self)
    }
}

pub struct ArbStateWrapper<'a, CTX: ArbitrumContextTr> {
    context: &'a mut CTX,
}

impl<'a, CTX: ArbitrumContextTr> ArbStateWrapper<'a, CTX> {
    pub fn new(context: &'a mut CTX) -> Self {
        ArbStateWrapper { context }
    }
}

impl<'a, CTX> ArbStateGetter<CTX> for ArbStateWrapper<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    fn programs(&mut self) -> Programs<'_, CTX> { 
        self.context.journal_mut().warm_account(ARBOS_STATE_ADDRESS).expect("arbos state must exist");
        Programs::new(self.context)
    }

    fn chain_owners<'b>(&'b mut self) -> StorageBackedAddressSet<'b, CTX> {
        let subkey = substorage(&B256::ZERO, ARBOS_CHAIN_OWNERS_KEY);
        StorageBackedAddressSet::new(self.context, subkey)
    }

    fn native_token_owners<'b>(&'b mut self) -> StorageBackedAddressSet<'b, CTX> {
        let subkey = substorage(&B256::ZERO, ARBOS_STATE_NATIVE_TOKEN_OWNER_KEY);
        StorageBackedAddressSet::new(self.context, subkey)
    }

    fn upgrade_timestamp(&mut self) ->  StorageBackedU64<'_, CTX> {
        StorageBackedU64::new(
            self.context,
            map_address(&B256::ZERO, &B256::from(U256::from(ARBOS_STATE_UPGRADE_TIMESTAMP_OFFSET as u64))),
        )
    }

    fn upgrade_version(&mut self) ->  StorageBackedU64<'_, CTX> {
        StorageBackedU64::new(
            self.context,
            map_address(&B256::ZERO, &B256::from(U256::from(ARBOS_STATE_UPGRADE_VERSION_OFFSET as u64))),
        )
    }

    fn network_fee_account(&mut self) ->  StorageBackedAddress<'_, CTX> {
        StorageBackedAddress::new(
            self.context,
            map_address(&B256::ZERO, &B256::from(U256::from(ARBOS_STATE_NETWORK_FEE_ACCOUNT_OFFSET as u64))),
        )
    }

    fn infra_fee_account(&mut self) ->  StorageBackedAddress<'_, CTX> {
        StorageBackedAddress::new(
            self.context,
            map_address(&B256::ZERO, &B256::from(U256::from(ARBOS_STATE_INFRA_FEE_ACCOUNT_OFFSET as u64))),
        )
    }

    fn chain_id(&mut self) ->  StorageBackedU256<'_, CTX> {
        StorageBackedU256::new(
            self.context,
            map_address(&B256::ZERO, &B256::from(U256::from(ARBOS_STATE_CHAIN_ID_OFFSET as u64))),
        )
    }

    fn genesis_block_num(&mut self) ->  StorageBackedU64<'_, CTX> {
        StorageBackedU64::new(
            self.context,
            map_address(&B256::ZERO, &B256::from(U256::from(ARBOS_STATE_GENESIS_BLOCK_NUM_OFFSET as u64))),
        )
    }

    fn brotli_compression_level(&mut self) ->  StorageBackedU64<'_, CTX> {
        StorageBackedU64::new(
            self.context,
            map_address(&B256::ZERO, &B256::from(U256::from(ARBOS_STATE_BROTLI_COMPRESSION_LEVEL_OFFSET as u64))),
        )
    }

    fn native_token_enabled_time(&mut self) ->  StorageBackedU64<'_, CTX> {
        StorageBackedU64::new(
            self.context,
            map_address(&B256::ZERO, &B256::from(U256::from(ARBOS_STATE_NATIVE_TOKEN_ENABLED_FROM_TIME_OFFSET as u64))),
        )
    }
}
