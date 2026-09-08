use std::ops::{Deref, DerefMut};

use alloy_evm::{Database, Evm, EvmEnv, EvmFactory, FromRecoveredTx};
use alloy_network::{AnyRpcTransaction, TransactionResponse};
use alloy_primitives::{Address, Bytes, U256, map::AddressSet};
use arbos_revm::{
    ArbitrumChain, ArbitrumContext, ArbitrumEvm,
    config::ArbitrumConfig,
    handler::ArbitrumHandler,
    local_context::ArbitrumLocalContext,
    precompiles::ArbitrumPrecompileProvider,
    state::{ArbState, ArbStateGetter, ArbosStateParams, types::StorageBackedTr},
    transaction::{ArbitrumTransaction, ArbitrumTransactionError},
};
use foundry_fork_db::DatabaseError;
use revm::{
    ExecuteEvm, InspectEvm, Inspector, Journal,
    context::{
        BlockEnv, Context, ContextTr, DBErrorMarker, JournalTr,
        result::{EVMError, HaltReason, ResultAndState},
    },
    handler::{EthFrame, FrameResult, SystemCallEvm, instructions::EthInstructions},
    inspector::{InspectorHandler, NoOpInspector},
    interpreter::{FrameInput, interpreter::EthInterpreter},
    primitives::{TxKind, hardfork::SpecId},
};

use crate::{
    FoundryChain, FoundryContextExt, FoundryInspectorExt, FoundryTransaction,
    FromAnyRpcTransaction,
    backend::{DatabaseExt, JournaledState},
    evm::{FoundryEvmFactory, FoundryPrecompiles, NestedEvm, NestedEvmFor, run_inspected_frame},
};

type ArbInstructions<DB> = EthInstructions<EthInterpreter, ArbitrumContext<DB>>;
type ArbPrecompiles<DB> = ArbitrumPrecompileProvider<ArbitrumContext<DB>>;
type ArbInnerEvm<DB, I> =
    ArbitrumEvm<ArbitrumContext<DB>, I, ArbPrecompiles<DB>, ArbInstructions<DB>, EthFrame>;

/// Fixed Alloy-facing handle for Arbitrum's context-dependent precompile provider.
#[derive(Clone, Debug, Default)]
pub struct ArbitrumPrecompiles {
    addresses: AddressSet,
}

impl FoundryPrecompiles for ArbitrumPrecompiles {
    fn addresses(&self) -> AddressSet {
        self.addresses.clone()
    }
}

/// Alloy EVM facade over the ArbOS-aware REVM implementation.
pub struct AlloyArbitrumEvm<DB: Database, I> {
    inner: ArbInnerEvm<DB, I>,
    inspect: bool,
    precompiles: ArbitrumPrecompiles,
}

impl<DB: Database, I> AlloyArbitrumEvm<DB, I> {
    const fn new(
        inner: ArbInnerEvm<DB, I>,
        inspect: bool,
        precompiles: ArbitrumPrecompiles,
    ) -> Self {
        Self { inner, inspect, precompiles }
    }

    pub fn into_inner(self) -> ArbInnerEvm<DB, I> {
        self.inner
    }
}

impl<DB: Database, I> Deref for AlloyArbitrumEvm<DB, I> {
    type Target = ArbitrumContext<DB>;

    fn deref(&self) -> &Self::Target {
        &self.inner.0.ctx
    }
}

impl<DB: Database, I> DerefMut for AlloyArbitrumEvm<DB, I> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner.0.ctx
    }
}

impl<DB, I> Evm for AlloyArbitrumEvm<DB, I>
where
    DB: Database,
    I: Inspector<ArbitrumContext<DB>>,
{
    type DB = DB;
    type Tx = ArbitrumTransaction;
    type Error = EVMError<DB::Error, ArbitrumTransactionError>;
    type HaltReason = HaltReason;
    type Spec = SpecId;
    type BlockEnv = BlockEnv;
    type Precompiles = ArbitrumPrecompiles;
    type Inspector = I;

    fn block(&self) -> &Self::BlockEnv {
        &self.inner.0.ctx.block
    }

    fn cfg_env(&self) -> &revm::context::CfgEnv<Self::Spec> {
        &self.inner.0.ctx.cfg.inner
    }

    fn chain_id(&self) -> u64 {
        self.inner.0.ctx.cfg.inner.chain_id
    }

    fn transact_raw(&mut self, tx: Self::Tx) -> Result<ResultAndState, Self::Error> {
        if self.inspect { self.inner.inspect_tx(tx) } else { self.inner.transact(tx) }
    }

    fn transact_system_call(
        &mut self,
        caller: Address,
        contract: Address,
        data: Bytes,
    ) -> Result<ResultAndState, Self::Error> {
        self.inner.system_call_with_caller(caller, contract, data).map_err(|err| match err {
            EVMError::Transaction(err) => EVMError::Transaction(err.into()),
            EVMError::Header(err) => EVMError::Header(err),
            EVMError::Database(err) => EVMError::Database(err),
            EVMError::Custom(err) => EVMError::Custom(err),
            EVMError::CustomAny(err) => EVMError::CustomAny(err),
        })
    }

    fn finish(self) -> (Self::DB, EvmEnv<Self::Spec>) {
        let Context { block, cfg, journaled_state, .. } = self.inner.0.ctx;
        (journaled_state.database, EvmEnv::new(cfg.inner, block))
    }

    fn set_inspector_enabled(&mut self, enabled: bool) {
        self.inspect = enabled;
    }

    fn components(&self) -> (&Self::DB, &Self::Inspector, &Self::Precompiles) {
        (&self.inner.0.ctx.journaled_state.database, &self.inner.0.inspector, &self.precompiles)
    }

    fn components_mut(&mut self) -> (&mut Self::DB, &mut Self::Inspector, &mut Self::Precompiles) {
        (
            &mut self.inner.0.ctx.journaled_state.database,
            &mut self.inner.0.inspector,
            &mut self.precompiles,
        )
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ArbitrumEvmFactory;

impl ArbitrumEvmFactory {
    pub fn create_evm_with_inspector_and_context<
        DB: Database,
        I: Inspector<ArbitrumContext<DB>>,
    >(
        &self,
        db: DB,
        env: EvmEnv,
        inspector: I,
        chain: ArbitrumChain,
    ) -> AlloyArbitrumEvm<DB, I> {
        Self::build(db, env, inspector, true, chain)
    }

    fn build<DB: Database, I: Inspector<ArbitrumContext<DB>>>(
        db: DB,
        input: EvmEnv,
        inspector: I,
        inspect: bool,
        chain: ArbitrumChain,
    ) -> AlloyArbitrumEvm<DB, I> {
        let spec = input.cfg_env.spec;
        let mut journal = Journal::new(db);
        journal.set_spec_id(spec);
        let mut cfg = ArbitrumConfig::from(input.cfg_env);
        cfg.debug_mode = chain.debug_mode();
        cfg.disable_auto_cache = chain.disable_auto_cache();
        cfg.disable_auto_activate = chain.disable_auto_activate();
        cfg.disable_stylus_deployment = chain.disable_stylus_deployment();
        let context = ArbitrumContext {
            journaled_state: journal,
            block: input.block_env,
            cfg,
            tx: ArbitrumTransaction::default(),
            chain,
            local: ArbitrumLocalContext::default(),
            error: Ok(()),
        };
        let provider = ArbitrumPrecompileProvider::new(spec);
        let precompiles = ArbitrumPrecompiles { addresses: provider.warm_addresses().collect() };
        AlloyArbitrumEvm::new(
            ArbitrumEvm::new_with_inspector(
                context,
                inspector,
                EthInstructions::new_mainnet_with_spec(spec),
                provider,
            ),
            inspect,
            precompiles,
        )
    }
}

impl EvmFactory for ArbitrumEvmFactory {
    type Evm<DB: Database, I: Inspector<ArbitrumContext<DB>>> = AlloyArbitrumEvm<DB, I>;
    type Context<DB: Database> = ArbitrumContext<DB>;
    type Tx = ArbitrumTransaction;
    type Error<DBError: DBErrorMarker> = EVMError<DBError, ArbitrumTransactionError>;
    type HaltReason = HaltReason;
    type Spec = SpecId;
    type BlockEnv = BlockEnv;
    type Precompiles = ArbitrumPrecompiles;

    fn create_evm<DB: Database>(&self, db: DB, input: EvmEnv) -> Self::Evm<DB, NoOpInspector> {
        Self::build(db, input, NoOpInspector, false, ArbitrumChain::default())
    }

    fn create_evm_with_inspector<DB: Database, I: Inspector<Self::Context<DB>>>(
        &self,
        db: DB,
        input: EvmEnv,
        inspector: I,
    ) -> Self::Evm<DB, I> {
        Self::build(db, input, inspector, true, ArbitrumChain::default())
    }
}

impl FoundryChain<ArbitrumTransaction> for ArbitrumChain {
    fn configure_stylus(&mut self, config: &foundry_config::stylus::StylusConfig) {
        self.configure_execution(
            config.debug_mode_stylus,
            config.disable_auto_cache_stylus,
            config.disable_auto_activate_stylus,
        );
        self.set_disable_stylus_deployment(config.disable_stylus_deployment);
    }
}

impl FoundryContextExt for ArbitrumContext<&mut dyn DatabaseExt<ArbitrumEvmFactory>> {
    type Spec = SpecId;

    fn block_mut(&mut self) -> &mut Self::Block {
        &mut self.block
    }
    fn tx_mut(&mut self) -> &mut Self::Tx {
        &mut self.tx
    }
    fn cfg_mut(&mut self) -> &mut Self::Cfg {
        &mut self.cfg
    }
    fn cfg_env(&self) -> &revm::context::CfgEnv<Self::Spec> {
        &self.cfg.inner
    }
    fn cfg_env_mut(&mut self) -> &mut revm::context::CfgEnv<Self::Spec> {
        &mut self.cfg.inner
    }
    fn db_journal_inner_mut(&mut self) -> (&mut Self::Db, &mut JournaledState) {
        (&mut self.journaled_state.database, &mut self.journaled_state.inner)
    }
    fn journal_inner(&self) -> &JournaledState {
        &self.journaled_state.inner
    }
    fn activate_stylus_program(&mut self, address: Address) -> eyre::Result<()> {
        let max_wasm_size = self
            .arb_state(None, false)
            .programs()
            .stylus_params()
            .get()
            .map_err(|err| eyre::eyre!("failed to read Stylus parameters: {err}"))?
            .max_wasm_size;
        let code_hash = self
            .journal_mut()
            .code_hash(address)
            .map_err(|err| eyre::eyre!("failed to load Stylus code hash: {err:?}"))?
            .data;
        let bytecode = self
            .journal_mut()
            .code(address)
            .map_err(|err| eyre::eyre!("failed to load Stylus bytecode: {err:?}"))?
            .data;
        let wasm = arbos_revm::stylus_executor::stylus_code(&bytecode, max_wasm_size)
            .map_err(|err| {
                eyre::eyre!("failed to decode Stylus bytecode: {}", String::from_utf8_lossy(&err))
            })?
            .ok_or_else(|| eyre::eyre!("program is not a Stylus WASM contract"))?;
        arbos_revm::state::program::activate_program(self, code_hash, &wasm, true)
            .map_err(|err| eyre::eyre!("failed to activate Stylus program: {err}"))?;
        Ok(())
    }
}

/// Initializes empty ArbOS state or applies explicit Stylus overrides to existing state.
pub fn initialize_arbitrum_backend<DB: Database + revm::DatabaseCommit>(
    db: DB,
    evm_env: &EvmEnv,
    stylus_config: &foundry_config::stylus::StylusConfig,
) -> eyre::Result<()> {
    let mut context = ArbitrumContext {
        journaled_state: Journal::new(db),
        block: evm_env.block_env.clone(),
        cfg: ArbitrumConfig::from(evm_env.cfg_env.clone()),
        tx: ArbitrumTransaction::default(),
        chain: ArbitrumChain::default(),
        local: ArbitrumLocalContext::default(),
        error: Ok(()),
    };
    context.journaled_state.set_spec_id(evm_env.cfg_env.spec);

    let initialized = context
        .arb_state(None, false)
        .arbos_version()
        .get()
        .map_err(|err| eyre::eyre!("failed to read ArbOS version: {err}"))?
        != 0;
    let arbos_version =
        stylus_config.arbos_version.unwrap_or(arbos_revm::constants::INITIAL_ARBOS_VERSION);
    let mut params = if initialized {
        context
            .arb_state(None, false)
            .get()
            .map_err(|err| eyre::eyre!("failed to read ArbOS state: {err}"))?
    } else {
        let mut params = ArbosStateParams::for_arbos_version(arbos_version);
        params.chain_id = U256::from(evm_env.cfg_env.chain_id);
        params.genesis_block_num = evm_env.block_env.number.saturating_to();
        params.upgrade_timestamp = evm_env.block_env.timestamp.saturating_to();
        params
    };
    let original_stylus_params = params.stylus_params.clone();
    let stylus = &mut params.stylus_params;
    if let Some(value) = stylus_config.stylus_version {
        stylus.version = value;
    }
    if let Some(value) = stylus_config.ink_price {
        stylus.ink_price = value;
    }
    if let Some(value) = stylus_config.max_stack_depth {
        stylus.max_stack_depth = value;
    }
    if let Some(value) = stylus_config.free_pages {
        stylus.free_pages = value;
    }
    if let Some(value) = stylus_config.page_gas {
        stylus.page_gas = value;
    }
    if let Some(value) = stylus_config.page_ramp {
        stylus.page_ramp = value;
    }
    if let Some(value) = stylus_config.page_limit {
        stylus.page_limit = value;
    }
    if let Some(value) = stylus_config.max_fragment_count {
        stylus.max_fragment_count = value;
    }
    if let Some(value) = stylus_config.min_init_gas {
        stylus.min_init_gas = value;
    }
    if let Some(value) = stylus_config.min_cached_init_gas {
        stylus.min_cached_init_gas = value;
    }
    if let Some(value) = stylus_config.init_cost_scalar {
        stylus.init_cost_scalar = value;
    }
    if let Some(value) = stylus_config.cached_cost_scalar {
        stylus.cached_cost_scalar = value;
    }
    if let Some(value) = stylus_config.expiry_days {
        stylus.expiry_days = value;
    }
    if let Some(value) = stylus_config.keepalive_days {
        stylus.keepalive_days = value;
    }
    if let Some(value) = stylus_config.block_cache_size {
        stylus.block_cache_size = value;
    }
    if let Some(value) = stylus_config.max_wasm_size {
        stylus.max_wasm_size = value;
    }
    if initialized {
        if params.stylus_params == original_stylus_params {
            return Ok(());
        }
        // Overrides on forks must not reset owners, pricing, retryables,
        // or other consensus state loaded from the remote backend.
        context
            .arb_state(None, false)
            .programs()
            .stylus_params()
            .set(&params.stylus_params)
            .map_err(|err| eyre::eyre!("failed to override Stylus parameters: {err}"))?;
    } else {
        context
            .arb_state(None, false)
            .initialize(&params)
            .map_err(|err| eyre::eyre!("failed to initialize ArbOS state: {err}"))?;
    }
    let changes = context
        .journaled_state
        .finalize()
        .into_iter()
        .map(|(address, account)| (address, account.with_touched_mark()))
        .collect();
    context.journaled_state.database.commit(changes);
    Ok(())
}

impl FoundryEvmFactory for ArbitrumEvmFactory {
    type Chain = ArbitrumChain;
    type FoundryContext<'db> = ArbitrumContext<&'db mut dyn DatabaseExt<Self>>;
    type FoundryEvm<'db, I: FoundryInspectorExt<Self::FoundryContext<'db>>> =
        AlloyArbitrumEvm<&'db mut dyn DatabaseExt<Self>, I>;

    fn initialize_backend(
        &self,
        db: &mut dyn DatabaseExt<Self>,
        evm_env: &EvmEnv,
        stylus_config: &foundry_config::stylus::StylusConfig,
    ) -> eyre::Result<()> {
        initialize_arbitrum_backend(db, evm_env, stylus_config)
    }

    fn create_evm_with_context<DB: Database>(
        &self,
        db: DB,
        evm_env: EvmEnv,
        chain_context: Self::Chain,
    ) -> Self::Evm<DB, NoOpInspector> {
        Self::build(db, evm_env, NoOpInspector, false, chain_context)
    }

    fn create_foundry_evm_with_inspector<'db, I: FoundryInspectorExt<Self::FoundryContext<'db>>>(
        &self,
        db: &'db mut dyn DatabaseExt<Self>,
        evm_env: EvmEnv,
        chain_context: Self::Chain,
        inspector: I,
    ) -> Self::FoundryEvm<'db, I> {
        Self::build(db, evm_env, inspector, true, chain_context)
    }

    fn create_foundry_nested_evm<'db>(
        &self,
        db: &'db mut dyn DatabaseExt<Self>,
        evm_env: EvmEnv,
        chain_context: Self::Chain,
        inspector: &'db mut dyn FoundryInspectorExt<Self::FoundryContext<'db>>,
    ) -> NestedEvmFor<'db, Self> {
        Box::new(Self::build(db, evm_env, inspector, true, chain_context).into_inner())
    }
}

impl<'db, I> NestedEvm for ArbInnerEvm<&'db mut dyn DatabaseExt<ArbitrumEvmFactory>, I>
where
    I: FoundryInspectorExt<ArbitrumContext<&'db mut dyn DatabaseExt<ArbitrumEvmFactory>>>,
{
    type Spec = SpecId;
    type Block = BlockEnv;
    type Tx = ArbitrumTransaction;
    type Chain = ArbitrumChain;
    type Journal = Journal<&'db mut dyn DatabaseExt<ArbitrumEvmFactory>>;

    fn journal_inner_mut(&mut self) -> &mut JournaledState {
        &mut self.0.ctx.journaled_state.inner
    }
    fn tx_mut(&mut self) -> &mut Self::Tx {
        &mut self.0.ctx.tx
    }
    fn chain_mut(&mut self) -> &mut Self::Chain {
        &mut self.0.ctx.chain
    }
    fn journal_mut(&mut self) -> &mut Self::Journal {
        &mut self.0.ctx.journaled_state
    }

    fn run_execution(&mut self, frame: FrameInput) -> Result<FrameResult, EVMError<DatabaseError>> {
        run_inspected_frame(self, ArbitrumHandler::default(), frame).map_err(map_arb_error)
    }

    fn transact_raw(&mut self, tx: Self::Tx) -> eyre::Result<ResultAndState> {
        self.0.ctx.set_tx(tx);
        let result = ArbitrumHandler::<
            _,
            EVMError<DatabaseError, ArbitrumTransactionError>,
            EthFrame,
        >::default()
        .inspect_run(self)?;
        Ok(ResultAndState::new(result, self.0.ctx.journaled_state.inner.state.clone()))
    }

    fn to_evm_env(&self) -> EvmEnv<Self::Spec, Self::Block> {
        EvmEnv::new(self.0.ctx.cfg.inner.clone(), self.0.ctx.block.clone())
    }
}

fn map_arb_error(
    error: EVMError<DatabaseError, ArbitrumTransactionError>,
) -> EVMError<DatabaseError> {
    match error {
        EVMError::Transaction(ArbitrumTransactionError::Base(error)) => {
            EVMError::Transaction(error)
        }
        EVMError::Transaction(error) => EVMError::Custom(error.to_string()),
        EVMError::Header(error) => EVMError::Header(error),
        EVMError::Database(error) => EVMError::Database(error),
        EVMError::Custom(error) => EVMError::Custom(error),
        EVMError::CustomAny(error) => EVMError::CustomAny(error),
    }
}

impl FoundryTransaction for ArbitrumTransaction {
    fn set_tx_type(&mut self, value: u8) {
        self.base.tx_type = value;
    }
    fn set_caller(&mut self, value: Address) {
        self.base.caller = value;
    }
    fn set_gas_limit(&mut self, value: u64) {
        self.base.gas_limit = value;
    }
    fn set_gas_price(&mut self, value: u128) {
        self.base.gas_price = value;
    }
    fn set_kind(&mut self, value: TxKind) {
        self.base.kind = value;
    }
    fn set_value(&mut self, value: U256) {
        self.base.value = value;
    }
    fn set_data(&mut self, value: Bytes) {
        self.base.data = value;
    }
    fn set_nonce(&mut self, value: u64) {
        self.base.nonce = value;
    }
    fn set_chain_id(&mut self, value: Option<u64>) {
        self.base.chain_id = value;
    }
    fn set_access_list(&mut self, value: revm::context::transaction::AccessList) {
        self.base.access_list = value;
    }
    fn authorization_list_mut(
        &mut self,
    ) -> &mut Vec<
        revm::context::either::Either<
            revm::context::transaction::SignedAuthorization,
            revm::context::transaction::RecoveredAuthorization,
        >,
    > {
        &mut self.base.authorization_list
    }
    fn set_gas_priority_fee(&mut self, value: Option<u128>) {
        self.base.gas_priority_fee = value;
    }
    fn set_blob_hashes(&mut self, value: Vec<alloy_primitives::B256>) {
        self.base.blob_hashes = value;
    }
    fn set_max_fee_per_blob_gas(&mut self, value: u128) {
        self.base.max_fee_per_blob_gas = value;
    }
    fn enveloped_tx(&self) -> Option<&Bytes> {
        self.enveloped_tx.as_ref()
    }
    fn set_enveloped_tx(&mut self, value: Bytes) {
        self.canonical_hash = Some(alloy_primitives::keccak256(&value));
        self.enveloped_tx = Some(value);
    }
}

impl FromAnyRpcTransaction for ArbitrumTransaction {
    fn from_any_rpc_transaction(tx: &AnyRpcTransaction) -> eyre::Result<Self> {
        tx.as_envelope().map(|envelope| Self::from_recovered_tx(envelope, tx.from())).ok_or_else(
            || eyre::eyre!("cannot convert unknown transaction type to ArbitrumTransaction"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{B256, keccak256};
    use arbos_revm::{
        constants::{ARBOS_ADDRESS, STYLUS_DISCRIMINANT},
        transaction::ArbitrumRetryTx,
    };
    use foundry_config::stylus::StylusConfig;
    use revm::{
        Database as _, DatabaseCommit,
        context::{TxEnv, result::ExecutionResult},
        database::InMemoryDB,
        state::{AccountInfo, Bytecode},
    };

    #[test]
    fn backend_initialization_preserves_executed_state_and_applies_only_explicit_overrides() {
        let mut db = InMemoryDB::default();
        let mut env: EvmEnv = EvmEnv::default();
        env.cfg_env.chain_id = 421_614;
        env.block_env.basefee = 0;
        let config = StylusConfig { arbos_version: Some(59), ..Default::default() };
        initialize_arbitrum_backend(&mut db, &env, &config).unwrap();

        let caller = Address::repeat_byte(0x11);
        let contract = Address::repeat_byte(0x22);
        db.insert_account_info(caller, AccountInfo::from_balance(U256::from(1_000_000_000_u64)));
        db.insert_account_info(
            contract,
            AccountInfo::default()
                .with_code(Bytecode::new_raw(Bytes::from_static(&[0x60, 0x2a, 0x60, 0, 0x55, 0]))),
        );
        let mut evm = ArbitrumEvmFactory.create_evm(&mut db, env.clone());
        let result = evm
            .transact_raw(
                TxEnv {
                    caller,
                    kind: TxKind::Call(contract),
                    gas_limit: 100_000,
                    chain_id: Some(421_614),
                    ..Default::default()
                }
                .into(),
            )
            .unwrap();
        assert!(result.result.is_success());
        drop(evm);
        db.commit(result.state);
        let arbos_storage = db.cache.accounts[&ARBOS_ADDRESS].storage.clone();

        initialize_arbitrum_backend(&mut db, &env, &StylusConfig::default()).unwrap();
        assert_eq!(db.cache.accounts[&ARBOS_ADDRESS].storage, arbos_storage);

        let overrides =
            StylusConfig { arbos_version: Some(61), ink_price: Some(13_579), ..Default::default() };
        initialize_arbitrum_backend(&mut db, &env, &overrides).unwrap();
        assert_eq!(db.storage(contract, U256::ZERO).unwrap(), U256::from(42));
        let mut evm = ArbitrumEvmFactory.create_evm(&mut db, env);
        let params = evm.arb_state(None, false).get().unwrap();
        assert_eq!(
            params.arbos_version, 59,
            "an override must not reinitialize existing ArbOS state"
        );
        assert_eq!(params.chain_id, U256::from(421_614));
        assert_eq!(params.stylus_params.ink_price, 13_579);
    }

    #[test]
    fn factory_enforces_stylus_deployment_policy() {
        for disabled in [false, true] {
            let mut db = InMemoryDB::default();
            let caller = Address::repeat_byte(0x33);
            db.insert_account_info(
                caller,
                AccountInfo::from_balance(U256::from(1_000_000_000_u64)),
            );
            let mut chain = ArbitrumChain::default();
            chain.configure_stylus(&StylusConfig {
                disable_stylus_deployment: disabled,
                ..Default::default()
            });
            let mut runtime = STYLUS_DISCRIMINANT.to_vec();
            runtime.push(0);
            let len = runtime.len() as u8;
            let mut initcode = vec![0x60, len, 0x60, 12, 0x60, 0, 0x39, 0x60, len, 0x60, 0, 0xf3];
            initcode.extend(runtime);
            let mut env: EvmEnv = EvmEnv::default();
            env.block_env.basefee = 0;
            let mut evm = ArbitrumEvmFactory.create_evm_with_inspector_and_context(
                &mut db,
                env,
                NoOpInspector,
                chain,
            );
            let result = evm
                .transact_raw(
                    TxEnv {
                        caller,
                        kind: TxKind::Create,
                        data: initcode.into(),
                        gas_limit: 100_000,
                        ..Default::default()
                    }
                    .into(),
                )
                .unwrap();
            assert_eq!(result.result.is_success(), !disabled, "{result:?}");
        }
    }

    #[test]
    fn retryable_execution_persists_across_factory_database_handoff() {
        for inspected in [false, true] {
            for reverts in [false, true] {
                let caller = Address::repeat_byte(0x31);
                let target = Address::repeat_byte(0x32);
                let refund_to = Address::repeat_byte(0x33);
                let network_fee_account = Address::repeat_byte(0x34);
                let ticket_id = B256::repeat_byte(0x44);
                let value = U256::from(7);
                let mut escrow_input = b"retryable escrow".to_vec();
                escrow_input.extend_from_slice(ticket_id.as_slice());
                let escrow = Address::from_slice(&keccak256(escrow_input)[12..]);

                let mut db = InMemoryDB::default();
                let mut env: EvmEnv = EvmEnv::default();
                env.cfg_env.chain_id = 421_614;
                env.block_env.basefee = 1;
                initialize_arbitrum_backend(&mut db, &env, &StylusConfig::default()).unwrap();
                db.insert_account_info(escrow, AccountInfo::from_balance(value));
                // Retry gas is prepaid to the fee account by the scheduling transaction.
                db.insert_account_info(
                    network_fee_account,
                    AccountInfo::from_balance(U256::from(100_000)),
                );
                // Store 42, then either stop or revert the storage and value transfer.
                let mut code = vec![0x60, 0x2a, 0x60, 0, 0x55];
                code.extend_from_slice(if reverts { &[0x60, 0, 0x60, 0, 0xfd] } else { &[0] });
                db.insert_account_info(
                    target,
                    AccountInfo::default().with_code(Bytecode::new_raw(code.into())),
                );

                let mut evm = ArbitrumEvmFactory.create_evm(db, env);
                evm.arb_state(None, false)
                    .network_fee_account()
                    .set(network_fee_account)
                    .unwrap();
                evm.arb_state(None, false)
                    .retryable_state()
                    .create_retryable(
                        ticket_id,
                        1_000_000,
                        caller,
                        Some(target),
                        value,
                        refund_to,
                        &Bytes::new(),
                    )
                    .unwrap();
                let changes = evm.journaled_state.finalize();
                evm.journaled_state.database.commit(changes);
                let (db, env) = evm.finish();

                let mut evm = ArbitrumEvmFactory.create_evm(db, env);
                evm.set_inspector_enabled(inspected);
                let result = evm
                    .transact_raw(
                        ArbitrumRetryTx {
                            chain_id: U256::from(421_614),
                            from: caller,
                            gas_limit: 100_000,
                            gas_fee_cap: U256::from(1),
                            to: Some(target),
                            value,
                            ticket_id,
                            refund_to,
                            max_refund: U256::from(100_000),
                            ..Default::default()
                        }
                        .into_transaction(),
                    )
                    .unwrap();
                assert_eq!(result.result.is_success(), !reverts, "{result:?}");
                assert_eq!(
                    matches!(result.result, ExecutionResult::Revert { .. }),
                    reverts,
                    "{result:?}"
                );
                let expected_refund = U256::from(100_000 - result.result.tx_gas_used());
                evm.journaled_state.database.commit(result.state);
                let (mut db, env) = evm.finish();

                assert_eq!(
                    db.storage(target, U256::ZERO).unwrap(),
                    U256::from(if reverts { 0 } else { 42 })
                );
                assert_eq!(
                    db.basic(target).unwrap().unwrap().balance,
                    if reverts { U256::ZERO } else { value }
                );
                assert_eq!(
                    db.basic(escrow).unwrap().unwrap_or_default().balance,
                    if reverts { value } else { U256::ZERO }
                );
                assert_eq!(db.basic(refund_to).unwrap().unwrap().balance, expected_refund);
                let mut evm = ArbitrumEvmFactory.create_evm(db, env);
                let timeout =
                    evm.arb_state(None, true).retryable(ticket_id).timeout().get().unwrap();
                assert_eq!(timeout, if reverts { 1_000_000 } else { 0 });
            }
        }
    }
}
