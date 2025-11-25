use alloy_sol_types::{SolCall, SolError, sol};
use revm::{
    context::{Block, JournalTr},
    interpreter::{Gas, InterpreterResult},
    precompile::PrecompileId,
    primitives::{
        Address, B256, Bytes, Log, U256, address, alloy_primitives::IntoLogData, keccak256,
    },
};

use crate::{
    ArbitrumContextTr,
    config::{ArbitrumConfigTr, ArbitrumStylusConfigTr},
    precompiles::{
        ExtendedPrecompile,
        macros::{emit_event, gas, return_revert, return_success, try_state},
    },
    state::{ArbState, ArbStateGetter, types::StorageBackedTr},
};

const ARBOS_STATE_RETRYABLE_LIFETIME_SECONDS: u64 = 7 * 24 * 60 * 60; // 1 week
sol! {
/**
 * @title Methods for managing retryables.
 * @notice Precompiled contract in every Arbitrum chain for retryable transaction related data retrieval and interactions. Exists at 0x000000000000000000000000000000000000006e
 */
interface ArbRetryableTx {
    /**
     * @notice Schedule an attempt to redeem a redeemable tx, donating all of the call's gas to the redeem.
     * Revert if ticketId does not exist.
     * @param ticketId unique identifier of retryable message: keccak256(keccak256(ArbchainId, inbox-sequence-number), uint(0) )
     * @return txId that the redeem attempt will have
     */
    function redeem(
        bytes32 ticketId
    ) external returns (bytes32);

    /**
     * @notice Return the minimum lifetime of redeemable txn.
     * @return lifetime in seconds
     */
    function getLifetime() external view returns (uint256);

    /**
     * @notice Return the timestamp when ticketId will age out, reverting if it does not exist
     * @param ticketId unique ticket identifier
     * @return timestamp for ticket's deadline
     */
    function getTimeout(
        bytes32 ticketId
    ) external view returns (uint256);

    /**
     * @notice Adds one lifetime period to the life of ticketId.
     * Donate gas to pay for the lifetime extension.
     * If successful, emits LifetimeExtended event.
     * Revert if ticketId does not exist, or if the timeout of ticketId is already at least one lifetime period in the future.
     * @param ticketId unique ticket identifier
     * @return new timeout of ticketId
     */
    function keepalive(
        bytes32 ticketId
    ) external returns (uint256);

    /**
     * @notice Return the beneficiary of ticketId.
     * Revert if ticketId doesn't exist.
     * @param ticketId unique ticket identifier
     * @return address of beneficiary for ticket
     */
    function getBeneficiary(
        bytes32 ticketId
    ) external view returns (address);

    /**
     * @notice Cancel ticketId and refund its callvalue to its beneficiary.
     * Revert if ticketId doesn't exist, or if called by anyone other than ticketId's beneficiary.
     * @param ticketId unique ticket identifier
     */
    function cancel(
        bytes32 ticketId
    ) external;

    /**
     * @notice Gets the redeemer of the current retryable redeem attempt.
     * Returns the zero address if the current transaction is not a retryable redeem attempt.
     * If this is an auto-redeem, returns the fee refund address of the retryable.
     */
    function getCurrentRedeemer() external view returns (address);

    /**
     * @notice Do not call. This method represents a retryable submission to aid explorers.
     * Calling it will always revert.
     */
    function submitRetryable(
        bytes32 requestId,
        uint256 l1BaseFee,
        uint256 deposit,
        uint256 callvalue,
        uint256 gasFeeCap,
        uint64 gasLimit,
        uint256 maxSubmissionFee,
        address feeRefundAddress,
        address beneficiary,
        address retryTo,
        bytes calldata retryData
    ) external;

    event TicketCreated(bytes32 indexed ticketId);
    event LifetimeExtended(bytes32 indexed ticketId, uint256 newTimeout);
    event RedeemScheduled(
        bytes32 indexed ticketId,
        bytes32 indexed retryTxHash,
        uint64 indexed sequenceNum,
        uint64 donatedGas,
        address gasDonor,
        uint256 maxRefund,
        uint256 submissionFeeRefund
    );
    event Canceled(bytes32 indexed ticketId);

    /// @dev DEPRECATED in favour of new RedeemScheduled event after the nitro upgrade
    event Redeemed(bytes32 indexed userTxHash);

    error NoTicketWithID();
    error NotCallable();
}

}

pub fn arb_retryable_tx_precompile<CTX: ArbitrumContextTr>() -> ExtendedPrecompile<CTX> {
    ExtendedPrecompile::new(
        PrecompileId::Custom(std::borrow::Cow::Borrowed("ArbRetryableTx")),
        address!("0x000000000000000000000000000000000000006e"),
        arb_retryable_tx_run::<CTX>,
    )
}
/// Run the precompile with the given context and input data.
/// Run the arb_retryable_tx precompile with the given context and input data.
fn arb_retryable_tx_run<CTX: ArbitrumContextTr>(
    context: &mut CTX,
    input: &[u8],
    target_address: &Address,
    caller_address: Address,
    _call_value: U256,
    _is_static: bool,
    gas_limit: u64,
) -> Result<Option<InterpreterResult>, String> {
    let mut gas = Gas::new(gas_limit);

    // decode selector
    if input.len() < 4 {
        return_revert!(gas, Bytes::from("Input too short"));
    }

    // decode selector
    let selector: [u8; 4] = input[0..4].try_into().unwrap();

    match selector {
        ArbRetryableTx::cancelCall::SELECTOR => {
            let call = ArbRetryableTx::cancelCall::abi_decode(input).unwrap();

            let beneficiary = {
                let mut arb_state = context.arb_state(Some(&mut gas));
                try_state!(gas, arb_state.retryable(call.ticketId).beneficiary().get())
            };

            if caller_address != beneficiary {
                return_revert!(gas, Bytes::from("only the beneficiary may cancel a retryable"));
            }

            // move any funds in escrow to the beneficiary (should be none if the retry succeeded --
            // see EndTxHook)
            let escrow_address = { retryable_escrow_address(call.ticketId) };

            let escrow_balance = context.balance(escrow_address).unwrap_or_default().data;

            if !escrow_balance.is_zero()
                && let Some(error) = context
                    .journal_mut()
                    .transfer(escrow_address, beneficiary, escrow_balance)
                    .unwrap()
            {
                return Ok(Some(InterpreterResult {
                    result: error.into(),
                    gas,
                    output: Bytes::default(),
                }));
            }

            let mut arb_state = context.arb_state(Some(&mut gas));
            let mut retryable = arb_state.retryable(call.ticketId);
            try_state!(gas, retryable.clear());

            let output =
                ArbRetryableTx::cancelCall::abi_encode_returns(&ArbRetryableTx::cancelReturn {});

            return_success!(gas, Bytes::from(output));
        }
        ArbRetryableTx::getBeneficiaryCall::SELECTOR => {
            let call = ArbRetryableTx::getBeneficiaryCall::abi_decode(input).unwrap();

            let beneficiary = {
                let mut arb_state = context.arb_state(Some(&mut gas));
                try_state!(gas, arb_state.retryable(call.ticketId).beneficiary().get())
            };

            if beneficiary == Address::ZERO {
                if context.cfg().stylus().arbos_version() >= 3 {
                    let output = ArbRetryableTx::NoTicketWithID {}.abi_encode();

                    return_revert!(gas, Bytes::from(output));
                }

                return_revert!(gas, Bytes::from("ticketId not found"));
            }

            let output = ArbRetryableTx::getBeneficiaryCall::abi_encode_returns(&beneficiary);

            return_success!(gas, Bytes::from(output));
        }
        ArbRetryableTx::getCurrentRedeemerCall::SELECTOR => {
            let output = ArbRetryableTx::getCurrentRedeemerCall::abi_encode_returns(&Address::ZERO);

            return_success!(gas, Bytes::from(output));
        }
        ArbRetryableTx::getLifetimeCall::SELECTOR => {
            let output = ArbRetryableTx::getLifetimeCall::abi_encode_returns(&U256::from(
                ARBOS_STATE_RETRYABLE_LIFETIME_SECONDS,
            ));

            return_success!(gas, Bytes::from(output));
        }
        ArbRetryableTx::getTimeoutCall::SELECTOR => {
            let call = ArbRetryableTx::getTimeoutCall::abi_decode(input).unwrap();

            let timeout = {
                let mut arb_state = context.arb_state(Some(&mut gas));
                try_state!(gas, arb_state.retryable(call.ticketId).timeout().get())
            };

            if timeout == 0 {
                if context.cfg().stylus().arbos_version() >= 3 {
                    let output = ArbRetryableTx::NoTicketWithID {}.abi_encode();

                    return_revert!(gas, Bytes::from(output));
                }

                return_revert!(gas, Bytes::from("ticketId not found"));
            }

            let output = ArbRetryableTx::getTimeoutCall::abi_encode_returns(&U256::from(timeout));

            return_success!(gas, Bytes::from(output));
        }
        ArbRetryableTx::keepaliveCall::SELECTOR => {
            let call = ArbRetryableTx::keepaliveCall::abi_decode(input).unwrap();

            let (timeout, windows_left, calldata_len) = {
                let mut arb_state = context.arb_state(Some(&mut gas));
                let mut retryable = arb_state.retryable(call.ticketId);

                let timeout = try_state!(gas, retryable.timeout().get());
                let calldata_len = try_state!(gas, retryable.calldata().get()).len();
                let windows_left = try_state!(gas, retryable.timeout_windows_left().get());

                (timeout, windows_left, calldata_len)
            };

            if timeout == 0 {
                if context.cfg().stylus().arbos_version() >= 3 {
                    let output = ArbRetryableTx::NoTicketWithID {}.abi_encode();

                    return_revert!(gas, Bytes::from(output));
                }

                return_revert!(gas, Bytes::from("ticketId not found"));
            }

            let nbytes = { 7 * 32 + 32 * calldata_len.div_ceil(32) };

            let update_cost = nbytes.div_ceil(32) as u64 * revm::interpreter::gas::SSTORE_SET / 100;

            gas!(gas, update_cost);

            let current_time = context.block().timestamp().saturating_to::<u64>();
            let window = current_time + ARBOS_STATE_RETRYABLE_LIFETIME_SECONDS;

            let new_timeout = timeout + windows_left * ARBOS_STATE_RETRYABLE_LIFETIME_SECONDS;

            if timeout > window {
                return_revert!(gas, Bytes::from("timeout too far into the future"));
            }

            let mut arb_state = context.arb_state(Some(&mut gas));
            try_state!(
                gas,
                arb_state.timeout_queue().push(U256::from_be_slice(call.ticketId.as_slice()))
            );

            let mut retryable = arb_state.retryable(call.ticketId);
            try_state!(gas, retryable.timeout_windows_left().set(windows_left.saturating_add(1)));

            emit_event!(
                context,
                Log {
                    address: *target_address,
                    data: ArbRetryableTx::LifetimeExtended {
                        ticketId: call.ticketId,
                        newTimeout: U256::from(new_timeout),
                    }
                    .into_log_data()
                },
                gas
            );

            let output =
                ArbRetryableTx::keepaliveCall::abi_encode_returns(&U256::from(new_timeout));

            return_success!(gas, Bytes::from(output));
        }
        ArbRetryableTx::redeemCall::SELECTOR => {
            let call = ArbRetryableTx::redeemCall::abi_decode(input).unwrap();

            let timeout = {
                let mut arb_state = context.arb_state(Some(&mut gas));
                try_state!(gas, arb_state.retryable(call.ticketId).timeout().get())
            };

            if timeout == 0 {
                if context.cfg().stylus().arbos_version() >= 3 {
                    let output = ArbRetryableTx::NoTicketWithID {}.abi_encode();

                    return_revert!(gas, Bytes::from(output));
                }

                return_revert!(gas, Bytes::from("ticketId not found"));
            }

            // For simplicity, we do not implement redeem logic here.

            let output = ArbRetryableTx::redeemCall::abi_encode_returns(&call.ticketId);

            return_success!(gas, Bytes::from(output));
        }
        ArbRetryableTx::submitRetryableCall::SELECTOR => {
            let _ = ArbRetryableTx::submitRetryableCall::abi_decode(input).unwrap();

            let output = ArbRetryableTx::NotCallable {}.abi_encode();

            return_revert!(gas, Bytes::from(output));
        }
        _ => return_revert!(gas, Bytes::from("Unknown function selector")),
    }
}

fn retryable_escrow_address(ticket_id: B256) -> Address {
    let mut hasher_input = Vec::with_capacity(32 + "retryable escrow".len());
    hasher_input.extend_from_slice(b"retryable escrow");
    hasher_input.extend_from_slice(ticket_id.as_ref());

    let hash = keccak256(&hasher_input);
    Address::from_slice(&hash[12..32])
}
