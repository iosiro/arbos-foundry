use std::{
    cmp::min,
    mem,
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex},
};

use crate::{
    api::ArbitrumContextTr,
    buffer,
    chain_config::ArbitrumChainInfoTr,
    constants::STYLUS_DISCRIMINANT,
    stylus::{build_evm_data, PROGRAM_CACHE},
    stylus_api::{wasm_account_touch, StylusHandler},
};
use arbutil::{
    evm::{
        api::{EvmApiMethod, Gas as ArbGas, VecReader},
        req::EvmApiRequestor,
        user::{UserOutcome, UserOutcomeKind},
    },
    Bytes32,
};
use revm::{
    context::{
        Cfg, ContextError, ContextSetters, ContextTr, CreateScheme, Evm, FrameStack, JournalTr,
        LocalContextTr,
    },
    handler::{
        instructions::{EthInstructions, InstructionProvider},
        EthFrame, EvmTr, FrameInitOrResult, FrameResult, FrameTr, ItemOrResult, PrecompileProvider,
    },
    inspector::{InspectorEvmTr, JournalExt},
    interpreter::{
        gas::{create2_cost, initcode_cost, sload_cost, sstore_cost, CREATE},
        interpreter::EthInterpreter,
        interpreter_action::FrameInit,
        interpreter_types::InputsTr,
        CallInput, CallInputs, CreateInputs, FrameInput, Gas, InputsImpl, InstructionResult,
        InterpreterAction, InterpreterResult,
    },
    primitives::{hardfork::SpecId, Address, Bytes, Log, B256},
    Database, Inspector,
};
use stylus::{
    brotli::{self, Dictionary},
    native::{self, NativeInstance},
    prover::{
        machine::Module,
        programs::{
            config::{CompileConfig, StylusConfig},
            StylusData,
        },
    },
    run::RunProgram,
};

use stylus::prover::programs::meter::MeteredMachine;

type EvmApiHandler<'a> =
    Arc<Box<dyn Fn(EvmApiMethod, Vec<u8>) -> (Vec<u8>, VecReader, arbutil::evm::api::Gas) + 'a>>;

pub struct ArbitrumEvm<CTX, INSP, P, I = EthInstructions<EthInterpreter, CTX>, F = EthFrame>(
    pub Evm<CTX, INSP, I, P, F>,
);

impl<CTX, I, INSP, P, F> ArbitrumEvm<CTX, INSP, P, I, F> {
    /// Create a new EVM instance with a given context, inspector, instruction set, and precompile
    /// provider.
    pub fn new_with_inspector(ctx: CTX, inspector: INSP, instruction: I, precompiles: P) -> Self {
        ArbitrumEvm(Evm {
            ctx,
            inspector,
            instruction,
            precompiles,
            frame_stack: FrameStack::new(),
        })
    }
}

impl<CTX, INSP, P, I, F> Deref for ArbitrumEvm<CTX, INSP, P, I, F>
where
    CTX: ArbitrumContextTr + ContextSetters,
    INSP: Inspector<CTX, I::InterpreterTypes>,
    I: InstructionProvider<Context = CTX, InterpreterTypes = EthInterpreter>,
    P: PrecompileProvider<CTX, Output = InterpreterResult>,
{
    type Target = Evm<CTX, INSP, I, P, F>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<CTX, INSP, P, I, F> DerefMut for ArbitrumEvm<CTX, INSP, P, I, F>
where
    CTX: ArbitrumContextTr + ContextSetters,
    INSP: Inspector<CTX, I::InterpreterTypes>,
    I: InstructionProvider<Context = CTX, InterpreterTypes = EthInterpreter>,
    P: PrecompileProvider<CTX, Output = InterpreterResult>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<CTX, INSP, P, I> EvmTr for ArbitrumEvm<CTX, INSP, P, I, EthFrame<EthInterpreter>>
where
    CTX: ArbitrumContextTr,
    I: InstructionProvider<Context = CTX, InterpreterTypes = EthInterpreter>,
    P: PrecompileProvider<CTX, Output = InterpreterResult>,
{
    type Context = CTX;
    type Instructions = I;
    type Precompiles = P;
    type Frame = EthFrame<EthInterpreter>;

    fn ctx(&mut self) -> &mut Self::Context {
        &mut self.0.ctx
    }

    fn ctx_ref(&self) -> &Self::Context {
        &self.0.ctx
    }

    fn ctx_instructions(&mut self) -> (&mut Self::Context, &mut Self::Instructions) {
        (&mut self.0.ctx, &mut self.0.instruction)
    }

    fn ctx_precompiles(&mut self) -> (&mut Self::Context, &mut Self::Precompiles) {
        (&mut self.0.ctx, &mut self.0.precompiles)
    }

    fn frame_stack(&mut self) -> &mut FrameStack<Self::Frame> {
        &mut self.0.frame_stack
    }

    fn frame_init(
        &mut self,
        frame_input: <Self::Frame as FrameTr>::FrameInit,
    ) -> Result<
        ItemOrResult<&mut Self::Frame, <Self::Frame as FrameTr>::FrameResult>,
        ContextError<<<Self::Context as ContextTr>::Db as Database>::Error>,
    > {
        self.0.frame_init(frame_input)
    }

    fn frame_run(
        &mut self,
    ) -> Result<
        FrameInitOrResult<Self::Frame>,
        ContextError<<<Self::Context as ContextTr>::Db as Database>::Error>,
    > {
        if let Some(action) = self.frame_run_stylus() {
            let frame = self.0.frame_stack.get();
            let context = &mut self.0.ctx;
            return frame.process_next_action(context, action).inspect(|i| {
                if i.is_result() {
                    frame.set_finished(true);
                }
            });
        }

        self.0.frame_run()
    }

    fn frame_return_result(
        &mut self,
        result: <Self::Frame as FrameTr>::FrameResult,
    ) -> Result<
        Option<<Self::Frame as FrameTr>::FrameResult>,
        ContextError<<<Self::Context as ContextTr>::Db as Database>::Error>,
    > {
        self.0.frame_return_result(result)
    }
}

// Shared data structure for Stylus execution context
struct StylusExecutionContext {
    target_address: Address,
    caller_address: Address,
    call_value: revm::primitives::U256,
    is_static: bool,
    gas_limit: u64,
    calldata: Bytes,
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
            let inputs = input.clone();

            move |req_type: arbutil::evm::api::EvmApiMethod,
                  req_data: Vec<u8>|
                  -> (Vec<u8>, VecReader, arbutil::evm::api::Gas) {
                let mut evm = evm.lock().unwrap();
                request_handler(&mut evm, inputs.clone(), is_static, req_type, req_data)
            }
        };

        let callback: EvmApiHandler<'_> = Arc::new(Box::new(callback));
        let unsafe_callback: &'static EvmApiHandler<'_> = unsafe { mem::transmute(&callback) };
        EvmApiRequestor::new(StylusHandler::new(unsafe_callback.clone()))
    }

    /// Extract common Stylus execution context from frame input
    fn extract_stylus_context(&mut self) -> Option<(StylusExecutionContext, B256, Bytes)> {
        let frame_input = {
            let frame = self.frame_stack().get();
            match frame.input {
                FrameInput::Call(ref input) => input.clone(),
                _ => return None,
            }
        };

        let bytecode_address = frame_input.bytecode_address;

        let (code_hash, bytecode) = {
            let ctx = self.ctx();
            if let Ok(code_hash) = ctx.journal_mut().code_hash(bytecode_address) {
                let bytecode = ctx.journal_mut().code(bytecode_address).unwrap().data;
                (code_hash.data, bytecode)
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
            caller_address: frame_input.caller,
            call_value: frame_input.value.get(),
            is_static: frame_input.is_static,
            gas_limit: frame_input.gas_limit,
            calldata,
        };

        Some((context, code_hash, bytecode))
    }

    /// Compile Stylus bytecode
    fn compile_stylus_bytecode(
        bytecode: &Bytes,
        code_hash: B256,
        stylus_env: &dyn ArbitrumChainInfoTr,
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
                stylus_env.stylus_version(),
                stylus_env.arbos_version() as u64,
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

    /// Core Stylus execution logic shared between inspected and non-inspected modes
    fn execute_stylus_program(
        &mut self,
        stylus_ctx: StylusExecutionContext,
        code_hash: B256,
        bytecode: Bytes,
        api_request_handler: impl Fn(
            &mut Self,
            InputsImpl,
            bool,
            EvmApiMethod,
            Vec<u8>,
        ) -> (Vec<u8>, VecReader, ArbGas),
    ) -> Option<InterpreterAction> {
        let context = self.ctx();

        let stylus_env = context.chain();

        let compile_config =
            CompileConfig::version(stylus_env.stylus_version(), stylus_env.debug_mode());
        let stylus_config = StylusConfig::new(
            stylus_env.stylus_version(),
            stylus_env.max_depth(),
            stylus_env.ink_price(),
        );

        let (serialized, _module, stylus_data) = {
            let mut cache = PROGRAM_CACHE.lock().unwrap();
            if let Ok((serialized, module, stylus_data)) =
                cache.try_get_or_insert(code_hash, || {
                    Self::compile_stylus_bytecode(
                        &bytecode,
                        code_hash,
                        stylus_env,
                        stylus_ctx.gas_limit,
                        &compile_config,
                    )
                })
            {
                (serialized.clone(), module.clone(), *stylus_data)
            } else {
                return None;
            }
        };

        let inputs = InputsImpl {
            target_address: stylus_ctx.target_address,
            caller_address: stylus_ctx.caller_address,
            input: revm::interpreter::CallInput::Bytes(Bytes::from(stylus_ctx.calldata.to_vec())),
            call_value: stylus_ctx.call_value,
            bytecode_address: Some(stylus_ctx.target_address),
        };

        let evm_data = build_evm_data(self.ctx(), inputs.clone());
        let evm_api =
            self.build_api_requestor(inputs.clone(), stylus_ctx.is_static, api_request_handler);

        let mut instance = unsafe {
            NativeInstance::deserialize(serialized.as_slice(), compile_config, evm_api, evm_data)
                .unwrap()
        };

        let mut gas = Gas::new(stylus_ctx.gas_limit);
        if !gas.record_cost(stylus_data.init_cost as u64) {
            return Some(InterpreterAction::Return(InterpreterResult {
                result: InstructionResult::OutOfGas,
                output: Default::default(),
                gas: Default::default(),
            }));
        }

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
        let (stylus_ctx, code_hash, bytecode) = self.extract_stylus_context()?;
        self.execute_stylus_program(
            stylus_ctx,
            code_hash,
            bytecode,
            |evm, inputs, is_static, req_type, data| evm.request(inputs, is_static, req_type, data),
        )
    }

    /// Handle contract calls (ContractCall, DelegateCall, StaticCall)
    fn handle_contract_call(
        &mut self,
        input: InputsImpl,
        is_static: bool,
        req_type: EvmApiMethod,
        data: Vec<u8>,
        call_handler: impl FnOnce(
            &mut Self,
            FrameInit,
        ) -> Result<
            FrameResult,
            ContextError<<<CTX as ContextTr>::Db as Database>::Error>,
        >,
    ) -> (Vec<u8>, VecReader, ArbGas) {
        let mut data = data;
        let bytecode_address = buffer::take_address(&mut data);
        let value = buffer::take_u256(&mut data);
        let gas_left = buffer::take_u64(&mut data);
        let gas_limit = buffer::take_u64(&mut data);
        let calldata = buffer::take_rest(&mut data);

        let is_static = matches!(req_type, EvmApiMethod::StaticCall) || is_static;
        let (target_address, caller) = if matches!(req_type, EvmApiMethod::DelegateCall) {
            (input.target_address, input.caller_address)
        } else {
            (bytecode_address, input.target_address)
        };

        if is_static && !value.is_zero() {
            return (Status::WriteProtection.into(), VecReader::new(vec![]), ArbGas(gas_left));
        }

        let gas_limit = if self
            .ctx()
            .cfg()
            .spec()
            .into()
            //.into_eth_spec()
            .is_enabled_in(SpecId::TANGERINE)
        {
            min(gas_left - gas_left / 64, gas_limit)
        } else {
            gas_limit
        };

        let mut gas = Gas::new(gas_limit);
        gas.spend_all();

        let first_frame_input = FrameInput::Call(Box::new(CallInputs {
            input: CallInput::Bytes(calldata),
            return_memory_offset: 0..0,
            gas_limit,
            bytecode_address,
            target_address,
            caller,
            value: revm::interpreter::CallValue::Transfer(value),
            scheme: revm::interpreter::CallScheme::Call,
            is_static,
        }));

        let next_action = InterpreterAction::NewFrame(first_frame_input);

        let frame_result: Result<_, ContextError<<<CTX as ContextTr>::Db as Database>::Error>> =
            self.0.frame_stack.get().process_next_action(&mut self.0.ctx, next_action);

        let original_frame_stack = mem::replace(&mut self.0.frame_stack, FrameStack::new());

        if let Ok(ItemOrResult::Item(frame_init)) = frame_result {
            let result = call_handler(self, frame_init);

            self.0.frame_stack = original_frame_stack;
            self.0.frame_stack().get().interpreter.memory.free_child_context();

            if let Ok(FrameResult::Call(call_outcome)) = result {
                gas.erase_cost(call_outcome.gas().remaining());
                return (
                    Status::Success.into(),
                    VecReader::new(call_outcome.output().to_vec()),
                    ArbGas(gas.spent()),
                );
            }
        }

        (Status::Failure.into(), VecReader::new(vec![]), ArbGas(gas.spent()))
    }

    /// Handle contract creation (Create1, Create2)
    fn handle_contract_creation(
        &mut self,
        input: InputsImpl,
        is_static: bool,
        req_type: EvmApiMethod,
        data: Vec<u8>,
        call_handler: impl FnOnce(
            &mut Self,
            FrameInit,
        ) -> Result<
            FrameResult,
            ContextError<<<CTX as ContextTr>::Db as Database>::Error>,
        >,
    ) -> (Vec<u8>, VecReader, ArbGas) {
        let is_create_2 = matches!(req_type, EvmApiMethod::Create2);
        let mut data = data;
        let gas_remaining = buffer::take_u64(&mut data);
        let value = buffer::take_u256(&mut data);
        let salt = is_create_2.then(|| buffer::take_u256(&mut data));
        let init_code = buffer::take_rest(&mut data);

        let spec = self.ctx().cfg().spec().into();

        if is_static {
            return (
                [vec![0x00], "write protection".as_bytes().to_vec()].concat(),
                VecReader::new(vec![]),
                ArbGas(0),
            );
        }

        let error_response = (
            [vec![0x01], Address::ZERO.to_vec()].concat(),
            VecReader::new(vec![]),
            ArbGas(gas_remaining),
        );

        if is_create_2 && !spec.is_enabled_in(SpecId::PETERSBURG) {
            return error_response;
        }

        let mut gas_cost = 0;
        let len = init_code.len();

        if len != 0 && spec.is_enabled_in(SpecId::SHANGHAI) {
            let max_initcode_size = self.ctx().cfg().max_code_size().saturating_mul(2);
            if len > max_initcode_size {
                return error_response;
            }
            gas_cost = initcode_cost(len);
        }

        let scheme = if is_create_2 {
            if let Some(check_cost) = create2_cost(len).and_then(|cost| gas_cost.checked_add(cost))
            {
                gas_cost = check_cost;
            } else {
                return error_response;
            };
            CreateScheme::Create2 { salt: salt.unwrap() }
        } else {
            gas_cost += CREATE;
            CreateScheme::Create
        };

        if gas_remaining < gas_cost {
            return (
                [vec![0x00], "out of gas".as_bytes().to_vec()].concat(),
                VecReader::new(vec![]),
                ArbGas(gas_remaining),
            );
        }

        let gas_limit = gas_remaining - gas_cost;

        let gas_stipend = if spec.is_enabled_in(SpecId::TANGERINE) { gas_limit / 64 } else { 0 };

        let mut gas = Gas::new(gas_limit);
        _ = gas.record_cost(gas_stipend);

        let first_frame_input = FrameInput::Create(Box::new(CreateInputs {
            caller: input.target_address,
            scheme,
            value,
            init_code,
            gas_limit: gas.remaining(),
        }));

        gas.spend_all();

        let next_action = InterpreterAction::NewFrame(first_frame_input);

        let frame_result: Result<_, ContextError<<<CTX as ContextTr>::Db as Database>::Error>> =
            self.0.frame_stack.get().process_next_action(&mut self.0.ctx, next_action);

        let original_frame_stack = mem::replace(&mut self.0.frame_stack, FrameStack::new());

        if let Ok(ItemOrResult::Item(frame_init)) = frame_result {
            let result = call_handler(self, frame_init);

            self.0.frame_stack = original_frame_stack;
            self.0.frame_stack().get().interpreter.memory.free_child_context();

            if let Ok(FrameResult::Create(create_outcome)) = result {
                if InstructionResult::Revert == *create_outcome.instruction_result() {
                    return (
                        [vec![0x00], create_outcome.output().to_vec()].concat(),
                        VecReader::new(vec![]),
                        ArbGas(gas.spent()),
                    );
                }

                gas.erase_cost(create_outcome.gas().remaining());
                if let Some(address) = create_outcome.address {
                    gas.erase_cost(create_outcome.gas().remaining() + gas_stipend);

                    return (
                        [vec![0x01], address.to_vec()].concat(),
                        VecReader::new(vec![]),
                        ArbGas(gas.spent()),
                    );
                }
            }
        }

        error_response
    }

    /// Handle log emission with closure-based log handling
    fn handle_emit_log<F>(
        &mut self,
        input: InputsImpl,
        data: Vec<u8>,
        log_handler: F,
    ) -> (Vec<u8>, VecReader, ArbGas)
    where
        F: FnOnce((&mut Self, Log)),
    {
        let mut data = data;
        let topic_count = buffer::take_u32(&mut data);
        let mut topics = Vec::with_capacity(topic_count as usize);
        for _ in 0..topic_count {
            topics.push(buffer::take_bytes32(&mut data));
        }
        let log_data = buffer::take_rest(&mut data);

        let log = Log::new_unchecked(input.target_address, topics, log_data);

        log_handler((self, log));

        (vec![], VecReader::new(vec![]), ArbGas(0))
    }

    pub(crate) fn request(
        &mut self,
        input: InputsImpl,
        is_static: bool,
        req_type: EvmApiMethod,
        data: Vec<u8>,
    ) -> (Vec<u8>, VecReader, ArbGas) {
        match req_type {
            EvmApiMethod::ContractCall | EvmApiMethod::DelegateCall | EvmApiMethod::StaticCall => {
                self.handle_contract_call(input, is_static, req_type, data, |evm, frame_init| {
                    evm.run_exec_loop(frame_init)
                })
            }

            EvmApiMethod::Create1 | EvmApiMethod::Create2 => self.handle_contract_creation(
                input,
                is_static,
                req_type,
                data,
                |evm, frame_init| evm.run_exec_loop(frame_init),
            ),

            EvmApiMethod::EmitLog => {
                self.handle_emit_log(input, data, |(evm, log): (&mut Self, Log)| {
                    let context = evm.ctx();
                    context.log(log);
                })
            }

            _ => self.request_inner(input, is_static, req_type, data),
        }
    }

    fn request_inner(
        &mut self,
        input: InputsImpl,
        is_static: bool,
        req_type: EvmApiMethod,
        data: Vec<u8>,
    ) -> (Vec<u8>, VecReader, ArbGas) {
        let context = self.ctx();
        let mut data = data;

        let spec = context.cfg().spec();

        match req_type {
            EvmApiMethod::GetBytes32 => {
                let slot = buffer::take_u256(&mut data);
                if let Some(result) = context.sload(input.target_address, slot) {
                    let gas = sload_cost(spec.into(), result.is_cold);
                    (result.to_be_bytes_vec(), VecReader::new(vec![]), ArbGas(gas))
                } else {
                    (vec![], VecReader::new(vec![]), ArbGas(0))
                }
            }

            EvmApiMethod::SetTrieSlots => {
                let gas_left = buffer::take_u64(&mut data);

                if is_static {
                    return (
                        Status::WriteProtection.into(),
                        VecReader::new(vec![]),
                        ArbGas(gas_left),
                    );
                }

                let mut total_cost = 0;
                while !data.is_empty() {
                    let (key, value) = (buffer::take_u256(&mut data), buffer::take_u256(&mut data));

                    match context.sstore(input.target_address, key, value) {
                        Some(result) => {
                            total_cost +=
                                sstore_cost(spec.clone().into(), &result.data, result.is_cold);

                            if gas_left < total_cost {
                                return (
                                    Status::OutOfGas.into(),
                                    VecReader::new(vec![]),
                                    ArbGas(gas_left),
                                );
                            }
                        }
                        _ => {
                            return (
                                Status::Failure.into(),
                                VecReader::new(vec![]),
                                ArbGas(gas_left),
                            )
                        }
                    }
                }

                (Status::Success.into(), VecReader::new(vec![]), ArbGas(total_cost))
            }

            EvmApiMethod::GetTransientBytes32 => {
                let slot = buffer::take_u256(&mut data);
                let result = context.tload(input.target_address, slot);
                (result.to_be_bytes_vec(), VecReader::new(vec![]), ArbGas(0))
            }

            EvmApiMethod::SetTransientBytes32 => {
                if is_static {
                    return (Status::WriteProtection.into(), VecReader::new(vec![]), ArbGas(0));
                }
                let key = buffer::take_u256(&mut data);
                let value = buffer::take_u256(&mut data);
                context.tstore(input.target_address, key, value);
                (Status::Success.into(), VecReader::new(vec![]), ArbGas(0))
            }
            EvmApiMethod::AccountBalance => {
                let address = buffer::take_address(&mut data);
                let balance = context.balance(address).unwrap();
                let gas = wasm_account_touch(context, balance.is_cold, false);
                (balance.to_be_bytes_vec(), VecReader::new(vec![]), ArbGas(gas))
            }

            EvmApiMethod::AccountCode => {
                let address = buffer::take_address(&mut data);
                let code = context.load_account_code(address).unwrap();
                let gas = wasm_account_touch(context, code.is_cold, true);
                (vec![], VecReader::new(code.to_vec()), ArbGas(gas))
            }

            EvmApiMethod::AccountCodeHash => {
                let address = buffer::take_address(&mut data);
                let code_hash = context.load_account_code_hash(address).unwrap();
                let gas = wasm_account_touch(context, code_hash.is_cold, false);
                (code_hash.to_vec(), VecReader::new(vec![]), ArbGas(gas))
            }

            EvmApiMethod::AddPages => {
                let _count = buffer::take_u16(&mut data);
                (Status::Success.into(), VecReader::new(vec![]), ArbGas(0))
            }

            EvmApiMethod::CaptureHostIO => {
                //let data = buffer::take_rest(&mut data);
                //println!("CaptureHostIO: {:?}", String::from_utf8_lossy(&data));
                (Status::Success.into(), VecReader::new(vec![]), ArbGas(0))
            }
            _ => unimplemented!("EVM API method not implemented: {:?}", req_type),
        }
    }

    /// Executes the main frame processing loop.
    ///
    /// This loop manages the frame stack, processing each frame until execution completes.
    /// For each iteration:
    /// 1. Calls the current frame
    /// 2. Handles the returned frame input or result
    /// 3. Creates new frames or propagates results as needed
    #[inline]
    fn run_exec_loop(
        &mut self,
        first_frame_input: FrameInit,
    ) -> Result<FrameResult, ContextError<<<CTX as ContextTr>::Db as Database>::Error>> {
        let res = self.frame_init(first_frame_input)?;

        if let ItemOrResult::Result(frame_result) = res {
            return Ok(frame_result);
        }

        loop {
            let call_or_result = self.frame_run()?;

            let result = match call_or_result {
                ItemOrResult::Item(init) => {
                    match self.frame_init(init)? {
                        ItemOrResult::Item(_) => {
                            continue;
                        }
                        // Do not pop the frame since no new frame was created
                        ItemOrResult::Result(result) => result,
                    }
                }
                ItemOrResult::Result(result) => result,
            };

            if let Some(result) = self.frame_return_result(result)? {
                return Ok(result);
            }
        }
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
        let (stylus_ctx, code_hash, bytecode) = self.extract_stylus_context()?;
        self.execute_stylus_program(
            stylus_ctx,
            code_hash,
            bytecode,
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

    /// Run inspection on execution loop.
    ///
    /// It will call:
    /// * [`Inspector::call`],[`Inspector::create`] to inspect call, create and eofcreate.
    /// * [`Inspector::call_end`],[`Inspector::create_end`] to inspect call, create and eofcreate
    ///   end.
    /// * [`Inspector::initialize_interp`] to inspect initialized interpreter.
    fn inspect_run_exec_loop(
        &mut self,
        first_frame_input: FrameInit,
    ) -> Result<FrameResult, ContextError<<<CTX as ContextTr>::Db as Database>::Error>> {
        let res = self.inspect_frame_init(first_frame_input)?;

        if let ItemOrResult::Result(frame_result) = res {
            return Ok(frame_result);
        }

        loop {
            let call_or_result = self.inspect_frame_run()?;

            let result = match call_or_result {
                ItemOrResult::Item(init) => {
                    match self.inspect_frame_init(init)? {
                        ItemOrResult::Item(_) => {
                            continue;
                        }
                        // Do not pop the frame since no new frame was created
                        ItemOrResult::Result(result) => result,
                    }
                }
                ItemOrResult::Result(result) => result,
            };

            if let Some(result) = self.frame_return_result(result)? {
                return Ok(result);
            }
        }
    }
}

enum Status {
    Success,
    Failure,
    OutOfGas,
    WriteProtection,
}

impl From<Status> for Vec<u8> {
    fn from(status: Status) -> Vec<u8> {
        match status {
            Status::Success => vec![0],
            Status::Failure => vec![1],
            Status::OutOfGas => vec![2],
            Status::WriteProtection => vec![3],
        }
    }
}
