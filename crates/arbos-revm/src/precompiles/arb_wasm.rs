#![allow(missing_docs)]

use crate::{
    chain::ArbitrumChainInfoTr, constants::{COST_SCALAR_PERCENT, MIN_CACHED_GAS_UNITS, MIN_INIT_GAS_UNITS, STYLUS_DISCRIMINANT}, precompiles::extension::ExtendedPrecompile, state::{program::{ProgramInfo, StylusParams}, ArbState, ArbStateGetter}, ArbitrumContextTr
};
use alloy_sol_types::{sol, SolCall, SolError};
use arbutil::evm::ARBOS_VERSION_STYLUS_CHARGING_FIXES;
use revm::{
    context::{Block, JournalTr},
    interpreter::{Gas, InstructionResult, InterpreterResult},
    precompile::PrecompileId,
    primitives::{address, Address, Bytes, B256, U256},
};
use std::fmt::Debug;
use stylus::prover::programs::config::CompileConfig;
use alloy_sol_types::SolInterface;

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

    let mut gas = Gas::new(gas_limit);


    let (params, _) = context.arb_state().programs().get_stylus_params();

    match selector {
        IArbWasm::activateProgramCall::SELECTOR => {
            let call = IArbWasm::activateProgramCall::abi_decode(&input).unwrap();

            if !gas.record_cost(STYLUS_ACTIVATION_FIXED_COST) {
                return Ok(Some(InterpreterResult {
                    result: InstructionResult::OutOfGas,
                    output: Default::default(),
                    gas,
                }));
            }
            
            let code_hash = if let Some(code_hash) = context.load_account_code_hash(call.program) {
                code_hash.data
            } else {
                return Ok(Some(InterpreterResult {
                    result: InstructionResult::Revert,
                    gas: Gas::new(gas_limit),
                    output: IArbWasm::ProgramNotWasm{}.abi_encode().into(),
                }));
            };

            let cached = if let Some(program_info) =  context.arb_state().programs().program_info(&code_hash) {    
                let expired = program_info.age > params.expiry_days as u32 * 24 * 60 * 60;
                // program is already activated
                if program_info.version == params.version && !expired {
                    return Ok(Some(InterpreterResult {
                        result: InstructionResult::Revert,
                        gas: Gas::new(gas_limit),
                        output: IArbWasm::ProgramUpToDate{}.abi_encode().into(),
                    }));
                }

                program_info.cached
            } else {
                false
            };


            let bytecode = context.journal_mut().code(call.program).ok().unwrap_or_default().data;

            if !bytecode.starts_with(STYLUS_DISCRIMINANT) {
                return Ok(Some(InterpreterResult {
                    result: InstructionResult::Revert,
                    gas: Gas::new(gas_limit),
                    output: Bytes::from("Not a Stylus program"),
                }));
            }

            let compile_config = CompileConfig::version(
                params.version,
                context.chain().debug_mode(),
            );

            let (_, module, stylus_data, gas_cost) = crate::stylus_executor::compile_stylus_bytecode(
                &bytecode,
                code_hash,
                context.chain().arbos_version_or_default(),
                params.version,
                params.page_limit,
                true,
                gas.remaining()
            )
            .unwrap();

            if !gas.record_cost(gas_cost) {
                return Ok(Some(InterpreterResult {
                    result: InstructionResult::OutOfGas,
                    output: Default::default(),
                    gas: Gas::new(gas_limit),
                }));
            }

            // transfer dataFee to network account
            // refund excess to caller

            if cached {
                println!("Program was cached");
            }

            let module_hash = B256::from_slice(module.hash().as_slice());

            // arbmath.IntToUint24(arbmath.DivCeil(info.asmEstimate, 1024))
            let estimate_kb = (stylus_data.asm_estimate + 1023) / 1024;

            // TODO: dataFee calculation
            let data_pricer =  context.arb_state().programs().get_data_pricer();
            println!("Data pricer: {:?}", data_pricer);
            let timestamp = context.block().timestamp();
            let data_free =  context.arb_state().programs().update_data_pricer_model(data_pricer, stylus_data.asm_estimate, timestamp.saturating_to());

            let program_info = ProgramInfo {
                version: compile_config.version,
                init_cost: stylus_data.init_cost,
                cached_cost: stylus_data.cached_init_cost,
                footprint: stylus_data.footprint,
                asm_estimated_kb: estimate_kb,
                age: params.expiry_days as u32,
                cached: false,
            };

            context.arb_state().programs().save_module_hash(&code_hash, &module_hash);
            context.arb_state().programs().save_program_info(&code_hash, &program_info);
            if !gas.record_cost(gas_cost) {
                return Ok(Some(InterpreterResult {
                    result: InstructionResult::OutOfGas,
                    output: Default::default(),
                    gas: Gas::new(gas_limit),
                }));
            }

            let data_fee = U256::from(data_free);

            if call_value < data_fee {
                return Ok(Some(InterpreterResult {
                    result: InstructionResult::Revert,
                    gas: Gas::new(gas_limit),
                    output: IArbWasm::ProgramInsufficientValue {
                        have: call_value,
                        want: U256::from(data_free),
                    }
                    .abi_encode()
                    .into(),
                }));
            }

            // refund excess
            let refund = call_value.saturating_sub(data_fee);
            if let Some(error) = context.journal_mut().transfer(*target_address, caller_address, refund).unwrap() {
               
                return Ok(Some(InterpreterResult {
                    result: error.into(),
                    gas: Gas::new(gas_limit),
                    output: Bytes::default()
                }));
            }

            let output = IArbWasm::activateProgramCall::abi_encode_returns(
                &IArbWasm::activateProgramReturn {
                    version: compile_config.version,
                    dataFee: U256::from(data_free),
                },
            );

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas, // Dummy gas usage
                output: Bytes::from(output),
            }))
        },
        IArbWasm::stylusVersionCall::SELECTOR => {
            let output = IArbWasm::stylusVersionCall::abi_encode_returns( &params.version);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas,
                output: Bytes::from(output),
            }))
        },
        IArbWasm::codehashVersionCall::SELECTOR => {
            let call = IArbWasm::codehashVersionCall::abi_decode(&input).unwrap();

            let program_info = match get_active_program(context, &call.codehash, &params)  {
                Ok(res) => res,
                Err(e) => {
                    return Ok(Some(InterpreterResult {
                        result: InstructionResult::Revert,
                        gas: Gas::new(gas_limit),
                        output: e.abi_encode().into(),
                    }));
                }
            };

            let output = IArbWasm::codehashVersionCall::abi_encode_returns( &program_info.version);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas,
                output: Bytes::from(output),
            }))
        },
        IArbWasm::codehashKeepaliveCall::SELECTOR => {
            let call = IArbWasm::codehashKeepaliveCall::abi_decode(&input).unwrap();            

            let mut program_info = match get_active_program(context, &call.codehash, &params)  {
                Ok(res) => res,
                Err(e) => {
                    return Ok(Some(InterpreterResult {
                        result: InstructionResult::Revert,
                        gas: Gas::new(gas_limit),
                        output: e.abi_encode().into(),
                    }));
                }
            };
 
            if program_info.age < params.keepalive_days as u32 * 24 * 60 * 60 {
                return Ok(Some(InterpreterResult {
                    result: InstructionResult::Revert,
                    gas: Gas::new(gas_limit),
                    output: IArbWasm::ProgramKeepaliveTooSoon {
                        ageInSeconds: program_info.age as u64,
                    }
                    .abi_encode()
                    .into(),
                }));
            }

            if program_info.version != params.version {
                return Ok(Some(InterpreterResult {
                    result: InstructionResult::Revert,
                    gas: Gas::new(gas_limit),
                    output: IArbWasm::ProgramNeedsUpgrade {
                        version: program_info.version,
                        stylusVersion: params.version,
                    }
                    .abi_encode()
                    .into(),
                }));
            }

            let data_pricer = context.arb_state().programs().get_data_pricer();
            let timestamp = context.block().timestamp();
            let data_fee = context.arb_state().programs().update_data_pricer_model(data_pricer, program_info.asm_estimated_kb.saturating_mul(1024), timestamp.saturating_to());

            program_info.age = 0;

             context.arb_state().programs().save_program_info(&call.codehash, &program_info);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas,
                output: Bytes::default(),
            }))
        },
        IArbWasm::codehashAsmSizeCall::SELECTOR => {
            let call = IArbWasm::codehashAsmSizeCall::abi_decode(&input).unwrap();

            let program_info = match get_active_program(context, &call.codehash, &params)  {
                Ok(res) => res,
                Err(e) => {
                    return Ok(Some(InterpreterResult {
                        result: InstructionResult::Revert,
                        gas: Gas::new(gas_limit),
                        output: e.abi_encode().into(),
                    }));
                }
            };

            let output = IArbWasm::codehashAsmSizeCall::abi_encode_returns( &program_info.asm_estimated_kb);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas,
                output: Bytes::from(output),
            }))
        },
        IArbWasm::programVersionCall::SELECTOR => {
            let call = IArbWasm::programVersionCall::abi_decode(&input).unwrap();

            let code_hash = if let Some(code_hash) = context.load_account_code_hash(call.program) {
                code_hash.data
            } else {
                return Ok(Some(InterpreterResult {
                    result: InstructionResult::Revert,
                    gas: Gas::new(gas_limit),
                    output: IArbWasm::ProgramNotWasm{}.abi_encode().into(),
                }));
            };

            let program_info = match get_active_program(context, &code_hash, &params)  {
                Ok(res) => res,
                Err(e) => {
                    return Ok(Some(InterpreterResult {
                        result: InstructionResult::Revert,
                        gas: Gas::new(gas_limit),
                        output: e.abi_encode().into(),
                    }));
                }
            };
           

            let output = IArbWasm::programVersionCall::abi_encode_returns( &program_info.version);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas,
                output: Bytes::from(output),
            }))
        },
        IArbWasm::programInitGasCall::SELECTOR => {
            let call = IArbWasm::programInitGasCall::abi_decode(&input).unwrap();

            let code_hash = if let Some(code_hash) = context.load_account_code_hash(call.program) {
                code_hash.data
            } else {
                return Ok(Some(InterpreterResult {
                    result: InstructionResult::Revert,
                    gas: Gas::new(gas_limit),
                    output: IArbWasm::ProgramNotWasm{}.abi_encode().into(),
                }));
            };

            let program_info = match get_active_program(context, &code_hash, &params)  {
                Ok(res) => res,
                Err(e) => {
                    return Ok(Some(InterpreterResult {
                        result: InstructionResult::Revert,
                        gas: Gas::new(gas_limit),
                        output: e.abi_encode().into(),
                    }));
                }
            };

            let cached_gas = crate::stylus_executor::init_gas(&program_info, &params);
            let init_gas = crate::stylus_executor::init_gas(&program_info, &params);

            let output = IArbWasm::programInitGasCall::abi_encode_returns( &IArbWasm::programInitGasReturn {
                gas: init_gas,
                gasWhenCached: cached_gas,
            });

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas,
                output: Bytes::from(output),
            }))
        },
        IArbWasm::programMemoryFootprintCall::SELECTOR => {
            let call = IArbWasm::programMemoryFootprintCall::abi_decode(&input).unwrap();

            let code_hash = if let Some(code_hash) = context.load_account_code_hash(call.program) {
                code_hash.data
            } else {
                return Ok(Some(InterpreterResult {
                    result: InstructionResult::Revert,
                    gas: Gas::new(gas_limit),
                    output: IArbWasm::ProgramNotWasm{}.abi_encode().into(),
                }));
            };

            let program_info = match get_active_program(context, &code_hash, &params)  {
                Ok(res) => res,
                Err(e) => {
                    return Ok(Some(InterpreterResult {
                        result: InstructionResult::Revert,
                        gas: Gas::new(gas_limit),
                        output: e.abi_encode().into(),
                    }));
                }
            };

            let output = IArbWasm::programMemoryFootprintCall::abi_encode_returns( &program_info.footprint);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas,
                output: Bytes::from(output),
            }))
        },
        IArbWasm::programTimeLeftCall::SELECTOR => {
            let call = IArbWasm::programTimeLeftCall::abi_decode(&input).unwrap();

            let code_hash = if let Some(code_hash) = context.load_account_code_hash(call.program) {
                code_hash.data
            } else {
                return Ok(Some(InterpreterResult {
                    result: InstructionResult::Revert,
                    gas: Gas::new(gas_limit),
                    output: IArbWasm::ProgramNotWasm{}.abi_encode().into(),
                }));
            };

            let program_info = match get_active_program(context, &code_hash, &params)  {
                Ok(res) => res,
                Err(e) => {
                    return Ok(Some(InterpreterResult {
                        result: InstructionResult::Revert,
                        gas: Gas::new(gas_limit),
                        output: e.abi_encode().into(),
                    }));
                }
            };

            let output = IArbWasm::programTimeLeftCall::abi_encode_returns( &(program_info.age as u64));

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas,
                output: Bytes::from(output),
            }))
        },
        IArbWasm::inkPriceCall::SELECTOR => {
            let output = IArbWasm::inkPriceCall::abi_encode_returns( &params.ink_price);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas,
                output: Bytes::from(output),
            }))
        },
        IArbWasm::maxStackDepthCall::SELECTOR => {

            let output = IArbWasm::maxStackDepthCall::abi_encode_returns( &params.max_stack_depth);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas,
                output: Bytes::from(output),
            }))
        },
        IArbWasm::freePagesCall::SELECTOR => {
            let output = IArbWasm::freePagesCall::abi_encode_returns( &params.free_pages);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas,
                output: Bytes::from(output),
            }))
        },
        IArbWasm::pageGasCall::SELECTOR => {
            let output = IArbWasm::pageGasCall::abi_encode_returns( &params.page_gas);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas,
                output: Bytes::from(output),
            }))
        },
        IArbWasm::pageRampCall::SELECTOR => {
            let output = IArbWasm::pageRampCall::abi_encode_returns( &params.page_ramp);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas,
                output: Bytes::from(output),
            }))
        },
        IArbWasm::pageLimitCall::SELECTOR => {
            let output = IArbWasm::pageLimitCall::abi_encode_returns( &params.page_limit);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas,
                output: Bytes::from(output),
            }))
        },
        IArbWasm::minInitGasCall::SELECTOR => {
            let output = IArbWasm::minInitGasCall::abi_encode_returns( &IArbWasm::minInitGasReturn {
                gas: params.min_init_gas as u64 * MIN_INIT_GAS_UNITS,
                cached: params.min_cached_init_gas as u64 * MIN_CACHED_GAS_UNITS,
            });

            if params.version < ARBOS_VERSION_STYLUS_CHARGING_FIXES as u16 {
                return Ok(Some(InterpreterResult {
                    result: InstructionResult::Revert,
                    gas: Gas::new(gas_limit),
                    output: Bytes::default(),
                }));
            }

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas,
                output: Bytes::from(output),
            }))
        },
        IArbWasm::initCostScalarCall::SELECTOR => {
            let output = IArbWasm::initCostScalarCall::abi_encode_returns( &(params.init_cost_scalar as u64 * COST_SCALAR_PERCENT));

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas,
                output: Bytes::from(output),
            }))
        },
        IArbWasm::expiryDaysCall::SELECTOR => {
            let output = IArbWasm::expiryDaysCall::abi_encode_returns( &params.expiry_days);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas,
                output: Bytes::from(output),
            }))
        },
        IArbWasm::keepaliveDaysCall::SELECTOR => {
            let output = IArbWasm::keepaliveDaysCall::abi_encode_returns( &params.keepalive_days);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas,
                output: Bytes::from(output),
            }))
        },
        IArbWasm::blockCacheSizeCall::SELECTOR => {
            let output = IArbWasm::blockCacheSizeCall::abi_encode_returns( &params.block_cache_size);

            Ok(Some(InterpreterResult {
                result: InstructionResult::Return,
                gas,
                output: Bytes::from(output),
            }))
        },
        _ => {
            return Ok(Some(InterpreterResult {
                result: InstructionResult::Revert,
                gas: Gas::new(gas_limit),
                output: Bytes::from("Unknown function selector"),
            }));
        }
    }
}

fn get_active_program<'a, CTX: ArbitrumContextTr>(context: &mut CTX, code_hash: &B256, params: &StylusParams) -> Result<ProgramInfo, IArbWasm::IArbWasmErrors> {

    let program_info = if let Some(program_info) = context.arb_state().programs().program_info(code_hash) {
        program_info
    } else {
        return Err(IArbWasm::IArbWasmErrors::ProgramNotActivated(IArbWasm::ProgramNotActivated{}));
    };

    if program_info.version == 0 {
        return Err(IArbWasm::IArbWasmErrors::ProgramNotActivated(IArbWasm::ProgramNotActivated{}));
    }

    if params.version != program_info.version {
        return Err(IArbWasm::IArbWasmErrors::ProgramNeedsUpgrade(IArbWasm::ProgramNeedsUpgrade {
            version: program_info.version,
            stylusVersion: params.version,
        }));
    }

    if program_info.age > params.expiry_days as u32 * 24 * 60 * 60 {
        return Err(IArbWasm::IArbWasmErrors::ProgramExpired(IArbWasm::ProgramExpired {
            ageInSeconds: program_info.age as u64,
        }));
    }
    
    Ok(program_info)
}
    