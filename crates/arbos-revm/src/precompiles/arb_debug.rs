use alloy_sol_types::{SolCall, SolError, sol};
use revm::{
    interpreter::{Gas, InstructionResult, InterpreterResult},
    precompile::PrecompileId,
    primitives::{Address, B256, Bytes, Log, U256, address, alloy_primitives::IntoLogData},
};

use crate::{
    ArbitrumContextTr,
    precompiles::extension::ExtendedPrecompile,
    state::{ArbState, ArbStateGetter},
};

sol! {
/**
 * @title A test contract whose methods are only accessible in debug mode
 * @notice Precompiled contract that exists in every Arbitrum chain at 0x00000000000000000000000000000000000000ff.
 */
interface ArbDebug {
    /// @notice Caller becomes a chain owner
    function becomeChainOwner() external;

    /// @notice Emit events with values based on the args provided
    function events(bool flag, bytes32 value) external payable returns (address, uint256);

    /// @notice Tries (and fails) to emit logs in a view context
    function eventsView() external view;

    // Events that exist for testing log creation and pricing
    event Basic(bool flag, bytes32 indexed value);
    event Mixed(
        bool indexed flag, bool not, bytes32 indexed value, address conn, address indexed caller
    );
    event Store(
        bool indexed flag, address indexed field, uint24 number, bytes32 value, bytes store
    );

    function customRevert(
        uint64 number
    ) external pure;

    function panic() external;

    function legacyError() external pure;

    error Custom(uint64, string, bool);
    error Unused();
}


}

pub fn arb_debug_precompile<CTX: ArbitrumContextTr>() -> ExtendedPrecompile<CTX> {
    ExtendedPrecompile::new(
        PrecompileId::Custom(std::borrow::Cow::Borrowed("ArbDebug")),
        address!("0x00000000000000000000000000000000000000ff"),
        arb_debug_run::<CTX>,
    )
}
/// Run the precompile with the given context and input data.
/// Run the arb_debug precompile with the given context and input data.
fn arb_debug_run<CTX: ArbitrumContextTr>(
    context: &mut CTX,
    input: &[u8],
    _target_address: &Address,
    caller_address: Address,
    _call_value: U256,
    is_static: bool,
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

    // decode selector
    let selector: [u8; 4] = input[0..4].try_into().unwrap();

    match selector {
        ArbDebug::becomeChainOwnerCall::SELECTOR => {
            let _ = ArbDebug::becomeChainOwnerCall::abi_decode(input).unwrap();

            context.arb_state().chain_owners().add(&caller_address);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::new(),
            }))
        }
        ArbDebug::eventsCall::SELECTOR => {
            let call = ArbDebug::eventsCall::abi_decode(input).unwrap();

            // TODO handle inspector mode

            // Emit events based on the args
            events(
                context,
                caller_address,
                is_static,
                gas_limit,
                call.flag,
                B256::from(call.value),
            )?;

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: ArbDebug::eventsCall::abi_encode_returns(&ArbDebug::eventsReturn {
                    _0: address!("0x00000000000000000000000000000000000000ff"),
                    _1: U256::from(gas_limit),
                })
                .into(),
            }))
        }
        ArbDebug::eventsViewCall::SELECTOR => {
            let _ = ArbDebug::eventsViewCall::abi_decode(input).unwrap();

            events(context, caller_address, is_static, gas_limit, true, B256::ZERO)
        }
        ArbDebug::legacyErrorCall::SELECTOR => {
            let _ = ArbDebug::legacyErrorCall::abi_decode(input).unwrap();

            Ok(Some(InterpreterResult {
                result: InstructionResult::Revert,
                gas: Gas::new(gas_limit),
                output: Bytes::from("example legacy error"),
            }))
        }
        ArbDebug::panicCall::SELECTOR => {
            let _ = ArbDebug::panicCall::abi_decode(input).unwrap();

            panic!("called ArbDebug's debug-only Panic method");
        }
        ArbDebug::customRevertCall::SELECTOR => {
            let call = ArbDebug::customRevertCall::abi_decode(input).unwrap();

            let error =
                ArbDebug::Custom::new((call.number, "example custom revert".to_string(), true));

            Ok(Some(InterpreterResult {
                result: InstructionResult::Revert,
                gas: Gas::new(gas_limit),
                output: Bytes::from(error.abi_encode()),
            }))
        }
        _ => {
            Ok(Some(InterpreterResult {
                result: InstructionResult::Revert,
                gas: Gas::new(gas_limit),
                output: Bytes::from("Unknown function selector"),
            }))
        }
    }
}

fn events<CTX: ArbitrumContextTr>(
    context: &mut CTX,
    caller_address: Address,
    is_static: bool,
    gas_limit: u64,
    flag: bool,
    value: B256,
) -> Result<Option<InterpreterResult>, String> {
    if is_static {
        return Ok(Some(InterpreterResult {
            result: InstructionResult::StateChangeDuringStaticCall,
            gas: Gas::new(gas_limit),
            output: Bytes::default(),
        }));
    }

    let mut gas = Gas::new(gas_limit);

    let log_data = ArbDebug::Basic { flag, value }.to_log_data();
    if let Some(gas_cost) =
        revm::interpreter::gas::log_cost(log_data.topics().len() as u8, log_data.data.len() as u64) &&  !gas.record_cost(gas_cost) {
            return Ok(Some(InterpreterResult {
                result: InstructionResult::OutOfGas,
                gas: Gas::new(gas_limit),
                output: Bytes::from("Out of gas"),
            }));
        }
    

    context.log(
        Log::new(
            address!("0x00000000000000000000000000000000000000ff"),
            log_data.topics().into(),
            log_data.data,
        )
        .unwrap(),
    );

    let log_data = ArbDebug::Mixed {
        flag,
        not: !flag,
        caller: caller_address,
        conn: address!("0x00000000000000000000000000000000000000ff"),
        value,
    }
    .to_log_data();

    if let Some(gas_cost) =
        revm::interpreter::gas::log_cost(log_data.topics().len() as u8, log_data.data.len() as u64) && 
        !gas.record_cost(gas_cost) {
            return Ok(Some(InterpreterResult {
                result: InstructionResult::OutOfGas,
                gas: Gas::new(gas_limit),
                output: Bytes::from("Out of gas"),
            }));        
    }

    context.log(
        Log::new(
            address!("0x00000000000000000000000000000000000000ff"),
            log_data.topics().into(),
            log_data.data,
        )
        .unwrap(),
    );

    Ok(Some(InterpreterResult {
        result: InstructionResult::Return,
        gas: Gas::new(gas_limit),
        output: Bytes::default(),
    }))
}
