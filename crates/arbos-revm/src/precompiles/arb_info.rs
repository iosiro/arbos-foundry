use alloy_sol_types::{SolCall, sol};
use revm::{
    interpreter::{Gas, InterpreterResult},
    precompile::PrecompileId,
    primitives::{Address, Bytes, U256, address},
};

use crate::{
    ArbitrumContextTr,
    precompiles::{
        ExtendedPrecompile,
        macros::{return_revert, return_success},
    },
};

sol! {
/// @title Lookup for basic info about accounts and contracts.
/// @notice Precompiled contract that exists in every Arbitrum chain at 0x0000000000000000000000000000000000000065.
interface ArbInfo {
    /// @notice Retrieves an account's balance
    function getBalance(address account) external view returns (uint256);

    /// @notice Retrieves a contract's deployed code
    function getCode(address account) external view returns (bytes memory);
}

}

pub fn arb_info_precompile<CTX: ArbitrumContextTr>() -> ExtendedPrecompile<CTX> {
    ExtendedPrecompile::new(
        PrecompileId::Custom(std::borrow::Cow::Borrowed("ArbInfo")),
        address!("0x0000000000000000000000000000000000000065"),
        arb_info_run::<CTX>,
    )
}
/// Run the precompile with the given context and input data.
/// Run the arb_info precompile with the given context and input data.
fn arb_info_run<CTX: ArbitrumContextTr>(
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
        ArbInfo::getBalanceCall::SELECTOR => {
            let call = ArbInfo::getBalanceCall::abi_decode(input).unwrap();

            let balance = context.balance(call.account).unwrap_or_default().data;

            let output = ArbInfo::getBalanceCall::abi_encode_returns(&balance);

            return_success!(gas, Bytes::from(output));
        }
        ArbInfo::getCodeCall::SELECTOR => {
            let call = ArbInfo::getCodeCall::abi_decode(input).unwrap();

            let code = context.load_account_code(call.account).unwrap_or_default().data;

            let output = ArbInfo::getCodeCall::abi_encode_returns(&code);

            return_success!(gas, Bytes::from(output));
        }
        _ => return_revert!(gas, Bytes::from("Unknown function selector")),
    }
}
