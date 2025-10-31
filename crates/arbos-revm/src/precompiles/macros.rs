use revm::{
    interpreter::{Gas, InstructionResult, InterpreterResult},
    primitives::Bytes,
};

const OUT_OF_GAS_MESSAGE: &[u8] = b"Out of gas";

pub(crate) fn out_of_gas(gas: Gas) -> InterpreterResult {
    out_of_gas_with_output(gas, Bytes::from_static(OUT_OF_GAS_MESSAGE))
}
pub(crate) fn out_of_gas_with_output(gas: Gas, output: Bytes) -> InterpreterResult {
    InterpreterResult { result: InstructionResult::OutOfGas, gas, output }
}

pub(crate) fn success_result(gas: Gas) -> InterpreterResult {
    success_result_with_output(gas, Bytes::default())
}

pub(crate) fn success_result_with_output(gas: Gas, output: Bytes) -> InterpreterResult {
    InterpreterResult { result: InstructionResult::Return, gas, output }
}

pub(crate) fn revert_result(gas: Gas) -> InterpreterResult {
    revert_result_with_output(gas, Bytes::default())
}

pub(crate) fn revert_result_with_output(gas: Gas, output: Bytes) -> InterpreterResult {
    InterpreterResult { result: InstructionResult::Revert, gas, output }
}

macro_rules! gas {
    ($gas:expr, $cost:expr) => {{
        if !$gas.record_cost($cost) {
            $gas.spend_all();
            return Ok(Some(crate::precompiles::macros::out_of_gas($gas)));
        }
    }};
}
pub(crate) use gas;

macro_rules! return_success {
    ($gas:expr, $output:expr) => {
        return Ok(Some(crate::precompiles::macros::success_result_with_output(
            $gas,
            $output.into(),
        )))
    };
    ($gas:expr) => {
        return Ok(Some(crate::precompiles::macros::success_result($gas)))
    };
}
pub(crate) use return_success;

macro_rules! return_revert {
    ($gas:expr, $output:expr) => {
        return Ok(Some(crate::precompiles::macros::revert_result_with_output($gas, $output.into())))
    };
    ($gas:expr) => {
        return Ok(Some(crate::precompiles::macros::revert_result($gas)))
    };
}

pub(crate) use return_revert;
