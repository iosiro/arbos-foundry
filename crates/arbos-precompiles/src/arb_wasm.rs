#![allow(missing_docs)]

use alloy_evm::{precompiles::{Precompile, PrecompileInput}, EvmInternals};
use alloy_sol_types::{sol, SolCall};
use revm::{precompile::{PrecompileError, PrecompileId, PrecompileOutput, PrecompileResult}, primitives::{keccak256, B256, U256}};
use std::borrow::Cow;

use crate::arb_wasm::IArbWasm::activateProgramReturn;

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

const ARB_WASM_PRECOMPILE_ID: PrecompileId = PrecompileId::Custom(Cow::Borrowed("ArbWasm"));

struct ArbWasm {}

impl ArbWasm {
    pub fn new() -> Self {
        Self {}
    }
}

impl Precompile for ArbWasm {

    fn precompile_id(&self) ->  &PrecompileId {
        &ARB_WASM_PRECOMPILE_ID
    }
    
    fn call(&self, input: PrecompileInput<'_>) -> PrecompileResult {
        // decode selector
        if input.data.len() < 4 {
            return Err(PrecompileError::Other("Unknown function selector".to_string()));
        }

        // decode selector
        let selector: [u8; 4] = input.data[0..4].try_into().unwrap();

        match selector {
            IArbWasm::activateProgramCall::SELECTOR => {
                let call = IArbWasm::activateProgramCall::abi_decode(&input.data).unwrap();
  
                let output = IArbWasm::activateProgramCall::abi_encode_returns(&activateProgramReturn{
                    version: 1, // Dummy version
                    dataFee: U256::ZERO, // Dummy data fee
                });
              
                Ok(PrecompileOutput::new(0, output.into())) // Replace 0 with actual gas used
            }
            IArbWasm::stylusVersionCall::SELECTOR => {
                todo!()  
            }
            _ => {
                return Err(PrecompileError::Other("Unknown function selector".to_string()));
            }
        }
    }
}


/// TODO this is mostly copy-pasted from arbos_state.rs and should be deduped
pub const ARBOS_STATE_PROGRAMS_KEY: u8 = 8;
pub const ARBOS_PROGRAMS_STATE_PARAMS_KEY: u8 = 0;
pub const ARBOS_PROGRAMS_STATE_PROGRAM_DATA_KEY: u8 = 1;
pub const ARBOS_PROGRAMS_STATE_MODULE_HASHES_KEY: u8 = 2;
pub const ARBOS_PROGRAMS_STATE_DATA_PRICER_KEY: u8 = 3;
pub const ARBOS_PROGRAMS_STATE_CACHE_MANAGERS_KEY: u8 = 4;

pub const ARBOS_STATE_ADDRESS: Address = address!("0xA4B05FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF");

pub const ARBOS_GENESIS_TIMESTAMP: u32 = 1672531200; // January 1, 2023 00:00:00 GMT

#[derive(Debug, Clone)]
pub struct ProgramInfo {
    pub version: u16,
    pub init_cost: u16,
    pub cached_cost: u16,
    pub footprint: u16,
    pub asm_estimated_kb: u32,
    pub ttl: u32, // age in seconds since activation
    pub cached: bool
}

pub fn program_info(context: &mut EvmInternals<'_>, code_hash: &B256) -> Option<ProgramInfo>
{
    let subkey = substorage(ARBOS_PROGRAMS_STATE_PROGRAM_DATA_KEY);

    let slot = map_address(&subkey, code_hash);
    println!("Loading program info for code hash: {code_hash:?} at slot: {slot:?} - subkey: {subkey:?}");

    if let Ok(state) = context.sload(ARBOS_STATE_ADDRESS, slot.into()) && !state.is_zero() {
        let data = state.data.to_be_bytes_vec();
        let version = u16::from_be_bytes([data[0], data[1]]);
        let init_cost = u16::from_be_bytes([data[2], data[3]]);
        let cached_cost = u16::from_be_bytes([data[4], data[5]]);
        let footprint = u16::from_be_bytes([data[6], data[7]]);
        let asm_estimated_kb = u32::from_be_bytes([0, data[8], data[9], data[10]]);
        let activated_at = u32::from_be_bytes([0, data[11], data[12], data[13]]);
        let cached = data[14] != 0;

        return Some(ProgramInfo {
            version,
            init_cost,
            cached_cost,
            footprint,
            asm_estimated_kb,
            ttl: context.block_timestamp().to::<u32>().saturating_sub(activated_at.saturating_sub(ARBOS_GENESIS_TIMESTAMP) * 3600), // convert to seconds
            cached
        });        
    }

    None
}

pub fn save_program_info(context: &mut EvmInternals<'_>, module_hash: &B256, info: &ProgramInfo) 
{
    let slot = map_address(&B256::ZERO, module_hash);

    let mut data = [0u8; 32];
    data[0..2].copy_from_slice(&info.version.to_be_bytes());
    data[2..4].copy_from_slice(&info.init_cost.to_be_bytes());
    data[4..6].copy_from_slice(&info.cached_cost.to_be_bytes());
    data[6..8].copy_from_slice(&info.footprint.to_be_bytes());
    data[8..11].copy_from_slice(&info.asm_estimated_kb.to_be_bytes()[1..4]);
    let activated_at = &info.ttl / 3600 + ARBOS_GENESIS_TIMESTAMP; // convert to hours
    data[11..15].copy_from_slice(&activated_at.to_be_bytes()[1..4]);
    data[14] = if info.cached { 1 } else { 0 };

    context.sstore(ARBOS_STATE_ADDRESS, slot.into(), U256::from_be_bytes(data));
}

fn substorage(index: u8) -> B256 {

    // keccak256([0x08])
    let mut subkey_bytes = Vec::with_capacity(1);
    subkey_bytes.push(ARBOS_STATE_PROGRAMS_KEY);
    let subkey_bytes = keccak256(subkey_bytes);
    // hash with index for specific substorage
    let mut subkey_bytes = subkey_bytes.as_slice().to_vec();
    subkey_bytes.push(index);
    keccak256(subkey_bytes)
}

fn map_address(storage_key: &B256, key: &B256) -> B256 {
    let key_bytes = key.as_slice();
    let boundary = key_bytes.len() - 1;

    // Concatenate storage_key and key[:boundary] for the hash
    let mut to_hash = Vec::with_capacity(storage_key.len() + boundary);
    to_hash.extend_from_slice(storage_key.as_slice());
    to_hash.extend_from_slice(&key_bytes[..boundary]);

    let digest = keccak256(&to_hash);

    let mut mapped = digest[..boundary].to_vec();
    mapped.push(key_bytes[boundary]);
    B256::from_slice(&mapped)
}