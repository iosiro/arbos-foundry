use alloy_primitives::Address;
use clap::Parser;
use serde::{Deserialize, Serialize};

/// ArbOS and Stylus execution settings used by local Arbitrum backends.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Parser)]
#[command(next_help_heading = "Stylus options")]
pub struct StylusConfig {
    /// ArbOS version used when initializing an empty local backend.
    #[arg(long = "arbos-version", value_name = "ARBOS_VERSION")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arbos_version: Option<u64>,

    /// Stylus version to use for Stylus programs.
    #[arg(long = "stylus-version", value_name = "STYLUS_VERSION")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stylus_version: Option<u16>,

    /// Price of ink in gas.
    #[arg(long = "stylus-ink-price", value_name = "INK_PRICE")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ink_price: Option<u32>,

    /// Maximum stack depth for Stylus programs.
    #[arg(long = "stylus-max-stack-depth", value_name = "STYLUS_MAX_STACK_DEPTH")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_stack_depth: Option<u32>,

    /// Number of free pages for Stylus programs.
    #[arg(long = "stylus-free-pages", value_name = "STYLUS_FREE_PAGES")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub free_pages: Option<u16>,

    /// Gas cost per page for Stylus programs.
    #[arg(long = "stylus-page-gas", value_name = "STYLUS_PAGE_GAS")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_gas: Option<u16>,

    /// Exponential page-cost ramp.
    #[arg(long = "stylus-page-ramp", value_name = "STYLUS_PAGE_RAMP")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_ramp: Option<u64>,

    /// Page limit for Stylus programs.
    #[arg(long = "stylus-page-limit", value_name = "STYLUS_PAGE_LIMIT")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_limit: Option<u16>,

    /// Maximum number of compiled Stylus fragments.
    #[arg(long = "stylus-max-fragment-count", value_name = "STYLUS_MAX_FRAGMENT_COUNT")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_fragment_count: Option<u8>,

    /// Minimum uncached initialization gas exponent.
    #[arg(long = "stylus-min-init-gas", value_name = "STYLUS_MIN_INIT_GAS")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_init_gas: Option<u8>,

    /// Minimum cached initialization gas exponent.
    #[arg(long = "stylus-min-cached-init-gas", value_name = "STYLUS_MIN_CACHED_INIT_GAS")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_cached_init_gas: Option<u8>,

    /// Initial program cost scalar.
    #[arg(long = "stylus-init-cost-scalar", value_name = "STYLUS_INIT_COST_SCALAR")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub init_cost_scalar: Option<u8>,

    /// Cached program cost scalar.
    #[arg(long = "stylus-cached-cost-scalar", value_name = "STYLUS_CACHED_COST_SCALAR")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_cost_scalar: Option<u8>,

    /// Days before a Stylus program expires.
    #[arg(long = "stylus-expiry-days", value_name = "STYLUS_EXPIRY_DAYS")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expiry_days: Option<u16>,

    /// Days to keep a Stylus program alive after last use.
    #[arg(long = "stylus-keepalive-days", value_name = "STYLUS_KEEPALIVE_DAYS")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keepalive_days: Option<u16>,

    /// Size of the per-block Stylus cache.
    #[arg(long = "stylus-block-cache-size", value_name = "STYLUS_BLOCK_CACHE_SIZE")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_cache_size: Option<u16>,

    /// Maximum compressed Wasm size.
    #[arg(long = "stylus-max-wasm-size", value_name = "STYLUS_MAX_WASM_SIZE")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_wasm_size: Option<u32>,

    /// Disables automatic caching of Stylus programs.
    #[arg(long = "stylus-disable-auto-cache")]
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disable_auto_cache_stylus: bool,

    /// Disables automatic activation of Stylus programs.
    #[arg(long = "stylus-disable-auto-activate")]
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disable_auto_activate_stylus: bool,

    /// Enables Stylus debug mode.
    #[arg(long = "stylus-debug")]
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub debug_mode_stylus: bool,

    /// Stylus deployer contract used by deployment cheatcodes.
    #[arg(long = "stylus-deployer-address", value_name = "STYLUS_DEPLOYER_ADDRESS")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployer_address: Option<Address>,

    /// Disables deployment of Stylus programs, primarily for EIP-3541 compatibility tests.
    #[arg(long = "stylus-disable-deployment")]
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disable_stylus_deployment: bool,
}

impl StylusConfig {
    /// Returns whether no Stylus option was explicitly configured.
    pub fn is_default(&self) -> bool {
        Self::default() == *self
    }
}
