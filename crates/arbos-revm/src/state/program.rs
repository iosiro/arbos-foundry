use arbutil::evm::WARM_SLOAD_GAS;
use revm::{
    interpreter::Gas,
    primitives::{B256, U256},
};

use crate::{
    ArbitrumContextTr, buffer,
    constants::{
        ARBOS_GENESIS_TIMESTAMP, ARBOS_PROGRAMS_STATE_CACHE_MANAGERS_KEY,
        ARBOS_PROGRAMS_STATE_DATA_PRICER_KEY, ARBOS_PROGRAMS_STATE_MODULE_HASHES_KEY,
        ARBOS_PROGRAMS_STATE_PARAMS_KEY, ARBOS_PROGRAMS_STATE_PROGRAM_DATA_KEY,
        INITIAL_CACHED_COST_SCALAR, INITIAL_DATA_PRICER_BYTES_PER_SECOND,
        INITIAL_DATA_PRICER_DEMAND, INITIAL_DATA_PRICER_INERTIA,
        INITIAL_DATA_PRICER_LAST_UPDATE_TIME, INITIAL_DATA_PRICER_MIN_PRICE, INITIAL_EXPIRY_DAYS,
        INITIAL_FREE_PAGES, INITIAL_INIT_COST_SCALAR, INITIAL_INK_PRICE, INITIAL_KEEPALIVE_DAYS,
        INITIAL_MAX_STACK_DEPTH, INITIAL_MAX_WASM_SIZE, INITIAL_MIN_CACHED_GAS,
        INITIAL_MIN_INIT_GAS, INITIAL_PAGE_GAS, INITIAL_PAGE_LIMIT, INITIAL_PAGE_RAMP,
        INITIAL_RECENT_CACHE_SIZE, INITIAL_STYLUS_VERSION,
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

impl Default for StylusParams {
    fn default() -> Self {
        Self {
            version: INITIAL_STYLUS_VERSION,
            ink_price: INITIAL_INK_PRICE,
            max_stack_depth: INITIAL_MAX_STACK_DEPTH,
            free_pages: INITIAL_FREE_PAGES,
            page_gas: INITIAL_PAGE_GAS,
            page_ramp: INITIAL_PAGE_RAMP,
            page_limit: INITIAL_PAGE_LIMIT,
            min_init_gas: INITIAL_MIN_INIT_GAS,
            min_cached_init_gas: INITIAL_MIN_CACHED_GAS,
            init_cost_scalar: INITIAL_INIT_COST_SCALAR,
            cached_cost_scalar: INITIAL_CACHED_COST_SCALAR,
            expiry_days: INITIAL_EXPIRY_DAYS,
            keepalive_days: INITIAL_KEEPALIVE_DAYS,
            block_cache_size: INITIAL_RECENT_CACHE_SIZE,
            max_wasm_size: INITIAL_MAX_WASM_SIZE,
        }
    }
}

pub struct StorageBackedStylusParams<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    context: &'a mut CTX,
    gas: Option<&'a mut Gas>,
    is_static: bool,
    subkey: B256,
}

impl<'a, CTX> StorageBackedStylusParams<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    pub fn new(
        context: &'a mut CTX,
        gas: Option<&'a mut Gas>,
        is_static: bool,
        subkey: B256,
    ) -> Self {
        Self { context, gas, is_static, subkey }
    }

    pub fn set(&mut self, params: &StylusParams) -> Result<(), ArbosStateError> {
        let slot = map_address(&self.subkey, &B256::ZERO);

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
        data[26..30].copy_from_slice(&params.max_wasm_size.to_be_bytes());

        let value = U256::from_be_bytes(data);
        StorageBackedB256::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
            .set(B256::from(value))
    }

    pub fn get(&mut self) -> Result<StylusParams, ArbosStateError> {
        let slot = map_address(&self.subkey, &B256::ZERO);

        if let Some(gas) = self.gas.as_deref_mut()
            && !gas.record_cost(WARM_SLOAD_GAS.0)
        {
            return Err(ArbosStateError::OutOfGas);
        }

        let data = StorageBackedB256::new(self.context, None, true, slot).get()?;

        let mut params = StylusParams::default();

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
            params.max_wasm_size = buffer::take_u32(&mut data);

            return Ok(params);
        }

        Ok(params)
    }
}

const DATA_PRICER_DEMAND_OFFSET: u8 = 0;
const DATA_PRICER_BYTES_PER_SECOND_OFFSET: u8 = 1;
const DATA_PRICER_LAST_UPDATE_TIME_OFFSET: u8 = 2;
const DATA_PRICER_MIN_PRICE_OFFSET: u8 = 3;
const DATA_PRICER_INERTIA_OFFSET: u8 = 4;

#[derive(Debug, Clone)]
pub struct DataPricerParams {
    pub demand: u32,
    pub bytes_per_second: u32,
    pub last_update_time: u64,
    pub min_price: u32,
    pub inertia: u32,
}

impl Default for DataPricerParams {
    fn default() -> Self {
        Self {
            demand: INITIAL_DATA_PRICER_DEMAND,
            bytes_per_second: INITIAL_DATA_PRICER_BYTES_PER_SECOND,
            last_update_time: INITIAL_DATA_PRICER_LAST_UPDATE_TIME,
            min_price: INITIAL_DATA_PRICER_MIN_PRICE,
            inertia: INITIAL_DATA_PRICER_INERTIA,
        }
    }
}

pub struct DataPricer<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    context: &'a mut CTX,
    gas: Option<&'a mut Gas>,
    is_static: bool,
    subkey: B256,
}

impl<'a, CTX> DataPricer<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    pub fn new(
        context: &'a mut CTX,
        gas: Option<&'a mut Gas>,
        is_static: bool,
        subkey: B256,
    ) -> Self {
        Self { context, gas, is_static, subkey }
    }

    fn demand(&mut self) -> StorageBackedU32<'_, CTX> {
        let slot =
            map_address(&self.subkey, &B256::from(U256::from(DATA_PRICER_DEMAND_OFFSET as u64)));
        StorageBackedU32::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
    }

    fn bytes_per_second(&mut self) -> StorageBackedU32<'_, CTX> {
        let slot = map_address(
            &self.subkey,
            &B256::from(U256::from(DATA_PRICER_BYTES_PER_SECOND_OFFSET as u64)),
        );
        StorageBackedU32::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
    }

    fn last_update_time(&mut self) -> StorageBackedU64<'_, CTX> {
        let slot = map_address(
            &self.subkey,
            &B256::from(U256::from(DATA_PRICER_LAST_UPDATE_TIME_OFFSET as u64)),
        );
        StorageBackedU64::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
    }

    fn min_price(&mut self) -> StorageBackedU32<'_, CTX> {
        let slot =
            map_address(&self.subkey, &B256::from(U256::from(DATA_PRICER_MIN_PRICE_OFFSET as u64)));
        StorageBackedU32::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
    }

    fn inertia(&mut self) -> StorageBackedU32<'_, CTX> {
        let slot =
            map_address(&self.subkey, &B256::from(U256::from(DATA_PRICER_INERTIA_OFFSET as u64)));
        StorageBackedU32::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
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

    pub fn set(&mut self, params: &DataPricerParams) -> Result<(), ArbosStateError> {
        self.demand().set(params.demand)?;
        self.bytes_per_second().set(params.bytes_per_second)?;
        self.last_update_time().set(params.last_update_time)?;
        self.min_price().set(params.min_price)?;
        self.inertia().set(params.inertia)?;

        Ok(())
    }

    pub fn get(&mut self) -> Result<DataPricerParams, ArbosStateError> {
        Ok(DataPricerParams {
            demand: self.demand().get()?,
            bytes_per_second: self.bytes_per_second().get()?,
            last_update_time: self.last_update_time().get()?,
            min_price: self.min_price().get()?,
            inertia: self.inertia().get()?,
        })
    }
}

#[derive(Debug, Default, Clone)]
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
    is_static: bool,
    subkey: B256,
}

impl<'a, CTX> Programs<'a, CTX>
where
    CTX: ArbitrumContextTr,
{
    pub fn new(
        context: &'a mut CTX,
        gas: Option<&'a mut Gas>,
        is_static: bool,
        subkey: B256,
    ) -> Self {
        Self { context, gas, is_static, subkey }
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

    pub fn module_hash(&mut self, code_hash: &B256) -> StorageBackedB256<'_, CTX> {
        let slot = map_address(&self.module_hashes_subkey(), code_hash);
        StorageBackedB256::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
    }

    pub fn program_info(
        &mut self,
        code_hash: &B256,
    ) -> Result<Option<ProgramInfo>, ArbosStateError> {
        let slot = map_address(&self.program_data_subkey(), code_hash);

        let data =
            StorageBackedB256::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
                .get()?;

        if !data.is_zero() && data.len() >= 15 {
            let version = u16::from_be_bytes([data[0], data[1]]);
            let init_cost = u16::from_be_bytes([data[2], data[3]]);
            let cached_cost = u16::from_be_bytes([data[4], data[5]]);
            let footprint = u16::from_be_bytes([data[6], data[7]]);
            let activated_at = u32::from_be_bytes([0, data[8], data[9], data[10]]);
            let asm_estimated_kb = u32::from_be_bytes([0, data[11], data[12], data[13]]);
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
                    .saturating_sub(activated_at * 3600 + ARBOS_GENESIS_TIMESTAMP),
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
        let activated_at =
            self.context.timestamp().to::<u32>().saturating_sub(ARBOS_GENESIS_TIMESTAMP) / 3600;
        data[11..14].copy_from_slice(&activated_at.to_be_bytes()[1..4]);
        data[14] = if info.cached { 1 } else { 0 };

        let value = U256::from_be_bytes(data);
        StorageBackedB256::new(self.context, self.gas.as_deref_mut(), self.is_static, slot)
            .set(B256::from(value))
    }

    pub fn get_active_program(
        &mut self,
        stylus_params: &StylusParams,
        code_hash: &B256,
    ) -> Result<ProgramInfo, ArbosStateError> {
        let program = self.program_info(code_hash)?;

        if let Some(program) = program {
            if program.version == 0 {
                return Err(ArbosStateError::ProgramNotActivated);
            }

            // check that the program is up to date
            let stylus_version = stylus_params.version;
            if program.version != stylus_version {
                return Err(ArbosStateError::ProgramNeedsUpgrade(program.version, stylus_version));
            }

            // ensure the program hasn't expired
            let max_age_seconds = (stylus_params.expiry_days as u32).saturating_mul(86400);
            if program.age > max_age_seconds {
                return Err(ArbosStateError::ProgramExpired(program.age));
            }

            Ok(program)
        } else {
            Err(ArbosStateError::ProgramNotActivated)
        }
    }

    // stylus params
    pub fn stylus_params(&mut self) -> StorageBackedStylusParams<'_, CTX> {
        let sub_key = self.params_subkey();
        StorageBackedStylusParams::new(
            self.context,
            self.gas.as_deref_mut(),
            self.is_static,
            sub_key,
        )
    }

    // data pricer
    pub fn data_pricer(&mut self) -> DataPricer<'_, CTX> {
        let sub_key = self.data_pricer_subkey();
        DataPricer::new(self.context, self.gas.as_deref_mut(), self.is_static, sub_key)
    }

    // cache managers address set
    pub fn cache_managers<'b>(&'b mut self) -> StorageBackedAddressSet<'b, CTX> {
        let sub_key = self.cache_managers_subkey();
        StorageBackedAddressSet::new(self.context, self.gas.as_deref_mut(), self.is_static, sub_key)
    }

    pub fn initialize(
        &mut self,
        stylus_params: &StylusParams,
        data_pricer_params: &DataPricerParams,
    ) -> Result<(), ArbosStateError> {
        self.stylus_params().set(stylus_params)?;
        self.data_pricer().set(data_pricer_params)
    }
}
