use alloy_sol_types::{SolCall, sol};
use revm::{
    interpreter::{Gas, InterpreterResult},
    precompile::PrecompileId,
    primitives::{Address, Bytes, U256, address},
};

use crate::{
    ArbitrumContextTr, generate_state_mut_table,
    macros::{interpreter_return, interpreter_revert},
    precompile_impl,
    precompiles::{ArbPrecompileLogic, ExtendedPrecompile, StateMutability, selector_or_revert},
};

sol! {

/// @title Deprecated - Info about the rollup just prior to the Nitro upgrade
/// @notice Precompiled contract in every Arbitrum chain for retryable transaction related data retrieval and interactions. Exists at 0x000000000000000000000000000000000000006f
interface ArbStatistics {
    /// @notice Get Arbitrum block number and other statistics as they were right before the Nitro upgrade.
    /// @return (
    ///      Number of accounts,
    ///      Total storage allocated (includes storage that was later deallocated),
    ///      Total ArbGas used,
    ///      Number of transaction receipt issued,
    ///      Number of contracts created,
    ///    )
    function getStats()
        external
        view
        returns (
            uint256,
            uint256,
            uint256,
            uint256,
            uint256,
            uint256
        );
}

}

pub fn arb_statistics_precompile<CTX: ArbitrumContextTr>() -> ExtendedPrecompile<CTX> {
    ExtendedPrecompile::new(
        PrecompileId::Custom(std::borrow::Cow::Borrowed("ArbStatistics")),
        address!("0x000000000000000000000000000000000000006f"),
        precompile_impl!(ArbStatisticsPrecompile),
    )
}

struct ArbStatisticsPrecompile;

impl<CTX: ArbitrumContextTr> ArbPrecompileLogic<CTX> for ArbStatisticsPrecompile {
    const STATE_MUT_TABLE: &'static [([u8; 4], StateMutability)] = generate_state_mut_table! {
        ArbStatistics => {
            getStatsCall(View),
        }
    };

    fn inner(
        context: &mut CTX,
        input: &[u8],
        _target_address: &Address,
        _caller_address: Address,
        _call_value: U256,
        _is_static: bool,
        gas_limit: u64,
    ) -> Option<InterpreterResult> {
        let mut gas = Gas::new(gas_limit);

        let selector = selector_or_revert!(gas, input);

        match selector {
            ArbStatistics::getStatsCall::SELECTOR => {
                let output = ArbStatistics::getStatsCall::abi_encode_returns(
                    &ArbStatistics::getStatsReturn::from((
                        context.block_number(),
                        U256::ZERO,
                        U256::ZERO,
                        U256::ZERO,
                        U256::ZERO,
                        U256::ZERO,
                    )),
                );

                interpreter_return!(gas, Bytes::from(output));
            }
            _ => interpreter_revert!(gas, Bytes::from("Unknown function selector")),
        }
    }
}
