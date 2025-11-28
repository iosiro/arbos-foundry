use std::{
    fmt::{self, Debug},
    sync::Arc,
};

use revm::{
    context::{Cfg, ContextTr, LocalContextTr},
    handler::PrecompileProvider,
    interpreter::{
        CallInput, Gas, InputsImpl, InstructionResult, InterpreterResult, gas::ISTANBUL_SLOAD_GAS,
    },
    precompile::{PrecompileError, PrecompileId, PrecompileSpecId, Precompiles},
    primitives::{
        Address, Bytes, HashMap, HashSet, SHORT_ADDRESS_CAP, U256, hardfork::SpecId, short_address,
    },
};

mod arb_address_table;
mod arb_aggregator;
mod arb_debug;
mod arb_gas_info;
mod arb_info;
mod arb_native_token_manager;
mod arb_owner;
mod arb_owner_public;
mod arb_retryable_tx;
mod arb_statistics;
mod arb_sys;
pub mod arb_wasm;
mod arb_wasm_cache;

use crate::{
    ArbitrumContextTr,
    macros::{interpreter_return, interpreter_revert},
    precompiles::{arb_wasm::arb_wasm_precompile, arb_wasm_cache::arb_wasm_cache_precompile},
    state::{ArbState, ArbStateGetter, try_state, types::StorageBackedTr},
    try_record_cost,
};

macro_rules! selector_or_revert {
    ($gas:expr, $input:expr) => {{
        if $input.len() < 4 {
            $gas.spend_all();
            return Some(crate::macros::interpreter_result_return_with_output(
                &mut $gas,
                revm::primitives::Bytes::from_static(b"Input too short"),
            ));
        }

        match <[u8; 4]>::try_from(&$input[0..4]) {
            Ok(selector) => selector,
            Err(_) => {
                $gas.spend_all();
                return Some(crate::macros::interpreter_result_return_with_output(
                    &mut $gas,
                    revm::primitives::Bytes::from_static(b"Invalid selector"),
                ));
            }
        }
    }};
}

pub(crate) use selector_or_revert;

pub struct ArbitrumPrecompileProvider<CTX: ArbitrumContextTr> {
    registry: Arc<PrecompileRegistry<CTX>>,
    spec: SpecId,
}

impl<CTX: ArbitrumContextTr> ArbitrumPrecompileProvider<CTX> {
    #[inline]
    pub fn warm_addresses(&self) -> Box<impl Iterator<Item = Address>> {
        Box::new(self.registry.address_iter().copied())
    }

    #[inline]
    pub fn contains(&self, addr: &Address) -> bool {
        self.registry.contains(addr)
    }

    pub fn new(spec: SpecId) -> Self {
        let mut registry = PrecompileRegistry::new(PrecompileSpecId::from_spec_id(spec));

        registry.register_many([
            // Arbitrum specific precompiles can be added here
            Precompile::Extended(arb_address_table::arb_address_table_precompile::<CTX>()),
            Precompile::Extended(arb_aggregator::arb_aggregator_precompile::<CTX>()),
            Precompile::Extended(arb_debug::arb_debug_precompile::<CTX>()),
            Precompile::Extended(arb_gas_info::arb_gas_info_precompile::<CTX>()),
            Precompile::Extended(arb_info::arb_info_precompile::<CTX>()),
            Precompile::Extended(arb_native_token_manager::arb_native_token_manager_precompile::<
                CTX,
            >()),
            Precompile::Extended(arb_owner_public::arb_owner_public_precompile::<CTX>()),
            Precompile::Extended(arb_owner::arb_owner_precompile::<CTX>()),
            Precompile::Extended(arb_retryable_tx::arb_retryable_tx_precompile::<CTX>()),
            Precompile::Extended(arb_statistics::arb_statistics_precompile::<CTX>()),
            Precompile::Extended(arb_sys::arb_sys_precompile::<CTX>()),
            Precompile::Extended(arb_wasm_precompile::<CTX>()),
            Precompile::Extended(arb_wasm_cache_precompile::<CTX>()),
        ]);
        Self { registry: Arc::new(registry), spec }
    }
}

impl<CTX: ArbitrumContextTr> Clone for ArbitrumPrecompileProvider<CTX> {
    fn clone(&self) -> Self {
        Self { registry: Arc::clone(&self.registry), spec: self.spec }
    }
}

impl<CTX: ArbitrumContextTr> Default for ArbitrumPrecompileProvider<CTX> {
    fn default() -> Self {
        let spec = SpecId::default();
        let registry = PrecompileRegistry::new(PrecompileSpecId::from_spec_id(spec));

        Self { registry: Arc::new(registry), spec }
    }
}

impl<CTX: ArbitrumContextTr> PrecompileProvider<CTX> for ArbitrumPrecompileProvider<CTX> {
    type Output = InterpreterResult;

    /// Update spec and regenerate registry.
    fn set_spec(&mut self, spec: <CTX::Cfg as Cfg>::Spec) -> bool {
        let new_spec = spec.into();
        if new_spec == self.spec {
            return false;
        }

        self.registry = Arc::new(PrecompileRegistry::new(PrecompileSpecId::from_spec_id(new_spec)));
        self.spec = new_spec;
        true
    }

    /// Executes a precompile if one exists for the address.
    fn run(
        &mut self,
        ctx: &mut CTX,
        address: &Address,
        inputs: &InputsImpl,
        is_static: bool,
        gas_limit: u64,
    ) -> Result<Option<InterpreterResult>, String> {
        let Some(precompile) = self.registry.get(address) else {
            return Ok(None);
        };

        // revert for mutating calls to code addresses other than their own
        if !is_static
            && inputs.target_address != inputs.bytecode_address.expect("bytecode must be set")
        {
            return Ok(Some(InterpreterResult {
                result: InstructionResult::Revert,
                output: Bytes::default(),
                gas: Gas::new(gas_limit),
            }));
        }

        // extract input bytes
        let input_bytes = match &inputs.input {
            CallInput::SharedBuffer(range) => ctx
                .local()
                .shared_memory_buffer_slice(range.clone())
                .map(|s| s.to_vec())
                .unwrap_or_default(),
            CallInput::Bytes(b) => b.to_vec(),
        };

        precompile.call(
            ctx,
            &input_bytes,
            address,
            inputs.caller_address,
            inputs.call_value,
            is_static,
            gas_limit,
        )
    }

    fn warm_addresses(&self) -> Box<impl Iterator<Item = Address>> {
        self.warm_addresses()
    }

    fn contains(&self, address: &Address) -> bool {
        self.contains(address)
    }
}

#[derive(Clone, Debug)]
pub struct PrecompileRegistry<CTX: ContextTr> {
    map: HashMap<Address, Precompile<CTX>>,
    address_set: HashSet<Address>,
    fast_lookup: Vec<Option<Precompile<CTX>>>,
    all_short: bool,
}

impl<CTX: ContextTr> Default for PrecompileRegistry<CTX> {
    fn default() -> Self {
        Self {
            map: HashMap::default(),
            address_set: HashSet::default(),
            fast_lookup: vec![None; SHORT_ADDRESS_CAP],
            all_short: true,
        }
    }
}

impl<CTX: ContextTr> PrecompileRegistry<CTX> {
    pub fn new(spec: PrecompileSpecId) -> Self {
        let mut reg = Self::default();

        let precompiles = Precompiles::new(spec);

        precompiles.addresses().for_each(|addr| {
            if let Some(p) = precompiles.get(addr) {
                reg.register(Precompile::Simple(p.clone()));
            }
        });

        reg
    }

    #[inline]
    pub fn contains(&self, address: &Address) -> bool {
        self.map.contains_key(address)
    }

    #[inline]
    pub fn get(&self, address: &Address) -> Option<&Precompile<CTX>> {
        if let Some(idx) = short_address(address) {
            return self.fast_lookup[idx].as_ref();
        }
        self.map.get(address)
    }

    #[inline]
    pub fn address_iter(&self) -> impl ExactSizeIterator<Item = &Address> {
        self.map.keys()
    }

    pub fn register(&mut self, item: Precompile<CTX>) {
        self.register_many(std::iter::once(item));
    }

    pub fn register_many(&mut self, items: impl IntoIterator<Item = Precompile<CTX>>) {
        let collected: Vec<_> = items.into_iter().collect();

        for p in &collected {
            if let Some(short) = short_address(p.address()) {
                self.fast_lookup[short] = Some(p.clone());
            } else {
                self.all_short = false;
            }
            self.address_set.insert(*p.address());
        }

        for p in collected.into_iter() {
            self.map.insert(*p.address(), p);
        }
    }

    pub fn difference(&self, other: &Self) -> Self {
        let mut out = Self::default();

        let missing = self
            .map
            .iter()
            .filter(|(addr, _)| !other.map.contains_key(*addr))
            .map(|(_, p)| p.clone());

        out.register_many(missing);
        out
    }

    pub fn intersection(&self, other: &Self) -> Self {
        let mut out = Self::default();

        let common = self
            .map
            .iter()
            .filter(|(addr, _)| other.map.contains_key(*addr))
            .map(|(_, p)| p.clone());

        out.register_many(common);
        out
    }
}

pub struct ExtendedPrecompile<CTX: ContextTr> {
    id: PrecompileId,
    address: Address,
    handler: Arc<ExtendedPrecompileFn<CTX>>,
}

impl<CTX: ContextTr> Clone for ExtendedPrecompile<CTX> {
    fn clone(&self) -> Self {
        Self { id: self.id.clone(), address: self.address, handler: Arc::clone(&self.handler) }
    }
}

impl<CTX: ContextTr> Debug for ExtendedPrecompile<CTX> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExtendedPrecompile")
            .field("id", &self.id)
            .field("address", &self.address)
            .finish()
    }
}

impl<CTX: ContextTr> ExtendedPrecompile<CTX> {
    pub fn new(id: PrecompileId, address: Address, handler: ExtendedPrecompileFn<CTX>) -> Self {
        Self { id, address, handler: Arc::new(handler) }
    }

    #[inline]
    pub fn id(&self) -> &PrecompileId {
        &self.id
    }

    #[inline]
    pub fn address(&self) -> &Address {
        &self.address
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub fn execute(
        &self,
        ctx: &mut CTX,
        input: &[u8],
        target: &Address,
        caller: Address,
        value: U256,
        is_static: bool,
        gas_limit: u64,
    ) -> Result<Option<InterpreterResult>, String> {
        (self.handler)(ctx, input, target, caller, value, is_static, gas_limit)
    }
}

pub type ExtendedPrecompileFn<CTX> = fn(
    &mut CTX,
    &[u8],
    &Address,
    Address,
    U256,
    bool,
    u64,
) -> Result<Option<InterpreterResult>, String>;

#[derive(Debug)]
pub enum Precompile<CTX: ContextTr> {
    Simple(revm::precompile::Precompile),
    Extended(ExtendedPrecompile<CTX>),
}

impl<CTX: ContextTr> Clone for Precompile<CTX> {
    fn clone(&self) -> Self {
        match self {
            Self::Simple(p) => Self::Simple(p.clone()),
            Self::Extended(p) => Self::Extended(p.clone()),
        }
    }
}

impl<CTX: ContextTr> Precompile<CTX> {
    #[inline]
    pub fn address(&self) -> &Address {
        match self {
            Self::Simple(p) => p.address(),
            Self::Extended(p) => p.address(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn call(
        &self,
        ctx: &mut CTX,
        input: &[u8],
        target: &Address,
        caller: Address,
        value: U256,
        is_static: bool,
        gas_limit: u64,
    ) -> Result<Option<InterpreterResult>, String> {
        match self {
            Self::Simple(p) => {
                let raw = p.execute(input, gas_limit);

                let mut result = InterpreterResult {
                    result: InstructionResult::Return,
                    gas: Gas::new(gas_limit),
                    output: Bytes::new(),
                };

                match raw {
                    Ok(output) => {
                        _ = result.gas.record_cost(output.gas_used);
                        result.result = if output.reverted {
                            InstructionResult::Revert
                        } else {
                            InstructionResult::Return
                        };
                        result.output = output.bytes;
                    }
                    Err(PrecompileError::Fatal(e)) => return Err(e),
                    Err(e) => {
                        result.result = if e.is_oog() {
                            InstructionResult::PrecompileOOG
                        } else {
                            InstructionResult::PrecompileError
                        };
                    }
                }

                Ok(Some(result))
            }
            Self::Extended(ext) => {
                ext.execute(ctx, input, target, caller, value, is_static, gas_limit)
            }
        }
    }
}

pub(crate) trait ArbPrecompileLogic<CTX: ArbitrumContextTr> {
    /// File-local state mutability table
    const STATE_MUT_TABLE: &'static [([u8; 4], StateMutability)];

    /// Inner execution
    fn inner(
        context: &mut CTX,
        input: &[u8],
        target_address: &Address,
        caller_address: Address,
        call_value: U256,
        is_static: bool,
        gas_limit: u64,
    ) -> Option<InterpreterResult>;

    fn run(
        context: &mut CTX,
        input: &[u8],
        target_address: &Address,
        caller_address: Address,
        call_value: U256,
        is_static: bool,
        gas_limit: u64,
    ) -> Option<InterpreterResult> {
        let mut gas = Gas::new(gas_limit);

        let selector = selector_or_revert!(gas, input);

        let args_cost =
            revm::interpreter::gas::VERYLOW * (((input.len() as u64).saturating_sub(4) + 31) / 32);

        try_record_cost!(gas, args_cost);

        let purity = match Self::STATE_MUT_TABLE.iter().find(|(sel, _)| *sel == selector) {
            Some((_, p)) => *p,
            None => interpreter_return!(gas),
        };

        if purity != StateMutability::Pure {
            try_record_cost!(gas, ISTANBUL_SLOAD_GAS);
        }

        if purity >= StateMutability::NonPayable && is_static {
            let _ = try_state!(
                gas,
                context.arb_state(Some(&mut gas)).l2_pricing().per_tx_gas_limit().get()
            );
        }

        // call the inner logic
        let outcome = if let Some(outcome) = Self::inner(
            context,
            input,
            target_address,
            caller_address,
            call_value,
            is_static,
            gas.remaining(),
        ) {
            outcome
        } else {
            // final precompile return value must be error
            interpreter_revert!(gas);
        };

        gas.spend_all();
        gas.erase_cost(outcome.gas.remaining());

        let result_data_cost =
            revm::interpreter::gas::VERYLOW * (((outcome.output.len() as u64) + 31) / 32);
        try_record_cost!(gas, result_data_cost);

        Some(InterpreterResult { result: outcome.result, gas, output: outcome.output })
    }
}

macro_rules! decode_call {
    ($gas:expr, $call:path, $input:expr) => {{
        match <$call as alloy_sol_types::SolCall>::abi_decode($input) {
            Ok(value) => value,
            Err(_) => {
                return Some(crate::macros::interpreter_result_revert_with_output(
                    &mut $gas,
                    revm::primitives::Bytes::from_static(b"Invalid calldata"),
                ));
            }
        }
    }};
}

pub(crate) use decode_call;

#[macro_export]
macro_rules! generate_state_mut_table {
    (
        $iface:ident => {
            $(
                $call:ident($mut:ident)
            ),* $(,)?
        }
    ) => {{
        const TABLE: &[([u8;4], StateMutability)] = &[
            $(
                (
                    <$iface::$call as alloy_sol_types::SolCall>::SELECTOR,
                    StateMutability::$mut
                )
            ),*
        ];

        TABLE
    }};
}
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub enum StateMutability {
    Pure,
    View,
    NonPayable,
    Payable,
}

#[macro_export]
macro_rules! precompile_impl {
    ($logic:ty) => {
        |context, input, target_address, caller_address, call_value, is_static, gas_limit| {
            Ok(<$logic>::run(
                context,
                input,
                target_address,
                caller_address,
                call_value,
                is_static,
                gas_limit,
            ))
        }
    };
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use super::ExtendedPrecompile;
    use alloy_sol_types::SolCall;
    use revm::{
        Journal,
        context::{BlockEnv, ContextTr, JournalTr, TxEnv},
        database::EmptyDBTyped,
        interpreter::{Gas, InstructionResult, InterpreterResult},
        primitives::{Address, U256, address},
    };

    use crate::{
        ArbitrumContext,
        config::ArbitrumConfig,
        constants::ARBOS_BATCH_POSTER_ADDRESS,
        local_context::ArbitrumLocalContext,
        precompiles::{
            arb_address_table, arb_aggregator, arb_debug, arb_gas_info, arb_info,
            arb_native_token_manager, arb_owner, arb_owner_public, arb_retryable_tx,
            arb_statistics, arb_sys,
        },
        state::{ArbState, ArbStateGetter},
    };

    type TestContext = ArbitrumContext<EmptyDBTyped<Infallible>>;

    fn setup() -> TestContext {
        let db = EmptyDBTyped::<Infallible>::default();

        ArbitrumContext {
            journaled_state: Journal::new(db),
            block: BlockEnv::default(),
            cfg: ArbitrumConfig::default(),
            tx: TxEnv::default(),
            chain: (),
            local: ArbitrumLocalContext::default(),
            error: Ok(()),
        }
    }

    fn exec(
        precompile: ExtendedPrecompile<TestContext>,
        context: &mut TestContext,
        input: Vec<u8>,
        caller: Address,
        is_static: bool,
    ) -> InterpreterResult {
        let target = *precompile.address();

        precompile
            .execute(context, &input, &target, caller, U256::ZERO, is_static, 1_000_000)
            .expect("precompile dispatch")
            .expect("precompile returned result")
    }

    #[test]
    fn arb_address_table_size_defaults_to_zero() {
        let mut context = setup();
        let precompile = arb_address_table::arb_address_table_precompile::<TestContext>();
        let input = arb_address_table::ArbAddressTable::sizeCall::abi_encode(
            &arb_address_table::ArbAddressTable::sizeCall {},
        );

        let result = exec(
            precompile,
            &mut context,
            input,
            address!("0xaa00000000000000000000000000000000000001"),
            true,
        );
        assert_eq!(result.result, InstructionResult::Return);

        let size = arb_address_table::ArbAddressTable::sizeCall::abi_decode_returns(&result.output)
            .expect("decode size()");
        assert_eq!(size, U256::ZERO);
    }

    #[test]
    fn arb_aggregator_returns_default_batch_poster() {
        let mut context = setup();
        let precompile = arb_aggregator::arb_aggregator_precompile::<TestContext>();
        let input = arb_aggregator::ArbAggregator::getDefaultAggregatorCall::abi_encode(
            &arb_aggregator::ArbAggregator::getDefaultAggregatorCall {},
        );

        let result = exec(
            precompile,
            &mut context,
            input,
            address!("0xaa00000000000000000000000000000000000002"),
            true,
        );
        assert_eq!(result.result, InstructionResult::Return);

        let default_agg =
            arb_aggregator::ArbAggregator::getDefaultAggregatorCall::abi_decode_returns(
                &result.output,
            )
            .expect("decode getDefaultAggregator()");
        assert_eq!(default_agg, ARBOS_BATCH_POSTER_ADDRESS);
    }

    #[test]
    fn arb_debug_can_promote_to_chain_owner() {
        let mut context = setup();
        let precompile = arb_debug::arb_debug_precompile::<TestContext>();
        let caller = address!("0x000000000000000000000000000000000000beef");
        let input = arb_debug::ArbDebug::becomeChainOwnerCall::abi_encode(
            &arb_debug::ArbDebug::becomeChainOwnerCall {},
        );

        let result = exec(precompile, &mut context, input, caller, false);
        assert_eq!(result.result, InstructionResult::Return);

        let mut gas = Gas::new(100_000);
        let is_owner =
            context.arb_state(Some(&mut gas)).is_chain_owner(caller).expect("owner lookup");
        assert!(is_owner);
    }

    #[test]
    fn arb_gas_info_prices_query_succeeds() {
        let mut context = setup();
        let precompile = arb_gas_info::arb_gas_info_precompile::<TestContext>();
        let input = arb_gas_info::ArbGasInfo::getPricesInWeiCall::abi_encode(
            &arb_gas_info::ArbGasInfo::getPricesInWeiCall {},
        );

        let result = exec(
            precompile,
            &mut context,
            input,
            address!("0xaa00000000000000000000000000000000000003"),
            true,
        );
        assert_eq!(result.result, InstructionResult::Return);

        let prices =
            arb_gas_info::ArbGasInfo::getPricesInWeiCall::abi_decode_returns(&result.output)
                .expect("decode getPricesInWei()");
        //let (_, _, _, base, _, total) = prices;
        assert!(prices._5 >= prices._3);
    }

    #[test]
    fn arb_info_reads_zero_balance() {
        let mut context = setup();
        let precompile = arb_info::arb_info_precompile::<TestContext>();
        let input =
            arb_info::ArbInfo::getBalanceCall::abi_encode(&arb_info::ArbInfo::getBalanceCall {
                account: address!("0xaa00000000000000000000000000000000000000"),
            });

        let result = exec(
            precompile,
            &mut context,
            input,
            address!("0xaa00000000000000000000000000000000000004"),
            true,
        );
        assert_eq!(result.result, InstructionResult::Return);

        let balance =
            arb_info::ArbInfo::getBalanceCall::abi_decode_returns(&result.output).expect("decode");
        assert_eq!(balance, U256::ZERO);
    }

    #[test]
    fn arb_owner_reports_chain_owner_set_in_state() {
        let mut context = setup();
        let owner = address!("0xaa00000000000000000000000000000000000005");
        let mut gas = Gas::new(100_000);
        context.arb_state(Some(&mut gas)).chain_owners().add(owner).expect("set owner");

        let precompile = arb_owner::arb_owner_precompile::<TestContext>();
        let input = arb_owner::ArbOwner::isChainOwnerCall::abi_encode(
            &arb_owner::ArbOwner::isChainOwnerCall { addr: owner },
        );

        let result = exec(precompile, &mut context, input, owner, true);
        assert_eq!(result.result, InstructionResult::Return);

        let is_owner = arb_owner::ArbOwner::isChainOwnerCall::abi_decode_returns(&result.output)
            .expect("decode");
        assert!(is_owner);
    }

    #[test]
    fn arb_owner_public_lists_chain_owner() {
        let mut context = setup();
        let owner = address!("0xaa00000000000000000000000000000000000006");
        let mut gas = Gas::new(100_000);
        context.arb_state(Some(&mut gas)).chain_owners().add(owner).expect("set owner");

        let precompile = arb_owner_public::arb_owner_public_precompile::<TestContext>();
        let input = arb_owner_public::ArbOwnerPublic::getAllChainOwnersCall::abi_encode(
            &arb_owner_public::ArbOwnerPublic::getAllChainOwnersCall {},
        );

        let result = exec(precompile, &mut context, input, owner, true);
        assert_eq!(result.result, InstructionResult::Return);

        let owners = arb_owner_public::ArbOwnerPublic::getAllChainOwnersCall::abi_decode_returns(
            &result.output,
        )
        .expect("decode owners");
        assert_eq!(owners, vec![owner]);
    }

    #[test]
    fn arb_retryable_tx_reports_lifetime() {
        let mut context = setup();
        let precompile = arb_retryable_tx::arb_retryable_tx_precompile::<TestContext>();
        let input = arb_retryable_tx::ArbRetryableTx::getLifetimeCall::abi_encode(
            &arb_retryable_tx::ArbRetryableTx::getLifetimeCall {},
        );

        let result = exec(
            precompile,
            &mut context,
            input,
            address!("0xaa00000000000000000000000000000000000007"),
            true,
        );
        assert_eq!(result.result, InstructionResult::Return);

        let lifetime =
            arb_retryable_tx::ArbRetryableTx::getLifetimeCall::abi_decode_returns(&result.output)
                .expect("decode lifetime");
        assert_eq!(lifetime, U256::from(7 * 24 * 60 * 60));
    }

    #[test]
    fn arb_statistics_returns_block_number() {
        let mut context = setup();
        let precompile = arb_statistics::arb_statistics_precompile::<TestContext>();
        let input = arb_statistics::ArbStatistics::getStatsCall::abi_encode(
            &arb_statistics::ArbStatistics::getStatsCall {},
        );

        let result = exec(
            precompile,
            &mut context,
            input,
            address!("0xaa00000000000000000000000000000000000008"),
            true,
        );
        assert_eq!(result.result, InstructionResult::Return);

        let stats = arb_statistics::ArbStatistics::getStatsCall::abi_decode_returns(&result.output)
            .expect("decode stats");
        assert_eq!(stats._0, context.block().number);
    }

    #[test]
    fn arb_sys_chain_id_matches_context() {
        let mut context = setup();
        let precompile = arb_sys::arb_sys_precompile::<TestContext>();
        let input =
            arb_sys::ArbSys::arbChainIDCall::abi_encode(&arb_sys::ArbSys::arbChainIDCall {});

        let result = exec(
            precompile,
            &mut context,
            input,
            address!("0xaa00000000000000000000000000000000000009"),
            true,
        );
        assert_eq!(result.result, InstructionResult::Return);

        let chain_id =
            arb_sys::ArbSys::arbChainIDCall::abi_decode_returns(&result.output).expect("decode id");
        assert_eq!(chain_id, context.cfg().chain_id);
    }

    #[test]
    fn arb_native_token_manager_rejects_unknown_caller() {
        let mut context = setup();
        let precompile =
            arb_native_token_manager::arb_native_token_manager_precompile::<TestContext>();
        let input =
            arb_native_token_manager::ArbNativeTokenManager::mintNativeTokenCall::abi_encode(
                &arb_native_token_manager::ArbNativeTokenManager::mintNativeTokenCall {
                    amount: U256::from(10),
                },
            );

        let result = exec(
            precompile,
            &mut context,
            input,
            address!("0xaa0000000000000000000000000000000000000a"),
            false,
        );
        assert_eq!(result.result, InstructionResult::Revert);
    }
}
