use std::ops::{Deref, DerefMut};

use revm::{
    context::{
        result::{EVMError, InvalidTransaction},
        transaction::TransactionError,
        Transaction, TxEnv,
    },
    handler::SystemCallTx,
    primitives::{Address, Bytes, TxKind, B256, U256},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ArbitrumTransactionError {
    Base(InvalidTransaction),
}

impl TransactionError for ArbitrumTransactionError {}

impl std::fmt::Display for ArbitrumTransactionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArbitrumTransactionError::Base(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ArbitrumTransactionError {}

impl From<InvalidTransaction> for ArbitrumTransactionError {
    fn from(value: InvalidTransaction) -> Self {
        Self::Base(value)
    }
}

impl<DBError> From<ArbitrumTransactionError> for EVMError<DBError, ArbitrumTransactionError> {
    fn from(value: ArbitrumTransactionError) -> Self {
        Self::Transaction(value)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ArbitrumTransaction {
    pub base: TxEnv,
}

impl ArbitrumTransaction {
    pub fn new(base: TxEnv) -> Self {
        Self { base }
    }
}

impl From<TxEnv> for ArbitrumTransaction {
    fn from(tx_env: TxEnv) -> Self {
        Self::new(tx_env)
    }
}

impl SystemCallTx for ArbitrumTransaction {
    fn new_system_tx_with_caller(
        caller: Address,
        system_contract_address: Address,
        data: Bytes,
    ) -> Self {
        ArbitrumTransaction::new(TxEnv::new_system_tx_with_caller(
            caller,
            system_contract_address,
            data,
        ))
    }

    fn new_system_tx(system_contract_address: Address, data: Bytes) -> Self {
        Self::new_system_tx_with_caller(
            revm::handler::SYSTEM_ADDRESS,
            system_contract_address,
            data,
        )
    }
}

impl Transaction for ArbitrumTransaction {
    type AccessListItem<'a> = <TxEnv as Transaction>::AccessListItem<'a>;
    type Authorization<'a> = <TxEnv as Transaction>::Authorization<'a>;

    fn tx_type(&self) -> u8 {
        self.base.tx_type()
    }

    fn caller(&self) -> Address {
        self.base.caller()
    }

    fn gas_limit(&self) -> u64 {
        self.base.gas_limit()
    }

    fn value(&self) -> U256 {
        self.base.value()
    }

    fn input(&self) -> &Bytes {
        self.base.input()
    }

    fn nonce(&self) -> u64 {
        self.base.nonce()
    }

    fn kind(&self) -> TxKind {
        self.base.kind()
    }

    fn chain_id(&self) -> Option<u64> {
        self.base.chain_id()
    }

    fn access_list(&self) -> Option<impl Iterator<Item = Self::AccessListItem<'_>>> {
        self.base.access_list()
    }

    fn max_priority_fee_per_gas(&self) -> Option<u128> {
        self.base.max_priority_fee_per_gas()
    }

    fn max_fee_per_gas(&self) -> u128 {
        self.base.max_fee_per_gas()
    }

    fn gas_price(&self) -> u128 {
        self.base.gas_price()
    }

    fn blob_versioned_hashes(&self) -> &[B256] {
        self.base.blob_versioned_hashes()
    }

    fn max_fee_per_blob_gas(&self) -> u128 {
        self.base.max_fee_per_blob_gas()
    }

    fn effective_gas_price(&self, base_fee: u128) -> u128 {
        self.base.effective_gas_price(base_fee)
    }

    fn authorization_list_len(&self) -> usize {
        self.base.authorization_list_len()
    }

    fn authorization_list(&self) -> impl Iterator<Item = Self::Authorization<'_>> {
        self.base.authorization_list()
    }
}

pub trait ArbitrumTxTr: Transaction {}

impl ArbitrumTxTr for ArbitrumTransaction {}

impl Deref for ArbitrumTransaction {
    type Target = TxEnv;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for ArbitrumTransaction {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

// TODO
impl ArbitrumTxTr for TxEnv {}
