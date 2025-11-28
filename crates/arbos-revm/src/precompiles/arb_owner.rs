use alloy_sol_types::{SolCall, sol};
use revm::{
    interpreter::{Gas, InterpreterResult},
    precompile::PrecompileId,
    primitives::{Address, Bytes, U256, address},
};

use crate::{
    ArbitrumContextTr, generate_state_mut_table,
    macros::{interpreter_return, interpreter_revert},
    precompile_impl,
    precompiles::{
        ArbPrecompileLogic, ExtendedPrecompile, StateMutability, decode_call, selector_or_revert,
    },
    state::{ArbState, ArbStateGetter, try_state, types::StorageBackedTr},
};

sol! {
///
/// @title Provides owners with tools for managing the rollup.
/// @notice Calls by non-owners will always revert.
/// Most of Arbitrum Classic's owner methods have been removed since they no longer make sense in Nitro:
/// - What were once chain parameters are now parts of ArbOS's state, and those that remain are set at genesis.
/// - ArbOS upgrades happen with the rest of the system rather than being independent
/// - Exemptions to address aliasing are no longer offered. Exemptions were intended to support backward compatibility for contracts deployed before aliasing was introduced, but no exemptions were ever requested.
/// Precompiled contract that exists in every Arbitrum chain at 0x0000000000000000000000000000000000000070.
///
///
interface ArbOwner {
    /// @notice Add account as a chain owner
    function addChainOwner(
        address newOwner
    ) external;

    /// @notice Remove account from the list of chain owners
    function removeChainOwner(
        address ownerToRemove
    ) external;

    /// @notice See if the user is a chain owner
    function isChainOwner(
        address addr
    ) external view returns (bool);

    /// @notice Retrieves the list of chain owners
    function getAllChainOwners() external view returns (address[] memory);

    /// @notice Sets the NativeTokenManagementFrom time
    /// Available in ArbOS version 41
    function setNativeTokenManagementFrom(
        uint64 timestamp
    ) external;

    /// @notice Add account as a native token owner
    /// Available in ArbOS version 41
    function addNativeTokenOwner(
        address newOwner
    ) external;

    /// @notice Remove account from the list of native token owners
    /// Available in ArbOS version 41
    function removeNativeTokenOwner(
        address ownerToRemove
    ) external;

    /// @notice See if the user is a native token owner
    /// Available in ArbOS version 41
    function isNativeTokenOwner(
        address addr
    ) external view returns (bool);

    /// @notice Retrieves the list of native token owners
    /// Available in ArbOS version 41
    function getAllNativeTokenOwners() external view returns (address[] memory);

    /// @notice Set how slowly ArbOS updates its estimate of the L1 basefee
    function setL1BaseFeeEstimateInertia(
        uint64 inertia
    ) external;

    /// @notice Set the L2 basefee directly, bypassing the pool calculus
    function setL2BaseFee(
        uint256 priceInWei
    ) external;

    /// @notice Set the minimum basefee needed for a transaction to succeed
    function setMinimumL2BaseFee(
        uint256 priceInWei
    ) external;

    /// @notice Set the computational speed limit for the chain
    function setSpeedLimit(
        uint64 limit
    ) external;

    /// @notice Set the maximum size a tx (and block) can be
    function setMaxTxGasLimit(
        uint64 limit
    ) external;

    /// @notice Set the L2 gas pricing inertia
    function setL2GasPricingInertia(
        uint64 sec
    ) external;

    /// @notice Set the L2 gas backlog tolerance
    function setL2GasBacklogTolerance(
        uint64 sec
    ) external;

    /// @notice Get the network fee collector
    function getNetworkFeeAccount() external view returns (address);

    /// @notice Get the infrastructure fee collector
    function getInfraFeeAccount() external view returns (address);

    /// @notice Set the network fee collector
    function setNetworkFeeAccount(
        address newNetworkFeeAccount
    ) external;

    /// @notice Set the infrastructure fee collector
    function setInfraFeeAccount(
        address newInfraFeeAccount
    ) external;

    /// @notice Upgrades ArbOS to the requested version at the requested timestamp
    function scheduleArbOSUpgrade(uint64 newVersion, uint64 timestamp) external;

    /// @notice Sets equilibration units parameter for L1 price adjustment algorithm
    function setL1PricingEquilibrationUnits(
        uint256 equilibrationUnits
    ) external;

    /// @notice Sets inertia parameter for L1 price adjustment algorithm
    function setL1PricingInertia(
        uint64 inertia
    ) external;

    /// @notice Sets reward recipient address for L1 price adjustment algorithm
    function setL1PricingRewardRecipient(
        address recipient
    ) external;

    /// @notice Sets reward amount for L1 price adjustment algorithm, in wei per unit
    function setL1PricingRewardRate(
        uint64 weiPerUnit
    ) external;

    /// @notice Set how much ArbOS charges per L1 gas spent on transaction data.
    function setL1PricePerUnit(
        uint256 pricePerUnit
    ) external;

    /// @notice Sets the base charge (in L1 gas) attributed to each data batch in the calldata pricer
    function setPerBatchGasCharge(
        int64 cost
    ) external;

    ///
    /// @notice Sets the Brotli compression level used for fast compression
    /// Available in ArbOS version 12 with default level as 1
    ///
    function setBrotliCompressionLevel(
        uint64 level
    ) external;

    /// @notice Sets the cost amortization cap in basis points
    function setAmortizedCostCapBips(
        uint64 cap
    ) external;

    /// @notice Releases surplus funds from L1PricerFundsPoolAddress for use
    function releaseL1PricerSurplusFunds(
        uint256 maxWeiToRelease
    ) external returns (uint256);

    /// @notice Sets the amount of ink 1 gas buys
    /// @param price the conversion rate (must fit in a uint24)
    function setInkPrice(
        uint32 price
    ) external;

    /// @notice Sets the maximum depth (in wasm words) a wasm stack may grow
    function setWasmMaxStackDepth(
        uint32 depth
    ) external;

    /// @notice Sets the number of free wasm pages a tx gets
    function setWasmFreePages(
        uint16 pages
    ) external;

    /// @notice Sets the base cost of each additional wasm page
    function setWasmPageGas(
        uint16 gas
    ) external;

    /// @notice Sets the maximum number of pages a wasm may allocate
    function setWasmPageLimit(
        uint16 limit
    ) external;

    /// @notice Sets the maximum size of the uncompressed wasm code in bytes
    function setWasmMaxSize(
        uint32 size
    ) external;

    /// @notice Sets the minimum costs to invoke a program
    /// @param gas amount of gas paid in increments of 256 when not the program is not cached
    /// @param cached amount of gas paid in increments of 64 when the program is cached
    function setWasmMinInitGas(uint8 gas, uint16 cached) external;

    /// @notice Sets the linear adjustment made to program init costs.
    /// @param percent the adjustment (100% = no adjustment).
    function setWasmInitCostScalar(
        uint64 percent
    ) external;

    /// @notice Sets the number of days after which programs deactivate
    function setWasmExpiryDays(
        uint16 _days
    ) external;

    /// @notice Sets the age a program must be to perform a keepalive
    function setWasmKeepaliveDays(
        uint16 _days
    ) external;

    /// @notice Sets the number of extra programs ArbOS caches during a given block
    function setWasmBlockCacheSize(
        uint16 count
    ) external;

    /// @notice Adds account as a wasm cache manager
    function addWasmCacheManager(
        address manager
    ) external;

    /// @notice Removes account from the list of wasm cache managers
    function removeWasmCacheManager(
        address manager
    ) external;

    /// @notice Sets serialized chain config in ArbOS state
    function setChainConfig(
        string calldata chainConfig
    ) external;

    ///
    /// @notice Sets the increased calldata price feature on or off (EIP-7623)
    /// Available in ArbOS version 40 with default as false
    ///
    function setCalldataPriceIncrease(
        bool enable
    ) external;

    /// Emitted when a successful call is made to this precompile
    event OwnerActs(bytes4 indexed method, address indexed owner, bytes data);
}
}

pub fn arb_owner_precompile<CTX: ArbitrumContextTr>() -> ExtendedPrecompile<CTX> {
    ExtendedPrecompile::new(
        PrecompileId::Custom(std::borrow::Cow::Borrowed("ArbOwner")),
        address!("0x0000000000000000000000000000000000000070"),
        precompile_impl!(ArbOwnerPrecompile),
    )
}
struct ArbOwnerPrecompile;

impl<CTX: ArbitrumContextTr> ArbPrecompileLogic<CTX> for ArbOwnerPrecompile {
    const STATE_MUT_TABLE: &'static [([u8; 4], StateMutability)] = generate_state_mut_table! {
        ArbOwner => {
            addChainOwnerCall(NonPayable),
            removeChainOwnerCall(NonPayable),
            isChainOwnerCall(View),
            getAllChainOwnersCall(View),
            setNativeTokenManagementFromCall(NonPayable),
            addNativeTokenOwnerCall(NonPayable),
            removeNativeTokenOwnerCall(NonPayable),
            isNativeTokenOwnerCall(View),
            getAllNativeTokenOwnersCall(View),
            setL1BaseFeeEstimateInertiaCall(NonPayable),
            setL2BaseFeeCall(NonPayable),
            setMinimumL2BaseFeeCall(NonPayable),
            setSpeedLimitCall(NonPayable),
            setMaxTxGasLimitCall(NonPayable),
            setL2GasPricingInertiaCall(NonPayable),
            setL2GasBacklogToleranceCall(NonPayable),
            getNetworkFeeAccountCall(View),
            getInfraFeeAccountCall(View),
            setNetworkFeeAccountCall(NonPayable),
            setInfraFeeAccountCall(NonPayable),
            scheduleArbOSUpgradeCall(NonPayable),
            setL1PricingEquilibrationUnitsCall(NonPayable),
            setL1PricingInertiaCall(NonPayable),
            setL1PricingRewardRecipientCall(NonPayable),
            setL1PricingRewardRateCall(NonPayable),
            setL1PricePerUnitCall(NonPayable),
            setPerBatchGasChargeCall(NonPayable),
            setBrotliCompressionLevelCall(NonPayable),
            setAmortizedCostCapBipsCall(NonPayable),
            releaseL1PricerSurplusFundsCall(NonPayable),
            setInkPriceCall(NonPayable),
            setWasmMaxStackDepthCall(NonPayable),
            setWasmFreePagesCall(NonPayable),
            setWasmPageGasCall(NonPayable),
            setWasmPageLimitCall(NonPayable),
            setWasmMaxSizeCall(NonPayable),
            setWasmMinInitGasCall(NonPayable),
            setWasmInitCostScalarCall(NonPayable),
            setWasmExpiryDaysCall(NonPayable),
            setWasmKeepaliveDaysCall(NonPayable),
            setWasmBlockCacheSizeCall(NonPayable),
            addWasmCacheManagerCall(NonPayable),
            removeWasmCacheManagerCall(NonPayable),
            setChainConfigCall(NonPayable),
            setCalldataPriceIncreaseCall(NonPayable),
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
        const NOT_CHAIN_OWNER: &str = "must be called by chain owner";
        let selector = selector_or_revert!(gas, input);

        match selector {
            ArbOwner::addChainOwnerCall::SELECTOR => {
                let call = decode_call!(gas, ArbOwner::addChainOwnerCall, input);
                let is_owner = try_state!(
                    gas,
                    context.arb_state(Some(&mut gas)).is_chain_owner(caller_address)
                );
                if !is_owner {
                    interpreter_revert!(gas, Bytes::from(NOT_CHAIN_OWNER));
                }
                try_state!(
                    gas,
                    context.arb_state(Some(&mut gas)).chain_owners().add(call.newOwner)
                );

                let output = ArbOwner::addChainOwnerCall::abi_encode_returns(
                    &ArbOwner::addChainOwnerReturn {},
                );

                interpreter_return!(gas, Bytes::from(output));
            }
            ArbOwner::addNativeTokenOwnerCall::SELECTOR => {
                let call = decode_call!(gas, ArbOwner::addNativeTokenOwnerCall, input);
                let is_owner = try_state!(
                    gas,
                    context.arb_state(Some(&mut gas)).is_chain_owner(caller_address)
                );
                if !is_owner {
                    interpreter_revert!(gas, Bytes::from(NOT_CHAIN_OWNER));
                }
                try_state!(
                    gas,
                    context.arb_state(Some(&mut gas)).native_token_owners().add(call.newOwner)
                );

                let output = ArbOwner::addNativeTokenOwnerCall::abi_encode_returns(
                    &ArbOwner::addNativeTokenOwnerReturn {},
                );

                interpreter_return!(gas, Bytes::from(output));
            }
            ArbOwner::addWasmCacheManagerCall::SELECTOR => {
                let call = decode_call!(gas, ArbOwner::addWasmCacheManagerCall, input);
                let is_owner = try_state!(
                    gas,
                    context.arb_state(Some(&mut gas)).is_chain_owner(caller_address)
                );
                if !is_owner {
                    interpreter_revert!(gas, Bytes::from(NOT_CHAIN_OWNER));
                }
                try_state!(
                    gas,
                    context.arb_state(Some(&mut gas)).programs().cache_managers().add(call.manager)
                );

                let output = ArbOwner::addWasmCacheManagerCall::abi_encode_returns(
                    &ArbOwner::addWasmCacheManagerReturn {},
                );

                interpreter_return!(gas, Bytes::from(output));
            }
            ArbOwner::isChainOwnerCall::SELECTOR => {
                let call = decode_call!(gas, ArbOwner::isChainOwnerCall, input);

                let is_owner =
                    try_state!(gas, context.arb_state(Some(&mut gas)).is_chain_owner(call.addr));

                let output = ArbOwner::isChainOwnerCall::abi_encode_returns(&is_owner);

                interpreter_return!(gas, Bytes::from(output));
            }
            ArbOwner::isNativeTokenOwnerCall::SELECTOR => {
                let call = decode_call!(gas, ArbOwner::isNativeTokenOwnerCall, input);

                let is_owner = try_state!(
                    gas,
                    context.arb_state(Some(&mut gas)).is_native_token_owner(call.addr)
                );

                let output = ArbOwner::isNativeTokenOwnerCall::abi_encode_returns(&is_owner);

                interpreter_return!(gas, Bytes::from(output));
            }
            ArbOwner::removeChainOwnerCall::SELECTOR => {
                let call = decode_call!(gas, ArbOwner::removeChainOwnerCall, input);
                let is_owner = try_state!(
                    gas,
                    context.arb_state(Some(&mut gas)).is_chain_owner(caller_address)
                );
                if !is_owner {
                    interpreter_revert!(gas, Bytes::from(NOT_CHAIN_OWNER));
                }
                try_state!(
                    gas,
                    context.arb_state(Some(&mut gas)).chain_owners().remove(&call.ownerToRemove)
                );

                let output = ArbOwner::removeChainOwnerCall::abi_encode_returns(
                    &ArbOwner::removeChainOwnerReturn {},
                );
                interpreter_return!(gas, Bytes::from(output));
            }
            ArbOwner::removeNativeTokenOwnerCall::SELECTOR => {
                let call = decode_call!(gas, ArbOwner::removeNativeTokenOwnerCall, input);
                let is_owner = try_state!(
                    gas,
                    context.arb_state(Some(&mut gas)).is_chain_owner(caller_address)
                );
                if !is_owner {
                    interpreter_revert!(gas, Bytes::from(NOT_CHAIN_OWNER));
                }
                try_state!(
                    gas,
                    context
                        .arb_state(Some(&mut gas))
                        .native_token_owners()
                        .remove(&call.ownerToRemove)
                );

                let output = ArbOwner::removeNativeTokenOwnerCall::abi_encode_returns(
                    &ArbOwner::removeNativeTokenOwnerReturn {},
                );
                interpreter_return!(gas, Bytes::from(output));
            }
            ArbOwner::removeWasmCacheManagerCall::SELECTOR => {
                let call = decode_call!(gas, ArbOwner::removeWasmCacheManagerCall, input);
                let is_owner = try_state!(
                    gas,
                    context.arb_state(Some(&mut gas)).is_chain_owner(caller_address)
                );
                if !is_owner {
                    interpreter_revert!(gas, Bytes::from(NOT_CHAIN_OWNER));
                }
                try_state!(
                    gas,
                    context
                        .arb_state(Some(&mut gas))
                        .programs()
                        .cache_managers()
                        .remove(&call.manager)
                );

                let output = ArbOwner::removeWasmCacheManagerCall::abi_encode_returns(
                    &ArbOwner::removeWasmCacheManagerReturn {},
                );
                interpreter_return!(gas, Bytes::from(output));
            }
            ArbOwner::getAllChainOwnersCall::SELECTOR => {
                let _ = decode_call!(gas, ArbOwner::getAllChainOwnersCall, input);
                let chains_owners =
                    try_state!(gas, context.arb_state(Some(&mut gas)).chain_owners().all());

                let output = ArbOwner::getAllChainOwnersCall::abi_encode_returns(&chains_owners);

                interpreter_return!(gas, Bytes::from(output));
            }
            ArbOwner::getAllNativeTokenOwnersCall::SELECTOR => {
                let _ = decode_call!(gas, ArbOwner::getAllNativeTokenOwnersCall, input);
                let native_token_owners =
                    try_state!(gas, context.arb_state(Some(&mut gas)).native_token_owners().all());

                let output =
                    ArbOwner::getAllNativeTokenOwnersCall::abi_encode_returns(&native_token_owners);

                interpreter_return!(gas, Bytes::from(output));
            }
            ArbOwner::setCalldataPriceIncreaseCall::SELECTOR => {
                let call = decode_call!(gas, ArbOwner::setCalldataPriceIncreaseCall, input);

                let is_owner = try_state!(
                    gas,
                    context.arb_state(Some(&mut gas)).is_chain_owner(caller_address)
                );
                if !is_owner {
                    interpreter_revert!(gas, Bytes::from(NOT_CHAIN_OWNER));
                }

                let mut arb_state = context.arb_state(Some(&mut gas));
                let mut l1_pricing = arb_state.l1_pricing();
                try_state!(
                    gas,
                    l1_pricing.gas_floor_per_token().set(if call.enable { 1 } else { 0 })
                );

                interpreter_return!(gas, Bytes::new());
            }
            _ => interpreter_revert!(gas, Bytes::from("Unknown selector")),
        }
    }
}
