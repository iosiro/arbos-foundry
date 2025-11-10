use std::{cmp::min, mem, sync::Arc};

use arbutil::evm::{
    api::{EvmApiMethod, Gas as ArbGas, VecReader},
    req::RequestHandler,
};
use revm::{
    Database,
    context::{Cfg, ContextError, ContextTr, CreateScheme, FrameStack},
    handler::{
        EvmTr, FrameResult, ItemOrResult, PrecompileProvider, instructions::InstructionProvider,
    },
    interpreter::{
        CallInput, CallInputs, CreateInputs, FrameInput, Gas, InputsImpl, InstructionResult,
        InterpreterAction, InterpreterResult,
        gas::{initcode_cost, warm_cold_cost},
        interpreter::EthInterpreter,
        interpreter_action::FrameInit,
    },
    primitives::{Address, Log, hardfork::SpecId},
};

use crate::{ArbitrumContextTr, ArbitrumEvm, buffer};

pub(crate) type HostCallFunc = dyn Fn(
    arbutil::evm::api::EvmApiMethod,
    Vec<u8>,
) -> (Vec<u8>, VecReader, arbutil::evm::api::Gas);

pub(crate) struct StylusHandler {
    pub handler: Arc<Box<HostCallFunc>>,
}

unsafe impl Send for StylusHandler {}

impl StylusHandler {
    pub(crate) fn new(handler: Arc<Box<HostCallFunc>>) -> Self {
        Self { handler }
    }
}

impl RequestHandler<VecReader> for StylusHandler {
    fn request(
        &mut self,
        req_type: EvmApiMethod,
        req_data: impl AsRef<[u8]>,
    ) -> (Vec<u8>, VecReader, ArbGas) {
        let data = req_data.as_ref().to_vec();
        let api = self.handler.clone();
        (api)(req_type, data)
    }
}

pub fn wasm_account_touch<CTX>(context: CTX, is_cold: bool, with_code: bool) -> u64
where
    CTX: ArbitrumContextTr,
{
    let code_cost = if with_code { context.cfg().max_code_size() as u64 / 24576 * 700 } else { 0 };
    code_cost + warm_cold_cost(is_cold)
}

impl<CTX, INSP, P, I> ArbitrumEvm<CTX, INSP, P, I>
where
    CTX: ArbitrumContextTr,
    I: InstructionProvider<Context = CTX, InterpreterTypes = EthInterpreter>,
    P: PrecompileProvider<CTX, Output = InterpreterResult>,
{
    /// Handle contract calls (ContractCall, DelegateCall, StaticCall)
    pub(crate) fn handle_contract_call(
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
        _ = gas.record_cost(100);

        let first_frame_input = FrameInput::Call(Box::new(CallInputs {
            input: CallInput::Bytes(calldata),
            return_memory_offset: 0..0,
            gas_limit: gas.remaining(),
            bytecode_address,
            target_address,
            caller,
            value: revm::interpreter::CallValue::Transfer(value),
            scheme: revm::interpreter::CallScheme::Call,
            is_static,
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

            if let Ok(FrameResult::Call(call_outcome)) = result {
                gas.erase_cost(call_outcome.gas().remaining());

                let status = if call_outcome.instruction_result().is_ok() {
                    Status::Success
                } else {
                    Status::Failure
                };

                return (
                    status.into(),
                    VecReader::new(call_outcome.output().to_vec()),
                    ArbGas(gas.spent()),
                );
            }
        }

        (Status::Failure.into(), VecReader::new(vec![]), ArbGas(gas.spent()))
    }

    /// Handle contract creation (Create1, Create2)
    pub(crate) fn handle_contract_creation(
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
            if let Some(check_cost) = revm::interpreter::gas::create2_cost(len)
                .and_then(|cost| gas_cost.checked_add(cost))
            {
                gas_cost = check_cost;
            } else {
                return error_response;
            };
            CreateScheme::Create2 { salt: salt.unwrap() }
        } else {
            gas_cost += revm::interpreter::gas::CREATE;
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
    pub(crate) fn handle_emit_log<F>(
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

    pub(crate) fn request_inner(
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
                    let gas = revm::interpreter::gas::sload_cost(spec.into(), result.is_cold);
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
                            total_cost += revm::interpreter::gas::sstore_cost(
                                spec.clone().into(),
                                &result.data,
                                result.is_cold,
                            );

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
                            );
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
}

enum Status {
    Success,
    Failure,
    OutOfGas,
    WriteProtection,
}

impl From<Status> for Vec<u8> {
    fn from(status: Status) -> Self {
        match status {
            Status::Success => vec![0],
            Status::Failure => vec![1],
            Status::OutOfGas => vec![2],
            Status::WriteProtection => vec![3],
        }
    }
}
