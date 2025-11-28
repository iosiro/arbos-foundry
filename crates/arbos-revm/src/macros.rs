use revm::{
    interpreter::{Gas, InstructionResult, InterpreterResult},
    primitives::Bytes,
};

pub(crate) const OUT_OF_GAS_MESSAGE: &[u8] = b"out of gas";

pub(crate) fn interpreter_result_return(gas: &mut Gas) -> InterpreterResult {
    interpreter_result_return_with_output(gas, Bytes::default())
}

pub(crate) fn interpreter_result_return_with_output(
    gas: &mut Gas,
    output: Bytes,
) -> InterpreterResult {
    InterpreterResult { result: InstructionResult::Return, gas: *gas, output }
}

pub(crate) fn interpreter_result_revert(gas: &mut Gas) -> InterpreterResult {
    interpreter_result_revert_with_output(gas, Bytes::default())
}

pub(crate) fn interpreter_result_revert_with_output(
    gas: &mut Gas,
    output: Bytes,
) -> InterpreterResult {
    InterpreterResult { result: InstructionResult::Revert, gas: *gas, output }
}

pub(crate) fn interpreter_result_revert_out_of_gas(gas: &mut Gas) -> InterpreterResult {
    gas.spend_all();
    InterpreterResult {
        result: InstructionResult::OutOfGas,
        gas: *gas,
        output: Bytes::from_static(OUT_OF_GAS_MESSAGE),
    }
}

pub(crate) fn record_cost_return(gas: &mut Gas, cost: u64) -> Option<InterpreterResult> {
    if !gas.record_cost(cost) { Some(interpreter_result_revert_out_of_gas(gas)) } else { None }
}

#[macro_export]
macro_rules! try_record_cost {
    ($gas:expr, $cost:expr) => {{
        if let Some(result) = crate::macros::record_cost_return(&mut $gas, $cost) {
            return Some(result);
        }
    }};
}

pub(crate) use try_record_cost;

macro_rules! interpreter_return {
    ($gas:expr, $output:expr) => {
        return Some(crate::macros::interpreter_result_return_with_output(&mut $gas, $output.into()))
    };
    ($gas:expr) => {
        return Some(crate::macros::interpreter_result_return(&mut $gas))
    };
}
pub(crate) use interpreter_return;

macro_rules! try_or_halt {
    ($gas:expr, $expr:expr) => {{
        match $expr {
            Ok(value) => value,
            Err(_) => {
                return Some(crate::macros::interpreter_result_revert_out_of_gas(&mut $gas));
            }
        }
    }};
}

pub(crate) use try_or_halt;

macro_rules! interpreter_revert {
    ($gas:expr, $output:expr) => {
        return Some(crate::macros::interpreter_result_revert_with_output(&mut $gas, $output.into()))
    };
    ($gas:expr) => {
        return Some(crate::macros::interpreter_result_revert(&mut $gas))
    };
}

pub(crate) use interpreter_revert;

macro_rules! emit_event {
    ($context:expr, $log:expr, $gas:expr) => {
        let log_cost = revm::interpreter::gas::log_cost(
            $log.data.topics().len() as u8,
            $log.data.data.len() as u64,
        );
        if let Some(log_cost) = log_cost {
            crate::macros::try_record_cost!(&mut $gas, log_cost)
        } else {
            return Some(crate::macros::interpreter_result_revert_out_of_gas(&mut $gas));
        }

        $context.journal_mut().log($log);
    };
}

pub(crate) use emit_event;
