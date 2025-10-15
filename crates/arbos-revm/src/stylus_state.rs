use revm::{context::JournalTr, primitives::{address, keccak256, Address, B256, U256}};

use crate::ArbitrumContextTr;

pub const ARBOS_STATE_PROGRAMS_KEY: u8 = 8;
pub const ARBOS_PROGRAMS_STATE_PARAMS_KEY: u8 = 0;
pub const ARBOS_PROGRAMS_STATE_PROGRAM_DATA_KEY: u8 = 1;
pub const ARBOS_PROGRAMS_STATE_MODULE_HASHES_KEY: u8 = 2;
pub const ARBOS_PROGRAMS_STATE_DATA_PRICER_KEY: u8 = 3;
pub const ARBOS_PROGRAMS_STATE_CACHE_MANAGERS_KEY: u8 = 4;

pub const ARBOS_STATE_ADDRESS: Address = address!("0xA4B05FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF");

pub const ARBOS_GENESIS_TIMESTAMP: u32 = 1672531200; // January 1, 2023 00:00:00 GMT

pub fn module_hash<CTX>(context: &mut CTX, code_hash: &B256) -> Option<B256> 
where CTX: ArbitrumContextTr 
{
    context.journal_mut().warm_account(ARBOS_STATE_ADDRESS).unwrap();

    let subkey = substorage(ARBOS_PROGRAMS_STATE_MODULE_HASHES_KEY);
    let slot = map_address(&subkey, code_hash);

    println!("Loading module hash for code hash: {code_hash:?} at slot: {slot:?} - subkey: {subkey:?}");

    if let Some(state) = context.sload(ARBOS_STATE_ADDRESS, slot.into()) {
        return Some(state.data.into());
    } 

    None
}

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

pub fn program_info<CTX>(context: &mut CTX, code_hash: &B256) -> Option<ProgramInfo>
where CTX: ArbitrumContextTr 
{
    let subkey = substorage(ARBOS_PROGRAMS_STATE_PROGRAM_DATA_KEY);

    let slot = map_address(&subkey, code_hash);
    println!("Loading program info for code hash: {code_hash:?} at slot: {slot:?} - subkey: {subkey:?}");
    context.journal_mut().warm_account(ARBOS_STATE_ADDRESS).unwrap();

    if let Ok(state) = context.journal_mut().sload(ARBOS_STATE_ADDRESS, slot.into()) && !state.is_zero() {
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
            ttl: context.timestamp().to::<u32>().saturating_sub(activated_at.saturating_sub(ARBOS_GENESIS_TIMESTAMP) * 3600), // convert to seconds
            cached
        });        
    }

    None
}

pub fn save_program_info<CTX>(context: &mut CTX, module_hash: &B256, info: &ProgramInfo) 
where CTX: ArbitrumContextTr 
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