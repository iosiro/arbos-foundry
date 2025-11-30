use alloy_sol_types::{SolCall, sol};
use revm::{
    context::JournalTr,
    interpreter::{Gas, InterpreterResult},
    precompile::PrecompileId,
    primitives::{Address, Bytes, Log, U256, address, alloy_primitives::IntoLogData},
};

use crate::{
    ArbitrumContextTr,
    config::ArbitrumConfigTr,
    constants::{ARBOS_L1_PRICER_FUNDS_ADDRESS, COST_SCALAR_PERCENT},
    generate_state_mut_table,
    macros::{emit_event, interpreter_return, interpreter_revert},
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

macro_rules! require_chain_owner {
    ($context:expr, $gas:expr, $caller:expr) => {
        if let Some(outcome) =
            crate::precompiles::arb_owner::require_chain_owner($context, &mut $gas, $caller)
        {
            return Some(outcome);
        }
    };
}

pub(crate) fn require_chain_owner<CTX: ArbitrumContextTr>(
    context: &mut CTX,
    mut gas: &mut Gas,
    caller: Address,
) -> Option<InterpreterResult> {
    let is_owner = try_state!(gas, context.arb_state(Some(gas), true).is_chain_owner(caller));
    if !is_owner {
        const NOT_CHAIN_OWNER: &str = "unauthorized caller to access-controlled method";
        return Some(crate::macros::interpreter_result_revert_with_output(
            gas,
            NOT_CHAIN_OWNER.into(),
        ));
    }
    None
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
        _target_address: &Address,
        caller_address: Address,
        _call_value: U256,
        is_static: bool,
        gas_limit: u64,
    ) -> Option<InterpreterResult> {
        let mut gas = Gas::new(gas_limit);

        let selector = selector_or_revert!(gas, input);

        require_chain_owner!(context, gas, caller_address);

        fn run_arbos_owner<CTX: ArbitrumContextTr>(
            context: &mut CTX,
            selector: [u8; 4],
            input: &[u8],
            is_static: bool,
            mut gas: Gas,
        ) -> Option<InterpreterResult> {
            match selector {
                ArbOwner::addChainOwnerCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::addChainOwnerCall, input);

                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .chain_owners()
                            .add(call.newOwner)
                    );

                    let output = ArbOwner::addChainOwnerCall::abi_encode_returns(
                        &ArbOwner::addChainOwnerReturn {},
                    );

                    interpreter_return!(gas, Bytes::from(output));
                }
                ArbOwner::addNativeTokenOwnerCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::addNativeTokenOwnerCall, input);

                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .native_token_owners()
                            .add(call.newOwner)
                    );

                    let output = ArbOwner::addNativeTokenOwnerCall::abi_encode_returns(
                        &ArbOwner::addNativeTokenOwnerReturn {},
                    );

                    interpreter_return!(gas, Bytes::from(output));
                }
                ArbOwner::addWasmCacheManagerCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::addWasmCacheManagerCall, input);

                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .programs()
                            .cache_managers()
                            .add(call.manager)
                    );

                    let output = ArbOwner::addWasmCacheManagerCall::abi_encode_returns(
                        &ArbOwner::addWasmCacheManagerReturn {},
                    );

                    interpreter_return!(gas, Bytes::from(output));
                }
                ArbOwner::isChainOwnerCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::isChainOwnerCall, input);

                    let is_owner = try_state!(
                        gas,
                        context.arb_state(Some(&mut gas), is_static).is_chain_owner(call.addr)
                    );

                    let output = ArbOwner::isChainOwnerCall::abi_encode_returns(&is_owner);

                    interpreter_return!(gas, Bytes::from(output));
                }
                ArbOwner::isNativeTokenOwnerCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::isNativeTokenOwnerCall, input);

                    let is_owner = try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .is_native_token_owner(call.addr)
                    );

                    let output = ArbOwner::isNativeTokenOwnerCall::abi_encode_returns(&is_owner);

                    interpreter_return!(gas, Bytes::from(output));
                }
                ArbOwner::removeChainOwnerCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::removeChainOwnerCall, input);

                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .chain_owners()
                            .remove(&call.ownerToRemove)
                    );

                    let output = ArbOwner::removeChainOwnerCall::abi_encode_returns(
                        &ArbOwner::removeChainOwnerReturn {},
                    );
                    interpreter_return!(gas, Bytes::from(output));
                }
                ArbOwner::removeNativeTokenOwnerCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::removeNativeTokenOwnerCall, input);

                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
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

                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
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
                    let chains_owners = try_state!(
                        gas,
                        context.arb_state(Some(&mut gas), is_static).chain_owners().all()
                    );

                    let output =
                        ArbOwner::getAllChainOwnersCall::abi_encode_returns(&chains_owners);

                    interpreter_return!(gas, Bytes::from(output));
                }
                ArbOwner::getAllNativeTokenOwnersCall::SELECTOR => {
                    let _ = decode_call!(gas, ArbOwner::getAllNativeTokenOwnersCall, input);
                    let native_token_owners = try_state!(
                        gas,
                        context.arb_state(Some(&mut gas), is_static).native_token_owners().all()
                    );

                    let output = ArbOwner::getAllNativeTokenOwnersCall::abi_encode_returns(
                        &native_token_owners,
                    );

                    interpreter_return!(gas, Bytes::from(output));
                }
                ArbOwner::setCalldataPriceIncreaseCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setCalldataPriceIncreaseCall, input);

                    let mut arb_state = context.arb_state(Some(&mut gas), is_static);
                    let mut l1_pricing = arb_state.l1_pricing();
                    try_state!(
                        gas,
                        l1_pricing.gas_floor_per_token().set(if call.enable { 1 } else { 0 })
                    );

                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setNativeTokenManagementFromCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setNativeTokenManagementFromCall, input);

                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .native_token_enabled_time()
                            .set(call.timestamp)
                    );

                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setL1BaseFeeEstimateInertiaCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setL1BaseFeeEstimateInertiaCall, input);

                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .l1_pricing()
                            .inertia()
                            .set(call.inertia)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setL2BaseFeeCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setL2BaseFeeCall, input);

                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .l2_pricing()
                            .base_fee_wei()
                            .set(call.priceInWei)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setMinimumL2BaseFeeCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setMinimumL2BaseFeeCall, input);

                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .l2_pricing()
                            .min_base_fee_wei()
                            .set(call.priceInWei)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setSpeedLimitCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setSpeedLimitCall, input);

                    if call.limit == 0 {
                        interpreter_revert!(gas, Bytes::from("speed limit must be nonzero"));
                    }
                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .l2_pricing()
                            .speed_limit_per_second()
                            .set(call.limit)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setMaxTxGasLimitCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setMaxTxGasLimitCall, input);

                    if context.cfg().arbos_version() < 50 {
                        try_state!(
                            gas,
                            context
                                .arb_state(Some(&mut gas), is_static)
                                .l2_pricing()
                                .per_block_gas_limit()
                                .set(call.limit)
                        );
                    } else {
                        try_state!(
                            gas,
                            context
                                .arb_state(Some(&mut gas), is_static)
                                .l2_pricing()
                                .per_tx_gas_limit()
                                .set(call.limit)
                        );
                    }

                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setL2GasPricingInertiaCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setL2GasPricingInertiaCall, input);

                    if call.sec == 0 {
                        interpreter_revert!(gas, Bytes::from("price inertia must be nonzero"));
                    }
                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .l2_pricing()
                            .pricing_inertia()
                            .set(call.sec)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setL2GasBacklogToleranceCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setL2GasBacklogToleranceCall, input);

                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .l2_pricing()
                            .backlog_tolerance()
                            .set(call.sec)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::getNetworkFeeAccountCall::SELECTOR => {
                    let _ = decode_call!(gas, ArbOwner::getNetworkFeeAccountCall, input);
                    let network_fee_account = try_state!(
                        gas,
                        context.arb_state(Some(&mut gas), is_static).network_fee_account().get()
                    );
                    let output = ArbOwner::getNetworkFeeAccountCall::abi_encode_returns(
                        &network_fee_account,
                    );
                    interpreter_return!(gas, Bytes::from(output));
                }
                ArbOwner::getInfraFeeAccountCall::SELECTOR => {
                    let _ = decode_call!(gas, ArbOwner::getInfraFeeAccountCall, input);
                    let infra_fee_account = try_state!(
                        gas,
                        context.arb_state(Some(&mut gas), is_static).infra_fee_account().get()
                    );
                    let output =
                        ArbOwner::getInfraFeeAccountCall::abi_encode_returns(&infra_fee_account);
                    interpreter_return!(gas, Bytes::from(output));
                }
                ArbOwner::setNetworkFeeAccountCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setNetworkFeeAccountCall, input);

                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .network_fee_account()
                            .set(call.newNetworkFeeAccount)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setInfraFeeAccountCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setInfraFeeAccountCall, input);

                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .infra_fee_account()
                            .set(call.newInfraFeeAccount)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::scheduleArbOSUpgradeCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::scheduleArbOSUpgradeCall, input);

                    let mut arb_state = context.arb_state(Some(&mut gas), is_static);
                    try_state!(gas, arb_state.upgrade_version().set(call.newVersion));
                    try_state!(gas, arb_state.upgrade_timestamp().set(call.timestamp));
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setL1PricingEquilibrationUnitsCall::SELECTOR => {
                    let call =
                        decode_call!(gas, ArbOwner::setL1PricingEquilibrationUnitsCall, input);

                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .l1_pricing()
                            .equilibration_units()
                            .set(call.equilibrationUnits)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setL1PricingInertiaCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setL1PricingInertiaCall, input);

                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .l1_pricing()
                            .inertia()
                            .set(call.inertia)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setL1PricingRewardRecipientCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setL1PricingRewardRecipientCall, input);

                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .l1_pricing()
                            .reward_recipient()
                            .set(call.recipient)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setL1PricingRewardRateCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setL1PricingRewardRateCall, input);

                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .l1_pricing()
                            .per_unit_reward()
                            .set(call.weiPerUnit)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setL1PricePerUnitCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setL1PricePerUnitCall, input);

                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .l1_pricing()
                            .price_per_unit()
                            .set(call.pricePerUnit)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setPerBatchGasChargeCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setPerBatchGasChargeCall, input);

                    if call.cost < 0 {
                        interpreter_revert!(gas, Bytes::from("negative cost not allowed"));
                    }
                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .l1_pricing()
                            .per_batch_gas_cost()
                            .set(call.cost as u64)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setBrotliCompressionLevelCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setBrotliCompressionLevelCall, input);

                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .brotli_compression_level()
                            .set(call.level)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setAmortizedCostCapBipsCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setAmortizedCostCapBipsCall, input);

                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .l1_pricing()
                            .amortized_cost_cap_bips()
                            .set(call.cap)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::releaseL1PricerSurplusFundsCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::releaseL1PricerSurplusFundsCall, input);

                    let balance =
                        context.balance(ARBOS_L1_PRICER_FUNDS_ADDRESS).unwrap_or_default().data;
                    let mut arb_state = context.arb_state(Some(&mut gas), is_static);
                    let mut l1_pricing = arb_state.l1_pricing();
                    let recognized = try_state!(gas, l1_pricing.l1_fees_available().get());

                    let transferable =
                        if balance > recognized { balance - recognized } else { U256::ZERO };
                    let amount = transferable.min(call.maxWeiToRelease);

                    if amount > U256::ZERO {
                        let new_total = recognized.saturating_add(amount);
                        try_state!(gas, l1_pricing.l1_fees_available().set(new_total));
                    }

                    let output =
                        ArbOwner::releaseL1PricerSurplusFundsCall::abi_encode_returns(&amount);
                    interpreter_return!(gas, Bytes::from(output));
                }
                ArbOwner::setInkPriceCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setInkPriceCall, input);

                    if call.price == 0 || call.price > 0xFFFFFF {
                        interpreter_revert!(
                            gas,
                            Bytes::from("ink price must be a positive uint24")
                        );
                    }

                    let mut params = try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .programs()
                            .stylus_params()
                            .get()
                    );
                    params.ink_price = call.price;
                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .programs()
                            .stylus_params()
                            .set(&params)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setWasmMaxStackDepthCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setWasmMaxStackDepthCall, input);

                    let mut params = try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .programs()
                            .stylus_params()
                            .get()
                    );
                    params.max_stack_depth = call.depth;
                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .programs()
                            .stylus_params()
                            .set(&params)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setWasmFreePagesCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setWasmFreePagesCall, input);

                    let mut params = try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .programs()
                            .stylus_params()
                            .get()
                    );
                    params.free_pages = call.pages;
                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .programs()
                            .stylus_params()
                            .set(&params)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setWasmPageGasCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setWasmPageGasCall, input);

                    let mut params = try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .programs()
                            .stylus_params()
                            .get()
                    );
                    params.page_gas = call.gas;
                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .programs()
                            .stylus_params()
                            .set(&params)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setWasmPageLimitCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setWasmPageLimitCall, input);

                    let mut params = try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .programs()
                            .stylus_params()
                            .get()
                    );
                    params.page_limit = call.limit;
                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .programs()
                            .stylus_params()
                            .set(&params)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setWasmMaxSizeCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setWasmMaxSizeCall, input);

                    let mut params = try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .programs()
                            .stylus_params()
                            .get()
                    );
                    params.max_wasm_size = call.size;
                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .programs()
                            .stylus_params()
                            .set(&params)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setWasmMinInitGasCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setWasmMinInitGasCall, input);

                    if call.cached > u16::from(u8::MAX) {
                        interpreter_revert!(gas, Bytes::from("cached gas too large"));
                    }
                    let mut params = try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .programs()
                            .stylus_params()
                            .get()
                    );
                    params.min_init_gas = call.gas;
                    params.min_cached_init_gas = call.cached as u8;
                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .programs()
                            .stylus_params()
                            .set(&params)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setWasmInitCostScalarCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setWasmInitCostScalarCall, input);

                    if call.percent == 0 {
                        interpreter_revert!(gas, Bytes::from("init cost scalar must be nonzero"));
                    }
                    let mut params = try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .programs()
                            .stylus_params()
                            .get()
                    );
                    let scaled = call
                        .percent
                        .saturating_add(COST_SCALAR_PERCENT - 1)
                        .saturating_div(COST_SCALAR_PERCENT);
                    params.init_cost_scalar = scaled.min(u8::MAX as u64) as u8;
                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .programs()
                            .stylus_params()
                            .set(&params)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setWasmExpiryDaysCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setWasmExpiryDaysCall, input);

                    let mut params = try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .programs()
                            .stylus_params()
                            .get()
                    );

                    params.expiry_days = call._days;
                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .programs()
                            .stylus_params()
                            .set(&params)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setWasmKeepaliveDaysCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setWasmKeepaliveDaysCall, input);

                    let mut params = try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .programs()
                            .stylus_params()
                            .get()
                    );

                    params.keepalive_days = call._days;
                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .programs()
                            .stylus_params()
                            .set(&params)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setWasmBlockCacheSizeCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setWasmBlockCacheSizeCall, input);

                    let mut params = try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .programs()
                            .stylus_params()
                            .get()
                    );
                    params.block_cache_size = call.count;
                    try_state!(
                        gas,
                        context
                            .arb_state(Some(&mut gas), is_static)
                            .programs()
                            .stylus_params()
                            .set(&params)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                ArbOwner::setChainConfigCall::SELECTOR => {
                    let call = decode_call!(gas, ArbOwner::setChainConfigCall, input);

                    let hash = revm::primitives::keccak256(&call.chainConfig);
                    let value = U256::from_be_bytes(hash.0);
                    try_state!(
                        gas,
                        context.arb_state(Some(&mut gas), is_static).chain_config().set(value)
                    );
                    interpreter_return!(gas, Bytes::new());
                }
                _ => interpreter_revert!(gas, Bytes::from("Unknown selector")),
            }
        }

        let result = run_arbos_owner(context, selector, input, is_static, gas);

        emit_event!(
            context,
            Log {
                address: address!("0x0000000000000000000000000000000000000070"),
                data: ArbOwner::OwnerActs {
                    method: selector.into(),
                    owner: caller_address,
                    data: Bytes::from(input[4..].to_vec())
                }
                .to_log_data(),
            },
            gas
        );

        result
    }
}
