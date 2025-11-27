use std::{
    cmp::max,
    mem,
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};

use arbutil::{
    Bytes20, Bytes32,
    evm::{
        EvmData,
        api::{EvmApiMethod, Gas as ArbGas, Ink, VecReader},
        req::EvmApiRequestor,
        user::{UserOutcome, UserOutcomeKind},
    },
    format::DebugBytes,
};

use lru::LruCache;
use revm::{
    Inspector, context::{Block, Cfg, ContextSetters, ContextTr, JournalTr, LocalContextTr, Transaction}, handler::{EvmTr, PrecompileProvider, instructions::InstructionProvider}, inspector::{InspectorEvmTr, JournalExt}, interpreter::{
        CallInput, FrameInput, Gas, InputsImpl, InstructionResult, InterpreterAction,
        InterpreterResult, gas::memory_gas, interpreter::EthInterpreter,
        interpreter_types::InputsTr,
    }, primitives::{Address, B256, Bytes, FixedBytes, Log, U256, alloy_primitives::U64, keccak256}
};
use stylus::{
    brotli::{self, Dictionary},
    native::{self, NativeInstance},
    prover::{
        machine::Module,
        programs::{
            StylusData,
            config::{CompileConfig, PricingParams, StylusConfig},
            meter::MeteredMachine,
        },
    },
    run::RunProgram,
};

use crate::{
    ArbitrumEvm,
    config::{ArbitrumConfigTr, ArbitrumStylusConfigTr},
    constants::{
        ARBOS_VERSION_STYLUS_FIXES, COST_SCALAR_PERCENT, MEMORY_EXPONENTS, MIN_CACHED_GAS_UNITS,
        MIN_INIT_GAS_UNITS, STYLUS_DISCRIMINANT,
    },
    context::ArbitrumContextTr,
    local_context::ArbitrumLocalContextTr,
    precompiles::arb_wasm::IArbWasm::{IArbWasmErrors, ProgramNotActivated},
    state::{ArbState, ArbStateGetter, program::ProgramInfo, types::ArbosStateError},
    stylus_api::StylusHandler,
};

type ProgramCacheEntry = (Vec<u8>, Module, StylusData);

lazy_static::lazy_static! {
    pub static ref PROGRAM_CACHE: Mutex<LruCache<FixedBytes<32>, ProgramCacheEntry>> = Mutex::new(LruCache::new(NonZeroUsize::new(1024).unwrap()));
}

type EvmApiHandler<'a> =
    Arc<Box<dyn Fn(EvmApiMethod, Vec<u8>) -> (Vec<u8>, VecReader, arbutil::evm::api::Gas) + 'a>>;

pub fn build_evm_data<CTX>(context: &CTX, input: InputsImpl) -> EvmData
where
    CTX: ArbitrumContextTr,
{
    let config_env = context.cfg();
    let arbos_env = context.cfg().stylus();

    let block_env = context.block();
    let tx_env = context.tx();

    let base_fee = block_env.basefee();

    let evm_data: EvmData = EvmData {
        arbos_version: arbos_env.arbos_version() as u64,
        block_basefee: Bytes32::from(U256::from(base_fee).to_be_bytes()),
        chainid: config_env.chain_id(),
        block_coinbase: Bytes20::try_from(block_env.beneficiary().as_slice()).unwrap(),
        block_gas_limit: U64::wrapping_from(block_env.gas_limit()).to::<u64>(),
        block_number: U64::wrapping_from(block_env.number()).to::<u64>(),
        block_timestamp: U64::wrapping_from(block_env.timestamp()).to::<u64>(),
        contract_address: Bytes20::try_from(input.target_address.as_slice()).unwrap(),
        module_hash: Bytes32::try_from(keccak256(input.target_address.as_slice()).as_slice())
            .unwrap(),
        msg_sender: Bytes20::try_from(input.caller_address.as_slice()).unwrap(),
        msg_value: Bytes32::try_from(input.call_value.to_be_bytes_vec()).unwrap(),
        tx_gas_price: Bytes32::from(
            U256::from(tx_env.effective_gas_price(base_fee as u128)).to_be_bytes(),
        ),
        tx_origin: Bytes20::try_from(tx_env.caller().as_slice()).unwrap(),
        reentrant: 0,
        return_data_len: 0,
        cached: true,
        tracing: true,
    };

    evm_data
}

// Shared data structure for Stylus execution context
pub(crate) struct StylusExecutionContext {
    target_address: Address,
    bytecode_address: Address,
    caller_address: Address,
    call_value: revm::primitives::U256,
    is_static: bool,
    gas_limit: u64,
    calldata: Bytes,
}

pub fn stylus_call_cost(new: u16, open: u16, ever: u16, free_pages: u16, page_gas: u16) -> u64 {
    let new_open = open.saturating_add(new);
    let new_ever = max(ever, new_open);

    if new_ever <= free_pages {
        return 0;
    }

    let sub_free = |pages: u16| pages.saturating_sub(free_pages);

    let adding = sub_free(new_open).saturating_sub(sub_free(open));
    let linear = (adding as u64).saturating_mul(page_gas as u64);
    let exp = |x: u16| -> u64 {
        if x < MEMORY_EXPONENTS.len() as u16 {
            return MEMORY_EXPONENTS[x as usize] as u64;
        }

        u64::MAX
    };

    let expand = exp(new_ever) - exp(ever);

    linear.saturating_add(expand)
}

pub fn init_gas_cost(init_cost: u16, min_init_gas: u8, init_cost_scaler: u8) -> u64 {
    let base = min_init_gas as u64 * MIN_INIT_GAS_UNITS;
    let dyno = (init_cost as u64).saturating_mul(init_cost_scaler as u64 * COST_SCALAR_PERCENT);
    base.saturating_add(dyno.div_ceil(100))
}

pub fn cached_gas_cost(
    cached_init_cost: u16,
    min_cached_init_gas: u8,
    cached_init_cost_scaler: u8,
) -> u64 {
    let base = min_cached_init_gas as u64 * MIN_CACHED_GAS_UNITS;
    let dyno = (cached_init_cost as u64)
        .saturating_mul(cached_init_cost_scaler as u64 * COST_SCALAR_PERCENT);
    base.saturating_add(dyno.div_ceil(100))
}

impl<CTX, INSP, P, I> ArbitrumEvm<CTX, INSP, P, I>
where
    CTX: ArbitrumContextTr,
    I: InstructionProvider<Context = CTX, InterpreterTypes = EthInterpreter>,
    P: PrecompileProvider<CTX, Output = InterpreterResult>,
{
    /// Common method to build API requestor for both inspected and non-inspected modes
    fn build_api_requestor(
        &mut self,
        input: InputsImpl,
        is_static: bool,
        request_handler: impl Fn(
            &mut Self,
            InputsImpl,
            bool,
            EvmApiMethod,
            Vec<u8>,
        ) -> (Vec<u8>, VecReader, ArbGas),
    ) -> EvmApiRequestor<VecReader, StylusHandler> {
        let evm = Arc::new(Mutex::new(self));

        let callback = {
            let evm = evm.clone();

            move |req_type: arbutil::evm::api::EvmApiMethod,
                  req_data: Vec<u8>|
                  -> (Vec<u8>, VecReader, arbutil::evm::api::Gas) {
                let mut evm = evm.lock().unwrap();
                request_handler(&mut evm, input.clone(), is_static, req_type, req_data)
            }
        };

        let callback: EvmApiHandler<'_> = Arc::new(Box::new(callback));
        let unsafe_callback: &'static EvmApiHandler<'_> = unsafe { mem::transmute(&callback) };
        EvmApiRequestor::new(StylusHandler::new(unsafe_callback.clone()))
    }

    /// Extract common Stylus execution context from frame input
    fn extract_stylus_context(&mut self) -> Option<(StylusExecutionContext, B256)> {
        let frame_input = {
            let frame = self.frame_stack().get();
            match frame.input {
                FrameInput::Call(ref input) => input.clone(),
                _ => return None,
            }
        };

        let bytecode_address = frame_input.bytecode_address;

        let code_hash = {
            let ctx = self.ctx();
            if let Ok(code_hash) = ctx.journal_mut().code_hash(bytecode_address) {
                code_hash.data
            } else {
                return None;
            }
        };

        let calldata = match &frame_input.input {
            CallInput::Bytes(calldata) => calldata.clone(),
            CallInput::SharedBuffer(range) => {
                if let Some(slice) = self.ctx().local().shared_memory_buffer_slice(range.clone()) {
                    Bytes::from(slice.to_vec())
                } else {
                    Bytes::new()
                }
            }
        };

        let context = StylusExecutionContext {
            target_address: frame_input.target_address,
            bytecode_address,
            caller_address: frame_input.caller,
            call_value: frame_input.value.get(),
            is_static: frame_input.is_static,
            gas_limit: frame_input.gas_limit,
            calldata,
        };

        Some((context, code_hash))
    }

    /// Core Stylus execution logic shared between inspected and non-inspected modes
    pub(crate) fn execute_stylus_program(
        &mut self,
        stylus_ctx: StylusExecutionContext,
        code_hash: B256,
        api_request_handler: impl Fn(
            &mut Self,
            InputsImpl,
            bool,
            EvmApiMethod,
            Vec<u8>,
        ) -> (Vec<u8>, VecReader, ArbGas),
    ) -> Option<InterpreterAction> {
        let mut gas = Gas::new(stylus_ctx.gas_limit);

        let (serialized, _module, stylus_data, stylus_params) = {
            // Use read lock to get cached program if available
            // if not available drop the read lock and acquire write lock to compile and insert
            let maybe_cached = {
                let mut cache = PROGRAM_CACHE.lock().unwrap();
                if let Some((serialized, module, stylus_data)) = cache.get(&code_hash).cloned() {
                    Some((serialized, module, stylus_data))
                } else {
                    None
                }
            };

            if let Some((serialized, module, stylus_data)) = maybe_cached {
                let stylus_params = {
                    let context = self.ctx();

                    match context.arb_state(None).programs().get_stylus_params() {
                        Ok(params) => params,
                        Err(e) => return Some(e.into()),
                    }
                };

                (serialized, module, stylus_data, stylus_params)
            } else {
                let context = self.ctx();

                let bytecode = context.journal_mut().code(stylus_ctx.bytecode_address).ok()?.data;

                let bytecode = match stylus_code(&bytecode)  {
                    Ok(code) => code,
                    Err(e) => {
                        return Some(InterpreterAction::Return(InterpreterResult {
                            result: InstructionResult::Revert,
                            output: e.into(),
                            gas,
                        }));
                    }
                };

                let stylus_params = {
                    match context.arb_state(None).programs().get_stylus_params() {
                        Ok(params) => params,
                        Err(e) => return Some(e.into()),
                    }
                };

                let compile_config = CompileConfig::version(
                    stylus_params.version,
                    context.cfg().stylus().debug_mode(),
                );

                let debug = context.cfg().stylus().debug_mode();

                let mut cache = PROGRAM_CACHE.lock().unwrap();
                match cache.try_get_or_insert::<_, Vec<u8>>(code_hash, || {
                    let serialized = stylus_compile(
                        &bytecode,
                        &compile_config,
                    )?;

                    let (module, stylus_data) = stylus_activate(
                        Some(&mut gas),
                        &Bytes::from(serialized.clone()),
                        code_hash,
                        context.cfg().stylus().arbos_version(),
                        stylus_params.version,
                        stylus_params.page_limit,
                        debug,
                    )?;

                    Ok((serialized, module, stylus_data))
                }) {
                    Ok((serialized, module, stylus_data)) => {
                        (serialized.clone(), module.clone(), stylus_data.clone(), stylus_params)
                    }
                    Err(e) => {
                        return Some(InterpreterAction::Return(InterpreterResult {
                            result: InstructionResult::Revert,
                            output: e.into(),
                            gas,
                        }));
                    }
                }   
            }
        };
            
   

        let (stylus_config, compile_config, evm_data) = {
            let context = self.ctx();

            let stylus_config = StylusConfig::new(
                stylus_params.version,
                stylus_params.max_stack_depth,
                stylus_params.ink_price,
            );

            let compile_config =
                CompileConfig::version(stylus_params.version, context.cfg().stylus().debug_mode());

            let evm_data = build_evm_data(
                self.ctx(),
                InputsImpl {
                    target_address: stylus_ctx.target_address,
                    caller_address: stylus_ctx.caller_address,
                    input: CallInput::Bytes(stylus_ctx.calldata.clone()),
                    call_value: stylus_ctx.call_value,
                    bytecode_address: Some(stylus_ctx.target_address),
                },
            );

            (stylus_config, compile_config, evm_data)
        };

        let program_info = match self
            .ctx()
            .arb_state(None)
            .programs()
            .get_active_program(&stylus_params, &code_hash)
        {
            Ok(Some(info)) => info,
            Ok(None) => {
                if !self.ctx().cfg().stylus().disable_auto_activate() {
                    ProgramInfo {
                        version: stylus_params.version,
                        init_cost: stylus_data.init_cost,
                        cached_cost: stylus_data.cached_init_cost,
                        footprint: stylus_data.footprint,
                        asm_estimated_kb: stylus_data.asm_estimate,
                        age: 0,
                        cached: !self.ctx().cfg().stylus().disable_auto_cache(),
                    }
                } else {
                    return Some(
                        ArbosStateError::ProgramError(IArbWasmErrors::ProgramNotActivated(
                            ProgramNotActivated,
                        ))
                        .into(),
                    );
                }
            }
            Err(e) => {
                return Some(e.into());
            }
        };

        let cached = program_info.cached
            || self.ctx().local_mut().insert_recent_wasm(code_hash, stylus_params.block_cache_size);

        let inputs = InputsImpl {
            target_address: stylus_ctx.target_address,
            caller_address: stylus_ctx.caller_address,
            input: CallInput::Bytes(Bytes::from(stylus_ctx.calldata.to_vec())),
            call_value: stylus_ctx.call_value,
            bytecode_address: Some(stylus_ctx.target_address),
        };

        let (call_cost, stylus_open_pages) = {
            let context = self.ctx();
            let wasm_open_pages = context.local().stylus_pages_open();

            let page_grow_cost = stylus_call_cost(
                stylus_data.footprint,
                wasm_open_pages,
                context.local().stylus_pages_ever(),
                stylus_params.free_pages,
                stylus_params.page_gas,
            );

            let program_cost = cached_gas_cost(
                stylus_data.cached_init_cost,
                stylus_params.min_cached_init_gas,
                stylus_params.cached_cost_scalar,
            );

            let init_cost = init_gas_cost(
                stylus_data.init_cost,
                stylus_params.min_init_gas,
                stylus_params.init_cost_scalar,
            );

            let mut cost = page_grow_cost;
            if cached || stylus_params.version > 1 {
                cost = cost.saturating_add(program_cost);
            }
            if !cached {
                cost = cost.saturating_add(init_cost);
            }

            (cost, wasm_open_pages)
        };

        if !gas.record_cost(call_cost) {
            return Some(InterpreterAction::Return(InterpreterResult {
                result: InstructionResult::OutOfGas,
                output: Default::default(),
                gas: Default::default(),
            }));
        }

        {
            self.ctx().local_mut().add_stylus_pages_open(stylus_data.footprint);
        }

        let evm_api =
            self.build_api_requestor(inputs.clone(), stylus_ctx.is_static, api_request_handler);

        let mut instance = unsafe {
            NativeInstance::deserialize(serialized.as_slice(), compile_config, evm_api, evm_data)
                .unwrap()
        };

        let ink_limit = stylus_config.pricing.gas_to_ink(arbutil::evm::api::Gas(gas.remaining()));
        gas.spend_all();

        let bytecode = match inputs.input() {
            CallInput::Bytes(bytes) => bytes,
            CallInput::SharedBuffer(_) => todo!(),
        };

        let outcome = match instance.run_main(bytecode, stylus_config, ink_limit) {
            Err(e) | Ok(UserOutcome::Failure(e)) => UserOutcome::Failure(e.wrap_err("call failed")),
            Ok(outcome) => outcome,
        };

        let mut gas_left = stylus_config.pricing.ink_to_gas(instance.ink_left().into()).0;

        let (kind, data) = outcome.into_data();

        let result = match kind {
            UserOutcomeKind::Success => revm::interpreter::InstructionResult::Return,
            UserOutcomeKind::Revert => revm::interpreter::InstructionResult::Revert,
            UserOutcomeKind::Failure => revm::interpreter::InstructionResult::Revert,
            UserOutcomeKind::OutOfInk => revm::interpreter::InstructionResult::OutOfGas,
            UserOutcomeKind::OutOfStack => {
                gas_left = 0;
                revm::interpreter::InstructionResult::StackOverflow
            }
        };

        gas.erase_cost(gas_left);

        self.ctx().local_mut().set_stylus_pages_open(stylus_open_pages);

        if data.len() > 0 && self.ctx().cfg().stylus().arbos_version() >= ARBOS_VERSION_STYLUS_FIXES
        {
            let evm_cost = memory_gas(data.len());

            if gas.limit() < evm_cost {
                gas.spend_all();
                return Some(InterpreterAction::Return(InterpreterResult {
                    result: InstructionResult::OutOfGas,
                    output: Default::default(),
                    gas,
                }));
            }

            let max_gas_to_return = gas.limit().saturating_sub(evm_cost);
            // gas.remaining = min(gas.remaining, max_gas_to_return)
            // gas.spent = gas.limit - gas.remaining
            if gas.remaining() > max_gas_to_return {
                gas.set_spent(gas.limit() - max_gas_to_return);
            }
        }
        Some(InterpreterAction::Return(InterpreterResult { result, output: data.into(), gas }))
    }

    pub fn frame_run_stylus(&mut self) -> Option<InterpreterAction> {
        let (stylus_ctx, code_hash) = self.extract_stylus_context()?;
        self.execute_stylus_program(
            stylus_ctx,
            code_hash,
            |evm, inputs, is_static, req_type, data| evm.request(inputs, is_static, req_type, data),
        )
    }
}

impl<CTX, INSP, P, I> ArbitrumEvm<CTX, INSP, P, I>
where
    CTX: ArbitrumContextTr,
    CTX::Journal: JournalExt,
    I: InstructionProvider<Context = CTX, InterpreterTypes = EthInterpreter>,
    P: PrecompileProvider<CTX, Output = InterpreterResult>,
    CTX: ContextSetters,
    INSP: Inspector<CTX>,
{
    pub fn inspect_frame_run_stylus(&mut self) -> Option<InterpreterAction> {
        let (stylus_ctx, code_hash) = self.extract_stylus_context()?;
        self.execute_stylus_program(
            stylus_ctx,
            code_hash,
            |evm, inputs, is_static, req_type, data| {
                evm.inspect_request(inputs, is_static, req_type, data)
            },
        )
    }

    pub(crate) fn inspect_request(
        &mut self,
        input: InputsImpl,
        is_static: bool,
        req_type: EvmApiMethod,
        data: Vec<u8>,
    ) -> (Vec<u8>, VecReader, ArbGas) {
        match req_type {
            EvmApiMethod::ContractCall | EvmApiMethod::DelegateCall | EvmApiMethod::StaticCall => {
                self.handle_contract_call(input, is_static, req_type, data, |evm, frame_init| {
                    evm.inspect_run_exec_loop(frame_init)
                })
            }

            EvmApiMethod::Create1 | EvmApiMethod::Create2 => self.handle_contract_creation(
                input,
                is_static,
                req_type,
                data,
                |evm, frame_init| evm.inspect_run_exec_loop(frame_init),
            ),

            EvmApiMethod::EmitLog => {
                self.handle_emit_log(input, data, |(evm, log): (&mut Self, Log)| {
                    let (context, inspector, frame) = evm.ctx_inspector_frame();
                    context.log(log.clone());
                    inspector.log(&mut frame.interpreter, context, log);
                })
            }
            _ => self.request_inner(input, is_static, req_type, data),
        }
    }
}

pub fn stylus_code(bytecode: &[u8]) -> Result<Bytes, Vec<u8>> {
    let bytecode = if let Some(bytecode) = bytecode.strip_prefix(STYLUS_DISCRIMINANT) {
        let (dictionary, compressed_bytecode) =
            if let Some((dictionary, compressed_bytecode)) = bytecode.split_at_checked(1) {
                (dictionary, compressed_bytecode)
            } else {
                return Err("specified bytecode is not a Stylus program".as_bytes().to_vec());
            };

        let dictionary = match dictionary[0] {
            0x00 => Dictionary::Empty,
            0x01 => Dictionary::StylusProgram,
            t => return Err(format!("unsupported dictionary {}", t).as_bytes().to_vec()),
        };

        let bytecode = brotli::decompress(compressed_bytecode, dictionary).or_else(|err| {
            // Special case to allow deployment of uncompressed bytecode
            if dictionary == Dictionary::Empty {
                Ok(compressed_bytecode.to_vec())
            } else {
                return Err(format!("failed decompression: {}", err as u8).as_bytes().to_vec());
            }
        })?;

        bytecode
    } else {
        return Err("specified bytecode is not a Stylus program".as_bytes().to_vec());
    };

    Ok(Bytes::from(bytecode))
}

/// Compile Stylus bytecode
pub fn stylus_compile(
    bytecode: &Bytes,
    compile_config: &CompileConfig,
) -> Result<Vec<u8>, Vec<u8>> {
    let serialized = native::compile(
        &bytecode,
        compile_config.version,
        compile_config.debug.debug_funcs,
        wasmer_types::compilation::target::Target::default(),
        false,
    )
    .map_err(|e| e.debug_bytes())?;

    Ok(serialized)
}

pub fn stylus_activate(
    mut gas: Option<&mut Gas>,
    bytecode: &Bytes,
    code_hash: B256,
    arbos_version: u16,
    stylus_version: u16,
    page_limit: u16,
    debug: bool,
) -> Result<(Module, StylusData), Vec<u8>> {

    let mut activation_gas = if let Some(gas) = gas.as_deref_mut() {
        let remaining = gas.remaining();
        gas.spend_all();
        remaining
    } else {
        u64::MAX
    };

    let (module, stylus_data) = match native::activate(
        bytecode,
        &Bytes32::from(code_hash.0),
        stylus_version,
        arbos_version as u64,
        page_limit,
        debug,
        &mut activation_gas,
    ) {
        Ok(res) => res,
        Err(e) => return Err(e.debug_bytes()),
    };

    if let Some(gas) = gas {
        gas.erase_cost(activation_gas);
    }

    return Ok((module, stylus_data));
}

pub fn ink_to_gas_ceil(pricing: PricingParams, ink: Ink) -> u64 {
    ink.0.div_ceil(pricing.ink_price as u64)
}

pub fn cache_program(
    code_hash: B256,
    serialized: Vec<u8>,
    module: Module,
    stylus_data: StylusData,
) {
    let mut cache = PROGRAM_CACHE.lock().unwrap();
    cache.get_or_insert(code_hash, || (serialized, module, stylus_data));
}
