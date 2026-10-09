//! Account-existence normalization at the remote-state boundary.

use alloy_primitives::{Address, B256, U256};
use revm::{bytecode::Bytecode, database::DatabaseRef, state::AccountInfo};
use std::ops::{Deref, DerefMut};

/// Adapts RPC account data before it enters the local state cache.
///
/// RPC balance, nonce, and code queries return default values for missing accounts.
/// On post-EIP-161 chains those empty remote accounts are absent, not existing empty
/// accounts. Local cache entries are deliberately not filtered: an empty account
/// can be created or touched during execution, and Stylus observes that distinction.
#[derive(Clone, Debug)]
pub struct RemoteAccountDB<DB>(pub DB);

impl<DB> Deref for RemoteAccountDB<DB> {
    type Target = DB;

    fn deref(&self) -> &DB {
        &self.0
    }
}

impl<DB> DerefMut for RemoteAccountDB<DB> {
    fn deref_mut(&mut self) -> &mut DB {
        &mut self.0
    }
}

impl<DB: DatabaseRef> DatabaseRef for RemoteAccountDB<DB> {
    type Error = DB::Error;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        Ok(self.0.basic_ref(address)?.filter(|info| !info.is_empty()))
    }

    fn code_by_hash_ref(&self, hash: B256) -> Result<Bytecode, Self::Error> {
        self.0.code_by_hash_ref(hash)
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        self.0.storage_ref(address, index)
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        self.0.block_hash_ref(number)
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
        let mut db = CacheDB::new(RemoteAccountDB(remote));
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
            let mut db = CacheDB::new(RemoteAccountDB(remote));
            assert_eq!(db.basic_ref(address).unwrap(), Some(info.clone()));
            assert_eq!(db.basic(address).unwrap(), Some(info));
        }
    }
}
