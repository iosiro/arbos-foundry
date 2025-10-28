use alloy_sol_types::{SolCall, SolError, sol};
use revm::{
    interpreter::{Gas, InstructionResult, InterpreterResult},
    precompile::PrecompileId,
    primitives::{Address, Bytes, U256, address},
};

use crate::{ArbitrumContextTr, precompiles::extension::ExtendedPrecompile};

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
        ArbGasInfo::getAmortizedCostCapBipsCall::SELECTOR => {}
        ArbGasInfo::getGasAccountingParamsCall::SELECTOR => {}
        ArbGasInfo::getGasBacklogCall::SELECTOR => {}
        ArbGasInfo::getL1BaseFeeEstimateCall::SELECTOR => {}
        ArbGasInfo::getL1BaseFeeEstimateInertiaCall::SELECTOR => {}
        ArbGasInfo::getL1FeesAvailableCall::SELECTOR => {}
        ArbGasInfo::getL1PricingEquilibrationUnitsCall::SELECTOR => {}
        ArbGasInfo::getL1PricingFundsDueForRewardsCall::SELECTOR => {}
        ArbGasInfo::getL1PricingSurplusCall::SELECTOR => {}
        ArbGasInfo::getLastL1PricingSurplusCall::SELECTOR => {}
        ArbGasInfo::getLastL1PricingUpdateTimeCall::SELECTOR => {}
        ArbGasInfo::getMinimumGasPriceCall::SELECTOR => {}
        ArbGasInfo::getPerBatchGasChargeCall::SELECTOR => {}
        ArbGasInfo::getPricesInArbGasCall::SELECTOR => {}
        ArbGasInfo::getPricesInArbGasWithAggregatorCall::SELECTOR => {}
        ArbGasInfo::getPricesInWeiCall::SELECTOR => {}
        ArbGasInfo::getPricesInWeiWithAggregatorCall::SELECTOR => {}
        ArbGasInfo::getCurrentTxL1GasFeesCall::SELECTOR => {}
        ArbGasInfo::getPricingInertiaCall::SELECTOR => {}
        ArbGasInfo::getL1RewardRateCall::SELECTOR => {}
        ArbGasInfo::getL1RewardRecipientCall::SELECTOR => {}
        ArbGasInfo::getL1GasPriceEstimateCall::SELECTOR => {}
        _ => {
            return Ok(Some(InterpreterResult {
                result: InstructionResult::Revert,
                gas: Gas::new(gas_limit),
                output: Bytes::from("Unknown function selector"),
            }));
        }
    };

    Err("Not implemented".to_string())
}
