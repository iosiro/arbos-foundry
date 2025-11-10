use revm::{
    interpreter::{Gas, InstructionResult, InterpreterResult},
    primitives::Bytes,
};

pub (crate) const OUT_OF_GAS_MESSAGE: &[u8] = b"Out of gas";

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
            return Ok(Some(revm::interpreter::InterpreterResult {
                result: revm::interpreter::InstructionResult::OutOfGas,
                gas: $gas,
                output: revm::primitives::Bytes::from_static(
                    crate::precompiles::macros::OUT_OF_GAS_MESSAGE,
                ),
            }));
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

macro_rules! emit_event {
    ($context:expr, $log:expr, $gas:expr) => {
        let log_cost = revm::interpreter::gas::log_cost(
            $log.data.topics().len() as u8,
            $log.data.data.len() as u64,
        );
        if let Some(log_cost) = log_cost {
            gas!($gas, log_cost)
        } else {
            $gas.spend_all();
            return Ok(Some(revm::interpreter::InterpreterResult {
                result: revm::interpreter::InstructionResult::OutOfGas,
                gas: $gas,
                output: revm::primitives::Bytes::from_static(
                    crate::precompiles::macros::OUT_OF_GAS_MESSAGE,
                ),
            }));
        }

        $context.journal_mut().log($log);
    };
}

pub(crate) use emit_event;
