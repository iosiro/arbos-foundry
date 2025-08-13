use std::{boxed::Box, sync::Arc, vec::Vec};

use arbutil::evm::{
    api::{EvmApiMethod, Gas, VecReader},
    req::RequestHandler,
};
use revm::{context::Cfg, interpreter::gas::warm_cold_cost};

use crate::api::ArbitrumContextTr;

pub(crate) type HostCallFunc = dyn Fn(
    arbutil::evm::api::EvmApiMethod,
    Vec<u8>,
) -> (Vec<u8>, VecReader, arbutil::evm::api::Gas);

pub(crate) struct StylusHandler {
    pub handler: Arc<Box<HostCallFunc>>,
}

unsafe impl Send for StylusHandler {}

impl StylusHandler {
    pub(crate) fn new(handler: Arc<Box<HostCallFunc>>) -> Self {
        Self { handler }
    }
}

impl RequestHandler<VecReader> for StylusHandler {
    fn request(
        &mut self,
        req_type: EvmApiMethod,
        req_data: impl AsRef<[u8]>,
    ) -> (Vec<u8>, VecReader, Gas) {
        let data = req_data.as_ref().to_vec();
        let api = self.handler.clone();
        (api)(req_type, data)
    }
}

pub fn wasm_account_touch<CTX>(context: CTX, is_cold: bool, with_code: bool) -> u64
where
    CTX: ArbitrumContextTr,
{
    let code_cost = if with_code { context.cfg().max_code_size() as u64 / 24576 * 700 } else { 0 };
    code_cost + warm_cold_cost(is_cold)
}
