use revm::{
    interpreter::Gas,
    primitives::{B256, U256},
};

use crate::{
    ArbitrumContextTr, buffer,
    config::{ArbitrumConfigTr, ArbitrumStylusConfigTr},
    constants::{
        ARBOS_GENESIS_TIMESTAMP, ARBOS_PROGRAMS_STATE_CACHE_MANAGERS_KEY,
        ARBOS_PROGRAMS_STATE_DATA_PRICER_KEY, ARBOS_PROGRAMS_STATE_MODULE_HASHES_KEY,
        ARBOS_PROGRAMS_STATE_PARAMS_KEY, ARBOS_PROGRAMS_STATE_PROGRAM_DATA_KEY,
        INITIAL_MAX_WASM_SIZE,
    },
    state::types::{
        ArbosStateError, StorageBackedAddressSet, StorageBackedB256, StorageBackedTr,
        StorageBackedU32, StorageBackedU64, map_address, substorage,
    },
};

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

const DATA_PRICER_DEMAND_OFFSET: u8 = 0;
const DATA_PRICER_BYTES_PER_SECOND_OFFSET: u8 = 1;
const DATA_PRICER_LAST_UPDATE_TIME_OFFSET: u8 = 2;
const DATA_PRICER_MIN_PRICE_OFFSET: u8 = 3;
const DATA_PRICER_INERTIA_OFFSET: u8 = 4;

pub struct DataPricer<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    context: &'a mut CTX,
    gas: Option<&'a mut Gas>,
    subkey: B256,
}

impl<'a, CTX> DataPricer<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    pub fn new(context: &'a mut CTX, gas: Option<&'a mut Gas>, subkey: B256) -> Self {
        Self { context, gas, subkey }
    }

    fn demand(&mut self) -> StorageBackedU32<'_, CTX> {
        let slot =
            map_address(&self.subkey, &B256::from(U256::from(DATA_PRICER_DEMAND_OFFSET as u64)));
        StorageBackedU32::new(self.context, self.gas.as_deref_mut(), slot)
    }

    fn bytes_per_second(&mut self) -> StorageBackedU32<'_, CTX> {
        let slot = map_address(
            &self.subkey,
            &B256::from(U256::from(DATA_PRICER_BYTES_PER_SECOND_OFFSET as u64)),
        );
        StorageBackedU32::new(self.context, self.gas.as_deref_mut(), slot)
    }

    fn last_update_time(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = map_address(
            &self.subkey,
            &B256::from(U256::from(DATA_PRICER_LAST_UPDATE_TIME_OFFSET as u64)),
        );
        StorageBackedU64::new(self.context, self.gas.as_deref_mut(), slot)
    }

    fn min_price(&mut self) -> StorageBackedU32<'_, CTX> {
        let slot =
            map_address(&self.subkey, &B256::from(U256::from(DATA_PRICER_MIN_PRICE_OFFSET as u64)));
        StorageBackedU32::new(self.context, self.gas.as_deref_mut(), slot)
    }

    fn inertia(&mut self) -> StorageBackedU32<'_, CTX> {
        let slot =
            map_address(&self.subkey, &B256::from(U256::from(DATA_PRICER_INERTIA_OFFSET as u64)));
        StorageBackedU32::new(self.context, self.gas.as_deref_mut(), slot)
    }

    pub fn update(&mut self, temp_bytes: u32, time: u64) -> Result<u64, ArbosStateError> {
        let bytes_per_second = self.bytes_per_second().get()?;

        let mut demand = self.demand().get()?;

        let last_update_time = self.last_update_time().get()?;

        let min_price = self.min_price().get()?;

        let inertia = self.inertia().get()?;

        let credit = bytes_per_second.saturating_mul(time.saturating_sub(last_update_time) as u32);
        demand = demand.saturating_sub(credit);
        demand = demand.saturating_add(temp_bytes);

        self.demand().set(demand)?;
        self.last_update_time().set(time)?;

        let exponent = (demand as f64) / (inertia as f64);
        let multiplier = f64::exp(exponent);
        let cost_per_byte = (min_price as f64 * multiplier).floor() as u64;
        Ok(cost_per_byte.saturating_mul(temp_bytes as u64))
    }
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

pub struct Programs<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    context: &'a mut CTX,
    gas: Option<&'a mut Gas>,
    subkey: B256,
}

impl<'a, CTX> Programs<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    pub fn new(context: &'a mut CTX, gas: Option<&'a mut Gas>, subkey: B256) -> Self {
        Self { context, gas, subkey }
    }

    fn params_subkey(&self) -> B256 {
        substorage(&self.subkey, ARBOS_PROGRAMS_STATE_PARAMS_KEY)
    }
    fn program_data_subkey(&self) -> B256 {
        substorage(&self.subkey, ARBOS_PROGRAMS_STATE_PROGRAM_DATA_KEY)
    }
    fn module_hashes_subkey(&self) -> B256 {
        substorage(&self.subkey, ARBOS_PROGRAMS_STATE_MODULE_HASHES_KEY)
    }
    fn data_pricer_subkey(&self) -> B256 {
        substorage(&self.subkey, ARBOS_PROGRAMS_STATE_DATA_PRICER_KEY)
    }
    fn cache_managers_subkey(&self) -> B256 {
        substorage(&self.subkey, ARBOS_PROGRAMS_STATE_CACHE_MANAGERS_KEY)
    }

    pub fn get_module_hash(&mut self, code_hash: &B256) -> Result<B256, ArbosStateError> {
        let slot = map_address(&self.module_hashes_subkey(), code_hash);
        StorageBackedB256::new(self.context, self.gas.as_deref_mut(), slot).get()
    }

    pub fn save_module_hash(
        &mut self,
        code_hash: &B256,
        module_hash: B256,
    ) -> Result<(), ArbosStateError> {
        let slot = map_address(&self.module_hashes_subkey(), code_hash);
        StorageBackedB256::new(self.context, self.gas.as_deref_mut(), slot).set(module_hash)
    }

    pub fn program_info(
        &mut self,
        code_hash: &B256,
    ) -> Result<Option<ProgramInfo>, ArbosStateError> {
        let slot = map_address(&self.program_data_subkey(), code_hash);

        let data = StorageBackedB256::new(self.context, self.gas.as_deref_mut(), slot).get()?;

        if !data.is_zero() && data.len() >= 15 {
            let version = u16::from_be_bytes([data[0], data[1]]);
            let init_cost = u16::from_be_bytes([data[2], data[3]]);
            let cached_cost = u16::from_be_bytes([data[4], data[5]]);
            let footprint = u16::from_be_bytes([data[6], data[7]]);
            let asm_estimated_kb = u32::from_be_bytes([0, data[8], data[9], data[10]]);
            let activated_at = u32::from_be_bytes([0, data[11], data[12], data[13]]);
            let cached = data[14] != 0;

            return Ok(Some(ProgramInfo {
                version,
                init_cost,
                cached_cost,
                footprint,
                asm_estimated_kb,
                age: self
                    .context
                    .timestamp()
                    .to::<u32>()
                    .saturating_sub(activated_at.saturating_sub(ARBOS_GENESIS_TIMESTAMP) * 3600),
                cached,
            }));
        }

        Ok(None)
    }

    pub fn save_program_info(
        &mut self,
        code_hash: &B256,
        info: &ProgramInfo,
    ) -> Result<(), ArbosStateError> {
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
        StorageBackedB256::new(self.context, self.gas.as_deref_mut(), slot).set(B256::from(value))
    }

    // stylus params read/write
    pub fn get_stylus_params(&mut self) -> Result<StylusParams, ArbosStateError> {
        let subkey = self.params_subkey();
        let slot = map_address(&subkey, &B256::ZERO);

        // if let Some(gas) = self.gas.as_deref_mut() {
        //     if !gas.record_cost(WARM_SLOAD_GAS.0) {
        //         return Err(ArbosStateError::OutOfGas);
        //     }
        // }
        let data = StorageBackedB256::new(self.context, None, slot).get()?;

        let mut params = StylusParams::zero();

        if !data.is_zero() {
            let mut data = data.to_vec();
            params.version = buffer::take_u16(&mut data);
            params.ink_price = buffer::take_u24(&mut data);
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

            if self.context.cfg().stylus().arbos_version() >= 40 {
                params.max_wasm_size = buffer::take_u32(&mut data);
            }

            params.page_ramp = self.context.cfg().stylus().page_ramp();

            if params.max_wasm_size == 0 {
                params.max_wasm_size = INITIAL_MAX_WASM_SIZE;
            }

            return Ok(params);
        }

        // Load defaults
        params.version = self.context.cfg().stylus().stylus_version();
        params.ink_price = self.context.cfg().stylus().ink_price();
        params.max_stack_depth = self.context.cfg().stylus().max_stack_depth();
        params.free_pages = self.context.cfg().stylus().free_pages();
        params.page_gas = self.context.cfg().stylus().page_gas();
        params.page_ramp = self.context.cfg().stylus().page_ramp();
        params.page_limit = self.context.cfg().stylus().page_limit();
        params.min_init_gas = self.context.cfg().stylus().min_init_gas();
        params.min_cached_init_gas = self.context.cfg().stylus().min_cached_init_gas();
        params.init_cost_scalar = self.context.cfg().stylus().init_cost_scalar();
        params.cached_cost_scalar = self.context.cfg().stylus().cached_cost_scalar();
        params.expiry_days = self.context.cfg().stylus().expiry_days();
        params.keepalive_days = self.context.cfg().stylus().keepalive_days();
        params.block_cache_size = self.context.cfg().stylus().block_cache_size();
        params.max_wasm_size = self.context.cfg().stylus().max_wasm_size();

        Ok(params)
    }

    pub fn save_stylus_params(&mut self, params: &StylusParams) -> Result<(), ArbosStateError> {
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

        if self.context.cfg().stylus().arbos_version() >= 40 {
            data[26..30].copy_from_slice(&params.max_wasm_size.to_be_bytes());
        }

        StorageBackedB256::new(self.context, self.gas.as_deref_mut(), slot).set(B256::from(data))
    }

    // data pricer
    pub fn data_pricer(&mut self) -> DataPricer<'_, CTX> {
        let sub_key = self.data_pricer_subkey();
        DataPricer::new(self.context, self.gas.as_deref_mut(), sub_key)
    }

    // cache managers address set
    pub fn cache_managers<'b>(&'b mut self) -> StorageBackedAddressSet<'b, CTX> {
        let sub_key = self.cache_managers_subkey();
        StorageBackedAddressSet::new(self.context, self.gas.as_deref_mut(), sub_key)
    }
}
