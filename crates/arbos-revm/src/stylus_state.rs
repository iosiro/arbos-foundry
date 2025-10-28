use arbutil::evm::{ARBOS_VERSION_STYLUS_CHARGING_FIXES, ARBOS_VERSION_STYLUS_LAST_CODE_CACHE_FIX};
use revm::{
    context::{Cfg, JournalTr},
    interpreter::{gas::{sload_cost, sstore_cost}, Gas},
    primitives::{address, bytes::buf, keccak256, Address, B256, U256},
};

use crate::{
    ArbitrumContextTr,
    buffer::{self, take_u16},
    chain::ArbitrumChainInfoTr,
    constants::{INITIAL_MAX_WASM_SIZE, INITIAL_PAGE_RAMP},
};




pub fn get_module_hash<CTX>(context: &mut CTX, code_hash: &B256) -> Option<B256>
where
    CTX: ArbitrumContextTr,
{
    // context.journal_mut().warm_account(ARBOS_STATE_ADDRESS).unwrap();

    let subkey = substorage(&substorage(&B256::ZERO, ARBOS_STATE_PROGRAMS_KEY), ARBOS_PROGRAMS_STATE_MODULE_HASHES_KEY);
    let slot = map_address(&subkey, code_hash);

    println!(
        "Loading module hash for code hash: {code_hash:?} at slot: {slot:?} - subkey: {subkey:?}"
    );

    if let Some(state) = context.sload(ARBOS_STATE_ADDRESS, slot.into()) {
        return Some(state.data.into());
    }

    None
}

pub fn save_module_hash<CTX>(context: &mut CTX, code_hash: &B256, module_hash: &B256)
where
    CTX: ArbitrumContextTr,
{
    let subkey = substorage(&substorage(&B256::ZERO, ARBOS_STATE_PROGRAMS_KEY), ARBOS_PROGRAMS_STATE_MODULE_HASHES_KEY);

    let slot = map_address(&subkey, code_hash);
    println!(
        "Save module hash for code hash: {code_hash:?} at slot: {slot:?} - subkey: {subkey:?}"
    );

    context.sstore(ARBOS_STATE_ADDRESS, slot.into(), (*module_hash).into());
}

#[derive(Debug, Clone)]
pub struct ProgramInfo {
    pub version: u16,
    pub init_cost: u16,
    pub cached_cost: u16,
    pub footprint: u16,
    pub asm_estimated_kb: u32,
    pub age: u32, // age in seconds since activation
    pub cached: bool,
}

pub fn program_info<CTX>(context: &mut CTX, code_hash: &B256) -> Option<ProgramInfo>
where
    CTX: ArbitrumContextTr,
{
    let subkey = substorage(&substorage(&B256::ZERO, ARBOS_STATE_PROGRAMS_KEY), ARBOS_PROGRAMS_STATE_PROGRAM_DATA_KEY);

    let slot = map_address(&subkey, code_hash);
    println!(
        "Loading program info for code hash: {code_hash:?} at slot: {slot:?} - subkey: {subkey:?}"
    );
    // context.journal_mut().warm_account(ARBOS_STATE_ADDRESS).unwrap();

    if let Ok(state) = context.journal_mut().sload(ARBOS_STATE_ADDRESS, slot.into())
        && !state.is_zero()
    {
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
            age: context
                .timestamp()
                .to::<u32>()
                .saturating_sub(activated_at.saturating_sub(ARBOS_GENESIS_TIMESTAMP) * 3600), // convert to seconds
            cached,
        });
    }

    None
}

pub fn save_program_info<CTX>(context: &mut CTX, code_hash: &B256, info: &ProgramInfo) -> u64
where
    CTX: ArbitrumContextTr,
{
    let subkey = substorage(&substorage(&B256::ZERO, ARBOS_STATE_PROGRAMS_KEY), ARBOS_PROGRAMS_STATE_PROGRAM_DATA_KEY);

    let slot = map_address(&subkey, code_hash);
    println!(
        "Save program info for code hash: {code_hash:?} at slot: {slot:?} - subkey: {subkey:?}"
    );

    let mut data = [0u8; 32];
    data[0..2].copy_from_slice(&info.version.to_be_bytes());
    data[2..4].copy_from_slice(&info.init_cost.to_be_bytes());
    data[4..6].copy_from_slice(&info.cached_cost.to_be_bytes());
    data[6..8].copy_from_slice(&info.footprint.to_be_bytes());
    data[8..11].copy_from_slice(&info.asm_estimated_kb.to_be_bytes()[1..4]);
    let activated_at = &info.age / 3600 + ARBOS_GENESIS_TIMESTAMP; // convert to hours
    data[11..14].copy_from_slice(&activated_at.to_be_bytes()[1..4]);
    data[14] = if info.cached { 1 } else { 0 };

    let value = U256::from_be_bytes(data);

    let res = context.sstore(ARBOS_STATE_ADDRESS, slot.into(), value).unwrap();

    sstore_cost(context.cfg().spec().into(), &res, true)
}

fn substorage(root: &B256, index: u8) -> B256 {
    let mut subkey_bytes = if root.is_zero() {
        Vec::with_capacity(1)
    } else {
        root.as_slice().to_vec()
    };
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

#[derive(Debug, Clone)]
pub struct StylusParams {
    pub version: u16,
    pub ink_price: u32,
    pub max_stack_depth: u32,
    pub free_pages: u16,
    pub page_gas: u16,
    pub page_ramp: u64,
    pub page_limit: u16,
    pub min_init_gas: u8,
    pub min_cached_init_gas: u8,
    pub init_cost_scalar: u8,
    pub cached_cost_scalar: u8,
    pub expiry_days: u16,
    pub keepalive_days: u16,
    pub block_cache_size: u16,
    pub max_wasm_size: u32,
}

impl StylusParams {
    fn zero() -> Self {
        Self {
            version: 0,
            ink_price: 0,
            max_stack_depth: 0,
            free_pages: 0,
            page_gas: 0,
            page_ramp: 0,
            page_limit: 0,
            min_init_gas: 0,
            min_cached_init_gas: 0,
            init_cost_scalar: 0,
            cached_cost_scalar: 0,
            expiry_days: 0,
            keepalive_days: 0,
            block_cache_size: 0,
            max_wasm_size: 0,
        }
    }
}

pub fn get_stylus_params<CTX: ArbitrumContextTr>(context: &mut CTX) -> (StylusParams, u64) {
    let subkey = substorage(&substorage(&B256::ZERO, ARBOS_STATE_PROGRAMS_KEY), ARBOS_PROGRAMS_STATE_PARAMS_KEY);
    let slot = map_address(&subkey, &B256::ZERO);

    let gas_cost = sload_cost(context.cfg().spec().into(), false);
    context.journal_mut().warm_account(ARBOS_STATE_ADDRESS).unwrap();

    // Initialize with all zero values
    let mut params = StylusParams::zero();

    let state = context.sload(ARBOS_STATE_ADDRESS, slot.into()).expect("Stylus params must be set");

    if !state.data.is_zero() {
        let mut data = state.data.to_be_bytes_vec();

        params.version = buffer::take_u16(&mut data);
        params.ink_price = buffer::take_u32(&mut data);
        params.max_stack_depth = buffer::take_u32(&mut data);
        params.free_pages = buffer::take_u16(&mut data);
        params.page_gas = buffer::take_u16(&mut data);
        params.page_limit = buffer::take_u16(&mut data);
        params.min_init_gas = buffer::take_u8(&mut data);
        params.min_cached_init_gas = buffer::take_u8(&mut data);
        params.init_cost_scalar = buffer::take_u8(&mut data);
        params.cached_cost_scalar = buffer::take_u8(&mut data);
        params.expiry_days = buffer::take_u16(&mut data);
        params.keepalive_days = buffer::take_u16(&mut data);
        params.block_cache_size = buffer::take_u16(&mut data);

        if context.chain().arbos_version_or_default() >= 40 {
            params.max_wasm_size = buffer::take_u32(&mut data);
        }

        // `page_ramp` is not stored in ARBOS params. Always pulled dynamically
        // from override or default at runtime.
        params.page_ramp = context.chain().page_ramp_or_default();

        // If we upgrade from a version that didn't store max_wasm_size, set default
        if params.max_wasm_size == 0 {
            params.max_wasm_size = INITIAL_MAX_WASM_SIZE;
        }

        return (params, gas_cost);
    }

    params.version = context.chain().stylus_version_or_default();
    params.ink_price = context.chain().ink_price_or_default();
    params.max_stack_depth = context.chain().max_stack_depth_or_default();
    params.free_pages = context.chain().free_pages_or_default();
    params.page_gas = context.chain().page_gas_or_default();
    params.page_ramp = context.chain().page_ramp_or_default();
    params.page_limit = context.chain().page_limit_or_default();
    params.min_init_gas = context.chain().min_init_gas_or_default();
    params.min_cached_init_gas = context.chain().min_cached_init_gas_or_default();
    params.init_cost_scalar = context.chain().init_cost_scalar_or_default();
    params.cached_cost_scalar = context.chain().cached_cost_scalar_or_default();
    params.expiry_days = context.chain().expiry_days_or_default();
    params.keepalive_days = context.chain().keepalive_days_or_default();
    params.block_cache_size = context.chain().block_cache_size_or_default();
    params.max_wasm_size = context.chain().max_wasm_size_or_default();

    (params, gas_cost)
}

pub fn save_stylus_params<CTX: ArbitrumContextTr>(context: &mut CTX, params: &StylusParams) {
    let subkey = substorage(&substorage(&B256::ZERO, ARBOS_STATE_PROGRAMS_KEY), ARBOS_PROGRAMS_STATE_PARAMS_KEY);
    let slot = map_address(&subkey, &B256::ZERO);

    let mut data = [0u8; 32];
    data[0..2].copy_from_slice(&params.version.to_be_bytes());
    data[2..6].copy_from_slice(&params.ink_price.to_be_bytes());
    data[6..10].copy_from_slice(&params.max_stack_depth.to_be_bytes());
    data[10..12].copy_from_slice(&params.free_pages.to_be_bytes());
    data[12..14].copy_from_slice(&params.page_gas.to_be_bytes());
    // `page_ramp` is not stored in ARBOS params. Always pulled dynamically
    // from override or default at runtime.
    data[14..16].copy_from_slice(&params.page_limit.to_be_bytes());
    data[16] = params.min_init_gas;
    data[17] = params.min_cached_init_gas;
    data[18] = params.init_cost_scalar;
    data[19] = params.cached_cost_scalar;
    data[20..22].copy_from_slice(&params.expiry_days.to_be_bytes());
    data[22..24].copy_from_slice(&params.keepalive_days.to_be_bytes());
    data[24..26].copy_from_slice(&params.block_cache_size.to_be_bytes());

    if context.chain().arbos_version_or_default() >= 40 {
        data[26..30].copy_from_slice(&params.max_wasm_size.to_be_bytes());
    }

    context.sstore(ARBOS_STATE_ADDRESS, slot.into(), U256::from_be_bytes(data));
}

// called when the evm is initialized to set the stylus params in storage.
pub fn init_stylus_params<CTX: ArbitrumContextTr>(context: &mut CTX) {

    // ignore gas cost here; this is only called at initialization time
    let (mut params, _) = get_stylus_params(context);

    if let Some(version) = context.chain().stylus_version() {
        params.version = version;
    }
    if let Some(ink_price) = context.chain().ink_price() {
        params.ink_price = ink_price;
    }
    if let Some(max_stack_depth) = context.chain().max_stack_depth() {
        params.max_stack_depth = max_stack_depth;
    }
    if let Some(free_pages) = context.chain().free_pages() {
        params.free_pages = free_pages;
    }
    if let Some(page_gas) = context.chain().page_gas() {
        params.page_gas = page_gas;
    }
    if let Some(page_ramp) = context.chain().page_ramp() {
        params.page_ramp = page_ramp;
    }
    if let Some(page_limit) = context.chain().page_limit() {
        params.page_limit = page_limit;
    }
    if let Some(min_init_gas) = context.chain().min_init_gas() {
        params.min_init_gas = min_init_gas;
    }
    if let Some(min_cached_init_gas) = context.chain().min_cached_init_gas() {
        params.min_cached_init_gas = min_cached_init_gas;
    }
    if let Some(init_cost_scalar) = context.chain().init_cost_scalar() {
        params.init_cost_scalar = init_cost_scalar;
    }
    if let Some(cached_cost_scalar) = context.chain().cached_cost_scalar() {
        params.cached_cost_scalar = cached_cost_scalar;
    }
    if let Some(expiry_days) = context.chain().expiry_days() {
        params.expiry_days = expiry_days;
    }
    if let Some(keepalive_days) = context.chain().keepalive_days() {
        params.keepalive_days = keepalive_days;
    }
    if let Some(block_cache_size) = context.chain().block_cache_size() {
        params.block_cache_size = block_cache_size;
    }
    if let Some(max_wasm_size) = context.chain().max_wasm_size() {
        params.max_wasm_size = max_wasm_size;
    }

    save_stylus_params(context, &params);
}

#[derive(Debug, Clone)]
pub struct DataPricer {
    demand: u32,
    bytes_per_second: u32,
    last_update_time: u64,
    min_price: u32,
    inertia: u32,
}

const DATA_PRICER_DEMAND_OFFSET: u8 = 0;
const DATA_PRICER_BYTES_PER_SECOND_OFFSET: u8 = 1;
const DATA_PRICER_LAST_UPDATE_TIME_OFFSET: u8 = 2;
const DATA_PRICER_MIN_PRICE_OFFSET: u8 = 3;
const DATA_PRICER_INERTIA_OFFSET: u8 = 4;

pub fn get_data_pricer<CTX: ArbitrumContextTr>(context: &mut CTX) -> DataPricer {
    let subkey = substorage(&substorage(&B256::ZERO, ARBOS_STATE_PROGRAMS_KEY), ARBOS_PROGRAMS_STATE_DATA_PRICER_KEY);

    let demand = context.journal_mut().sload(ARBOS_STATE_ADDRESS, map_address(&subkey, &B256::from(U256::from(DATA_PRICER_DEMAND_OFFSET))).into()).unwrap_or_default().data;
    let bytes_per_second = context.journal_mut().sload(ARBOS_STATE_ADDRESS, map_address(&subkey, &B256::from(U256::from(DATA_PRICER_BYTES_PER_SECOND_OFFSET))).into()).unwrap_or_default().data;
    let last_update_time = context.journal_mut().sload(ARBOS_STATE_ADDRESS, map_address(&subkey, &B256::from(U256::from(DATA_PRICER_LAST_UPDATE_TIME_OFFSET))).into()).unwrap_or_default().data;
    let min_price = context.journal_mut().sload(ARBOS_STATE_ADDRESS, map_address(&subkey, &B256::from(U256::from(DATA_PRICER_MIN_PRICE_OFFSET))).into()).unwrap_or_default().data;
    let inertia = context.journal_mut().sload(ARBOS_STATE_ADDRESS, map_address(&subkey, &B256::from(U256::from(DATA_PRICER_INERTIA_OFFSET))).into()).unwrap_or_default().data;

    DataPricer {
        demand: demand.saturating_to(),
        bytes_per_second: bytes_per_second.saturating_to(),
        last_update_time: last_update_time.saturating_to(),
        min_price: min_price.saturating_to(),
        inertia: inertia.saturating_to(),
    }
}

pub fn update_data_pricer_model<CTX: ArbitrumContextTr>(context: &mut CTX, data_price: DataPricer, temp_bytes: u32, time: u64) -> u64 {
    let subkey = substorage(&substorage(&B256::ZERO, ARBOS_STATE_PROGRAMS_KEY), ARBOS_PROGRAMS_STATE_DATA_PRICER_KEY);

    let mut demand = data_price.demand;
    let bytes_per_second = data_price.bytes_per_second;
    let last_update_time = data_price.last_update_time;
    let min_price = data_price.min_price;
    let inertia = data_price.inertia;

    let passed = time.saturating_sub(last_update_time) as u32;
    let credit = bytes_per_second.saturating_mul(passed);
    demand = demand.saturating_sub(credit);
    demand = demand.saturating_add(temp_bytes);

    context.sstore(ARBOS_STATE_ADDRESS, map_address(&subkey, &B256::from(U256::from(DATA_PRICER_DEMAND_OFFSET))).into(), U256::from(demand));
    context.sstore(ARBOS_STATE_ADDRESS, map_address(&subkey, &B256::from(U256::from(DATA_PRICER_LAST_UPDATE_TIME_OFFSET))).into(), U256::from(time));

    let exponent = (demand as f64) / (inertia as f64);
    let multiplier = f64::exp(exponent);
    let cost_per_byte = (min_price as f64 * multiplier).floor() as u64;
    let cost_in_wei = cost_per_byte.saturating_mul(temp_bytes as u64);

    cost_in_wei
}

pub fn is_cache_manager<CTX: ArbitrumContextTr>(context: &mut CTX, address: &Address) -> bool {
    let subkey = substorage(&substorage(&B256::ZERO, ARBOS_STATE_PROGRAMS_KEY), ARBOS_PROGRAMS_STATE_CACHE_MANAGERS_KEY);
    let by_address = substorage(&subkey, 0);
    let by_address_slot = map_address(&by_address, &B256::left_padding_from(address.as_slice()));
    let by_address_value = context.journal_mut().sload(ARBOS_STATE_ADDRESS, by_address_slot.into()).unwrap().data;

    !by_address_value.is_zero()
}


pub fn all_cache_managers<CTX: ArbitrumContextTr>(context: &mut CTX) -> Vec<Address> {
    // sto.OpenCachedSubStorage(cacheManagersKey)
    let subkey = substorage(&substorage(&B256::ZERO, ARBOS_STATE_PROGRAMS_KEY), ARBOS_PROGRAMS_STATE_CACHE_MANAGERS_KEY);

    let address_set_size = context.journal_mut().sload(ARBOS_STATE_ADDRESS, map_address(&subkey, &B256::from(U256::from(0))).into()).unwrap().data;

    let address_set_size = address_set_size.saturating_to::<usize>();

    let mut found = Vec::new();
    for i in 0..address_set_size {
        let manager_address = context.journal_mut().sload(ARBOS_STATE_ADDRESS, map_address(&subkey, &B256::from(U256::from(i as u64 + 1))).into()).unwrap().data;
        let manager_address = Address::from_slice(&manager_address.to_be_bytes_vec()[12..32]);
        found.push(manager_address);
    }

    found
}

pub fn add_cache_manager<CTX: ArbitrumContextTr>(context: &mut CTX, address: &Address) {
    let subkey = substorage(&substorage(&B256::ZERO, ARBOS_STATE_PROGRAMS_KEY), ARBOS_PROGRAMS_STATE_CACHE_MANAGERS_KEY);

    let address_set_size_slot = map_address(&subkey, &B256::from(U256::from(0)));

    let address_set_size = context.journal_mut().sload(ARBOS_STATE_ADDRESS, address_set_size_slot.into()).unwrap().data;
    
    let mut address_set_size_u64 = address_set_size.saturating_to::<u64>();

    let new_index = address_set_size_u64 + 1;

    let manager_slot = map_address(&subkey, &B256::from(U256::from(new_index)));

    context.sstore(ARBOS_STATE_ADDRESS, manager_slot.into(), B256::left_padding_from(address.as_slice()).into());

    address_set_size_u64 += 1;
    context.sstore(ARBOS_STATE_ADDRESS, address_set_size_slot.into(), U256::from(address_set_size_u64));
}

pub fn remove_cache_manager<CTX: ArbitrumContextTr>(context: &mut CTX, address: &Address) {
    let subkey = substorage(&substorage(&B256::ZERO, ARBOS_STATE_PROGRAMS_KEY), ARBOS_PROGRAMS_STATE_CACHE_MANAGERS_KEY);
    let by_address = substorage(&subkey, 0);
    let by_address_slot = map_address(&by_address, &B256::left_padding_from(address.as_slice()));

    context.sstore(ARBOS_STATE_ADDRESS, by_address_slot.into(), U256::from(0u64));
}

pub fn is_chain_manager<CTX: ArbitrumContextTr>(context: &mut CTX, address: &Address) -> bool {
    let owners_address_set = substorage(&B256::ZERO, ARBOS_CHAIN_OWNERS_KEY);

    //let owners_address_set_size = map_address(&owners_address_set, &B256::ZERO);

    let owners_by_address = substorage(&owners_address_set, 0);

    let slot = map_address(&owners_by_address, &B256::left_padding_from(address.as_slice()));

    let value = context.journal_mut().sload(ARBOS_STATE_ADDRESS, slot.into()).unwrap().data;

    !value.is_zero() 
}

pub fn all_chain_managers<CTX: ArbitrumContextTr>(context: &mut CTX) -> Vec<Address> {
    let owners_address_set = substorage(&B256::ZERO, ARBOS_CHAIN_OWNERS_KEY);

    let owners_address_set_size = context.journal_mut().sload(ARBOS_STATE_ADDRESS, map_address(&owners_address_set, &B256::from(U256::from(0))).into()).unwrap().data;

    let owners_address_set_size = owners_address_set_size.saturating_to::<usize>();

    let mut found = Vec::new();
    for i in 0..owners_address_set_size {
        let owner_address = context.journal_mut().sload(ARBOS_STATE_ADDRESS, map_address(&owners_address_set, &B256::from(U256::from(i as u64 + 1))).into()).unwrap().data;
        let owner_address = Address::from_slice(&owner_address.to_be_bytes_vec()[12..32]);
        found.push(owner_address);
    }

    found
}

pub fn add_chain_manager<CTX: ArbitrumContextTr>(context: &mut CTX, address: &Address) {
    let owners_address_set = substorage(&B256::ZERO, ARBOS_CHAIN_OWNERS_KEY);

    let owners_address_set_size_slot = map_address(&owners_address_set, &B256::from(U256::from(0)));

    let owners_address_set_size = context.journal_mut().sload(ARBOS_STATE_ADDRESS, owners_address_set_size_slot.into()).unwrap().data;
    
    let mut owners_address_set_size_u64 = owners_address_set_size.saturating_to::<u64>();

    let new_index = owners_address_set_size_u64 + 1;

    let owner_slot = map_address(&owners_address_set, &B256::from(U256::from(new_index)));

    context.sstore(ARBOS_STATE_ADDRESS, owner_slot.into(), B256::left_padding_from(address.as_slice()).into());

    owners_address_set_size_u64 += 1;
    context.sstore(ARBOS_STATE_ADDRESS, owners_address_set_size_slot.into(), U256::from(owners_address_set_size_u64));
}

pub fn is_native_token_owner<CTX: ArbitrumContextTr>(context: &mut CTX, address: &Address) -> bool {
    let subkey = substorage(&B256::ZERO, ARBOS_STATE_NATIVE_TOKEN_OWNER_KEY);
    let slot = map_address(&subkey, &B256::left_padding_from(address.as_slice()));

    let value = context.journal_mut().sload(ARBOS_STATE_ADDRESS, slot.into()).unwrap().data;

    !value.is_zero()
}

pub fn add_native_token_owner<CTX: ArbitrumContextTr>(context: &mut CTX, address: &Address) {
    let subkey = substorage(&B256::ZERO, ARBOS_STATE_NATIVE_TOKEN_OWNER_KEY);
    let slot = map_address(&subkey, &B256::left_padding_from(address.as_slice()));

    context.sstore(ARBOS_STATE_ADDRESS, slot.into(), U256::from(1u64));
}

pub fn remove_native_token_owner<CTX: ArbitrumContextTr>(context: &mut CTX, address: &Address) {
    let subkey = substorage(&B256::ZERO, ARBOS_STATE_NATIVE_TOKEN_OWNER_KEY);
    let slot = map_address(&subkey, &B256::left_padding_from(address.as_slice()));

    context.sstore(ARBOS_STATE_ADDRESS, slot.into(), U256::from(0u64));
}

pub fn all_native_token_owners<CTX: ArbitrumContextTr>(context: &mut CTX) -> Vec<Address> {
    let subkey = substorage(&B256::ZERO, ARBOS_STATE_NATIVE_TOKEN_OWNER_KEY);

    let address_set_size = context.journal_mut().sload(ARBOS_STATE_ADDRESS, map_address(&subkey, &B256::from(U256::from(0))).into()).unwrap().data;

    let address_set_size = address_set_size.saturating_to::<usize>();

    let mut found = Vec::new();
    for i in 0..address_set_size {
        let owner_address = context.journal_mut().sload(ARBOS_STATE_ADDRESS, map_address(&subkey, &B256::from(U256::from(i as u64 + 1))).into()).unwrap().data;
        let owner_address = Address::from_slice(&owner_address.to_be_bytes_vec()[12..32]);
        found.push(owner_address);
    }

    found
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_substorage() {
        let programs = substorage(&substorage(&B256::ZERO, ARBOS_STATE_PROGRAMS_KEY), ARBOS_STATE_PROGRAMS_KEY);
        let cache = substorage(&programs, ARBOS_PROGRAMS_STATE_CACHE_MANAGERS_KEY);
        let by_address = substorage(&cache, 0);
        println!("programs: {:?}", programs);
        println!("cache: {:?}", cache);
        println!("by_address: {:?}", by_address);
    }
}