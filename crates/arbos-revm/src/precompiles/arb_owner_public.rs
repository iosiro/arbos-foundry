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
    state::{ArbState, ArbStateGetter},
};

sol! {

/// @title Provides non-owners with info about the current chain owners.
/// @notice Precompiled contract that exists in every Arbitrum chain at 0x000000000000000000000000000000000000006b.
interface ArbOwnerPublic {
    /// @notice See if the user is a chain owner
    function isChainOwner(
        address addr
    ) external view returns (bool);

    /**
     * @notice Rectify the list of chain owners
     * If successful, emits ChainOwnerRectified event
     * Available in ArbOS version 11
     */
    function rectifyChainOwner(
        address ownerToRectify
    ) external;

    /// @notice Retrieves the list of chain owners
    function getAllChainOwners() external view returns (address[] memory);

    /// @notice See if the user is a native token owner
    /// Available in ArbOS version 41
    function isNativeTokenOwner(
        address addr
    ) external view returns (bool);

    /// @notice Retrieves the list of native token owners
    /// Available in ArbOS version 41
    function getAllNativeTokenOwners() external view returns (address[] memory);

    /// @notice Gets the network fee collector
    function getNetworkFeeAccount() external view returns (address);

    /// @notice Get the infrastructure fee collector
    function getInfraFeeAccount() external view returns (address);

    /// @notice Get the Brotli compression level used for fast compression
    function getBrotliCompressionLevel() external view returns (uint64);

    /// @notice Get the next scheduled ArbOS version upgrade and its activation timestamp.
    /// Returns (0, 0) if no ArbOS upgrade is scheduled.
    /// Available in ArbOS version 20.
    function getScheduledUpgrade()
        external
        view
        returns (uint64 arbosVersion, uint64 scheduledForTimestamp);

    /**
     * @notice Checks if the increased calldata price feature (EIP-7623) is enabled
     * Available in ArbOS version 40 with default as false
     */
    function isCalldataPriceIncreaseEnabled() external view returns (bool);

    event ChainOwnerRectified(address rectifiedOwner);
}

}

pub fn arb_owner_public_precompile<CTX: ArbitrumContextTr>() -> ExtendedPrecompile<CTX> {
    ExtendedPrecompile::new(
        PrecompileId::Custom(std::borrow::Cow::Borrowed("ArbOwnerPublic")),
        address!("0x000000000000000000000000000000000000006b"),
        arb_owner_public_run::<CTX>,
    )
}
/// Run the precompile with the given context and input data.
/// Run the arb_owner_public precompile with the given context and input data.
fn arb_owner_public_run<CTX: ArbitrumContextTr>(
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
        ArbOwnerPublic::isChainOwnerCall::SELECTOR => {
            let call = ArbOwnerPublic::isChainOwnerCall::abi_decode(input).unwrap();

            let is_owner = context.arb_state().chain_owners().contains(&call.addr);

            let output = ArbOwnerPublic::isChainOwnerCall::abi_encode_returns(&is_owner);

            return_success!(gas, Bytes::from(output));
        }
        ArbOwnerPublic::isNativeTokenOwnerCall::SELECTOR => {
            let call = ArbOwnerPublic::isNativeTokenOwnerCall::abi_decode(input).unwrap();

            let is_owner = context.arb_state().native_token_owners().contains(&call.addr);

            let output = ArbOwnerPublic::isNativeTokenOwnerCall::abi_encode_returns(&is_owner);

            return_success!(gas, Bytes::from(output));
        }
        ArbOwnerPublic::getAllChainOwnersCall::SELECTOR => {
            let _ = ArbOwnerPublic::getAllChainOwnersCall::abi_decode(input).unwrap();
            let chains_owners = context.arb_state().chain_owners().all();

            let output = ArbOwnerPublic::getAllChainOwnersCall::abi_encode_returns(&chains_owners);

            return_success!(gas, Bytes::from(output));
        }
        ArbOwnerPublic::getAllNativeTokenOwnersCall::SELECTOR => {
            let _ = ArbOwnerPublic::getAllNativeTokenOwnersCall::abi_decode(input).unwrap();
            let native_token_owners = context.arb_state().native_token_owners().all();

            let output = ArbOwnerPublic::getAllNativeTokenOwnersCall::abi_encode_returns(
                &native_token_owners,
            );

            return_success!(gas, Bytes::from(output));
        }
        ArbOwnerPublic::getNetworkFeeAccountCall::SELECTOR => {
            let _ = ArbOwnerPublic::getNetworkFeeAccountCall::abi_decode(input).unwrap();
            let network_fee_account = context.arb_state().network_fee_account().get();

            let output =
                ArbOwnerPublic::getNetworkFeeAccountCall::abi_encode_returns(&network_fee_account);

            return_success!(gas, Bytes::from(output));
        }
        ArbOwnerPublic::getInfraFeeAccountCall::SELECTOR => {
            let _ = ArbOwnerPublic::getInfraFeeAccountCall::abi_decode(input).unwrap();
            let infra_fee_account = context.arb_state().infra_fee_account().get();
            let output =
                ArbOwnerPublic::getInfraFeeAccountCall::abi_encode_returns(&infra_fee_account);
            return_success!(gas, Bytes::from(output));
        }
        ArbOwnerPublic::getBrotliCompressionLevelCall::SELECTOR => {
            let _ = ArbOwnerPublic::getBrotliCompressionLevelCall::abi_decode(input).unwrap();
            let compression_level = context.arb_state().brotli_compression_level().get();
            let output = ArbOwnerPublic::getBrotliCompressionLevelCall::abi_encode_returns(
                &compression_level,
            );
            return_success!(gas, Bytes::from(output));
        }
        ArbOwnerPublic::getScheduledUpgradeCall::SELECTOR => {
            let _ = ArbOwnerPublic::getScheduledUpgradeCall::abi_decode(input).unwrap();
            let upgrade_version = context.arb_state().upgrade_version().get();
            let upgrade_timestamp = context.arb_state().upgrade_timestamp().get();
            let output = ArbOwnerPublic::getScheduledUpgradeCall::abi_encode_returns(
                &ArbOwnerPublic::getScheduledUpgradeReturn {
                    arbosVersion: upgrade_version,
                    scheduledForTimestamp: upgrade_timestamp,
                },
            );
            return_success!(gas, Bytes::from(output));
        }
        ArbOwnerPublic::isCalldataPriceIncreaseEnabledCall::SELECTOR => {
            todo!()
        }
        _ => return_revert!(gas, Bytes::from("Unknown selector")),
    }
}
