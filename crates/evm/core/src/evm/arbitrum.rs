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
        if initialized {
            return Ok(());
        }

        let arbos_version =
            stylus_config.arbos_version.unwrap_or(arbos_revm::constants::INITIAL_ARBOS_VERSION);
        let mut params = ArbosStateParams::for_arbos_version(arbos_version);
        params.chain_id = U256::from(evm_env.cfg_env.chain_id);
        params.genesis_block_num = evm_env.block_env.number.saturating_to();
        params.upgrade_timestamp = evm_env.block_env.timestamp.saturating_to();
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
        context
            .arb_state(None, false)
            .initialize(&params)
            .map_err(|err| eyre::eyre!("failed to initialize ArbOS state: {err}"))?;
        let changes = context
            .journaled_state
            .finalize()
            .into_iter()
            .map(|(address, account)| (address, account.with_touched_mark()))
            .collect();
        context.journaled_state.database.commit(changes);
        Ok(())
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
