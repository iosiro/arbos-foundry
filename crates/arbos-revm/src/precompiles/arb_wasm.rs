#![allow(missing_docs)]

use crate::{
    ArbitrumContextTr, config::{ArbitrumConfigTr, ArbitrumStylusConfigTr}, constants::{
        COST_SCALAR_PERCENT, MIN_CACHED_GAS_UNITS, MIN_INIT_GAS_UNITS, STYLUS_DISCRIMINANT,
    }, local_context::ArbitrumLocalContextTr, precompiles::{
        ExtendedPrecompile,
        macros::{emit_event, return_revert, return_success, try_state},
    }, record_cost, state::{
        ArbState, ArbStateGetter,
        program::{ProgramInfo, StylusParams},
        types::{ArbosStateError, StorageBackedTr},
    }, stylus_executor::cache_program
};
use alloy_sol_types::{SolCall, SolError, sol};
use arbutil::evm::ARBOS_VERSION_STYLUS_CHARGING_FIXES;
use revm::{
    context::{Block, JournalTr},
    interpreter::{Gas, InterpreterResult},
    precompile::PrecompileId,
    primitives::{Address, B256, Bytes, Log, U256, address, alloy_primitives::IntoLogData},
};
use std::fmt::Debug;
use stylus::prover::programs::config::CompileConfig;

sol! {
#[derive(Debug)] // Keep this list small to avoid unnecessary bloat.
#[sol(abi)]
interface IArbWasm {
    /// @notice Activate a wasm program
    /// @param program the program to activate
    /// @return version the stylus version the program was activated against
    /// @return dataFee the data fee paid to store the activated program
    function activateProgram(
        address program
    ) external payable returns (uint16 version, uint256 dataFee);

    /// @notice Gets the latest stylus version
    /// @return version the stylus version
    function stylusVersion() external view returns (uint16 version);

    /// @notice Gets the stylus version the program with codehash was most recently activated against
    /// @return version the program version (reverts for EVM contracts)
    function codehashVersion(
        bytes32 codehash
    ) external view returns (uint16 version);

    /// @notice Extends a program's expiration date.
    /// Reverts if too soon or if the program is not up to date.
    function codehashKeepalive(
        bytes32 codehash
    ) external payable;

    /// @notice Gets a program's asm size.
    /// Reverts if program is not active.
    /// @return size the size in bytes
    function codehashAsmSize(
        bytes32 codehash
    ) external view returns (uint32 size);

    /// @notice Gets the stylus version the program was most recently activated against
    /// @return version the program version (reverts for EVM contracts)
    function programVersion(
        address program
    ) external view returns (uint16 version);

    /// @notice Gets the cost to invoke the program
    /// @return gas the amount of gas
    /// @return gasWhenCached the amount of gas if the program was recently used
    function programInitGas(
        address program
    ) external view returns (uint64 gas, uint64 gasWhenCached);

    /// @notice Gets the memory footprint of the program at the given address in pages
    /// @return footprint the memory footprint of program in pages (reverts for EVM contracts)
    function programMemoryFootprint(
        address program
    ) external view returns (uint16 footprint);

    /// @notice Gets the amount of time remaining until the program expires
    /// @return _secs the time left in seconds (reverts for EVM contracts)
    function programTimeLeft(
        address program
    ) external view returns (uint64 _secs);

    /// @notice Gets the conversion rate between gas and ink
    /// @return price the amount of ink 1 gas buys
    function inkPrice() external view returns (uint32 price);

    /// @notice Gets the wasm stack size limit
    /// @return depth the maximum depth (in wasm words) a wasm stack may grow
    function maxStackDepth() external view returns (uint32 depth);

    /// @notice Gets the number of free wasm pages a program gets
    /// @return pages the number of wasm pages (2^16 bytes)
    function freePages() external view returns (uint16 pages);

    /// @notice Gets the base cost of each additional wasm page (2^16 bytes)
    /// @return gas base amount of gas needed to grow another wasm page
    function pageGas() external view returns (uint16 gas);

    /// @notice Gets the ramp that drives exponential memory costs
    /// @return ramp bits representing the floating point value
    function pageRamp() external view returns (uint64 ramp);

    /// @notice Gets the maximum number of pages a wasm may allocate
    /// @return limit the number of pages
    function pageLimit() external view returns (uint16 limit);

    /// @notice Gets the minimum costs to invoke a program
    /// @return gas amount of gas in increments of 256 when not cached
    /// @return cached amount of gas in increments of 64 when cached
    function minInitGas() external view returns (uint64 gas, uint64 cached);

    /// @notice Gets the linear adjustment made to program init costs.
    /// @return percent the adjustment (100% = no adjustment).
    function initCostScalar() external view returns (uint64 percent);

    /// @notice Gets the number of days after which programs deactivate
    /// @return _days the number of days
    function expiryDays() external view returns (uint16 _days);

    /// @notice Gets the age a program must be to perform a keepalive
    /// @return _days the number of days
    function keepaliveDays() external view returns (uint16 _days);

    /// @notice Gets the number of extra programs ArbOS caches during a given block.
    /// @return count the number of same-block programs.
    function blockCacheSize() external view returns (uint16 count);

    /// @notice Emitted when a program is activated
    event ProgramActivated(
        bytes32 indexed codehash,
        bytes32 moduleHash,
        address program,
        uint256 dataFee,
        uint16 version
    );
    /// @notice Emitted when a program's lifetime is extended
    event ProgramLifetimeExtended(bytes32 indexed codehash, uint256 dataFee);

    /// @notice Reverts if the program is not a wasm program
    error ProgramNotWasm();
    /// @notice Reverts if the program is not active
    error ProgramNotActivated();
    /// @notice Reverts if the program is expired
    error ProgramNeedsUpgrade(uint16 version, uint16 stylusVersion);
    /// @notice Reverts if the program is too large
    error ProgramExpired(uint64 ageInSeconds);
    /// @notice Reverts if the program is up to date
    error ProgramUpToDate();
    /// @notice Reverts if the program keepalive is too soon
    error ProgramKeepaliveTooSoon(uint64 ageInSeconds);
    /// @notice Reverts if the program has insufficient value
    error ProgramInsufficientValue(uint256 have, uint256 want);
}
}

const STYLUS_ACTIVATION_FIXED_COST: u64 = 1659168;

pub fn arb_wasm_precompile<CTX: ArbitrumContextTr>() -> ExtendedPrecompile<CTX> {
    ExtendedPrecompile::new(
        PrecompileId::Custom(std::borrow::Cow::Borrowed("ArbWasm")),
        address!("0x0000000000000000000000000000000000000071"),
        arb_wasm_run::<CTX>,
    )
}
/// Run the precompile with the given context and input data.
fn arb_wasm_run<CTX: ArbitrumContextTr>(
    context: &mut CTX,
    input: &[u8],
    target_address: &Address,
    caller_address: Address,
    call_value: U256,
    _is_static: bool,
    gas_limit: u64,
) -> Result<Option<InterpreterResult>, String> {
    let mut gas = Gas::new(gas_limit);

    // decode selector
    if input.len() < 4 {
        gas.spend_all();
        return_revert!(gas, Bytes::from("Input too short"));
    }

    // decode selector
    let selector: [u8; 4] = input[0..4].try_into().unwrap();

    let params = try_state!(gas, context.arb_state(Some(&mut gas)).programs().get_stylus_params());

    match selector {
        IArbWasm::activateProgramCall::SELECTOR => {
            let call = IArbWasm::activateProgramCall::abi_decode(input).unwrap();

            record_cost!(gas, STYLUS_ACTIVATION_FIXED_COST);

            let code_hash = if let Some(code_hash) = context.load_account_code_hash(call.program) {
                code_hash.data
            } else {
                return_revert!(gas, IArbWasm::ProgramNotWasm {}.abi_encode());
            };

            if !gas.record_cost(revm::interpreter::gas::COLD_ACCOUNT_ACCESS_COST) {
                return_revert!(gas, Bytes::from("Out of gas"));
            }

            let cached = if let Some(program_info) = try_state!(
                gas,
                context.arb_state(Some(&mut gas)).programs().program_info(&code_hash)
            ) {
                let expired = program_info.age > params.expiry_days as u32 * 24 * 60 * 60;
                
                // program is already activated
                if program_info.version == params.version && !expired {
                    return_revert!(gas, IArbWasm::ProgramUpToDate {}.abi_encode());
                }

                program_info.cached
            } else {
                false
            };

            let bytecode = context.journal_mut().code(call.program).ok().unwrap_or_default().data;

            if !bytecode.starts_with(STYLUS_DISCRIMINANT) {
                return_revert!(gas, Bytes::from("Not a Stylus program"));
            }

            let compile_config =
                CompileConfig::version(params.version, context.cfg().stylus().debug_mode());

            let debug = context.cfg().stylus().debug_mode();

            let open_pages = context.local().stylus_pages_open();

            let (serialized, module, stylus_data) = match crate::stylus_executor::compile_stylus_bytecode(
                Some(&mut gas),
                &bytecode,
                code_hash,
                context.cfg().stylus().arbos_version(),
                params.version,
                &compile_config,
                params.page_limit.saturating_sub(open_pages),
                debug,
            ) {
                Ok(res) => res,
                Err(e) => {
                    return_revert!(gas, e);
                }
            };

            // We do not evict cached programs on re-activation since the WASM cache is shared across threads
            // and we cannot evict from other threads here. ProgramInfo cached field handles the cache status.
            if cached {
                try_state!(
                    gas,
                    context
                        .arb_state(Some(&mut gas))
                        .programs()
                        .module_hash(&code_hash)
                        .get()
                );
            }

            let module_hash = B256::from_slice(module.hash().as_slice());

            let estimate_kb = stylus_data.asm_estimate.div_ceil(1024);

            let timestamp = context.block().timestamp();
            let data_free = try_state!(
                gas,
                context
                    .arb_state(Some(&mut gas))
                    .programs()
                    .data_pricer()
                    .update(stylus_data.asm_estimate, timestamp.saturating_to(),)
            );

            let data_fee = U256::from(data_free);

            let program_info = ProgramInfo {
                version: compile_config.version,
                init_cost: stylus_data.init_cost,
                cached_cost: stylus_data.cached_init_cost,
                footprint: stylus_data.footprint,
                asm_estimated_kb: estimate_kb,
                age: params.expiry_days as u32,
                cached,
            };

            try_state!(
                gas,
                context
                    .arb_state(Some(&mut gas))
                    .programs()
                    .module_hash(&code_hash)
                    .set(module_hash)
            );


       

            if call_value < data_fee {
                return_revert!(
                    gas,
                    IArbWasm::ProgramInsufficientValue { have: call_value, want: data_fee }
                        .abi_encode()
                );
            }

            let fee_recipient =
                try_state!(gas, context.arb_state(Some(&mut gas)).network_fee_account().get());

            if let Some(error) = context
                .journal_mut()
                .transfer(*target_address, fee_recipient, data_fee)
                .unwrap()
            {
                return Ok(Some(InterpreterResult {
                    result: error.into(),
                    gas,
                    output: Bytes::default(),
                }));
            }

            let refund = call_value.saturating_sub(data_fee);
            if let Some(error) =
                context.journal_mut().transfer(*target_address, caller_address, refund).unwrap()
            {
                return Ok(Some(InterpreterResult {
                    result: error.into(),
                    gas,
                    output: Bytes::default(),
                }));
            }

            if cached {
                cache_program(code_hash, serialized, module, stylus_data);
            }

            try_state!(
                gas,
                context
                    .arb_state(Some(&mut gas))
                    .programs()
                    .save_program_info(&code_hash, &program_info)
            );

            let output = IArbWasm::activateProgramCall::abi_encode_returns(
                &IArbWasm::activateProgramReturn {
                    version: compile_config.version,
                    dataFee: U256::from(data_free),
                },
            );

            // Dummy gas usage
            return_success!(gas, Bytes::from(output));
        }
        IArbWasm::stylusVersionCall::SELECTOR => {
            let output = IArbWasm::stylusVersionCall::abi_encode_returns(&params.version);

            return_success!(gas, Bytes::from(output));
        }
        IArbWasm::codehashVersionCall::SELECTOR => {
            let call = IArbWasm::codehashVersionCall::abi_decode(input).unwrap();

            let program_info = try_state!(gas, get_active_program(context, &mut gas, &call.codehash, &params));


            let output = IArbWasm::codehashVersionCall::abi_encode_returns(&program_info.version);

            return_success!(gas, Bytes::from(output));
        }
        IArbWasm::codehashKeepaliveCall::SELECTOR => {
            let call = IArbWasm::codehashKeepaliveCall::abi_decode(input).unwrap();

            let mut program_info = try_state!(gas, get_active_program(context, &mut gas, &call.codehash, &params));


            if program_info.age < params.keepalive_days as u32 * 24 * 60 * 60 {
                return_revert!(
                    gas,
                    IArbWasm::ProgramKeepaliveTooSoon { ageInSeconds: program_info.age as u64 }
                        .abi_encode()
                );
            }

            if program_info.version != params.version {
                return_revert!(
                    gas,
                    IArbWasm::ProgramNeedsUpgrade {
                        version: program_info.version,
                        stylusVersion: params.version,
                    }
                    .abi_encode()
                );
            }

            let timestamp = context.block().timestamp();
            let data_fee = try_state!(
                gas,
                context.arb_state(Some(&mut gas)).programs().data_pricer().update(
                    program_info.asm_estimated_kb.saturating_mul(1024),
                    timestamp.saturating_to(),
                )
            );

            if call_value < U256::from(data_fee) {
                return_revert!(
                    gas,
                    IArbWasm::ProgramInsufficientValue {
                        have: call_value,
                        want: U256::from(data_fee),
                    }
                    .abi_encode()
                );
            }

            let fee_recipient =
                try_state!(gas, context.arb_state(Some(&mut gas)).network_fee_account().get());

            if let Some(error) = context
                .journal_mut()
                .transfer(*target_address, fee_recipient, U256::from(data_fee))
                .unwrap()
            {
                return Ok(Some(InterpreterResult {
                    result: error.into(),
                    gas,
                    output: Bytes::default(),
                }));
            }

            // refund excess
            let refund = call_value.saturating_sub(U256::from(data_fee));
            if let Some(error) =
                context.journal_mut().transfer(*target_address, caller_address, refund).unwrap()
                && !refund.is_zero()
            {
                return Ok(Some(InterpreterResult {
                    result: error.into(),
                    gas,
                    output: Bytes::default(),
                }));
            }

            program_info.age = 0;

            try_state!(
                gas,
                context
                    .arb_state(Some(&mut gas))
                    .programs()
                    .save_program_info(&call.codehash, &program_info)
            );

            // emit ProgramLifetimeExtended
            emit_event!(
                context,
                Log {
                    address: *target_address,
                    data: IArbWasm::ProgramLifetimeExtended {
                        codehash: call.codehash,
                        dataFee: U256::from(data_fee),
                    }
                    .into_log_data()
                },
                gas
            );

            return_success!(gas);
        }
        IArbWasm::codehashAsmSizeCall::SELECTOR => {
            let call = IArbWasm::codehashAsmSizeCall::abi_decode(input).unwrap();

            let program_info = try_state!(gas, get_active_program(context, &mut gas, &call.codehash, &params));


            let output =
                IArbWasm::codehashAsmSizeCall::abi_encode_returns(&program_info.asm_estimated_kb);

            return_success!(gas, Bytes::from(output));
        }
        IArbWasm::programVersionCall::SELECTOR => {
            let call = IArbWasm::programVersionCall::abi_decode(input).unwrap();

            let code_hash = if let Some(code_hash) = context.load_account_code_hash(call.program) {
                code_hash.data
            } else {
                return_revert!(gas, IArbWasm::ProgramNotWasm {}.abi_encode());
            };

            let program_info = try_state!(gas, get_active_program(context, &mut gas, &code_hash, &params));

            let output = IArbWasm::programVersionCall::abi_encode_returns(&program_info.version);

            return_success!(gas, Bytes::from(output));
        }
        IArbWasm::programInitGasCall::SELECTOR => {
            let call = IArbWasm::programInitGasCall::abi_decode(input).unwrap();

            let code_hash = if let Some(code_hash) = context.load_account_code_hash(call.program) {
                code_hash.data
            } else {
                return_revert!(gas, IArbWasm::ProgramNotWasm {}.abi_encode());
            };

            let program_info = try_state!(gas, get_active_program(context, &mut gas, &code_hash, &params));


            let cached_gas = crate::stylus_executor::init_gas_cost(
                program_info.cached_cost,
                params.min_cached_init_gas,
                params.init_cost_scalar,
            );
            let init_gas = crate::stylus_executor::init_gas_cost(
                program_info.init_cost,
                params.min_init_gas,
                params.init_cost_scalar,
            );

            let output =
                IArbWasm::programInitGasCall::abi_encode_returns(&IArbWasm::programInitGasReturn {
                    gas: init_gas,
                    gasWhenCached: cached_gas,
                });

            return_success!(gas, Bytes::from(output));
        }
        IArbWasm::programMemoryFootprintCall::SELECTOR => {
            let call = IArbWasm::programMemoryFootprintCall::abi_decode(input).unwrap();

            let code_hash = if let Some(code_hash) = context.load_account_code_hash(call.program) {
                code_hash.data
            } else {
                return_revert!(gas, IArbWasm::ProgramNotWasm {}.abi_encode());
            };

            let program_info = try_state!(gas, get_active_program(context, &mut gas, &code_hash, &params));

            let output =
                IArbWasm::programMemoryFootprintCall::abi_encode_returns(&program_info.footprint);

            return_success!(gas, Bytes::from(output));
        }
        IArbWasm::programTimeLeftCall::SELECTOR => {
            let call = IArbWasm::programTimeLeftCall::abi_decode(input).unwrap();

            let code_hash = if let Some(code_hash) = context.load_account_code_hash(call.program) {
                code_hash.data
            } else {
                return_revert!(gas, IArbWasm::ProgramNotWasm {}.abi_encode());
            };

            let program_info = try_state!(gas, get_active_program(context, &mut gas, &code_hash, &params));

            let output =
                IArbWasm::programTimeLeftCall::abi_encode_returns(&(program_info.age as u64));

            return_success!(gas, Bytes::from(output));
        }
        IArbWasm::inkPriceCall::SELECTOR => {
            let output = IArbWasm::inkPriceCall::abi_encode_returns(&params.ink_price);

            return_success!(gas, Bytes::from(output));
        }
        IArbWasm::maxStackDepthCall::SELECTOR => {
            let output = IArbWasm::maxStackDepthCall::abi_encode_returns(&params.max_stack_depth);

            return_success!(gas, Bytes::from(output));
        }
        IArbWasm::freePagesCall::SELECTOR => {
            let output = IArbWasm::freePagesCall::abi_encode_returns(&params.free_pages);

            return_success!(gas, Bytes::from(output));
        }
        IArbWasm::pageGasCall::SELECTOR => {
            let output = IArbWasm::pageGasCall::abi_encode_returns(&params.page_gas);

            return_success!(gas, Bytes::from(output));
        }
        IArbWasm::pageRampCall::SELECTOR => {
            let output = IArbWasm::pageRampCall::abi_encode_returns(&params.page_ramp);

            return_success!(gas, Bytes::from(output));
        }
        IArbWasm::pageLimitCall::SELECTOR => {
            let output = IArbWasm::pageLimitCall::abi_encode_returns(&params.page_limit);

            return_success!(gas, Bytes::from(output));
        }
        IArbWasm::minInitGasCall::SELECTOR => {
            let output =
                IArbWasm::minInitGasCall::abi_encode_returns(&IArbWasm::minInitGasReturn {
                    gas: params.min_init_gas as u64 * MIN_INIT_GAS_UNITS,
                    cached: params.min_cached_init_gas as u64 * MIN_CACHED_GAS_UNITS,
                });

            if params.version < ARBOS_VERSION_STYLUS_CHARGING_FIXES as u16 {
                return_revert!(gas);
            }

            return_success!(gas, Bytes::from(output));
        }
        IArbWasm::initCostScalarCall::SELECTOR => {
            let output = IArbWasm::initCostScalarCall::abi_encode_returns(
                &(params.init_cost_scalar as u64 * COST_SCALAR_PERCENT),
            );

            return_success!(gas, Bytes::from(output));
        }
        IArbWasm::expiryDaysCall::SELECTOR => {
            let output = IArbWasm::expiryDaysCall::abi_encode_returns(&params.expiry_days);

            return_success!(gas, Bytes::from(output));
        }
        IArbWasm::keepaliveDaysCall::SELECTOR => {
            let output = IArbWasm::keepaliveDaysCall::abi_encode_returns(&params.keepalive_days);

            return_success!(gas, Bytes::from(output));
        }
        IArbWasm::blockCacheSizeCall::SELECTOR => {
            let output = IArbWasm::blockCacheSizeCall::abi_encode_returns(&params.block_cache_size);

            return_success!(gas, Bytes::from(output));
        }
        _ => return_revert!(gas, Bytes::from("Unknown function selector")),
    }
}

fn get_active_program<CTX: ArbitrumContextTr>(
    context: &mut CTX,
    mut gas: &mut Gas,
    code_hash: &B256,
    params: &StylusParams,
) -> Result<ProgramInfo, ArbosStateError> {
    let program_info = if let Some(program_info) =
        context.arb_state(Some(&mut gas)).programs().program_info(code_hash)?
    {
        program_info
    } else {
        return Err(ArbosStateError::ProgramError(IArbWasm::IArbWasmErrors::ProgramNotActivated(
            IArbWasm::ProgramNotActivated {},
        )));
    };

    if program_info.version == 0 {
        return Err(ArbosStateError::ProgramError(IArbWasm::IArbWasmErrors::ProgramNotActivated(
            IArbWasm::ProgramNotActivated {},
        )));
    }

    if params.version != program_info.version {
        return Err(ArbosStateError::ProgramError(IArbWasm::IArbWasmErrors::ProgramNeedsUpgrade(
            IArbWasm::ProgramNeedsUpgrade {
                version: program_info.version,
                stylusVersion: params.version,
            },
        )));
    }

    if program_info.age > params.expiry_days as u32 * 24 * 60 * 60 {
        return Err(ArbosStateError::ProgramError(IArbWasm::IArbWasmErrors::ProgramExpired(IArbWasm::ProgramExpired {
            ageInSeconds: program_info.age as u64,
        })));
    }

    Ok(program_info)
}
