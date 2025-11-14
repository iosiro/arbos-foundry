use std::ops::{Deref, DerefMut};

use auto_impl::auto_impl;
use revm::{
    context::{Cfg, CfgEnv},
    primitives::hardfork::SpecId,
};

use crate::constants::{
    INITIAL_ARBOS_VERSION, INITIAL_CACHED_COST_SCALAR, INITIAL_EXPIRY_DAYS, INITIAL_FREE_PAGES,
    INITIAL_INIT_COST_SCALAR, INITIAL_INK_PRICE, INITIAL_KEEPALIVE_DAYS, INITIAL_MAX_STACK_DEPTH,
    INITIAL_MAX_WASM_SIZE, INITIAL_MIN_CACHED_GAS, INITIAL_MIN_INIT_GAS, INITIAL_PAGE_GAS,
    INITIAL_PAGE_LIMIT, INITIAL_PAGE_RAMP, INITIAL_RECENT_CACHE_SIZE, INITIAL_STYLUS_VERSION,
};

#[auto_impl(&, &mut, Box, Arc)]
pub trait ArbitrumConfigTr: Cfg {
    type StylusConfigType: ArbitrumStylusConfigTr;

    fn stylus(&self) -> &Self::StylusConfigType;
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct ArbitrumConfig<SPEC = SpecId> {
    pub inner: CfgEnv<SPEC>,
    pub stylus: StylusConfig,
}

impl<SPEC> ArbitrumConfig<SPEC> {
    pub fn new_with_spec(spec: SPEC) -> Self
    where
        SPEC: Into<SpecId> + Copy,
    {
        Self { inner: CfgEnv::new_with_spec(spec), stylus: StylusConfig::default() }
    }
}

impl<SPEC> Default for ArbitrumConfig<SPEC>
where
    SPEC: Into<SpecId> + Copy + Default,
{
    fn default() -> Self {
        Self { inner: CfgEnv::default(), stylus: StylusConfig::default() }
    }
}

impl<SPEC: Into<SpecId> + Copy> Cfg for ArbitrumConfig<SPEC> {
    type Spec = SPEC;

    fn chain_id(&self) -> u64 {
        self.inner.chain_id()
    }

    fn tx_chain_id_check(&self) -> bool {
        self.inner.tx_chain_id_check()
    }

    fn tx_gas_limit_cap(&self) -> u64 {
        self.inner.tx_gas_limit_cap()
    }

    fn spec(&self) -> Self::Spec {
        self.inner.spec()
    }

    fn max_blobs_per_tx(&self) -> Option<u64> {
        self.inner.max_blobs_per_tx()
    }

    fn max_code_size(&self) -> usize {
        self.inner.max_code_size()
    }

    fn max_initcode_size(&self) -> usize {
        self.inner.max_initcode_size()
    }

    fn is_eip3607_disabled(&self) -> bool {
        self.inner.is_eip3607_disabled()
    }

    fn is_eip3541_disabled(&self) -> bool {
        self.inner.is_eip3541_disabled()
    }

    fn is_balance_check_disabled(&self) -> bool {
        self.inner.is_balance_check_disabled()
    }

    fn is_block_gas_limit_disabled(&self) -> bool {
        self.inner.is_block_gas_limit_disabled()
    }

    fn is_nonce_check_disabled(&self) -> bool {
        self.inner.is_nonce_check_disabled()
    }

    fn is_base_fee_check_disabled(&self) -> bool {
        self.inner.is_base_fee_check_disabled()
    }

    fn is_priority_fee_check_disabled(&self) -> bool {
        self.inner.is_priority_fee_check_disabled()
    }

    fn is_fee_charge_disabled(&self) -> bool {
        self.inner.is_fee_charge_disabled()
    }
}

impl<SPEC> ArbitrumConfigTr for ArbitrumConfig<SPEC>
where
    SPEC: Into<SpecId> + Copy + Copy,
{
    type StylusConfigType = StylusConfig;

    fn stylus(&self) -> &Self::StylusConfigType {
        &self.stylus
    }
}

impl<SPEC: Into<SpecId> + Copy> ArbitrumConfig<SPEC> {
    pub fn new(inner: CfgEnv<SPEC>, stylus: StylusConfig) -> Self {
        Self { inner, stylus }
    }
}

impl<SPEC> Deref for ArbitrumConfig<SPEC>
where
    SPEC: Into<SpecId> + Copy,
{
    type Target = CfgEnv<SPEC>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<SPEC> DerefMut for ArbitrumConfig<SPEC>
where
    SPEC: Into<SpecId> + Copy,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

pub trait ArbitrumStylusConfigTr {
    fn arbos_version(&self) -> u16;
    fn stylus_version(&self) -> u16;
    fn ink_price(&self) -> u32;
    fn max_stack_depth(&self) -> u32;
    fn free_pages(&self) -> u16;
    fn page_gas(&self) -> u16;
    fn page_ramp(&self) -> u64;
    fn page_limit(&self) -> u16;
    fn min_init_gas(&self) -> u8;
    fn min_cached_init_gas(&self) -> u8;
    fn init_cost_scalar(&self) -> u8;
    fn cached_cost_scalar(&self) -> u8;
    fn expiry_days(&self) -> u16;
    fn keepalive_days(&self) -> u16;
    fn block_cache_size(&self) -> u16;
    fn max_wasm_size(&self) -> u32;

    fn debug_mode(&self) -> bool;
    fn disable_auto_cache(&self) -> bool;
    fn disable_auto_activate(&self) -> bool;
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StylusConfig {
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub arbos_version: Option<u16>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub stylus_version: Option<u16>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub ink_price: Option<u32>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub max_stack_depth: Option<u32>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub free_pages: Option<u16>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub page_gas: Option<u16>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub page_ramp: Option<u64>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub page_limit: Option<u16>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub min_init_gas: Option<u8>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub min_cached_init_gas: Option<u8>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub init_cost_scalar: Option<u8>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub cached_cost_scalar: Option<u8>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub expiry_days: Option<u16>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub keepalive_days: Option<u16>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub block_cache_size: Option<u16>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub max_wasm_size: Option<u32>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "std::ops::Not::not"))]
    pub debug_mode: bool,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "std::ops::Not::not"))]
    pub disable_auto_cache: bool,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "std::ops::Not::not"))]
    pub disable_auto_activate: bool,
}

impl ArbitrumStylusConfigTr for StylusConfig {
    fn arbos_version(&self) -> u16 {
        self.arbos_version.unwrap_or(INITIAL_ARBOS_VERSION)
    }

    fn stylus_version(&self) -> u16 {
        self.stylus_version.unwrap_or(INITIAL_STYLUS_VERSION)
    }

    fn ink_price(&self) -> u32 {
        self.ink_price.unwrap_or(INITIAL_INK_PRICE)
    }

    fn max_stack_depth(&self) -> u32 {
        self.max_stack_depth.unwrap_or(INITIAL_MAX_STACK_DEPTH)
    }

    fn free_pages(&self) -> u16 {
        self.free_pages.unwrap_or(INITIAL_FREE_PAGES)
    }

    fn page_gas(&self) -> u16 {
        self.page_gas.unwrap_or(INITIAL_PAGE_GAS)
    }

    fn page_ramp(&self) -> u64 {
        self.page_ramp.unwrap_or(INITIAL_PAGE_RAMP)
    }

    fn page_limit(&self) -> u16 {
        self.page_limit.unwrap_or(INITIAL_PAGE_LIMIT)
    }

    fn min_init_gas(&self) -> u8 {
        self.min_init_gas.unwrap_or(INITIAL_MIN_INIT_GAS)
    }

    fn min_cached_init_gas(&self) -> u8 {
        self.min_cached_init_gas.unwrap_or(INITIAL_MIN_CACHED_GAS)
    }

    fn init_cost_scalar(&self) -> u8 {
        self.init_cost_scalar.unwrap_or(INITIAL_INIT_COST_SCALAR)
    }

    fn cached_cost_scalar(&self) -> u8 {
        self.cached_cost_scalar.unwrap_or(INITIAL_CACHED_COST_SCALAR)
    }

    fn expiry_days(&self) -> u16 {
        self.expiry_days.unwrap_or(INITIAL_EXPIRY_DAYS)
    }

    fn keepalive_days(&self) -> u16 {
        self.keepalive_days.unwrap_or(INITIAL_KEEPALIVE_DAYS)
    }

    fn block_cache_size(&self) -> u16 {
        self.block_cache_size.unwrap_or(INITIAL_RECENT_CACHE_SIZE)
    }

    fn max_wasm_size(&self) -> u32 {
        self.max_wasm_size.unwrap_or(INITIAL_MAX_WASM_SIZE)
    }

    fn debug_mode(&self) -> bool {
        self.debug_mode
    }

    fn disable_auto_cache(&self) -> bool {
        self.disable_auto_cache
    }

    fn disable_auto_activate(&self) -> bool {
        self.disable_auto_activate
    }
}
