//! EVM traits.

use crate::Database;
use alloy_primitives::{Address, Log, B256, U256};
use core::{error::Error, fmt, fmt::Debug};
use revm::{
    context::{Block, BlockEnv, ContextTr, DBErrorMarker, JournalTr},
    context_interface::block::BlobExcessGasAndPrice,
    interpreter::{SStoreResult, StateLoad},
    primitives::{StorageKey, StorageValue},
    state::{Account, AccountInfo, Bytecode},
};

/// Erased error type.
#[derive(thiserror::Error, Debug)]
#[error(transparent)]
pub struct ErasedError(Box<dyn Error + Send + Sync + 'static>);

impl ErasedError {
    /// Creates a new [`ErasedError`].
    pub fn new(error: impl Error + Send + Sync + 'static) -> Self {
        Self(Box::new(error))
    }
}

impl DBErrorMarker for ErasedError {}

/// Errors returned by [`EvmInternals`].
#[derive(Debug, thiserror::Error)]
pub enum EvmInternalsError {
    /// Database error.
    #[error(transparent)]
    Database(ErasedError),
}

impl EvmInternalsError {
    /// Creates a new [`EvmInternalsError::Database`]
    pub fn database(err: impl Error + Send + Sync + 'static) -> Self {
        Self::Database(ErasedError::new(err))
    }
}

/// dyn-compatible trait for accessing and modifying EVM internals, particularly the journal.
///
/// This trait provides an abstraction over journal operations without exposing
/// associated types, making it object-safe and suitable for dynamic dispatch.
trait EvmInternalsTr: Database<Error = ErasedError> + Debug {
    fn load_account(
        &mut self,
        address: Address,
    ) -> Result<StateLoad<&mut Account>, EvmInternalsError>;

    fn load_account_code(
        &mut self,
        address: Address,
    ) -> Result<StateLoad<&mut Account>, EvmInternalsError>;

    fn sload(
        &mut self,
        address: Address,
        key: StorageKey,
    ) -> Result<StateLoad<StorageValue>, EvmInternalsError>;

    fn touch_account(&mut self, address: Address);

    fn set_code(&mut self, address: Address, code: Bytecode);

    fn sstore(
        &mut self,
        address: Address,
        key: StorageKey,
        value: StorageValue,
    ) -> Result<StateLoad<SStoreResult>, EvmInternalsError>;

    fn log(&mut self, log: Log);

    fn block(&self) -> &dyn Block;
}

/// Helper internal struct for implementing [`EvmInternals`].
#[derive(Debug)]
struct EvmInternalsImpl<'a, T>(&'a mut T);

impl<T> revm::Database for EvmInternalsImpl<'_, T>
where
    T: ContextTr,
    T::Db: Database,
{
    type Error = ErasedError;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        self.0.db_mut().basic(address).map_err(ErasedError::new)
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.0.db_mut().code_by_hash(code_hash).map_err(ErasedError::new)
    }

    fn storage(
        &mut self,
        address: Address,
        index: StorageKey,
    ) -> Result<StorageValue, Self::Error> {
        self.0.db_mut().storage(address, index).map_err(ErasedError::new)
    }

    fn block_hash(&mut self, number: u64) -> Result<B256, Self::Error> {
        self.0.db_mut().block_hash(number).map_err(ErasedError::new)
    }
}

impl<T> EvmInternalsTr for EvmInternalsImpl<'_, T>
where
    T: ContextTr + Debug,
    T::Db: Database,
{
    fn load_account(
        &mut self,
        address: Address,
    ) -> Result<StateLoad<&mut Account>, EvmInternalsError> {
        self.0.journal_mut().load_account(address).map_err(EvmInternalsError::database)
    }

    fn load_account_code(
        &mut self,
        address: Address,
    ) -> Result<StateLoad<&mut Account>, EvmInternalsError> {
        self.0.journal_mut().load_account_code(address).map_err(EvmInternalsError::database)
    }

    fn sload(
        &mut self,
        address: Address,
        key: StorageKey,
    ) -> Result<StateLoad<StorageValue>, EvmInternalsError> {
        self.0.journal_mut().sload(address, key).map_err(EvmInternalsError::database)
    }

    fn touch_account(&mut self, address: Address) {
        self.0.journal_mut().touch_account(address);
    }

    fn set_code(&mut self, address: Address, code: Bytecode) {
        self.0.journal_mut().set_code(address, code);
    }

    fn sstore(
        &mut self,
        address: Address,
        key: StorageKey,
        value: StorageValue,
    ) -> Result<StateLoad<SStoreResult>, EvmInternalsError> {
        self.0.journal_mut().sstore(address, key, value).map_err(EvmInternalsError::database)
    }

    fn log(&mut self, log: Log) {
        self.0.journal_mut().log(log);
    }

    fn block(&self) -> &dyn Block {
        self.0.block()
    }
}

/// Helper type exposing hooks into EVM and access to evm internal settings.
pub struct EvmInternals<'a> {
    internals: Box<dyn EvmInternalsTr + 'a>,
    //block_env: &'a (dyn Block + 'a),
}

impl<'a> EvmInternals<'a> {
    /// Creates a new [`EvmInternals`] instance.
    pub fn new<T>(journal: &'a mut T) -> Self
    where
        T: ContextTr + Debug,
        T::Db: Database,
    {
        Self { internals: Box::new(EvmInternalsImpl(journal)) }
    }

    /// Returns the  evm's block information.
    pub fn block_env(&self) -> &dyn Block {
        self.internals.block()
    }

    /// Returns the current block number.
    pub fn block_number(&self) -> U256 {
        self.block_env().number()
    }

    /// Returns the current block timestamp.
    pub fn block_timestamp(&self) -> U256 {
        self.block_env().timestamp()
    }

    /// Returns a mutable reference to [`Database`] implementation with erased error type.
    ///
    /// Users should prefer using other methods for accessing state that rely on cached state in the
    /// journal instead.
    pub fn db_mut(&mut self) -> impl Database<Error = ErasedError> + '_ {
        &mut *self.internals
    }

    /// Loads an account.
    pub fn load_account(
        &mut self,
        address: Address,
    ) -> Result<StateLoad<&mut Account>, EvmInternalsError> {
        self.internals.load_account(address)
    }

    /// Loads code of an account.
    pub fn load_account_code(
        &mut self,
        address: Address,
    ) -> Result<StateLoad<&mut Account>, EvmInternalsError> {
        self.internals.load_account_code(address)
    }

    /// Loads a storage slot.
    pub fn sload(
        &mut self,
        address: Address,
        key: StorageKey,
    ) -> Result<StateLoad<StorageValue>, EvmInternalsError> {
        self.internals.sload(address, key)
    }

    /// Touches the account.
    pub fn touch_account(&mut self, address: Address) {
        self.internals.touch_account(address);
    }

    /// Sets bytecode to the account.
    pub fn set_code(&mut self, address: Address, code: Bytecode) {
        self.internals.set_code(address, code);
    }

    /// Stores the storage value in Journal state.
    pub fn sstore(
        &mut self,
        address: Address,
        key: StorageKey,
        value: StorageValue,
    ) -> Result<StateLoad<SStoreResult>, EvmInternalsError> {
        self.internals.sstore(address, key, value)
    }

    /// Logs the log in Journal state.
    pub fn log(&mut self, log: Log) {
        self.internals.log(log);
    }
}

impl<'a> fmt::Debug for EvmInternals<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EvmInternals")
            .field("internals", &self.internals)
            .field("block_env", &"{{}}")
            .finish_non_exhaustive()
    }
}
/// Trait for mutating block parameters. Enables method chaining.
pub trait BlockSetter: Block {
    /// Set block number.
    fn set_number(&mut self, number: U256) -> &mut Self;
    /// Set beneficiary address.
    fn set_beneficiary(&mut self, beneficiary: Address) -> &mut Self;
    /// Set block timestamp.
    fn set_timestamp(&mut self, timestamp: U256) -> &mut Self;
    /// Set gas limit.
    fn set_gas_limit(&mut self, gas_limit: u64) -> &mut Self;
    /// Set base fee.
    fn set_basefee(&mut self, basefee: u64) -> &mut Self;
    /// Set difficulty.
    fn set_difficulty(&mut self, difficulty: U256) -> &mut Self;
    /// Set previous random value.
    fn set_prevrandao(&mut self, prevrandao: Option<B256>) -> &mut Self;
    /// Set blob excess gas and price.
    fn set_blob_excess_gas_and_price(
        &mut self,
        blob_excess_gas_and_price: Option<BlobExcessGasAndPrice>,
    ) -> &mut Self;
}

impl BlockSetter for BlockEnv {
    fn set_number(&mut self, number: U256) -> &mut Self {
        self.number = number;
        self
    }

    fn set_beneficiary(&mut self, beneficiary: Address) -> &mut Self {
        self.beneficiary = beneficiary;
        self
    }

    fn set_timestamp(&mut self, timestamp: U256) -> &mut Self {
        self.timestamp = timestamp;
        self
    }

    fn set_gas_limit(&mut self, gas_limit: u64) -> &mut Self {
        self.gas_limit = gas_limit;
        self
    }

    fn set_basefee(&mut self, basefee: u64) -> &mut Self {
        self.basefee = basefee;
        self
    }

    fn set_difficulty(&mut self, difficulty: U256) -> &mut Self {
        self.difficulty = difficulty;
        self
    }

    fn set_prevrandao(&mut self, prevrandao: Option<B256>) -> &mut Self {
        self.prevrandao = prevrandao;
        self
    }

    fn set_blob_excess_gas_and_price(
        &mut self,
        blob_excess_gas_and_price: Option<BlobExcessGasAndPrice>,
    ) -> &mut Self {
        self.blob_excess_gas_and_price = blob_excess_gas_and_price;
        self
    }
}
