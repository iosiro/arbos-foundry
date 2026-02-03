//! EVM abstraction trait.
//!
//! This module provides the `Evm` trait which abstracts over EVM implementations,
//! allowing for flexible transaction execution and state management.
//!
//! Vendored from `alloy-evm` to remove the dependency.

use crate::{env::EvmEnv, tx::IntoTxEnv};
use alloy_primitives::{Address, Bytes};
use core::{error::Error, fmt::Debug, hash::Hash};
use revm::{
    DatabaseCommit,
    context::result::ExecutionResult,
    context_interface::result::{HaltReasonTr, ResultAndState},
};

/// A wrapper trait for [`revm::Database`] that constrains the error type to be
/// `Error + Send + Sync + 'static` and requires the type to be `Debug`.
pub trait Database: revm::Database<Error: Error + Send + Sync + 'static> + Debug {}

impl<T> Database for T where T: revm::Database<Error: Error + Send + Sync + 'static> + Debug {}

/// An instance of an Ethereum virtual machine.
///
/// This trait abstracts over different EVM implementations, providing a common interface
/// for transaction execution, state access, and configuration management.
pub trait Evm {
    /// The database type used by this EVM.
    type DB;

    /// The transaction type used by this EVM.
    type Tx: IntoTxEnv<Self::Tx>;

    /// The error type produced by this EVM.
    type Error: EvmError;

    /// The halt reason type produced by this EVM.
    type HaltReason: HaltReasonTr + Send + Sync + 'static;

    /// The spec ID type used by this EVM.
    type Spec: Debug + Copy + Hash + Eq + Send + Sync + Default + 'static;

    /// The block environment type used by this EVM.
    type BlockEnv: BlockEnvironment;

    /// The precompiles type used by this EVM.
    type Precompiles;

    /// The inspector type used by this EVM.
    type Inspector;

    /// Returns the block environment.
    fn block(&self) -> &Self::BlockEnv;

    /// Returns the chain ID.
    fn chain_id(&self) -> u64;

    /// Executes a transaction with the given raw transaction data.
    fn transact_raw(
        &mut self,
        tx: Self::Tx,
    ) -> Result<ResultAndState<Self::HaltReason>, Self::Error>;

    /// Executes a transaction, converting the input into the EVM's transaction format.
    fn transact(
        &mut self,
        tx: impl IntoTxEnv<Self::Tx>,
    ) -> Result<ResultAndState<Self::HaltReason>, Self::Error> {
        self.transact_raw(tx.into_tx_env())
    }

    /// Executes a system call transaction.
    fn transact_system_call(
        &mut self,
        caller: Address,
        contract: Address,
        data: Bytes,
    ) -> Result<ResultAndState<Self::HaltReason>, Self::Error>;

    /// Returns a reference to the database.
    fn db(&self) -> &Self::DB {
        self.components().0
    }

    /// Returns a mutable reference to the database.
    fn db_mut(&mut self) -> &mut Self::DB {
        self.components_mut().0
    }

    /// Executes a transaction and commits the state changes.
    fn transact_commit(
        &mut self,
        tx: impl IntoTxEnv<Self::Tx>,
    ) -> Result<ExecutionResult<Self::HaltReason>, Self::Error>
    where
        Self::DB: DatabaseCommit,
    {
        let ResultAndState { result, state } = self.transact(tx)?;
        self.db_mut().commit(state);
        Ok(result)
    }

    /// Consumes the EVM and returns the database and environment.
    fn finish(self) -> (Self::DB, EvmEnv<Self::Spec>)
    where
        Self: Sized;

    /// Consumes the EVM and returns only the database.
    fn into_db(self) -> Self::DB
    where
        Self: Sized,
    {
        self.finish().0
    }

    /// Consumes the EVM and returns only the environment.
    fn into_env(self) -> EvmEnv<Self::Spec>
    where
        Self: Sized,
    {
        self.finish().1
    }

    /// Enables or disables the inspector.
    fn set_inspector_enabled(&mut self, enabled: bool);

    /// Enables the inspector.
    fn enable_inspector(&mut self) {
        self.set_inspector_enabled(true)
    }

    /// Disables the inspector.
    fn disable_inspector(&mut self) {
        self.set_inspector_enabled(false)
    }

    /// Returns a reference to the precompiles.
    fn precompiles(&self) -> &Self::Precompiles {
        self.components().2
    }

    /// Returns a mutable reference to the precompiles.
    fn precompiles_mut(&mut self) -> &mut Self::Precompiles {
        self.components_mut().2
    }

    /// Returns a reference to the inspector.
    fn inspector(&self) -> &Self::Inspector {
        self.components().1
    }

    /// Returns a mutable reference to the inspector.
    fn inspector_mut(&mut self) -> &mut Self::Inspector {
        self.components_mut().1
    }

    /// Returns references to the database, inspector, and precompiles.
    fn components(&self) -> (&Self::DB, &Self::Inspector, &Self::Precompiles);

    /// Returns mutable references to the database, inspector, and precompiles.
    fn components_mut(&mut self) -> (&mut Self::DB, &mut Self::Inspector, &mut Self::Precompiles);
}

/// Error type for EVM operations.
pub trait EvmError: core::error::Error + Send + Sync + 'static {}

impl<T> EvmError for T where T: core::error::Error + Send + Sync + 'static {}

/// Trait for types that can be used as block environments.
pub trait BlockEnvironment: revm::context::Block + Clone + Debug + Send + Sync + 'static {
    /// Returns a mutable reference to the inner block environment.
    fn inner_mut(&mut self) -> &mut revm::context::BlockEnv;
}

impl BlockEnvironment for revm::context::BlockEnv {
    fn inner_mut(&mut self) -> &mut revm::context::BlockEnv {
        self
    }
}
