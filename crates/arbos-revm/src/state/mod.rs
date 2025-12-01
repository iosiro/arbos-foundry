pub mod address_table;
pub mod arbos_state;
pub mod block_hashes;
pub mod l1_pricing;
pub mod l2_pricing;
pub mod program;
pub mod retryable;
pub mod types;
pub use arbos_state::{ArbState, ArbStateGetter, ArbStateWrapper, ArbosStateParams};
pub use types::ArbosStateError;

macro_rules! try_state {
    ($gas:expr, $expr:expr) => {{
        match $expr {
            Ok(value) => value,
            Err(crate::state::ArbosStateError::OutOfGas) => {
                return Some(crate::macros::interpreter_result_revert_out_of_gas(&mut $gas));
            }
            Err(err) => {
                return Some(crate::macros::interpreter_result_revert_with_output(
                    &mut $gas,
                    err.into(),
                ));
            }
        }
    }};
}

pub(crate) use try_state;
