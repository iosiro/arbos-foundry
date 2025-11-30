use alloy_sol_types::{SolCall, sol};
use revm::{
    context::JournalTr,
    interpreter::{Gas, InterpreterResult},
    precompile::PrecompileId,
    primitives::{Address, Bytes, Log, U256, address, alloy_primitives::IntoLogData},
};

use crate::{
    ArbitrumContextTr, generate_state_mut_table,
    macros::{emit_event, interpreter_return, interpreter_revert},
    precompile_impl,
    precompiles::{
        ArbPrecompileLogic, ExtendedPrecompile, StateMutability, decode_call, selector_or_revert,
    },
    state::{ArbState, ArbStateGetter, try_state, types::StorageBackedTr},
};

sol! {

/// @title Provides non-owners with info about the current chain owners.
/// @notice Precompiled contract that exists in every Arbitrum chain at 0x000000000000000000000000000000000000006b.
interface ArbOwnerPublic {
    /// @notice See if the user is a chain owner
    function isChainOwner(
        address addr
    ) external view returns (bool);

    ///
    /// @notice Rectify the list of chain owners
    /// If successful, emits ChainOwnerRectified event
    /// Available in ArbOS version 11
    ///
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

    ///
    /// @notice Checks if the increased calldata price feature (EIP-7623) is enabled
    /// Available in ArbOS version 40 with default as false
    ///
    function isCalldataPriceIncreaseEnabled() external view returns (bool);

    event ChainOwnerRectified(address rectifiedOwner);
}

}

pub fn arb_owner_public_precompile<CTX: ArbitrumContextTr>() -> ExtendedPrecompile<CTX> {
    ExtendedPrecompile::new(
        PrecompileId::Custom(std::borrow::Cow::Borrowed("ArbOwnerPublic")),
        address!("0x000000000000000000000000000000000000006b"),
        precompile_impl!(ArbOwnerPublicPrecompile),
    )
}

struct ArbOwnerPublicPrecompile;

impl<CTX: ArbitrumContextTr> ArbPrecompileLogic<CTX> for ArbOwnerPublicPrecompile {
    const STATE_MUT_TABLE: &'static [([u8; 4], StateMutability)] = generate_state_mut_table! {
        ArbOwnerPublic => {
            isChainOwnerCall(View),
            rectifyChainOwnerCall(NonPayable),
            getAllChainOwnersCall(View),
            isNativeTokenOwnerCall(View),
            getAllNativeTokenOwnersCall(View),
            getNetworkFeeAccountCall(View),
            getInfraFeeAccountCall(View),
            getBrotliCompressionLevelCall(View),
            getScheduledUpgradeCall(View),
            isCalldataPriceIncreaseEnabledCall(View),
        }
    };

    fn inner(
        context: &mut CTX,
        input: &[u8],
        _target_address: &Address,
        _caller_address: Address,
        _call_value: U256,
        is_static: bool,
        gas_limit: u64,
    ) -> Option<InterpreterResult> {
        let mut gas = Gas::new(gas_limit);
        let selector = selector_or_revert!(gas, input);

        match selector {
            ArbOwnerPublic::isChainOwnerCall::SELECTOR => {
                let call = decode_call!(gas, ArbOwnerPublic::isChainOwnerCall, input);

                let is_owner = try_state!(
                    gas,
                    context.arb_state(Some(&mut gas), is_static).is_chain_owner(call.addr)
                );

                let output = ArbOwnerPublic::isChainOwnerCall::abi_encode_returns(&is_owner);

                interpreter_return!(gas, Bytes::from(output));
            }
            ArbOwnerPublic::isNativeTokenOwnerCall::SELECTOR => {
                let call = decode_call!(gas, ArbOwnerPublic::isNativeTokenOwnerCall, input);

                let is_owner = try_state!(
                    gas,
                    context.arb_state(Some(&mut gas), is_static).is_native_token_owner(call.addr)
                );

                let output = ArbOwnerPublic::isNativeTokenOwnerCall::abi_encode_returns(&is_owner);

                interpreter_return!(gas, Bytes::from(output));
            }
            ArbOwnerPublic::getAllChainOwnersCall::SELECTOR => {
                let _ = decode_call!(gas, ArbOwnerPublic::getAllChainOwnersCall, input);
                let chains_owners = try_state!(
                    gas,
                    context.arb_state(Some(&mut gas), is_static).chain_owners().all()
                );

                let output =
                    ArbOwnerPublic::getAllChainOwnersCall::abi_encode_returns(&chains_owners);

                interpreter_return!(gas, Bytes::from(output));
            }
            ArbOwnerPublic::getAllNativeTokenOwnersCall::SELECTOR => {
                let _ = decode_call!(gas, ArbOwnerPublic::getAllNativeTokenOwnersCall, input);
                let native_token_owners = try_state!(
                    gas,
                    context.arb_state(Some(&mut gas), is_static).native_token_owners().all()
                );

                let output = ArbOwnerPublic::getAllNativeTokenOwnersCall::abi_encode_returns(
                    &native_token_owners,
                );

                interpreter_return!(gas, Bytes::from(output));
            }
            ArbOwnerPublic::getNetworkFeeAccountCall::SELECTOR => {
                let _ = decode_call!(gas, ArbOwnerPublic::getNetworkFeeAccountCall, input);
                let network_fee_account = try_state!(
                    gas,
                    context.arb_state(Some(&mut gas), is_static).network_fee_account().get()
                );

                let output = ArbOwnerPublic::getNetworkFeeAccountCall::abi_encode_returns(
                    &network_fee_account,
                );

                interpreter_return!(gas, Bytes::from(output));
            }
            ArbOwnerPublic::getInfraFeeAccountCall::SELECTOR => {
                let _ = decode_call!(gas, ArbOwnerPublic::getInfraFeeAccountCall, input);
                let infra_fee_account = try_state!(
                    gas,
                    context.arb_state(Some(&mut gas), is_static).infra_fee_account().get()
                );
                let output =
                    ArbOwnerPublic::getInfraFeeAccountCall::abi_encode_returns(&infra_fee_account);
                interpreter_return!(gas, Bytes::from(output));
            }
            ArbOwnerPublic::getBrotliCompressionLevelCall::SELECTOR => {
                let _ = decode_call!(gas, ArbOwnerPublic::getBrotliCompressionLevelCall, input);
                let compression_level = try_state!(
                    gas,
                    context.arb_state(Some(&mut gas), is_static).brotli_compression_level().get()
                );
                let output = ArbOwnerPublic::getBrotliCompressionLevelCall::abi_encode_returns(
                    &compression_level,
                );
                interpreter_return!(gas, Bytes::from(output));
            }
            ArbOwnerPublic::getScheduledUpgradeCall::SELECTOR => {
                let _ = decode_call!(gas, ArbOwnerPublic::getScheduledUpgradeCall, input);
                let upgrade_version = try_state!(
                    gas,
                    context.arb_state(Some(&mut gas), is_static).upgrade_version().get()
                );
                let upgrade_timestamp = try_state!(
                    gas,
                    context.arb_state(Some(&mut gas), is_static).upgrade_timestamp().get()
                );
                let output = ArbOwnerPublic::getScheduledUpgradeCall::abi_encode_returns(
                    &ArbOwnerPublic::getScheduledUpgradeReturn {
                        arbosVersion: upgrade_version,
                        scheduledForTimestamp: upgrade_timestamp,
                    },
                );
                interpreter_return!(gas, Bytes::from(output));
            }
            ArbOwnerPublic::isCalldataPriceIncreaseEnabledCall::SELECTOR => {
                let _ =
                    decode_call!(gas, ArbOwnerPublic::isCalldataPriceIncreaseEnabledCall, input);
                let enabled = {
                    let mut arb_state = context.arb_state(Some(&mut gas), is_static);
                    let value = try_state!(gas, arb_state.l1_pricing().gas_floor_per_token().get());
                    value != 0
                };
                let output = ArbOwnerPublic::isCalldataPriceIncreaseEnabledCall::abi_encode_returns(
                    &enabled,
                );

                interpreter_return!(gas, Bytes::from(output));
            }
            ArbOwnerPublic::rectifyChainOwnerCall::SELECTOR => {
                let call = decode_call!(gas, ArbOwnerPublic::rectifyChainOwnerCall, input);

                let mut arb_state = context.arb_state(Some(&mut gas), is_static);
                try_state!(gas, arb_state.chain_owners().rectify(call.ownerToRectify));

                let log = Log {
                    data: ArbOwnerPublic::ChainOwnerRectified {
                        rectifiedOwner: call.ownerToRectify,
                    }
                    .into_log_data(),
                    ..Default::default()
                };

                emit_event!(context, log, gas);

                interpreter_return!(gas);
            }
            _ => interpreter_revert!(gas, Bytes::from("Unknown selector")),
        }
    }
}
