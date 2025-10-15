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
        api::{EvmApiMethod, Gas as ArbGas, VecReader},
        req::EvmApiRequestor,
        user::{UserOutcome, UserOutcomeKind},
    },
};

use lru::LruCache;
use revm::{
    Inspector,
    context::{Block, Cfg, ContextSetters, ContextTr, JournalTr, LocalContextTr, Transaction},
    handler::{EvmTr, PrecompileProvider, instructions::InstructionProvider},
    inspector::{InspectorEvmTr, JournalExt},
    interpreter::{
        CallInput, FrameInput, Gas, InputsImpl, InstructionResult, InterpreterAction,
        InterpreterResult, interpreter::EthInterpreter, interpreter_types::InputsTr,
    },
    primitives::{Address, B256, Bytes, FixedBytes, Log, U256, alloy_primitives::U64, keccak256},
};
use stylus::{
    brotli::{self, Dictionary},
    native::{self, NativeInstance},
    prover::{
        machine::Module,
        programs::{
            StylusData,
            config::{CompileConfig, StylusConfig},
            meter::MeteredMachine,
        },
    },
    run::RunProgram,
};

use crate::{
    ArbitrumEvm,
    chain::ArbitrumChainInfoTr,
    constants::{
        COST_SCALAR_PERCENT, INITIAL_CACHED_COST_SCALAR, INITIAL_FREE_PAGES,
        INITIAL_INIT_COST_SCALAR, INITIAL_MIN_CACHED_GAS, INITIAL_MIN_INIT_GAS, INITIAL_PAGE_GAS,
        MEMORY_EXPONENTS, MIN_CACHED_GAS_UNITS, MIN_INIT_GAS_UNITS, STYLUS_DISCRIMINANT,
    },
    context::ArbitrumContextTr,
    stylus_api::StylusHandler,
};

type ProgramCacheEntry = (Vec<u8>, Module, StylusData);

lazy_static::lazy_static! {
    pub static ref PROGRAM_CACHE: Mutex<LruCache<FixedBytes<32>, ProgramCacheEntry>> = Mutex::new(LruCache::new(NonZeroUsize::new(1024).unwrap()));
}

type EvmApiHandler<'a> =
    Arc<Box<dyn Fn(EvmApiMethod, Vec<u8>) -> (Vec<u8>, VecReader, arbutil::evm::api::Gas) + 'a>>;

pub fn build_evm_data<CTX>(context: &mut CTX, input: InputsImpl) -> EvmData
where
    CTX: ArbitrumContextTr,
{
    let config_env = context.cfg();
    let arbos_env = context.chain();

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

pub fn stylus_call_cost(new: u16, open: u16, ever: u16) -> u64 {
    let new_open = open.saturating_add(new);
    let new_ever = max(ever, new_open);

    if new_ever < INITIAL_FREE_PAGES as u16 {
        return 0;
    }

    let adding = new_open.saturating_sub(open).saturating_sub(INITIAL_FREE_PAGES as u16);
    let linear = (adding as u64).saturating_mul(INITIAL_PAGE_GAS);
    let exp = |x: u16| -> u64 {
        if x < MEMORY_EXPONENTS.len() as u16 {
            return MEMORY_EXPONENTS[x as usize] as u64;
        }

        u64::MAX
    };

    let expand = exp(new_ever) - exp(ever);

    linear.saturating_add(expand)
}

pub fn init_gas(params: StylusData) -> u64 {
    let base = MIN_INIT_GAS_UNITS * INITIAL_MIN_INIT_GAS;
    let dyno = (params.init_cost as u64)
        .saturating_mul(INITIAL_INIT_COST_SCALAR as u64 * COST_SCALAR_PERCENT);
    base.saturating_add(dyno.div_ceil(100))
}

pub fn cached_gas(params: StylusData) -> u64 {
    let base = INITIAL_MIN_CACHED_GAS * MIN_CACHED_GAS_UNITS;
    let dyno = (params.cached_init_cost as u64)
        .saturating_mul(INITIAL_CACHED_COST_SCALAR as u64 * COST_SCALAR_PERCENT);
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
        let context = self.ctx();

        let compile_config =
            CompileConfig::version(context.chain().stylus_version(), context.chain().debug_mode());

        let stylus_config = StylusConfig::new(
            context.chain().stylus_version(),
            context.chain().max_depth(),
            context.chain().ink_price(),
        );

        // 1. convert code_hash to module_hash by checking activated
        // 2. if not activated and activation is required, revert
        // 3. if not activated and activation is not required, activate
        // 3. if default cache is 

        let (serialized, _module, stylus_data) = {
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
                (serialized, module, stylus_data)
            } else {
                let bytecode = context.journal_mut().code(stylus_ctx.bytecode_address).ok()?.data;

                if !bytecode.starts_with(STYLUS_DISCRIMINANT) {
                    return None;
                }

                if let Ok((serialized, module, stylus_data)) = compile_stylus_bytecode(
                    &bytecode,
                    code_hash,
                    context.chain().arbos_version(),
                    context.chain().stylus_version(),
                    stylus_ctx.gas_limit,
                    &compile_config,
                ) {
                    let mut cache = PROGRAM_CACHE.lock().unwrap();
                    cache.get_or_insert(code_hash, || {
                        (serialized.clone(), module.clone(), stylus_data)
                    });

                    (serialized, module, stylus_data)
                } else {
                    return None;
                }
            }
        };


        // let stored_module_hash = crate::stylus_state::module_hash(context, &code_hash)?;
        // println!(
        //     "Stylus module hash from ArbOS state: {}, computed module hash: {}",
        //     stored_module_hash, module_hash
        // );

        let mut cached = false;

        if let Some(program_info) = crate::stylus_state::program_info(context, &code_hash) {
            cached = program_info.cached;

            if context.chain().auto_activate_stylus() {
                // auto-activate if not matching
                // println!("Found existing stylus program info: {:?}", program_info);
            } else {
                // if not auto-activate, require match
                if program_info.version != stylus_config.version
                    || program_info.init_cost != stylus_data.init_cost
                    || program_info.cached_cost != stylus_data.cached_init_cost
                    || program_info.footprint != stylus_data.footprint
                {
                    // mismatch, revert
                    println!(
                        "Stylus program info mismatch, reverting. Stored: {:?}, Current: {:?}",
                        program_info, stylus_data
                    );
                    return Some(InterpreterAction::Return(InterpreterResult {
                        result: InstructionResult::Revert,
                        output: Default::default(),
                        gas: Default::default(),
                    }));
                } else {
                    // println!("Stylus program info matched: {:?}", program_info);
                }
            }
        } else if !context.chain().auto_activate_stylus() {
            // require activation, but no info found, revert
            println!("No stylus program info found, reverting.");
            return Some(InterpreterAction::Return(InterpreterResult {
                result: InstructionResult::Revert,
                output: Default::default(),
                gas: Default::default(),
            }));
        } else {
            println!("No existing stylus program info found, auto-activating.");
        }

        // TODO: should we have a mode that only auto-caches newly deployed programs, but keep
        // existing programs as non-cached unless explicitly cached?
        cached = cached || context.chain().auto_cache_stylus();

        

        let inputs = InputsImpl {
            target_address: stylus_ctx.target_address,
            caller_address: stylus_ctx.caller_address,
            input: CallInput::Bytes(Bytes::from(stylus_ctx.calldata.to_vec())),
            call_value: stylus_ctx.call_value,
            bytecode_address: Some(stylus_ctx.target_address),
        };

        let mut call_cost = stylus_call_cost(stylus_data.footprint, 0, INITIAL_FREE_PAGES as u16);
        
        if cached {
            call_cost += cached_gas(stylus_data);
        } else {
            call_cost += init_gas(stylus_data);
        }

        let mut gas = Gas::new(stylus_ctx.gas_limit);
        if !gas.record_cost(call_cost) {
            return Some(InterpreterAction::Return(InterpreterResult {
                result: InstructionResult::OutOfGas,
                output: Default::default(),
                gas: Default::default(),
            }));
        }

        let evm_data = build_evm_data(self.ctx(), inputs.clone());
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

 /// Compile Stylus bytecode
    pub fn compile_stylus_bytecode(
        bytecode: &Bytes,
        code_hash: B256,
        arbos_version: u16,
        stylus_version: u16,
        _gas_limit: u64,
        compile_config: &CompileConfig,
    ) -> Result<(Vec<u8>, Module, StylusData), ()> {
        if let Some(bytecode) = bytecode.strip_prefix(STYLUS_DISCRIMINANT) {
            let (dictionary, compressed_bytecode) =
                if let Some((dictionary, compressed_bytecode)) = bytecode.split_at_checked(1) {
                    (dictionary, compressed_bytecode)
                } else {
                    return Err(());
                };

            let dictionary = match dictionary[0] {
                0x00 => Dictionary::Empty,
                0x01 => Dictionary::StylusProgram,
                _ => unreachable!(),
            };

            let bytecode = brotli::decompress(compressed_bytecode, dictionary)
                .or_else(|err| {
                    if dictionary == Dictionary::Empty {
                        Ok(compressed_bytecode.to_vec())
                    } else {
                        Err(err)
                    }
                })
                .unwrap();

            let mut activation_gas = 10_000_000u64;
            let (module, stylus_data) = native::activate(
                bytecode.as_slice(),
                &Bytes32::from(code_hash.0),
                stylus_version,
                arbos_version as u64,
                128,
                false,
                &mut activation_gas,
            )
            .unwrap();

            let bytecode = native::compile(
                bytecode.as_slice(),
                compile_config.version,
                false,
                wasmer_types::compilation::target::Target::default(),
                true,
            )
            .unwrap();

            return Ok((bytecode, module, stylus_data));
        }

        Err(())
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
