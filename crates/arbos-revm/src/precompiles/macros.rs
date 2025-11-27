use revm::{
    interpreter::{Gas, InstructionResult, InterpreterResult},
    primitives::Bytes,
};

pub(crate) const OUT_OF_GAS_MESSAGE: &[u8] = b"out of gas";

pub(crate) fn success_result(gas: &mut Gas) -> InterpreterResult {
    success_result_with_output(gas, Bytes::default())
}

pub(crate) fn success_result_with_output(gas: &mut Gas, output: Bytes) -> InterpreterResult {
    InterpreterResult { result: InstructionResult::Return, gas: *gas, output }
}

pub(crate) fn revert_result(gas: &mut Gas) -> InterpreterResult {
    revert_result_with_output(gas, Bytes::default())
}

pub(crate) fn revert_result_with_output(gas: &mut Gas, output: Bytes) -> InterpreterResult {
    InterpreterResult { result: InstructionResult::Revert, gas: *gas, output }
}

pub(crate) fn burnout_return_result() -> InterpreterResult {
    InterpreterResult {
        result: InstructionResult::Revert,
        gas: Gas::new(0),
        output: Bytes::default(),
    }
}

pub(crate) fn out_of_gas_result(gas: &mut Gas) -> InterpreterResult {
    gas.spend_all();
    InterpreterResult {
        result: InstructionResult::OutOfGas,
        gas: *gas,
        output: Bytes::from_static(OUT_OF_GAS_MESSAGE),
    }
}

pub(crate) fn record_cost_return(gas: &mut Gas, cost: u64) -> Option<InterpreterResult> {
    if !gas.record_cost(cost) { Some(out_of_gas_result(gas)) } else { None }
}

#[macro_export]
macro_rules! record_cost {
    ($gas:expr, $cost:expr) => {{
        if let Some(result) = crate::precompiles::macros::record_cost_return(&mut $gas, $cost) {
            return Ok(Some(result));
        }
    }};
}

pub(crate) use record_cost;

macro_rules! return_success {
    ($gas:expr, $output:expr) => {
        return Ok(Some(crate::precompiles::macros::success_result_with_output(
            &mut $gas,
            $output.into(),
        )))
    };
    ($gas:expr) => {
        return Ok(Some(crate::precompiles::macros::success_result(&mut $gas)))
    };
}
pub(crate) use return_success;

macro_rules! burnout_return {
    () => {
        return Ok(Some(crate::precompiles::macros::burnout_return_result()))
    };
}

pub(crate) use burnout_return;

macro_rules! return_revert {
    ($gas:expr, $output:expr) => {
        return Ok(Some(crate::precompiles::macros::revert_result_with_output(
            &mut $gas,
            $output.into(),
        )))
    };
    ($gas:expr) => {
        return Ok(Some(crate::precompiles::macros::revert_result(&mut $gas)))
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
            crate::precompiles::macros::record_cost!(&mut $gas, log_cost)
        } else {
            return Ok(Some(crate::precompiles::macros::out_of_gas_result(&mut $gas)));
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
                return Ok(Some(crate::precompiles::macros::out_of_gas_result(&mut $gas)));
            }
            Err(err) => {
                return Ok(Some(crate::precompiles::macros::revert_result_with_output(
                    &mut $gas,
                    err.into(),
                )));
            }
        }
    }};
}

pub(crate) use try_state;

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
            <$logic>::run(
                context,
                input,
                target_address,
                caller_address,
                call_value,
                is_static,
                gas_limit,
            )
        }
    };
}
