use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StylusConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arbos_version: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stylus_version: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ink_price: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_stack_depth: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub free_pages: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_gas: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_ramp: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_limit: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_init_gas: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_cached_init_gas: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub init_cost_scalar: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_cost_scalar: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expiry_days: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keepalive_days: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_cache_size: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_wasm_size: Option<u32>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub debug_mode: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disable_auto_cache: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disable_auto_activate: bool,
}

impl StylusConfig {
    pub fn is_default(&self) -> bool {
        Self::default() == *self
    }
}
