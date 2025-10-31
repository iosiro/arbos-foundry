use alloy_sol_types::{SolCall, SolError, sol};
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

/**
 * @title Methods for managing Stylus caches
 * @notice Precompiled contract that exists in every Arbitrum chain at 0x0000000000000000000000000000000000000072.
 * @notice Available in ArbOS version 30 and above
 */
interface ArbWasmCache {
    /// @notice See if the user is a cache manager.
    function isCacheManager(
        address manager
    ) external view returns (bool);

    /// @notice Retrieve all address managers.
    /// @return managers the list of managers.
    function allCacheManagers() external view returns (address[] memory managers);

    /// @dev Deprecated, replaced with cacheProgram
    /// @notice Available in ArbOS version 30 only
    function cacheCodehash(
        bytes32 codehash
    ) external;

    /// @notice Caches all programs with a codehash equal to the given address.
    /// @notice Reverts if the programs have expired.
    /// @notice Caller must be a cache manager or chain owner.
    /// @notice If you're looking for how to bid for position, interact with the chain's cache manager contract.
    /// @notice Available in ArbOS version 31 and above
    function cacheProgram(
        address addr
    ) external;

    /// @notice Evicts all programs with the given codehash.
    /// @notice Caller must be a cache manager or chain owner.
    function evictCodehash(
        bytes32 codehash
    ) external;

    /// @notice Gets whether a program is cached. Note that the program may be expired.
    function codehashIsCached(
        bytes32 codehash
    ) external view returns (bool);

    event UpdateProgramCache(address indexed manager, bytes32 indexed codehash, bool cached);

    /// @notice Reverts if the program is expired
    error ProgramNeedsUpgrade(uint16 version, uint16 stylusVersion);
    /// @notice Reverts if the program is too large
    error ProgramExpired(uint64 ageInSeconds);
}

}

pub fn arb_wasm_cache_precompile<CTX: ArbitrumContextTr>() -> ExtendedPrecompile<CTX> {
    ExtendedPrecompile::new(
        PrecompileId::Custom(std::borrow::Cow::Borrowed("ArbWasmCache")),
        address!("0x0000000000000000000000000000000000000072"),
        arbos_wasm_cache_run::<CTX>,
    )
}
/// Run the precompile with the given context and input data.
fn arbos_wasm_cache_run<CTX: ArbitrumContextTr>(
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
        ArbWasmCache::isCacheManagerCall::SELECTOR => {
            let call = ArbWasmCache::isCacheManagerCall::abi_decode(input).unwrap();
            let manager = call.manager;

            let is_manager = context.arb_state().programs().cache_managers().contains(&manager);

            let output = ArbWasmCache::isCacheManagerCall::abi_encode_returns(&is_manager);

            return_success!(gas, Bytes::from(output));
        }
        ArbWasmCache::allCacheManagersCall::SELECTOR => {
            let _call = ArbWasmCache::allCacheManagersCall::abi_decode(input).unwrap();

            let managers = context.arb_state().programs().cache_managers().all();

            let output = ArbWasmCache::allCacheManagersCall::abi_encode_returns(&managers);

            return_success!(gas, Bytes::from(output));
        }
        ArbWasmCache::cacheCodehashCall::SELECTOR => {
            if !has_access(context) {
                return_revert!(gas);
            }

            let call = ArbWasmCache::cacheCodehashCall::abi_decode(input).unwrap();
            let codehash = call.codehash;

            let (params, _) = context.arb_state().programs().get_stylus_params();

            let mut program_info = if let Some(program_info) =
                context.arb_state().programs().program_info(&codehash)
            {
                program_info
            } else {
                return_revert!(
                    gas,
                    ArbWasmCache::ProgramNeedsUpgrade { version: 0, stylusVersion: params.version }
                        .abi_encode()
                );
            };

            let output = ArbWasmCache::cacheCodehashCall::abi_encode_returns(
                &ArbWasmCache::cacheCodehashReturn {},
            );

            if program_info.cached {
                // already cached, no-op
                return_success!(gas, Bytes::from(output));
            }

            // TODO: burn cache cost
            program_info.cached = true;

            context.arb_state().programs().save_program_info(&codehash, &program_info);

            return_success!(gas, Bytes::from(output));
        }
        ArbWasmCache::cacheProgramCall::SELECTOR => {
            if !has_access(context) {
                return_revert!(gas);
            }

            let call = ArbWasmCache::cacheProgramCall::abi_decode(input).unwrap();
            let addr = call.addr;

            let (params, _) = context.arb_state().programs().get_stylus_params();

            let code_hash = if let Some(code_hash) = context.load_account_code_hash(addr) {
                code_hash.data
            } else {
                return_revert!(
                    gas,
                    ArbWasmCache::ProgramNeedsUpgrade { version: 0, stylusVersion: params.version }
                        .abi_encode()
                );
            };

            let mut program_info = if let Some(program_info) =
                context.arb_state().programs().program_info(&code_hash)
            {
                program_info
            } else {
                return_revert!(
                    gas,
                    ArbWasmCache::ProgramNeedsUpgrade { version: 0, stylusVersion: params.version }
                        .abi_encode()
                );
            };

            let output = ArbWasmCache::cacheProgramCall::abi_encode_returns(
                &ArbWasmCache::cacheProgramReturn {},
            );

            if program_info.cached {
                // already cached, no-op
                return_success!(gas, Bytes::from(output));
            }

            // TODO: burn cache cost

            program_info.cached = true;

            context.arb_state().programs().save_program_info(&code_hash, &program_info);

            return_success!(gas, Bytes::from(output));
        }
        ArbWasmCache::evictCodehashCall::SELECTOR => {
            if !has_access(context) {
                return_revert!(gas);
            }

            let call = ArbWasmCache::evictCodehashCall::abi_decode(input).unwrap();
            let codehash = call.codehash;

            let (params, _) = context.arb_state().programs().get_stylus_params();

            let mut program_info = if let Some(program_info) =
                context.arb_state().programs().program_info(&codehash)
            {
                program_info
            } else {
                return_revert!(
                    gas,
                    ArbWasmCache::ProgramNeedsUpgrade { version: 0, stylusVersion: params.version }
                        .abi_encode()
                );
            };

            let output = ArbWasmCache::evictCodehashCall::abi_encode_returns(
                &ArbWasmCache::evictCodehashReturn {},
            );

            if !program_info.cached {
                // already not cached, no-op
                return_success!(gas, Bytes::from(output));
            }

            program_info.cached = false;

            context.arb_state().programs().save_program_info(&codehash, &program_info);

            let output = ArbWasmCache::evictCodehashCall::abi_encode_returns(
                &ArbWasmCache::evictCodehashReturn {},
            );

            return_success!(gas, Bytes::from(output));
        }
        ArbWasmCache::codehashIsCachedCall::SELECTOR => {
            let call = ArbWasmCache::codehashIsCachedCall::abi_decode(input).unwrap();
            let codehash = call.codehash;

            let is_cached = if let Some(program_info) =
                context.arb_state().programs().program_info(&codehash)
            {
                program_info.cached
            } else {
                false
            };

            let output = ArbWasmCache::codehashIsCachedCall::abi_encode_returns(&is_cached);

            return_success!(gas, Bytes::from(output));
        }
        _ => return_revert!(gas, Bytes::from("Unknown selector")),
    }
}

fn has_access<CTX: ArbitrumContextTr>(context: &mut CTX) -> bool {
    let caller = context.caller();
    let is_cache_manager = context.arb_state().programs().cache_managers().contains(&caller);

    is_cache_manager || context.arb_state().chain_owners().contains(&caller)
}
