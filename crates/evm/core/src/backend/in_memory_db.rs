//! In-memory database.

use crate::state_snapshot::StateSnapshots;
use alloy_primitives::{Address, B256, U256};
use foundry_fork_db::DatabaseError;
use revm::{
    Database, DatabaseCommit,
    bytecode::Bytecode,
    database::{CacheDB, DatabaseRef, EmptyDB},
    primitives::AddressMap,
    state::{Account, AccountInfo},
};

/// Type alias for an in-memory database.
///
/// See [`EmptyDBWrapper`].
pub type FoundryEvmInMemoryDB = CacheDB<EmptyDBWrapper>;

/// In-memory [`Database`] for Anvil.
///
/// This acts like a wrapper type for [`FoundryEvmInMemoryDB`] but is capable of applying snapshots.
#[derive(Debug)]
pub struct MemDb {
    pub inner: FoundryEvmInMemoryDB,
    pub state_snapshots: StateSnapshots<FoundryEvmInMemoryDB>,
}

impl Default for MemDb {
    fn default() -> Self {
        Self { inner: CacheDB::new(Default::default()), state_snapshots: Default::default() }
    }
}

impl DatabaseRef for MemDb {
    type Error = DatabaseError;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        DatabaseRef::basic_ref(&self.inner, address)
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        DatabaseRef::code_by_hash_ref(&self.inner, code_hash)
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        DatabaseRef::storage_ref(&self.inner, address, index)
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        DatabaseRef::block_hash_ref(&self.inner, number)
    }
}

impl Database for MemDb {
    type Error = DatabaseError;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        Database::basic(&mut self.inner, address)
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        Database::code_by_hash(&mut self.inner, code_hash)
    }

    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        Database::storage(&mut self.inner, address, index)
    }

    fn block_hash(&mut self, number: u64) -> Result<B256, Self::Error> {
        Database::block_hash(&mut self.inner, number)
    }
}

impl DatabaseCommit for MemDb {
    fn commit(&mut self, changes: AddressMap<Account>) {
        DatabaseCommit::commit(&mut self.inner, changes)
    }
}

/// An empty database that always returns default values when queried.
///
/// This is just a simple wrapper for `revm::EmptyDB` but implements `DatabaseError` instead, this
/// way we can unify all different `Database` impls
///
/// Missing accounts remain absent. Returning a default account would make Stylus
/// `account_codehash` confuse a nonexistent account with an existing empty account.
/// `CacheDB::insert_account_info` clears `NotExisting` when an account is inserted later.
#[derive(Clone, Debug, Default)]
pub struct EmptyDBWrapper(EmptyDB);

impl DatabaseRef for EmptyDBWrapper {
    type Error = DatabaseError;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        Ok(self.0.basic_ref(address)?)
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        Ok(self.0.code_by_hash_ref(code_hash)?)
    }
    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        Ok(self.0.storage_ref(address, index)?)
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        Ok(self.0.block_hash_ref(number)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::b256;

    /// Ensures the `Database(Ref)` implementation for `revm::CacheDB` works as expected
    ///
    /// Demonstrates how calling `Database::basic` works if an account does not exist
    #[test]
    fn cache_db_insert_basic_non_existing() {
        let mut db = CacheDB::new(EmptyDB::default());
        let address = Address::random();
        // call `basic` on a non-existing account
        let info = Database::basic(&mut db, address).unwrap();
        assert!(info.is_none());

        let mut info = info.unwrap_or_default();
        info.balance = U256::from(500u64);

        // insert the modified account info
        db.insert_account_info(address, info);

        // now we can call `basic` again and it should return the inserted account info
        let info = Database::basic(&mut db, address).unwrap();
        assert!(info.is_some());
    }

    /// Demonstrates how to insert a new account but not mark it as non-existing
    #[test]
    fn cache_db_insert_basic_default() {
        let mut db = CacheDB::new(EmptyDB::default());
        let address = Address::random();

        // We use `basic_ref` here to ensure that the account is not marked as `NotExisting`.
        let info = DatabaseRef::basic_ref(&db, address).unwrap();
        assert!(info.is_none());
        let mut info = info.unwrap_or_default();
        info.balance = U256::from(500u64);

        // insert the modified account info
        db.insert_account_info(address, info.clone());

        let loaded = Database::basic(&mut db, address).unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap(), info)
    }

    /// Loading a missing account does not prevent inserting it later.
    #[test]
    fn mem_db_insert_basic_default() {
        let mut db = MemDb::default();
        let address = Address::from_word(b256!(
            "0x000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa96045"
        ));

        let info = Database::basic(&mut db, address).unwrap();
        assert!(info.is_none());
        assert!(DatabaseRef::basic_ref(&db, address).unwrap().is_none());
        let mut info = info.unwrap_or_default();
        info.balance = U256::from(500u64);

        // insert the modified account info
        db.inner.insert_account_info(address, info.clone());

        let loaded = Database::basic(&mut db, address).unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap(), info)
    }

    #[test]
    fn mem_db_distinguishes_missing_and_existing_empty() {
        let mut db = MemDb::default();
        let address = Address::random();
        assert!(Database::basic(&mut db, address).unwrap().is_none());
        db.inner.insert_account_info(address, AccountInfo::default());
        assert_eq!(Database::basic(&mut db, address).unwrap(), Some(AccountInfo::default()));
        assert_eq!(DatabaseRef::basic_ref(&db, address).unwrap(), Some(AccountInfo::default()));
    }
}
