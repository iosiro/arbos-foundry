use alloy_evm::{eth::EthEvmContext, precompiles::PrecompilesMap, EthEvm};
use revm::
    handler::EthPrecompiles
;

pub type EitherEvm<DB, I, P = PrecompilesMap<EthEvmContext<DB>, EthPrecompiles>> =
    EthEvm<DB, I, P>;