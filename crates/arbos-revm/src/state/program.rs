use revm::{context::{Cfg, JournalTr}, interpreter::gas::{sload_cost, sstore_cost}, primitives::{B256, U256}};

use crate::{buffer, chain::ArbitrumChainInfoTr, constants::INITIAL_MAX_WASM_SIZE, state::types::{map_address, substorage, StorageBackedAddressSet, StorageBackedU32, StorageBackedU64}, constants::{ARBOS_GENESIS_TIMESTAMP, ARBOS_PROGRAMS_STATE_CACHE_MANAGERS_KEY, ARBOS_PROGRAMS_STATE_DATA_PRICER_KEY, ARBOS_PROGRAMS_STATE_MODULE_HASHES_KEY, ARBOS_PROGRAMS_STATE_PARAMS_KEY, ARBOS_PROGRAMS_STATE_PROGRAM_DATA_KEY, ARBOS_STATE_ADDRESS, ARBOS_STATE_PROGRAMS_KEY}, ArbitrumContextTr};



// stylus params type
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
        Self { version: 0, ink_price: 0, max_stack_depth: 0, free_pages: 0, page_gas: 0, page_ramp: 0, page_limit: 0, min_init_gas: 0, min_cached_init_gas: 0, init_cost_scalar: 0, cached_cost_scalar: 0, expiry_days: 0, keepalive_days: 0, block_cache_size: 0, max_wasm_size: 0 }
    }
}

// data pricer type
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


pub struct Programs<'a, CTX>(&'a mut CTX, B256)
where
    CTX: ArbitrumContextTr;

impl<'a, CTX> Programs<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    pub fn new(context: &'a mut CTX) -> Self {
        let root = B256::ZERO;
        let subkey = substorage(&root, ARBOS_STATE_PROGRAMS_KEY);
        Self(context, subkey)
    }

    fn params_subkey(&self) -> B256 { substorage(&self.1, ARBOS_PROGRAMS_STATE_PARAMS_KEY) }
    fn program_data_subkey(&self) -> B256 { substorage(&self.1, ARBOS_PROGRAMS_STATE_PROGRAM_DATA_KEY) }
    fn module_hashes_subkey(&self) -> B256 { substorage(&self.1, ARBOS_PROGRAMS_STATE_MODULE_HASHES_KEY) }
    fn data_pricer_subkey(&self) -> B256 { substorage(&self.1, ARBOS_PROGRAMS_STATE_DATA_PRICER_KEY) }
    fn cache_managers_subkey(&self) -> B256 { substorage(&self.1, ARBOS_PROGRAMS_STATE_CACHE_MANAGERS_KEY) }

    pub fn get_module_hash(&mut self, code_hash: &B256) -> Option<B256> {
        let slot = map_address(&self.module_hashes_subkey(), code_hash);
        if let Some(state) = self.0.sload(ARBOS_STATE_ADDRESS, slot.into()) {
            return Some(state.data.into());
        }
        None
    }

    pub fn save_module_hash(&mut self, code_hash: &B256, module_hash: &B256) {
        let slot = map_address(&self.module_hashes_subkey(), code_hash);
        let _ = self.0.sstore(ARBOS_STATE_ADDRESS, slot.into(), (*module_hash).into());
    }

    pub fn program_info(&mut self, code_hash: &B256) -> Option<ProgramInfo> {
        let slot = map_address(&self.program_data_subkey(), code_hash);

        // warm account where useful
        let _ = self.0.journal_mut().warm_account(ARBOS_STATE_ADDRESS);

        if let Ok(state) = self.0.journal_mut().sload(ARBOS_STATE_ADDRESS, slot.into()) && !state.is_zero() {
            let data = state.data.to_be_bytes_vec();
            if data.len() < 15 { return None; }
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
                age: self.0.timestamp().to::<u32>().saturating_sub(activated_at.saturating_sub(ARBOS_GENESIS_TIMESTAMP) * 3600),
                cached,
            });
        }
        None
    }

    pub fn save_program_info(&mut self, code_hash: &B256, info: &ProgramInfo) -> u64 {
        let slot = map_address(&self.program_data_subkey(), code_hash);
        let mut data = [0u8; 32];
        data[0..2].copy_from_slice(&info.version.to_be_bytes());
        data[2..4].copy_from_slice(&info.init_cost.to_be_bytes());
        data[4..6].copy_from_slice(&info.cached_cost.to_be_bytes());
        data[6..8].copy_from_slice(&info.footprint.to_be_bytes());
        data[8..11].copy_from_slice(&info.asm_estimated_kb.to_be_bytes()[1..4]);
        let activated_at = info.age / 3600 + ARBOS_GENESIS_TIMESTAMP; // convert to hours
        data[11..14].copy_from_slice(&activated_at.to_be_bytes()[1..4]);
        data[14] = if info.cached { 1 } else { 0 };

        let value = U256::from_be_bytes(data);
        let res = self.0.sstore(ARBOS_STATE_ADDRESS, slot.into(), value).unwrap();
        sstore_cost(self.0.cfg().spec().into(), &res, true)
    }

    // stylus params read/write
    pub fn get_stylus_params(&mut self) -> (StylusParams, u64) {
        let subkey = self.params_subkey();
        let slot = map_address(&subkey, &B256::ZERO);

        let gas_cost = sload_cost(self.0.cfg().spec().into(), false);
        let _ = self.0.journal_mut().warm_account(ARBOS_STATE_ADDRESS);

        let mut params = StylusParams::zero();

        if let Some(state) = self.0.sload(ARBOS_STATE_ADDRESS, slot.into()) {
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

                if self.0.chain().arbos_version_or_default() >= 40 {
                    params.max_wasm_size = buffer::take_u32(&mut data);
                }

                params.page_ramp = self.0.chain().page_ramp_or_default();

                if params.max_wasm_size == 0 {
                    params.max_wasm_size = INITIAL_MAX_WASM_SIZE;
                }

                return (params, gas_cost);
            }
        }

        // Load defaults
        params.version = self.0.chain().stylus_version_or_default();
        params.ink_price = self.0.chain().ink_price_or_default();
        params.max_stack_depth = self.0.chain().max_stack_depth_or_default();
        params.free_pages = self.0.chain().free_pages_or_default();
        params.page_gas = self.0.chain().page_gas_or_default();
        params.page_ramp = self.0.chain().page_ramp_or_default();
        params.page_limit = self.0.chain().page_limit_or_default();
        params.min_init_gas = self.0.chain().min_init_gas_or_default();
        params.min_cached_init_gas = self.0.chain().min_cached_init_gas_or_default();
        params.init_cost_scalar = self.0.chain().init_cost_scalar_or_default();
        params.cached_cost_scalar = self.0.chain().cached_cost_scalar_or_default();
        params.expiry_days = self.0.chain().expiry_days_or_default();
        params.keepalive_days = self.0.chain().keepalive_days_or_default();
        params.block_cache_size = self.0.chain().block_cache_size_or_default();
        params.max_wasm_size = self.0.chain().max_wasm_size_or_default();

        (params, gas_cost)
    }

    pub fn save_stylus_params(&mut self, params: &StylusParams) {
        let subkey = self.params_subkey();
        let slot = map_address(&subkey, &B256::ZERO);

        let mut data = [0u8; 32];
        data[0..2].copy_from_slice(&params.version.to_be_bytes());
        data[2..6].copy_from_slice(&params.ink_price.to_be_bytes());
        data[6..10].copy_from_slice(&params.max_stack_depth.to_be_bytes());
        data[10..12].copy_from_slice(&params.free_pages.to_be_bytes());
        data[12..14].copy_from_slice(&params.page_gas.to_be_bytes());
        data[14..16].copy_from_slice(&params.page_limit.to_be_bytes());
        data[16] = params.min_init_gas;
        data[17] = params.min_cached_init_gas;
        data[18] = params.init_cost_scalar;
        data[19] = params.cached_cost_scalar;
        data[20..22].copy_from_slice(&params.expiry_days.to_be_bytes());
        data[22..24].copy_from_slice(&params.keepalive_days.to_be_bytes());
        data[24..26].copy_from_slice(&params.block_cache_size.to_be_bytes());

        if self.0.chain().arbos_version_or_default() >= 40 {
            data[26..30].copy_from_slice(&params.max_wasm_size.to_be_bytes());
        }

        let _ = self.0.sstore(ARBOS_STATE_ADDRESS, slot.into(), U256::from_be_bytes(data));
    }

    // data pricer
    pub fn get_data_pricer(&mut self) -> DataPricer {

        let demand =  StorageBackedU32::new(self.0, map_address(&self.data_pricer_subkey(), &B256::from(U256::from(DATA_PRICER_DEMAND_OFFSET as u64)))).get();
        let bytes_per_second = StorageBackedU32::new(self.0, map_address(&self.data_pricer_subkey(), &B256::from(U256::from(DATA_PRICER_BYTES_PER_SECOND_OFFSET as u64)))).get();
        let last_update_time = StorageBackedU64::new(self.0, map_address(&self.data_pricer_subkey(), &B256::from(U256::from(DATA_PRICER_LAST_UPDATE_TIME_OFFSET as u64)))).get();
        let min_price = StorageBackedU32::new(self.0, map_address(&self.data_pricer_subkey(), &B256::from(U256::from(DATA_PRICER_MIN_PRICE_OFFSET as u64)))).get();
        let inertia = StorageBackedU32::new(self.0, map_address(&self.data_pricer_subkey(), &B256::from(U256::from(DATA_PRICER_INERTIA_OFFSET as u64)))).get();

        DataPricer {
            demand: demand as u32,
            bytes_per_second: bytes_per_second as u32,
            last_update_time: last_update_time,
            min_price: min_price as u32,
            inertia: inertia as u32,
        }
    }

    pub fn update_data_pricer_model(&mut self, data_price: DataPricer, temp_bytes: u32, time: u64) -> u64 {
        let subkey = self.data_pricer_subkey();

        let mut demand = data_price.demand;
        let bytes_per_second = data_price.bytes_per_second;
        let last_update_time = data_price.last_update_time;
        let min_price = data_price.min_price;
        let inertia = data_price.inertia;

        let passed = time.saturating_sub(last_update_time) as u32;
        let credit = bytes_per_second.saturating_mul(passed);
        demand = demand.saturating_sub(credit);
        demand = demand.saturating_add(temp_bytes);

        // store updated values
        StorageBackedU32::new(self.0, map_address(&subkey, &B256::from(U256::from(DATA_PRICER_DEMAND_OFFSET as u64)))).set(demand);
        StorageBackedU32::new(self.0, map_address(&subkey, &B256::from(U256::from(DATA_PRICER_BYTES_PER_SECOND_OFFSET as u64)))).set(bytes_per_second);
        StorageBackedU64::new(self.0, map_address(&subkey, &B256::from(U256::from(DATA_PRICER_LAST_UPDATE_TIME_OFFSET as u64)))).set(time);
        StorageBackedU32::new(self.0, map_address(&subkey, &B256::from(U256::from(DATA_PRICER_MIN_PRICE_OFFSET as u64)))).set(min_price);
        StorageBackedU32::new(self.0, map_address(&subkey, &B256::from(U256::from(DATA_PRICER_INERTIA_OFFSET as u64)))).set(inertia);

        let exponent = (demand as f64) / (inertia as f64);
        let multiplier = f64::exp(exponent);
        let cost_per_byte = (min_price as f64 * multiplier).floor() as u64;
        let cost_in_wei = cost_per_byte.saturating_mul(temp_bytes as u64);

        cost_in_wei
    }

    // cache managers address set
    pub fn cache_managers<'b>(&'b mut self) -> StorageBackedAddressSet<'b, CTX> { StorageBackedAddressSet::new(self.0, self.cache_managers_subkey()) }
}
