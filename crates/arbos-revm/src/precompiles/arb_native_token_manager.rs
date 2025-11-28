use core::panic;

use crate::{
    ArbitrumContextTr,
    generate_state_mut_table,
    macros::{interpreter_return, interpreter_revert},
    precompile_impl,
    precompiles::{
        ArbPrecompileLogic, ExtendedPrecompile, StateMutability, decode_call, selector_or_revert,
    },
    state::{ArbState, ArbStateGetter, ArbosStateError, try_state}, try_record_cost,
};

use alloy_sol_types::{SolCall, sol};
use revm::{
    context::JournalTr,
    interpreter::{Gas, InterpreterResult},
    precompile::PrecompileId,
    primitives::{Address, Bytes, U256, address},
};

sol! {
///
/// @title Enables minting and burning native tokens.
/// @notice Authorized callers are added/removed through ArbOwner precompile.
/// Precompiled contract that exists in every Arbitrum chain at 0x0000000000000000000000000000000000000073.
/// Available in ArbOS version 41 and above
///
interface ArbNativeTokenManager {
    ///
    /// @notice Emitted when some amount of the native gas token is minted to a NativeTokenOwner.
    ///
    event NativeTokenMinted(address indexed to, uint256 amount);

    ///
    /// @notice Emitted when some amount of the native gas token is burned from a NativeTokenOwner.
    ///
    event NativeTokenBurned(address indexed from, uint256 amount);

    ///
    /// @notice In case the caller is authorized,
    /// mints some amount of the native gas token for this chain to the caller.
    ///
    function mintNativeToken(
        uint256 amount
    ) external;

    ///
    /// @notice In case the caller is authorized,
    /// burns some amount of the native gas token for this chain from the caller.
    ///
    function burnNativeToken(
        uint256 amount
    ) external;
}

}

pub fn arb_native_token_manager_precompile<CTX: ArbitrumContextTr>() -> ExtendedPrecompile<CTX> {
    ExtendedPrecompile::new(
        PrecompileId::Custom(std::borrow::Cow::Borrowed("ArbNativeTokenManager")),
        address!("0x0000000000000000000000000000000000000073"),
        precompile_impl!(ArbNativeTokenManagerPrecompile),
    )
}

const MINT_BURN_GAS_COST: u64 =
    revm::interpreter::gas::WARM_STORAGE_READ_COST + revm::interpreter::gas::CALLVALUE;

struct ArbNativeTokenManagerPrecompile;

impl<CTX: ArbitrumContextTr> ArbPrecompileLogic<CTX> for ArbNativeTokenManagerPrecompile {
    const STATE_MUT_TABLE: &'static [([u8; 4], StateMutability)] = generate_state_mut_table! {
        ArbNativeTokenManager => {
            mintNativeTokenCall(NonPayable),
            burnNativeTokenCall(NonPayable),
        }
    };

    fn inner(
        context: &mut CTX,
        input: &[u8],
        target_address: &Address,
        caller_address: Address,
        call_value: U256,
        is_static: bool,
        gas_limit: u64,
    ) -> Option<InterpreterResult> {
        let mut gas = Gas::new(gas_limit);
        let selector = selector_or_revert!(gas, input);

        match selector {
            ArbNativeTokenManager::mintNativeTokenCall::SELECTOR => {
                if !try_state!(gas, has_access(context, &mut gas, caller_address)) {
                    interpreter_revert!(gas);
                }

                try_record_cost!(gas, MINT_BURN_GAS_COST);

                let call = decode_call!(gas, ArbNativeTokenManager::mintNativeTokenCall, input);
                context
                    .journal_mut()
                    .balance_incr(caller_address, call.amount)
                    .expect("Failed to mint native token");

                let output = ArbNativeTokenManager::mintNativeTokenCall::abi_encode_returns(
                    &ArbNativeTokenManager::mintNativeTokenReturn {},
                );

                interpreter_return!(gas, Bytes::from(output));
            }
            ArbNativeTokenManager::burnNativeTokenCall::SELECTOR => {
                if !try_state!(gas, has_access(context, &mut gas, caller_address)) {
                    interpreter_revert!(gas);
                }

                try_record_cost!(gas, MINT_BURN_GAS_COST);

                let call = decode_call!(gas, ArbNativeTokenManager::burnNativeTokenCall, input);
                let balance = context.balance(caller_address).unwrap_or_default().data;

                if balance.checked_sub(call.amount).is_none() {
                    interpreter_revert!(gas, Bytes::from("burn amount exceeds balance"));
                };

                match context.journal_mut().transfer(caller_address, *target_address, call.amount) {
                    Ok(None) => {
                        let output = ArbNativeTokenManager::burnNativeTokenCall::abi_encode_returns(
                            &ArbNativeTokenManager::burnNativeTokenReturn {},
                        );
                        interpreter_return!(gas, Bytes::from(output));
                    }
                    Ok(Some(err)) => {
                        Some(InterpreterResult { result: err.into(), gas, output: Bytes::default() })
                    }
                    Err(e) => panic!("Failed to burn native token: {}", e),
                }
            }
            _ => interpreter_revert!(gas, Bytes::from("Unknown function selector")),
        }
    }
}


fn has_access<CTX: ArbitrumContextTr>(
    context: &mut CTX,
    gas: &mut Gas,
    caller: Address,
) -> Result<bool, ArbosStateError> {
    context.arb_state(Some(gas)).is_native_token_owner(caller)
}
