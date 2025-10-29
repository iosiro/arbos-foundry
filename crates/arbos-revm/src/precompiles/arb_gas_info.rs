use alloy_sol_types::{SolCall, sol};
use revm::{
    context::Block,
    interpreter::{Gas, InstructionResult, InterpreterResult},
    precompile::PrecompileId,
    primitives::{Address, Bytes, I256, U256, address},
};

use crate::{
    ArbitrumContextTr,
    chain::ArbitrumChainInfoTr,
    constants::ARBOS_L1_PRICER_FUNDS_ADDRESS,
    precompiles::extension::ExtendedPrecompile,
    state::{ArbState, ArbStateGetter},
};

const ARBOS_GAS_INFO_ASSUMED_SIMPLE_TX_SIZE: u64 = 140;

sol! {
/// @title Provides insight into the cost of using the chain.
/// @notice These methods have been adjusted to account for Nitro's heavy use of calldata compression.
/// Of note to end-users, we no longer make a distinction between non-zero and zero-valued calldata bytes.
/// Precompiled contract that exists in every Arbitrum chain at 0x000000000000000000000000000000000000006c.
interface ArbGasInfo {
    /// @notice Get gas prices for a provided aggregator
    /// @return return gas prices in wei
    ///        (
    ///            per L2 tx,
    ///            per L1 calldata byte
    ///            per storage allocation,
    ///            per ArbGas base,
    ///            per ArbGas congestion,
    ///            per ArbGas total
    ///        )
    function getPricesInWeiWithAggregator(
        address aggregator
    ) external view returns (uint256, uint256, uint256, uint256, uint256, uint256);

    /// @notice Get gas prices. Uses the caller's preferred aggregator, or the default if the caller doesn't have a preferred one.
    /// @return return gas prices in wei
    ///        (
    ///            per L2 tx,
    ///            per L1 calldata byte
    ///            per storage allocation,
    ///            per ArbGas base,
    ///            per ArbGas congestion,
    ///            per ArbGas total
    ///        )
    function getPricesInWei()
        external
        view
        returns (uint256, uint256, uint256, uint256, uint256, uint256);

    /// @notice Get prices in ArbGas for the supplied aggregator
    /// @return (per L2 tx, per L1 calldata byte, per storage allocation)
    function getPricesInArbGasWithAggregator(
        address aggregator
    ) external view returns (uint256, uint256, uint256);

    /// @notice Get prices in ArbGas. Assumes the callers preferred validator, or the default if caller doesn't have a preferred one.
    /// @return (per L2 tx, per L1 calldata byte, per storage allocation)
    function getPricesInArbGas() external view returns (uint256, uint256, uint256);

    /// @notice Get the gas accounting parameters. `gasPoolMax` is always zero, as the exponential pricing model has no such notion.
    /// @return (speedLimitPerSecond, gasPoolMax, maxTxGasLimit)
    function getGasAccountingParams() external view returns (uint256, uint256, uint256);

    /// @notice Get the minimum gas price needed for a tx to succeed
    function getMinimumGasPrice() external view returns (uint256);

    /// @notice Get ArbOS's estimate of the L1 basefee in wei
    function getL1BaseFeeEstimate() external view returns (uint256);

    /// @notice Get how slowly ArbOS updates its estimate of the L1 basefee
    function getL1BaseFeeEstimateInertia() external view returns (uint64);

    /// @notice Get the L1 pricer reward rate, in wei per unit
    /// Available in ArbOS version 11
    function getL1RewardRate() external view returns (uint64);

    /// @notice Get the L1 pricer reward recipient
    /// Available in ArbOS version 11
    function getL1RewardRecipient() external view returns (address);

    /// @notice Deprecated -- Same as getL1BaseFeeEstimate()
    function getL1GasPriceEstimate() external view returns (uint256);

    /// @notice Get L1 gas fees paid by the current transaction
    function getCurrentTxL1GasFees() external view returns (uint256);

    /// @notice Get the backlogged amount of gas burnt in excess of the speed limit
    function getGasBacklog() external view returns (uint64);

    /// @notice Get how slowly ArbOS updates the L2 basefee in response to backlogged gas
    function getPricingInertia() external view returns (uint64);

    /// @notice Get the forgivable amount of backlogged gas ArbOS will ignore when raising the basefee
    function getGasBacklogTolerance() external view returns (uint64);

    /// @notice Returns the surplus of funds for L1 batch posting payments (may be negative).
    function getL1PricingSurplus() external view returns (int256);

    /// @notice Returns the base charge (in L1 gas) attributed to each data batch in the calldata pricer
    function getPerBatchGasCharge() external view returns (int64);

    /// @notice Returns the cost amortization cap in basis points
    function getAmortizedCostCapBips() external view returns (uint64);

    /// @notice Returns the available funds from L1 fees
    function getL1FeesAvailable() external view returns (uint256);

    /// @notice Returns the equilibration units parameter for L1 price adjustment algorithm
    /// Available in ArbOS version 20
    function getL1PricingEquilibrationUnits() external view returns (uint256);

    /// @notice Returns the last time the L1 calldata pricer was updated.
    /// Available in ArbOS version 20
    function getLastL1PricingUpdateTime() external view returns (uint64);

    /// @notice Returns the amount of L1 calldata payments due for rewards (per the L1 reward rate)
    /// Available in ArbOS version 20
    function getL1PricingFundsDueForRewards() external view returns (uint256);

    /// @notice Returns the amount of L1 calldata posted since the last update.
    /// Available in ArbOS version 20
    function getL1PricingUnitsSinceUpdate() external view returns (uint64);

    /// @notice Returns the L1 pricing surplus as of the last update (may be negative).
    /// Available in ArbOS version 20
    function getLastL1PricingSurplus() external view returns (int256);
}

}

pub fn arb_gas_info_precompile<CTX: ArbitrumContextTr>() -> ExtendedPrecompile<CTX> {
    ExtendedPrecompile::new(
        PrecompileId::Custom(std::borrow::Cow::Borrowed("ArbGasInfo")),
        address!("0x000000000000000000000000000000000000006c"),
        arb_gas_info_run::<CTX>,
    )
}
/// Run the precompile with the given context and input data.
/// Run the arb_info precompile with the given context and input data.
fn arb_gas_info_run<CTX: ArbitrumContextTr>(
    context: &mut CTX,
    input: &[u8],
    _target_address: &Address,
    _caller_address: Address,
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
        ArbGasInfo::getAmortizedCostCapBipsCall::SELECTOR => {
            let amortized_cost_cap_bips =
                context.arb_state().l1_pricing().amortized_cost_cap_bips().get();

            let output = ArbGasInfo::getAmortizedCostCapBipsCall::abi_encode_returns(
                &amortized_cost_cap_bips,
            );

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }))
        }
        ArbGasInfo::getGasAccountingParamsCall::SELECTOR => {
            let speed_limit_per_second =
                context.arb_state().l2_pricing().speed_limit_per_second().get();

            let max_tx_gas_limit = context.arb_state().l2_pricing().per_block_gas_limit().get();

            let output = ArbGasInfo::getGasAccountingParamsCall::abi_encode_returns(
                &ArbGasInfo::getGasAccountingParamsReturn {
                    _0: U256::from(speed_limit_per_second),
                    _1: U256::from(max_tx_gas_limit),
                    _2: U256::from(max_tx_gas_limit),
                },
            );

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }))
        }
        ArbGasInfo::getGasBacklogCall::SELECTOR => {
            let gas_backlog = context.arb_state().l2_pricing().gas_backlog().get();

            let output = ArbGasInfo::getGasBacklogCall::abi_encode_returns(&gas_backlog);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }))
        }
        ArbGasInfo::getL1BaseFeeEstimateCall::SELECTOR => {
            let l1_base_fee_estimate = context.arb_state().l1_pricing().price_per_unit().get();

            let output =
                ArbGasInfo::getL1BaseFeeEstimateCall::abi_encode_returns(&l1_base_fee_estimate);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }))
        }
        ArbGasInfo::getL1BaseFeeEstimateInertiaCall::SELECTOR => {
            let pricing_inertia = context.arb_state().l1_pricing().inertia().get();

            let output =
                ArbGasInfo::getL1BaseFeeEstimateInertiaCall::abi_encode_returns(&pricing_inertia);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }))
        }
        ArbGasInfo::getL1FeesAvailableCall::SELECTOR => {
            let l1_fees_available = context.arb_state().l1_pricing().l1_fees_available().get();

            let output = ArbGasInfo::getL1FeesAvailableCall::abi_encode_returns(&l1_fees_available);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }))
        }
        ArbGasInfo::getL1PricingEquilibrationUnitsCall::SELECTOR => {
            let equilibration_units = context.arb_state().l1_pricing().equilibration_units().get();

            let output = ArbGasInfo::getL1PricingEquilibrationUnitsCall::abi_encode_returns(
                &equilibration_units,
            );

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }))
        }
        ArbGasInfo::getL1PricingFundsDueForRewardsCall::SELECTOR => {
            let funds_due_for_rewards =
                context.arb_state().l1_pricing().funds_due_for_rewards().get();

            let output = ArbGasInfo::getL1PricingFundsDueForRewardsCall::abi_encode_returns(
                &U256::from(funds_due_for_rewards),
            );

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }))
        }
        ArbGasInfo::getL1PricingSurplusCall::SELECTOR => {
            let l1_pricing_surplus = context.arb_state().l1_pricing().last_surplus().get();

            let output =
                ArbGasInfo::getL1PricingSurplusCall::abi_encode_returns(&l1_pricing_surplus);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }))
        }
        ArbGasInfo::getLastL1PricingSurplusCall::SELECTOR => {
            let funds_due_for_refund =
                { context.arb_state().l1_pricing().batch_poster_table().total_funds_due().get() };

            let funds_due_for_rewards =
                { context.arb_state().l1_pricing().funds_due_for_rewards().get() };

            let need_funds = funds_due_for_refund.wrapping_add(funds_due_for_rewards);

            let have_funds = if context.chain().arbos_version().unwrap_or_default() < 10 {
                let arb_pricer_funds =
                    context.balance(ARBOS_L1_PRICER_FUNDS_ADDRESS).unwrap_or_default();
                arb_pricer_funds.data
            } else {
                context.arb_state().l1_pricing().l1_fees_available().get()
            };

            let surplus = I256::from(have_funds) - need_funds;

            let output = ArbGasInfo::getLastL1PricingSurplusCall::abi_encode_returns(&surplus);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }))
        }
        ArbGasInfo::getLastL1PricingUpdateTimeCall::SELECTOR => {
            let last_update_time = context.arb_state().l1_pricing().last_update_time().get();

            let output =
                ArbGasInfo::getLastL1PricingUpdateTimeCall::abi_encode_returns(&last_update_time);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }))
        }
        ArbGasInfo::getMinimumGasPriceCall::SELECTOR => {
            let minimum_gas_price = context.arb_state().l2_pricing().min_base_fee_wei().get();

            let output = ArbGasInfo::getMinimumGasPriceCall::abi_encode_returns(&minimum_gas_price);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }))
        }
        ArbGasInfo::getPerBatchGasChargeCall::SELECTOR => {
            let per_batch_gas_charge = context.arb_state().l1_pricing().per_batch_gas_cost().get();

            let output = ArbGasInfo::getPerBatchGasChargeCall::abi_encode_returns(
                &(per_batch_gas_charge as i64),
            );

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }))
        }
        ArbGasInfo::getPricesInArbGasCall::SELECTOR => {
            let l1_gas_price = { context.arb_state().l1_pricing().price_per_unit().get() };

            let l2_gas_price = { context.block().basefee() };

            let wei_for_l1_calldata = l1_gas_price.saturating_mul(U256::from(
                revm::interpreter::gas::NON_ZERO_BYTE_MULTIPLIER_ISTANBUL,
            ));

            if context.chain().arbos_version().unwrap_or_default() < 4 {
                let mut gas_for_l1_calldata = U256::ZERO;
                if l2_gas_price > 0 {
                    gas_for_l1_calldata =
                        wei_for_l1_calldata.wrapping_div(U256::from(l2_gas_price));
                }

                let per_l2_tx = U256::from(ARBOS_GAS_INFO_ASSUMED_SIMPLE_TX_SIZE);

                let output = ArbGasInfo::getPricesInArbGasCall::abi_encode_returns(
                    &ArbGasInfo::getPricesInArbGasReturn {
                        _0: per_l2_tx,
                        _1: gas_for_l1_calldata,
                        _2: U256::from(revm::interpreter::gas::SSTORE_SET),
                    },
                );

                Ok(Some(InterpreterResult {
                    result: InstructionResult::Return,
                    gas: Gas::new(gas_limit),
                    output: Bytes::from(output),
                }))
            } else {
                let wei_per_l2_tx = wei_for_l1_calldata
                    .saturating_mul(U256::from(ARBOS_GAS_INFO_ASSUMED_SIMPLE_TX_SIZE));
                let mut gas_for_l1_calldata = U256::ZERO;
                let mut gas_per_l2_tx = U256::ZERO;
                if l2_gas_price > 0 {
                    gas_for_l1_calldata =
                        wei_for_l1_calldata.wrapping_div(U256::from(l2_gas_price));
                    gas_per_l2_tx = wei_per_l2_tx.wrapping_div(U256::from(l2_gas_price));
                }

                let output = ArbGasInfo::getPricesInArbGasCall::abi_encode_returns(
                    &ArbGasInfo::getPricesInArbGasReturn {
                        _0: gas_per_l2_tx,
                        _1: gas_for_l1_calldata,
                        _2: U256::from(revm::interpreter::gas::SSTORE_SET),
                    },
                );

                Ok(Some(InterpreterResult {
                    result: InstructionResult::Return,
                    gas: Gas::new(gas_limit),
                    output: Bytes::from(output),
                }))
            }
        }
        ArbGasInfo::getPricesInArbGasWithAggregatorCall::SELECTOR => {
            let l1_gas_price = { context.arb_state().l1_pricing().price_per_unit().get() };

            let l2_gas_price = { context.block().basefee() };

            let wei_for_l1_calldata = l1_gas_price.saturating_mul(U256::from(
                revm::interpreter::gas::NON_ZERO_BYTE_MULTIPLIER_ISTANBUL,
            ));

            if context.chain().arbos_version().unwrap_or_default() < 4 {
                let mut gas_for_l1_calldata = U256::ZERO;
                if l2_gas_price > 0 {
                    gas_for_l1_calldata =
                        wei_for_l1_calldata.wrapping_div(U256::from(l2_gas_price));
                }

                let per_l2_tx = U256::from(ARBOS_GAS_INFO_ASSUMED_SIMPLE_TX_SIZE);

                let output = ArbGasInfo::getPricesInArbGasCall::abi_encode_returns(
                    &ArbGasInfo::getPricesInArbGasReturn {
                        _0: per_l2_tx,
                        _1: gas_for_l1_calldata,
                        _2: U256::from(revm::interpreter::gas::SSTORE_SET),
                    },
                );

                Ok(Some(InterpreterResult {
                    result: InstructionResult::Return,
                    gas: Gas::new(gas_limit),
                    output: Bytes::from(output),
                }))
            } else {
                let wei_per_l2_tx = wei_for_l1_calldata
                    .saturating_mul(U256::from(ARBOS_GAS_INFO_ASSUMED_SIMPLE_TX_SIZE));
                let mut gas_for_l1_calldata = U256::ZERO;
                let mut gas_per_l2_tx = U256::ZERO;
                if l2_gas_price > 0 {
                    gas_for_l1_calldata =
                        wei_for_l1_calldata.wrapping_div(U256::from(l2_gas_price));
                    gas_per_l2_tx = wei_per_l2_tx.wrapping_div(U256::from(l2_gas_price));
                }

                let output = ArbGasInfo::getPricesInArbGasWithAggregatorCall::abi_encode_returns(
                    &ArbGasInfo::getPricesInArbGasWithAggregatorReturn {
                        _0: gas_per_l2_tx,
                        _1: gas_for_l1_calldata,
                        _2: U256::from(revm::interpreter::gas::SSTORE_SET),
                    },
                );

                Ok(Some(InterpreterResult {
                    result: InstructionResult::Return,
                    gas: Gas::new(gas_limit),
                    output: Bytes::from(output),
                }))
            }
        }
        ArbGasInfo::getPricesInWeiCall::SELECTOR => {
            let l1_gas_price = { context.arb_state().l1_pricing().price_per_unit().get() };

            let l2_gas_price = { context.block().basefee() };

            let wei_for_l1_calldata = l1_gas_price.saturating_mul(U256::from(
                revm::interpreter::gas::NON_ZERO_BYTE_MULTIPLIER_ISTANBUL,
            ));

            let wei_per_l2_tx = wei_for_l1_calldata
                .saturating_mul(U256::from(ARBOS_GAS_INFO_ASSUMED_SIMPLE_TX_SIZE));

            let per_arb_gas_base = l2_gas_price;
            let per_arb_gas_congestion = U256::ZERO;
            let per_arb_gas_total = l2_gas_price;

            let wei_for_l2_storage = U256::from(revm::interpreter::gas::SSTORE_SET)
                .saturating_mul(U256::from(l2_gas_price));

            let output = ArbGasInfo::getPricesInWeiCall::abi_encode_returns(
                &ArbGasInfo::getPricesInWeiReturn {
                    _0: wei_per_l2_tx,
                    _1: wei_for_l1_calldata,
                    _2: wei_for_l2_storage,
                    _3: U256::from(per_arb_gas_base),
                    _4: per_arb_gas_congestion,
                    _5: U256::from(per_arb_gas_total),
                },
            );

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }))
        }
        ArbGasInfo::getPricesInWeiWithAggregatorCall::SELECTOR => {
            let l1_gas_price = { context.arb_state().l1_pricing().price_per_unit().get() };

            let l2_gas_price = { context.block().basefee() };

            let wei_for_l1_calldata = l1_gas_price.saturating_mul(U256::from(
                revm::interpreter::gas::NON_ZERO_BYTE_MULTIPLIER_ISTANBUL,
            ));

            let wei_per_l2_tx = wei_for_l1_calldata
                .saturating_mul(U256::from(ARBOS_GAS_INFO_ASSUMED_SIMPLE_TX_SIZE));

            let per_arb_gas_base = l2_gas_price;
            let per_arb_gas_congestion = U256::ZERO;
            let per_arb_gas_total = l2_gas_price;

            let wei_for_l2_storage = U256::from(revm::interpreter::gas::SSTORE_SET)
                .saturating_mul(U256::from(l2_gas_price));

            let output = ArbGasInfo::getPricesInWeiWithAggregatorCall::abi_encode_returns(
                &ArbGasInfo::getPricesInWeiWithAggregatorReturn {
                    _0: wei_per_l2_tx,
                    _1: wei_for_l1_calldata,
                    _2: wei_for_l2_storage,
                    _3: U256::from(per_arb_gas_base),
                    _4: per_arb_gas_congestion,
                    _5: U256::from(per_arb_gas_total),
                },
            );

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }))
        }
        ArbGasInfo::getCurrentTxL1GasFeesCall::SELECTOR => {
            let output = ArbGasInfo::getCurrentTxL1GasFeesCall::abi_encode_returns(&U256::ZERO);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }))
        }
        ArbGasInfo::getPricingInertiaCall::SELECTOR => {
            let pricing_inertia = context.arb_state().l2_pricing().pricing_inertia().get();

            let output = ArbGasInfo::getPricingInertiaCall::abi_encode_returns(&pricing_inertia);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }))
        }
        ArbGasInfo::getL1RewardRateCall::SELECTOR => {
            let l1_reward_rate = context.arb_state().l1_pricing().per_unit_reward().get();

            let output = ArbGasInfo::getL1RewardRateCall::abi_encode_returns(&l1_reward_rate);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }))
        }
        ArbGasInfo::getL1RewardRecipientCall::SELECTOR => {
            let l1_reward_recipient = context.arb_state().l1_pricing().reward_recipient().get();

            let output =
                ArbGasInfo::getL1RewardRecipientCall::abi_encode_returns(&l1_reward_recipient);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
            }))
        }
        ArbGasInfo::getL1GasPriceEstimateCall::SELECTOR => {
            let l1_gas_price_estimate = context.arb_state().l1_pricing().price_per_unit().get();

            let output =
                ArbGasInfo::getL1GasPriceEstimateCall::abi_encode_returns(&l1_gas_price_estimate);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(gas_limit),
                output: Bytes::from(output),
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
