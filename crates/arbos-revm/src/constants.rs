const STYLUS_EOF_MAGIC: u8 = 0xEF;
const STYLUS_EOF_MAGIC_SUFFIX: u8 = 0xF0;
const STYLUS_EOF_VERSION: u8 = 0x00;

pub const STYLUS_DISCRIMINANT: &[u8] =
    &[STYLUS_EOF_MAGIC, STYLUS_EOF_MAGIC_SUFFIX, STYLUS_EOF_VERSION];

pub const INITIAL_MAX_WASM_SIZE: usize = 128 * 1024; // max decompressed wasm size (programs are also bounded by compressed size)
pub const INITIAL_STACK_DEPTH: usize = 4 * 65536; // 4 page stack.
pub const INITIAL_FREE_PAGES: u32 = 2; // 2 pages come free
pub const INITIAL_PAGE_GAS: u64 = 1000; // linear cost per allocation.
pub const INITIAL_PAGE_RAMP: u64 = 620674314; // targets 8
pub const INITIAL_PAGE_LIMIT: u32 = 128; // reject wasms with memories larger than 8MB.
pub const INITIAL_INK_PRICE: u64 = 10000; // 1 evm
pub const INITIAL_MIN_INIT_GAS: u64 = 72; // charge 72 * 128 = 9216 gas.
pub const INITIAL_MIN_CACHED_GAS: u64 = 11; // charge 11
pub const INITIAL_INIT_COST_SCALAR: u32 = 50; // scale costs 1:1 (100%)
pub const INITIAL_CACHED_COST_SCALAR: u32 = 50; // scale costs
pub const INITIAL_EXPIRY_DAYS: u32 = 365; // deactivate after 1 year.
pub const INITIAL_KEEPALIVE_DAYS: u32 = 31; // wait a month
pub const INITIAL_RECENT_CACHE_SIZE: usize = 32; // cache the 32 most recent programs.

pub const MIN_INIT_GAS_UNITS: u64 = 128;
pub const MIN_CACHED_GAS_UNITS: u64 = 32;
pub const COST_SCALAR_PERCENT: u64 = 2;

pub const MEMORY_EXPONENTS: [u32; 129] = [
    1, 1, 1, 1, 1, 1, 2, 2, 2, 3, 3, 4, 5, 5, 6, 7, 8, 9, 11, 12, 14, 17, 19, 22, 25, 29, 33, 38,
    43, 50, 57, 65, 75, 85, 98, 112, 128, 147, 168, 193, 221, 253, 289, 331, 379, 434, 497, 569,
    651, 745, 853, 976, 1117, 1279, 1463, 1675, 1917, 2194, 2511, 2874, 3290, 3765, 4309, 4932,
    5645, 6461, 7395, 8464, 9687, 11087, 12689, 14523, 16621, 19024, 21773, 24919, 28521, 32642,
    37359, 42758, 48938, 56010, 64104, 73368, 83971, 96106, 109994, 125890, 144082, 164904, 188735,
    216010, 247226, 282953, 323844, 370643, 424206, 485509, 555672, 635973, 727880, 833067, 953456,
    1091243, 1248941, 1429429, 1636000, 1872423, 2143012, 2452704, 2807151, 3212820, 3677113,
    4208502, 4816684, 5512756, 6309419, 7221210, 8264766, 9459129, 10826093, 12390601, 14181199,
    16230562, 18576084, 21260563, 24332984, 27849408, 31873999,
];
