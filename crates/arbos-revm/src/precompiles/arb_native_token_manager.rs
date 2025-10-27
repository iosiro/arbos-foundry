use alloy_sol_types::{sol, SolCall, SolError};
use revm::{context::JournalTr, interpreter::{gas, Gas, InstructionResult, InterpreterResult}, precompile::PrecompileId, primitives::{address, Address, Bytes, U256}};
use wasmer_types::compilation::target;

use crate::{precompiles::extension::ExtendedPrecompile, ArbitrumContextTr};

sol!{
/**
 * @title Enables minting and burning native tokens.
 * @notice Authorized callers are added/removed through ArbOwner precompile.
 *         Precompiled contract that exists in every Arbitrum chain at 0x0000000000000000000000000000000000000073.
 *         Available in ArbOS version 41 and above
 */
interface ArbNativeTokenManager {
    /**
     * @notice Emitted when some amount of the native gas token is minted to a NativeTokenOwner.
     */
    event NativeTokenMinted(address indexed to, uint256 amount);

    /**
     * @notice Emitted when some amount of the native gas token is burned from a NativeTokenOwner.
     */
    event NativeTokenBurned(address indexed from, uint256 amount);

    /**
     * @notice In case the caller is authorized,
     * mints some amount of the native gas token for this chain to the caller.
     */
    function mintNativeToken(
        uint256 amount
    ) external;

    /**
     * @notice In case the caller is authorized,
     * burns some amount of the native gas token for this chain from the caller.
     */
    function burnNativeToken(
        uint256 amount
    ) external;
}

}

pub fn arb_native_token_manager_precompile<CTX: ArbitrumContextTr>() -> ExtendedPrecompile<CTX> {
    ExtendedPrecompile::new(
        PrecompileId::Custom(std::borrow::Cow::Borrowed("ArbNativeTokenManager")),
        address!("0x0000000000000000000000000000000000000073"),
        arb_native_token_manager_run::<CTX>,
    )
}

const MINT_BURN_GAS_COST: u64 = gas::WARM_STORAGE_READ_COST + gas::CALLVALUE;

/// Run the precompile with the given context and input data.
/// Run the arb_native_token_manager precompile with the given context and input data.
fn arb_native_token_manager_run<CTX: ArbitrumContextTr>(
    context: &mut CTX,
    input: &[u8],
    target_address: &Address,
    caller_address: Address,
    _call_value: U256,
    _is_static: bool,
    gas_limit: u64,
) -> Result<Option<InterpreterResult>, String> {

    // decode selector
    if input.len() < 4 {
        return Ok(Some(InterpreterResult {
            result: InstructionResult::Revert,
            gas: Gas::new(gas_limit),
            output: Bytes::from("Input too short"),
        }));
    }

    let mut gas = Gas::new(gas_limit);

    // decode selector
    let selector: [u8; 4] = input[0..4].try_into().unwrap();

    match selector {
        ArbNativeTokenManager::mintNativeTokenCall::SELECTOR => {
            if !has_access(context, caller_address) {
                return Ok(Some(InterpreterResult {
                    result: InstructionResult::Revert,
                    gas: Gas::new(gas_limit),
                    output: Bytes::default(),
                }));
            }

            if !gas.record_cost(MINT_BURN_GAS_COST) {
                return Ok(Some(InterpreterResult {
                    result: InstructionResult::OutOfGas,
                    gas: Gas::new(gas_limit),
                    output: Bytes::from("Out of gas"),
                }));
            }

            let call = ArbNativeTokenManager::mintNativeTokenCall::abi_decode(&input).unwrap();
            context.journal_mut().balance_incr(caller_address, call.amount).into()
        },
        ArbNativeTokenManager::burnNativeTokenCall::SELECTOR => {
            if !has_access(context, caller_address) {
                return Ok(Some(InterpreterResult {
                    result: InstructionResult::Revert,
                    gas: Gas::new(gas_limit),
                    output: Bytes::default(),
                }));
            }

             if !gas.record_cost(MINT_BURN_GAS_COST) {
                return Ok(Some(InterpreterResult {
                    result: InstructionResult::OutOfGas,
                    gas: Gas::new(gas_limit),
                    output: Bytes::from("Out of gas"),
                }));
            }

            let call = ArbNativeTokenManager::burnNativeTokenCall::abi_decode(&input).unwrap();
            let balance = context.balance(caller_address).unwrap_or_default().data;

            let balance = if Ok(balance) = balance.checked_sub(call.amount) {
                balance
            } else {
                return Ok(Some(InterpreterResult {
                    result: InstructionResult::Revert,
                    gas: Gas::new(gas_limit),
                    output: Bytes::from("burn amount exceeds balance"),
                }));
            };
        

            context.journal_mut().transfer(caller_address, target_address, call.amount).into()
        },
        _ => {
            return Ok(Some(InterpreterResult {
                result: InstructionResult::Revert,
                gas: Gas::new(gas_limit),
                output: Bytes::from("Unknown function selector"),
            }));
        }
    }
}

fn has_access<CTX: ArbitrumContextTr>(
    context: &mut CTX,
    caller: Address,
) -> bool {
    context
        .arb_state_mut()
        .native_token_owners_mut()
        .contains(&caller)
}