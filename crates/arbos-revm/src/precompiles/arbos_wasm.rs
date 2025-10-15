#![allow(missing_docs)]

use alloy_sol_types::{sol, SolCall};
use revm::{context::{JournalTr, LocalContextTr}, interpreter::{Gas, InputsImpl, InstructionResult, InterpreterResult}, primitives::{Bytes, U256}};
use stylus::prover::programs::config::CompileConfig;

use crate::{chain::ArbitrumChainInfoTr, constants::STYLUS_DISCRIMINANT, stylus_state::ProgramInfo, ArbitrumContextTr};

sol!{
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

/// Run the precompile with the given context and input data.
fn arbos_wasm_run<CTX: ArbitrumContextTr>(context: &mut CTX, inputs: &InputsImpl, is_static: bool, gas_limit: u64) -> InterpreterResult {
    let input_bytes = match &inputs.input {
        revm::interpreter::CallInput::SharedBuffer(range) => {
            if let Some(slice) = context.local().shared_memory_buffer_slice(range.clone()) {
                slice.to_vec()
            } else {
                vec![]
            }
        }
        revm::interpreter::CallInput::Bytes(bytes) => bytes.0.to_vec(),
    };
  
    // decode selector
    if input_bytes.len() < 4 {
        return InterpreterResult {
            result: InstructionResult::Revert,
            gas: Gas::new(gas_limit),
            output: Bytes::from("Input too short"),
        };
    }

        // decode selector
       let selector: [u8; 4] = input_bytes[0..4].try_into().unwrap();

        match selector {
            IArbWasm::activateProgramCall::SELECTOR => {
                let call = IArbWasm::activateProgramCall::abi_decode(&input_bytes).unwrap();

                let code_hash = if let Some(code_hash) = context.load_account_code_hash(call.program) {
                    code_hash.data
                } else {
                    return InterpreterResult {
                        result: InstructionResult::Revert,
                        gas: Gas::new(gas_limit),
                        output: Bytes::from("Program not found"),
                    };
                };

                if let Some(program_info) = crate::stylus_state::program_info(context, &code_hash) {
                    // program is already activated
                    return InterpreterResult {
                        result: InstructionResult::Revert,
                        gas: Gas::new(gas_limit),
                        output: Bytes::from("Program already activated"),
                    };
                };

                let bytecode = context.journal_mut().code(call.program).ok()?.data;

                if !bytecode.starts_with(STYLUS_DISCRIMINANT) {
                    return InterpreterResult {
                        result: InstructionResult::Revert,
                        gas: Gas::new(gas_limit),
                        output: Bytes::from("Not a Stylus program"),
                    };
                }

                let compile_config =
                    CompileConfig::version(context.chain().stylus_version(), context.chain().debug_mode());

                let (_, module, stylus_data) = crate::stylus_executor::compile_stylus_bytecode(
                    &bytecode,
                    code_hash,
                    context.chain().arbos_version(),
                    context.chain().stylus_version(),
                    gas_limit,
                    &compile_config,
                ).unwrap();

                // transfer dataFee to network account
                // refund excess to caller

                let program_info = ProgramInfo {
                    version: compile_config.version,
                    init_cost: stylus_data.init_cost,
                    cached_cost: stylus_data.cached_init_cost,
                    footprint: stylus_data.footprint,
                    asm_estimated_kb: stylus_data.asm_estimate,
                    ttl: 365 * 24 * 60 * 60, // 1 year in seconds. TODO: Use params.ExpiryDays
                    cached: false
                };
  
                let output = IArbWasm::activateProgramCall::abi_encode_returns(&IArbWasm::activateProgramReturn{
                    version: compile_config.version,
                    dataFee: U256::ZERO,
                });
              
                InterpreterResult {
                    result: InstructionResult::Return,
                    gas: Gas::new(gas_limit / 2), // Dummy gas usage
                    output: Bytes::from(output),
                }
            }
            IArbWasm::stylusVersionCall::SELECTOR => {
                todo!()  
            }
            _ => {
                return InterpreterResult {
                    result: InstructionResult::Revert,
                    gas: Gas::new(gas_limit),
                    output: Bytes::from("Unknown function selector"),
                };
            }
        }
    
}