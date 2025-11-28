use crate::{
    ArbitrumContextTr, generate_state_mut_table,
    macros::{interpreter_return, interpreter_revert},
    precompile_impl,
    precompiles::{
        ArbPrecompileLogic, ExtendedPrecompile, decode_call,
        StateMutability, selector_or_revert,
    },
    state::{ArbState, ArbStateGetter, try_state}, try_record_cost,
};
use alloy_sol_types::{SolCall, SolError, sol};
use revm::{
    interpreter::{Gas, InstructionResult, InterpreterResult},
    precompile::PrecompileId,
    primitives::{Address, B256, Bytes, Log, U256, address, alloy_primitives::IntoLogData},
};

sol! {
///
/// @title A test contract whose methods are only accessible in debug mode
/// @notice Precompiled contract that exists in every Arbitrum chain at 0x00000000000000000000000000000000000000ff.
///
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
        precompile_impl!(ArbDebugPrecompile),
    )
}

struct ArbDebugPrecompile;

impl<CTX: ArbitrumContextTr> ArbPrecompileLogic<CTX> for ArbDebugPrecompile {
    const STATE_MUT_TABLE: &'static [([u8; 4], StateMutability)] = generate_state_mut_table! {
        ArbDebug => {
            becomeChainOwnerCall(NonPayable),
            eventsCall(Payable),
            eventsViewCall(View),
            customRevertCall(Pure),
            panicCall(NonPayable),
            legacyErrorCall(Pure),
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
            ArbDebug::becomeChainOwnerCall::SELECTOR => {
                let _ = decode_call!(gas, ArbDebug::becomeChainOwnerCall, input);

                try_state!(
                    gas,
                    context.arb_state(Some(&mut gas)).chain_owners().add(caller_address)
                );

                interpreter_return!(gas);
            }
            ArbDebug::eventsCall::SELECTOR => {
                let call = decode_call!(gas, ArbDebug::eventsCall, input);

                // TODO handle inspector mode

                // Emit events based on the args
                events(
                    context,
                    caller_address,
                    is_static,
                    gas_limit,
                    call.flag,
                    B256::from(call.value),
                );

                interpreter_return!(
                    gas,
                    ArbDebug::eventsCall::abi_encode_returns(&ArbDebug::eventsReturn::from((
                        address!("0x00000000000000000000000000000000000000ff"),
                        U256::from(gas_limit),
                    )))
                );
            }
            ArbDebug::eventsViewCall::SELECTOR => {
                let _ = decode_call!(gas, ArbDebug::eventsViewCall, input);

                events(context, caller_address, is_static, gas_limit, true, B256::ZERO)
            }
            ArbDebug::legacyErrorCall::SELECTOR => {
                let _ = decode_call!(gas, ArbDebug::legacyErrorCall, input);

                interpreter_revert!(gas, Bytes::from("example legacy error"));
            }
            ArbDebug::panicCall::SELECTOR => {
                let _ = decode_call!(gas, ArbDebug::panicCall, input);

                panic!("called ArbDebug's debug-only Panic method");
            }
            ArbDebug::customRevertCall::SELECTOR => {
                let call = decode_call!(gas, ArbDebug::customRevertCall, input);

                let error =
                    ArbDebug::Custom::new((call.number, "example custom revert".to_string(), true));

                interpreter_revert!(gas, error.abi_encode());
            }
            _ => interpreter_revert!(gas, Bytes::from("Unknown function selector")),
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
) -> Option<InterpreterResult> {
    let mut gas = Gas::new(gas_limit);

    if is_static {
        return Some(InterpreterResult {
            result: InstructionResult::StateChangeDuringStaticCall,
            gas,
            output: Bytes::default(),
        });
    }

    let log_data = ArbDebug::Basic { flag, value }.to_log_data();
    if let Some(gas_cost) =
        revm::interpreter::gas::log_cost(log_data.topics().len() as u8, log_data.data.len() as u64)
    {
        let mut gas = &mut gas;
        try_record_cost!(gas, gas_cost);
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
        revm::interpreter::gas::log_cost(log_data.topics().len() as u8, log_data.data.len() as u64)
    {
        try_record_cost!(gas, gas_cost);
    }

    context.log(
        Log::new(
            address!("0x00000000000000000000000000000000000000ff"),
            log_data.topics().into(),
            log_data.data,
        )
        .unwrap(),
    );

    interpreter_return!(gas);
}
