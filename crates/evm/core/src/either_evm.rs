use alloy_evm::{EthEvm, eth::EthEvmContext, precompiles::PrecompilesMap};
use revm::handler::EthPrecompiles;

pub type EitherEvm<DB, I, P = PrecompilesMap<EthEvmContext<DB>, EthPrecompiles>> = EthEvm<DB, I, P>;
