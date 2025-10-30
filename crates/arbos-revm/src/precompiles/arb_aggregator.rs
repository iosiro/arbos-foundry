use alloy_sol_types::{SolCall, sol};
use revm::{
    interpreter::{Gas, InstructionResult, InterpreterResult},
    precompile::PrecompileId,
    primitives::{Address, Bytes, U256, address},
};

use crate::{
    ArbitrumContextTr,
    constants::ARBOS_BATCH_POSTER_ADDRESS,
    precompiles::extension::ExtendedPrecompile,
    state::{ArbState, ArbStateGetter},
};

sol! {
/// @title Provides aggregators and their users methods for configuring how they participate in L1 aggregation.
/// @notice Precompiled contract that exists in every Arbitrum chain at 0x000000000000000000000000000000000000006d
interface ArbAggregator {
    /// @notice Deprecated, customization of preferred aggregator is no longer supported
    /// @notice Get the address of an arbitrarily chosen batch poster.
    /// @param addr ignored
    /// @return (batchPosterAddress, true)
    function getPreferredAggregator(
        address addr
    ) external view returns (address, bool);

    /// @notice Deprecated, there is no longer a single preferred aggregator, use getBatchPosters instead
    /// @notice Get default aggregator.
    function getDefaultAggregator() external view returns (address);

    /// @notice Get a list of all current batch posters
    /// @return Batch poster addresses
    function getBatchPosters() external view returns (address[] memory);

    /// @notice Adds newBatchPoster as a batch poster
    /// This reverts unless called by a chain owner
    /// @param newBatchPoster New batch poster
    function addBatchPoster(
        address newBatchPoster
    ) external;

    /// @notice Get the address where fees to batchPoster are sent.
    /// @param batchPoster The batch poster to get the fee collector for
    /// @return The fee collectors address. This will sometimes but not always be the same as the batch poster's address.
    function getFeeCollector(
        address batchPoster
    ) external view returns (address);

    /// @notice Set the address where fees to batchPoster are sent.
    /// This reverts unless called by the batch poster, its fee collector, or a chain owner
    /// @param batchPoster The batch poster to set the fee collector for
    /// @param newFeeCollector The new fee collector to set
    function setFeeCollector(address batchPoster, address newFeeCollector) external;

    /// @notice Deprecated, always returns zero
    /// @notice Get the tx base fee (in approximate L1 gas) for aggregator
    /// @param aggregator The aggregator to get the base fee for
    function getTxBaseFee(
        address aggregator
    ) external view returns (uint256);

    /// @notice Deprecated, is now a no-op
    /// @notice Set the tx base fee (in approximate L1 gas) for aggregator
    /// Revert unless called by aggregator or the chain owner
    /// Revert if feeInL1Gas is outside the chain's allowed bounds
    /// @param aggregator The aggregator to set the fee for
    /// @param feeInL1Gas The base fee in L1 gas
    function setTxBaseFee(address aggregator, uint256 feeInL1Gas) external;
}
}

pub fn arb_aggregator_precompile<CTX: ArbitrumContextTr>() -> ExtendedPrecompile<CTX> {
    ExtendedPrecompile::new(
        PrecompileId::Custom(std::borrow::Cow::Borrowed("ArbAggregator")),
        address!("0x000000000000000000000000000000000000006d"),
        arb_aggregator_run::<CTX>,
    )
}
/// Run the precompile with the given context and input data.
/// Run the arb_aggregator precompile with the given context and input data.
fn arb_aggregator_run<CTX: ArbitrumContextTr>(
    context: &mut CTX,
    input: &[u8],
    _target_address: &Address,
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

    // decode selector
    let selector: [u8; 4] = input[0..4].try_into().unwrap();

    match selector {
        ArbAggregator::addBatchPosterCall::SELECTOR => {
            if !context.arb_state().chain_owners().contains(&caller_address) {
                return Ok(Some(InterpreterResult {
                    result: InstructionResult::Revert,
                    gas: Gas::new(gas_limit),
                    output: Bytes::from("must be called by chain owner"),
                }));
            }

            let call = ArbAggregator::addBatchPosterCall::abi_decode(input).unwrap();

            if context.arb_state().l1_pricing().batch_poster_table().contains(&call.newBatchPoster)
            {
                return Ok(Some(InterpreterResult {
                    result: InstructionResult::Return,
                    gas: Gas::new(gas_limit),
                    output: Bytes::default(),
                }));
            }

            context
                .arb_state()
                .l1_pricing()
                .batch_poster_table()
                .add(&call.newBatchPoster, &call.newBatchPoster);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::new(),
            }))
        }
        ArbAggregator::getBatchPostersCall::SELECTOR => {
            let posters = context.arb_state().l1_pricing().batch_poster_table().all();

            let output = ArbAggregator::getBatchPostersCall::abi_encode_returns(&posters);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }))
        }
        ArbAggregator::getDefaultAggregatorCall::SELECTOR => {
            let output = ArbAggregator::getDefaultAggregatorCall::abi_encode_returns(
                &ARBOS_BATCH_POSTER_ADDRESS,
            );

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }))
        }
        ArbAggregator::getFeeCollectorCall::SELECTOR => {
            let call = ArbAggregator::getFeeCollectorCall::abi_decode(input).unwrap();

            let mut arb_state = context.arb_state();
            let mut l1_pricing = arb_state.l1_pricing();
            let mut batch_poster_table = l1_pricing.batch_poster_table();
            let mut batch_poster_state = batch_poster_table.get(&call.batchPoster);

            let fee_collector = batch_poster_state.pay_recipient().get();

            let output = ArbAggregator::getFeeCollectorCall::abi_encode_returns(&fee_collector);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }))
        }
        ArbAggregator::getPreferredAggregatorCall::SELECTOR => {
            let output = ArbAggregator::getPreferredAggregatorCall::abi_encode_returns(
                &ArbAggregator::getPreferredAggregatorReturn {
                    _0: ARBOS_BATCH_POSTER_ADDRESS,
                    _1: true,
                },
            );

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }))
        }
        ArbAggregator::setFeeCollectorCall::SELECTOR => {
            let call = ArbAggregator::setFeeCollectorCall::abi_decode(input).unwrap();

            let is_chain_owner = { context.arb_state().chain_owners().contains(&caller_address) };

            let mut arb_state = context.arb_state();
            let mut l1_pricing = arb_state.l1_pricing();
            let mut batch_poster_table = l1_pricing.batch_poster_table();
            let mut batch_poster_state = batch_poster_table.get(&call.batchPoster);

            let current_fee_collector = batch_poster_state.pay_recipient().get();

            if caller_address != call.batchPoster
                && caller_address != current_fee_collector
                && !is_chain_owner
            {
                return Ok(Some(InterpreterResult {
                    result: InstructionResult::Revert,
                    gas: Gas::new(gas_limit),
                    output: Bytes::from(
                        "only a batch poster (or its fee collector / chain owner) may change its fee collector",
                    ),
                }));
            }

            batch_poster_state.pay_recipient().set(&call.newFeeCollector);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::new(),
            }))
        }
        ArbAggregator::getTxBaseFeeCall::SELECTOR => {
            let output = ArbAggregator::getTxBaseFeeCall::abi_encode_returns(&U256::ZERO);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }))
        }
        ArbAggregator::setTxBaseFeeCall::SELECTOR => Ok(Some(InterpreterResult {
            result: InstructionResult::Return,
            gas: Gas::new(gas_limit),
            output: Bytes::new(),
        })),
        _ => Ok(Some(InterpreterResult {
            result: InstructionResult::Revert,
            gas: Gas::new(gas_limit),
            output: Bytes::from("Function not implemented"),
        })),
    }
}
