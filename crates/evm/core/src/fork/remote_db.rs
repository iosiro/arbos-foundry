//! Account-existence normalization at the remote-state boundary.

use alloy_primitives::{Address, B256, U256};
use revm::{
    bytecode::Bytecode, database::DatabaseRef, primitives::hardfork::SpecId, state::AccountInfo,
};
use std::ops::{Deref, DerefMut};

/// Adapts RPC account data before it enters the local state cache.
///
/// RPC balance, nonce, and code queries return default values for missing accounts.
/// On post-EIP-161 chains those empty remote accounts are absent, not existing empty
/// accounts. Local cache entries are deliberately not filtered: an empty account
/// can be created or touched during execution, and Stylus observes that distinction.
/// Before EIP-161, preserve the RPC's empty account information: empty accounts
/// could persist on-chain and treating them as absent changes CALL gas accounting.
#[derive(Clone, Debug)]
pub struct RemoteAccountDB<DB> {
    inner: DB,
    state_clear: bool,
}

impl<DB> RemoteAccountDB<DB> {
    /// Wraps remote state using the post-EIP-161 account-existence rules.
    pub const fn new(inner: DB) -> Self {
        Self { inner, state_clear: true }
    }

    /// Returns the underlying remote database.
    pub const fn inner(&self) -> &DB {
        &self.inner
    }

    /// Wraps remote state using the selected execution hardfork's account rules.
    pub const fn with_spec(inner: DB, spec: SpecId) -> Self {
        Self { inner, state_clear: spec.is_enabled_in(SpecId::SPURIOUS_DRAGON) }
    }

    /// Updates account rules before loading remote state for execution.
    pub const fn set_spec_id(&mut self, spec: SpecId) {
        self.state_clear = spec.is_enabled_in(SpecId::SPURIOUS_DRAGON);
    }

    pub(crate) fn normalize_account(&self, info: AccountInfo) -> Option<AccountInfo> {
        (!self.state_clear || !info.is_empty()).then_some(info)
    }
}

impl<DB> Deref for RemoteAccountDB<DB> {
    type Target = DB;

    fn deref(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DerefMut for RemoteAccountDB<DB> {
    fn deref_mut(&mut self) -> &mut DB {
        &mut self.inner
    }
}

impl<DB: DatabaseRef> DatabaseRef for RemoteAccountDB<DB> {
    type Error = DB::Error;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        Ok(self.inner.basic_ref(address)?.and_then(|info| self.normalize_account(info)))
    }

    fn code_by_hash_ref(&self, hash: B256) -> Result<Bytecode, Self::Error> {
        self.inner.code_by_hash_ref(hash)
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        self.inner.storage_ref(address, index)
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        self.inner.block_hash_ref(number)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use revm::{
        Database,
        database::{CacheDB, EmptyDB},
    };

    #[test]
    fn fork_remote_empty_is_missing_but_local_empty_exists() {
        let address = Address::random();
        let mut remote = CacheDB::new(EmptyDB::default());
        remote.insert_account_info(address, AccountInfo::default());
        let mut db = CacheDB::new(RemoteAccountDB::new(remote));
        assert!(db.basic_ref(address).unwrap().is_none());
        assert!(db.basic(address).unwrap().is_none());
        db.insert_account_info(address, AccountInfo::default());
        assert_eq!(db.basic_ref(address).unwrap(), Some(AccountInfo::default()));
        assert_eq!(db.basic(address).unwrap(), Some(AccountInfo::default()));
    }

    #[test]
    fn fork_remote_nonempty_accounts_keep_their_hashes() {
        for info in [
            AccountInfo { balance: U256::from(1), ..Default::default() },
            AccountInfo { nonce: 1, ..Default::default() },
            AccountInfo { code_hash: alloy_primitives::keccak256([0]), ..Default::default() },
        ] {
            let address = Address::random();
            let mut remote = CacheDB::new(EmptyDB::default());
            remote.insert_account_info(address, info.clone());
            let mut db = CacheDB::new(RemoteAccountDB::new(remote));
            assert_eq!(db.basic_ref(address).unwrap(), Some(info.clone()));
            assert_eq!(db.basic(address).unwrap(), Some(info));
        }
    }

    #[test]
    fn fork_remote_empty_accounts_follow_state_clear_hardfork() {
        let address = Address::with_last_byte(4);
        let mut remote = CacheDB::new(EmptyDB::default());
        remote.insert_account_info(address, AccountInfo::default());
        let mut db = RemoteAccountDB::with_spec(remote, SpecId::HOMESTEAD);
        assert_eq!(db.basic_ref(address).unwrap(), Some(AccountInfo::default()));
        db.set_spec_id(SpecId::SPURIOUS_DRAGON);
        assert_eq!(db.basic_ref(address).unwrap(), None);
        db.set_spec_id(SpecId::HOMESTEAD);
        assert_eq!(db.basic_ref(address).unwrap(), Some(AccountInfo::default()));
    }
}
