use alloy_sol_types::{SolCall, SolError, sol};
use revm::{
    context::JournalTr,
    interpreter::{Gas, InterpreterResult},
    precompile::PrecompileId,
    primitives::{Address, Bytes, Log, U256, address, alloy_primitives::IntoLogData},
};

use crate::{
    ArbitrumContextTr, generate_state_mut_table, precompile_impl,
    precompiles::{
         ArbPrecompileLogic, ExtendedPrecompile,
        macros::{StateMutability, emit_event, return_revert, return_success, try_state},
    },
    record_cost,
    state::{
        ArbState, ArbStateGetter,
        types::{ArbosStateError, StorageBackedTr},
    },
};

sol! {

///
/// @title Methods for managing Stylus caches
/// @notice Precompiled contract that exists in every Arbitrum chain at 0x0000000000000000000000000000000000000072.
/// @notice Available in ArbOS version 30 and above
///
interface IArbWasmCache {
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
        precompile_impl!(ArbWasmCache),
    )
}

struct ArbWasmCache {}

impl<CTX: ArbitrumContextTr> ArbPrecompileLogic<CTX> for ArbWasmCache {
    const STATE_MUT_TABLE: &'static [([u8; 4], StateMutability)] = generate_state_mut_table! {
        IArbWasmCache => {
            isCacheManagerCall(View),
            allCacheManagersCall(View),
            cacheCodehashCall(NonPayable),
            cacheProgramCall(NonPayable),
            evictCodehashCall(NonPayable),
            codehashIsCachedCall(View),
        }
    };

    fn inner(
        context: &mut CTX,
        input: &[u8],
        target_address: &Address,
        caller_address: Address,
        _call_value: U256,
        _is_static: bool,
        gas_limit: u64,
    ) -> InterpreterResult {
        let mut gas = Gas::new(gas_limit);

        // decode selector
        if input.len() < 4 {
            gas.spend_all();
            return_revert!(gas, Bytes::from("Input too short"));
        }

        // decode selector
        let selector: [u8; 4] = input[0..4].try_into().unwrap();

        match selector {
            IArbWasmCache::isCacheManagerCall::SELECTOR => {
                let call = IArbWasmCache::isCacheManagerCall::abi_decode(input).unwrap();
                let manager = call.manager;

                let is_manager = try_state!(
                    gas,
                    context.arb_state(Some(&mut gas)).cache_managers().contains(manager)
                );

                let output = IArbWasmCache::isCacheManagerCall::abi_encode_returns(&is_manager);

                return_success!(gas, Bytes::from(output));
            }
            IArbWasmCache::allCacheManagersCall::SELECTOR => {
                let _call = IArbWasmCache::allCacheManagersCall::abi_decode(input).unwrap();

                let managers =
                    try_state!(gas, context.arb_state(Some(&mut gas)).cache_managers().all());

                let output = IArbWasmCache::allCacheManagersCall::abi_encode_returns(&managers);

                return_success!(gas, Bytes::from(output));
            }
            IArbWasmCache::cacheCodehashCall::SELECTOR => {
                if !try_state!(gas, has_access(context, caller_address, &mut gas)) {
                    return_revert!(gas);
                }

                let call = IArbWasmCache::cacheCodehashCall::abi_decode(input).unwrap();
                let codehash = call.codehash;

                let params = try_state!(
                    gas,
                    context.arb_state(Some(&mut gas)).programs().get_stylus_params()
                );

                let mut program_info = if let Some(program_info) = try_state!(
                    gas,
                    context.arb_state(Some(&mut gas)).programs().program_info(&codehash)
                ) {
                    program_info
                } else {
                    return_revert!(
                        gas,
                        IArbWasmCache::ProgramNeedsUpgrade {
                            version: 0,
                            stylusVersion: params.version
                        }
                        .abi_encode()
                    );
                };

                let output = IArbWasmCache::cacheCodehashCall::abi_encode_returns(
                    &IArbWasmCache::cacheCodehashReturn {},
                );

                if program_info.cached {
                    // already cached, no-op
                    return_success!(gas, Bytes::from(output));
                }

                // TODO: burn cache cost
                program_info.cached = true;

                try_state!(
                    gas,
                    context
                        .arb_state(Some(&mut gas))
                        .programs()
                        .save_program_info(&codehash, &program_info)
                );

                return_success!(gas, Bytes::from(output));
            }
            IArbWasmCache::cacheProgramCall::SELECTOR => {
                if !try_state!(gas, has_access(context, caller_address, &mut gas)) {
                    return_revert!(gas);
                }

                let call = IArbWasmCache::cacheProgramCall::abi_decode(input).unwrap();
                let addr = call.addr;

                let params = try_state!(
                    gas,
                    context.arb_state(Some(&mut gas)).programs().get_stylus_params()
                );

                let code_hash = try_state!(gas, context.arb_state(Some(&mut gas)).code_hash(addr));

                let mut program_info = try_state!(
                    gas,
                    context.arb_state(Some(&mut gas)).programs().program_info(&code_hash)
                )
                .unwrap_or_default();

                if program_info.version != params.version {
                    return_revert!(
                        gas,
                        IArbWasmCache::ProgramNeedsUpgrade {
                            version: program_info.version,
                            stylusVersion: params.version
                        }
                        .abi_encode()
                    );
                }

                if program_info.age > params.expiry_days as u32 * 86400 {
                    return_revert!(
                        gas,
                        IArbWasmCache::ProgramExpired { ageInSeconds: program_info.age as u64 }
                            .abi_encode()
                    );
                }

                let output = IArbWasmCache::cacheProgramCall::abi_encode_returns(
                    &IArbWasmCache::cacheProgramReturn {},
                );

                if program_info.cached {
                    // already cached, no-op
                    return_success!(gas, Bytes::from(output));
                }

                // emit event cost
                emit_event!(
                    context,
                    Log {
                        address: *target_address,
                        data: IArbWasmCache::UpdateProgramCache {
                            manager: caller_address,
                            codehash: code_hash,
                            cached: true
                        }
                        .into_log_data()
                    },
                    gas
                );

                record_cost!(gas, program_info.init_cost as u64);

                try_state!(
                    gas,
                    context.arb_state(Some(&mut gas)).programs().module_hash(&code_hash).get()
                );

                program_info.cached = true;

                try_state!(
                    gas,
                    context
                        .arb_state(Some(&mut gas))
                        .programs()
                        .save_program_info(&code_hash, &program_info)
                );

                return_success!(gas, Bytes::from(output));
            }
            IArbWasmCache::evictCodehashCall::SELECTOR => {
                if !try_state!(gas, has_access(context, caller_address, &mut gas)) {
                    return_revert!(gas);
                }

                let call = IArbWasmCache::evictCodehashCall::abi_decode(input).unwrap();
                let code_hash = call.codehash;

                let _ = try_state!(
                    gas,
                    context.arb_state(Some(&mut gas)).programs().get_stylus_params()
                );

                let mut program_info = try_state!(
                    gas,
                    context.arb_state(Some(&mut gas)).programs().program_info(&code_hash)
                )
                .unwrap_or_default();

                let output = IArbWasmCache::evictCodehashCall::abi_encode_returns(
                    &IArbWasmCache::evictCodehashReturn {},
                );

                if !program_info.cached {
                    // if not cached, no-op
                    return_success!(gas, Bytes::from(output));
                }

                // emit event cost
                emit_event!(
                    context,
                    Log {
                        address: *target_address,
                        data: IArbWasmCache::UpdateProgramCache {
                            manager: caller_address,
                            codehash: code_hash,
                            cached: false
                        }
                        .into_log_data()
                    },
                    gas
                );

                record_cost!(gas, program_info.init_cost as u64);

                try_state!(
                    gas,
                    context.arb_state(Some(&mut gas)).programs().module_hash(&code_hash).get()
                );

                program_info.cached = false;

                try_state!(
                    gas,
                    context
                        .arb_state(Some(&mut gas))
                        .programs()
                        .save_program_info(&code_hash, &program_info)
                );

                return_success!(gas, Bytes::from(output));
            }
            IArbWasmCache::codehashIsCachedCall::SELECTOR => {
                let call = IArbWasmCache::codehashIsCachedCall::abi_decode(input).unwrap();
                let codehash = call.codehash;

                let is_cached = if let Some(program_info) = try_state!(
                    gas,
                    context.arb_state(Some(&mut gas)).programs().program_info(&codehash)
                ) {
                    program_info.cached
                } else {
                    false
                };

                let output = IArbWasmCache::codehashIsCachedCall::abi_encode_returns(&is_cached);

                return_success!(gas, Bytes::from(output));
            }
            _ => return_revert!(gas, Bytes::from("Unknown selector")),
        }
    }
}

fn has_access<CTX: ArbitrumContextTr>(
    context: &mut CTX,
    caller: Address,
    gas: &mut Gas,
) -> Result<bool, ArbosStateError> {
    let mut arb_state = context.arb_state(Some(gas));
    if arb_state.cache_managers().contains(caller)? {
        return Ok(true);
    }

    arb_state.is_chain_owner(caller)
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use alloy_sol_types::SolCall;
    use revm::{
        Journal,
        context::{BlockEnv, JournalTr, TxEnv},
        database::EmptyDBTyped,
        primitives::{U256, address},
    };

    use crate::{
        ArbitrumContext,
        config::ArbitrumConfig,
        local_context::ArbitrumLocalContext,
        precompiles::{ArbPrecompileLogic, arb_wasm_cache::ArbWasmCache},
    };

    fn setup() -> ArbitrumContext<EmptyDBTyped<Infallible>> {
        let db = EmptyDBTyped::<Infallible>::default();

        let context = ArbitrumContext {
            journaled_state: {
                let journal = Journal::new(db);
                journal
            },
            block: BlockEnv::default(),
            cfg: ArbitrumConfig::default(),
            tx: TxEnv::default(),
            chain: (),
            local: ArbitrumLocalContext::default(),
            error: Ok(()),
        };

        context
    }

    #[test]
    fn test_wasm_cache_code_hash_is_cached() {
        let mut context = setup();

        let codehash = [0u8; 32];

        let input =
            crate::precompiles::arb_wasm_cache::IArbWasmCache::codehashIsCachedCall::abi_encode(
                &crate::precompiles::arb_wasm_cache::IArbWasmCache::codehashIsCachedCall {
                    codehash: codehash.into(),
                },
            );

        let result = ArbWasmCache::run(
            &mut context,
            &input,
            &crate::precompiles::arb_wasm_cache::arb_wasm_cache_precompile::<
                ArbitrumContext<EmptyDBTyped<Infallible>>,
            >()
            .address,
            address!("0x0000000000000000000000000000000000000001"),
            U256::ZERO,
            true,
            1_000_000,
        )
        .unwrap();

        assert!(result.is_some());
        let result = result.unwrap();
        let output = result.output;
        let decoded = crate::precompiles::arb_wasm_cache::IArbWasmCache::codehashIsCachedCall::abi_decode_returns(&output).unwrap();
        assert_eq!(decoded, false);

        // gas cost should be 1606
        assert_eq!(result.gas.spent(), 1606);
    }
}
