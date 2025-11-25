use revm::{
    interpreter::{Gas, InstructionResult, InterpreterResult},
    primitives::Bytes,
};

pub(crate) const OUT_OF_GAS_MESSAGE: &[u8] = b"Out of gas";

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

pub(crate) fn out_of_gas_result(mut gas: Gas) -> InterpreterResult {
    gas.spend_all();
    InterpreterResult {
        result: InstructionResult::OutOfGas,
        gas,
        output: Bytes::from_static(OUT_OF_GAS_MESSAGE),
    }
}

macro_rules! gas {
    ($gas:expr, $cost:expr) => {{
        if !$gas.record_cost($cost) {
            return Ok(Some(crate::precompiles::macros::out_of_gas_result($gas)));
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
            return Ok(Some(crate::precompiles::macros::out_of_gas_result($gas)));
        }

        $context.journal_mut().log($log);
    };
}

pub(crate) use emit_event;

macro_rules! try_state {
    ($gas:expr, $expr:expr) => {{
        match $expr {
            Ok(value) => value,
            Err(crate::state::types::ArbosStateError::OutOfGas) => {
                return Ok(Some(crate::precompiles::macros::out_of_gas_result($gas)));
            }
            Err(err) => return Err(err.into()),
        }
    }};
}

pub(crate) use try_state;
