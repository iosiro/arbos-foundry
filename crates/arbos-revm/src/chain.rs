use crate::constants::{
    INITIAL_ARBOS_VERSION, INITIAL_CACHED_COST_SCALAR, INITIAL_EXPIRY_DAYS,
    INITIAL_FREE_PAGES, INITIAL_INIT_COST_SCALAR, INITIAL_INK_PRICE, INITIAL_KEEPALIVE_DAYS,
    INITIAL_MAX_STACK_DEPTH, INITIAL_MAX_WASM_SIZE, INITIAL_MIN_CACHED_GAS, INITIAL_MIN_INIT_GAS,
    INITIAL_PAGE_GAS, INITIAL_PAGE_LIMIT, INITIAL_PAGE_RAMP, INITIAL_RECENT_CACHE_SIZE,
    INITIAL_STYLUS_VERSION,
};

pub trait ArbitrumChainInfoTr {
    fn arbos_version(&self) -> Option<u16>;
    fn stylus_version(&self) -> Option<u16>;
    fn ink_price(&self) -> Option<u32>;
    fn max_stack_depth(&self) -> Option<u32>;
    fn free_pages(&self) -> Option<u16>;
    fn page_gas(&self) -> Option<u16>;
    fn page_ramp(&self) -> Option<u64>;
    fn page_limit(&self) -> Option<u16>;
    fn min_init_gas(&self) -> Option<u8>;
    fn min_cached_init_gas(&self) -> Option<u8>;
    fn init_cost_scalar(&self) -> Option<u8>;
    fn cached_cost_scalar(&self) -> Option<u8>;
    fn expiry_days(&self) -> Option<u16>;
    fn keepalive_days(&self) -> Option<u16>;
    fn block_cache_size(&self) -> Option<u16>;
    fn max_wasm_size(&self) -> Option<u32>;


    fn arbos_version_or_default(&self) -> u16;
    fn stylus_version_or_default(&self) -> u16;
    fn ink_price_or_default(&self) -> u32;
    fn max_stack_depth_or_default(&self) -> u32;
    fn free_pages_or_default(&self) -> u16;
    fn page_gas_or_default(&self) -> u16;
    fn page_ramp_or_default(&self) -> u64;
    fn page_limit_or_default(&self) -> u16;
    fn min_init_gas_or_default(&self) -> u8;
    fn min_cached_init_gas_or_default(&self) -> u8;
    fn init_cost_scalar_or_default(&self) -> u8;
    fn cached_cost_scalar_or_default(&self) -> u8;
    fn expiry_days_or_default(&self) -> u16;
    fn keepalive_days_or_default(&self) -> u16;
    fn block_cache_size_or_default(&self) -> u16;
    fn max_wasm_size_or_default(&self) -> u32; 

    fn debug_mode(&self) -> bool;
    fn enforce_activate_stylus(&self) -> bool;
    fn enforce_cache_stylus(&self) -> bool;
}

#[derive(Clone, Debug, Default)]
pub struct ArbitrumChainInfo {
    pub arbos_version: Option<u16>,
    pub stylus_version: Option<u16>,
    pub ink_price: Option<u32>,
    pub max_stack_depth: Option<u32>,
    pub free_pages: Option<u16>,
    pub page_gas: Option<u16>,
    pub page_ramp: Option<u64>,
    pub page_limit: Option<u16>,
    pub min_init_gas: Option<u8>,
    pub min_cached_init_gas: Option<u8>,
    pub init_cost_scalar: Option<u8>,
    pub cached_cost_scalar: Option<u8>,
    pub expiry_days: Option<u16>,
    pub keepalive_days: Option<u16>,
    pub block_cache_size: Option<u16>,
    pub max_wasm_size: Option<u32>,

    pub debug_mode: bool,
    pub enforce_activate_stylus: bool,
    pub enforce_cache_stylus: bool,
}

impl ArbitrumChainInfoTr for ArbitrumChainInfo {
    fn arbos_version(&self) -> Option<u16> {
        self.arbos_version
    }

    fn stylus_version(&self) -> Option<u16> {
        self.stylus_version
    }

    fn ink_price(&self) -> Option<u32> {
        self.ink_price
    }

    fn max_stack_depth(&self) -> Option<u32> {
        self.max_stack_depth
    }

    fn free_pages(&self) -> Option<u16> {
        self.free_pages
    }

    fn page_gas(&self) -> Option<u16> {
        self.page_gas
    }

    fn page_ramp(&self) -> Option<u64> {
        self.page_ramp
    }

    fn page_limit(&self) -> Option<u16> {
        self.page_limit
    }

    fn min_init_gas(&self) -> Option<u8> {
        self.min_init_gas
    }

    fn min_cached_init_gas(&self) -> Option<u8> {
        self.min_cached_init_gas
    }

    fn init_cost_scalar(&self) -> Option<u8> {
        self.init_cost_scalar
    }

    fn cached_cost_scalar(&self) -> Option<u8> {
        self.cached_cost_scalar
    }

    fn expiry_days(&self) -> Option<u16> {
        self.expiry_days
    }

    fn keepalive_days(&self) -> Option<u16> {
        self.keepalive_days
    }

    fn block_cache_size(&self) -> Option<u16> {
        self.block_cache_size
    }

    fn max_wasm_size(&self) -> Option<u32> {
        self.max_wasm_size
    }
 
    fn arbos_version_or_default(&self) -> u16 {
        self.arbos_version.unwrap_or(INITIAL_ARBOS_VERSION)
    }

    fn stylus_version_or_default(&self) -> u16 {
        self.stylus_version.unwrap_or(INITIAL_STYLUS_VERSION)
    }

    fn ink_price_or_default(&self) -> u32 {
        self.ink_price.unwrap_or(INITIAL_INK_PRICE)
    }

    fn max_stack_depth_or_default(&self) -> u32 {
        self.max_stack_depth.unwrap_or(INITIAL_MAX_STACK_DEPTH)
    }

    fn free_pages_or_default(&self) -> u16 {
        self.free_pages.unwrap_or(INITIAL_FREE_PAGES)
    }

    fn page_gas_or_default(&self) -> u16 {
        self.page_gas.unwrap_or(INITIAL_PAGE_GAS)
    }

    fn page_ramp_or_default(&self) -> u64 {
        self.page_ramp.unwrap_or(INITIAL_PAGE_RAMP)
    }

    fn page_limit_or_default(&self) -> u16 {
        self.page_limit.unwrap_or(INITIAL_PAGE_LIMIT)
    }

    fn min_init_gas_or_default(&self) -> u8 {
        self.min_init_gas.unwrap_or(INITIAL_MIN_INIT_GAS)
    }

    fn min_cached_init_gas_or_default(&self) -> u8 {
        self.min_cached_init_gas.unwrap_or(INITIAL_MIN_CACHED_GAS)
    }

    fn init_cost_scalar_or_default(&self) -> u8 {
        self.init_cost_scalar.unwrap_or(INITIAL_INIT_COST_SCALAR)
    }

    fn cached_cost_scalar_or_default(&self) -> u8 {
        self.cached_cost_scalar.unwrap_or(INITIAL_CACHED_COST_SCALAR)
    }

    fn expiry_days_or_default(&self) -> u16 {
        self.expiry_days.unwrap_or(INITIAL_EXPIRY_DAYS)
    }

    fn keepalive_days_or_default(&self) -> u16 {
        self.keepalive_days.unwrap_or(INITIAL_KEEPALIVE_DAYS)
    }

    fn block_cache_size_or_default(&self) -> u16 {
        self.block_cache_size.unwrap_or(INITIAL_RECENT_CACHE_SIZE)
    }

    fn max_wasm_size_or_default(&self) -> u32 {
        self.max_wasm_size.unwrap_or(INITIAL_MAX_WASM_SIZE)
    }

    fn debug_mode(&self) -> bool {
        self.debug_mode
    }

    fn enforce_activate_stylus(&self) -> bool {
        self.enforce_activate_stylus
    }

    fn enforce_cache_stylus(&self) -> bool {
        self.enforce_cache_stylus
    }      
}
