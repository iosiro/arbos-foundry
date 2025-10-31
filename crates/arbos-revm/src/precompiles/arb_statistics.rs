use alloy_sol_types::{SolCall, sol};
use revm::{
    interpreter::{Gas, InterpreterResult},
    precompile::PrecompileId,
    primitives::{Address, Bytes, U256, address},
};

use crate::{
    ArbitrumContextTr,
    precompiles::{
        extension::ExtendedPrecompile,
        macros::{return_revert, return_success},
    },
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
        arb_statistics_run::<CTX>,
    )
}
/// Run the precompile with the given context and input data.
/// Run the arb_statistics precompile with the given context and input data.
fn arb_statistics_run<CTX: ArbitrumContextTr>(
    context: &mut CTX,
    input: &[u8],
    _target_address: &Address,
    _caller_address: Address,
    _call_value: U256,
    _is_static: bool,
    gas_limit: u64,
) -> Result<Option<InterpreterResult>, String> {
    let gas = Gas::new(gas_limit);
    // decode selector
    if input.len() < 4 {
        return_revert!(gas, Bytes::from("Input too short"));
    }

    // decode selector
    let selector: [u8; 4] = input[0..4].try_into().unwrap();

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

            return_success!(gas, Bytes::from(output));
        }
        _ => return_revert!(gas, Bytes::from("Unknown function selector")),
    }
}
