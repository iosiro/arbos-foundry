pub trait ArbitrumChainInfoTr {
    fn arbos_version(&self) -> u16;
    fn stylus_version(&self) -> u16;
    fn max_depth(&self) -> u32;
    fn ink_price(&self) -> u32;
    fn debug_mode(&self) -> bool;

    fn auto_activate_stylus(&self) -> bool;
    fn auto_cache_stylus(&self) -> bool;
}

pub struct ArbitrumChainInfo {
    pub arbos_version: u16,
    pub stylus_version: u16,
    pub max_depth: u32,
    pub ink_price: u32,
    pub debug_mode: bool,

    pub auto_activate_stylus: bool,
    pub auto_cache_stylus: bool,
}

impl Default for ArbitrumChainInfo {
    fn default() -> Self {
        Self {
            arbos_version: 32,
            stylus_version: 2,
            max_depth: 4 * 65536,
            ink_price: 10000,
            debug_mode: false,

            auto_activate_stylus: false,
            auto_cache_stylus: false,
        }
    }
}

impl ArbitrumChainInfoTr for ArbitrumChainInfo {
    fn arbos_version(&self) -> u16 {
        self.arbos_version
    }

    fn stylus_version(&self) -> u16 {
        self.stylus_version
    }

    fn max_depth(&self) -> u32 {
        self.max_depth
    }

    fn ink_price(&self) -> u32 {
        self.ink_price
    }

    fn debug_mode(&self) -> bool {
        self.debug_mode
    }

    fn auto_activate_stylus(&self) -> bool {
        self.auto_activate_stylus
    }

    fn auto_cache_stylus(&self) -> bool {
        self.auto_cache_stylus
    }
}
