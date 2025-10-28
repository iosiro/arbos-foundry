use revm::{
    context::{Cfg, LocalContextTr},
    handler::PrecompileProvider,
    interpreter::{CallInput, Gas, InputsImpl, InstructionResult, InterpreterResult},
    precompile::PrecompileSpecId,
    primitives::{Address, Bytes, hardfork::SpecId},
};

use crate::{
    ArbitrumContextTr,
    precompiles::{
        arb_wasm::arb_wasm_precompile,
        arb_wasm_cache::arb_wasm_cache_precompile,
        extension::{ExtendedPrecompile, Precompile, Precompiles, PrecompilesContextTr},
    },
};
use std::sync::Arc;

mod arb_address_table;
mod arb_aggregator;
mod arb_debug;
mod arb_gas_info;
mod arb_info;
mod arb_native_token_manager;
mod arb_owner;
mod arb_owner_public;
mod arb_retryable_tx;
mod arb_statistics;
mod arb_sys;
mod arb_wasm;
mod arb_wasm_cache;

mod extension;

pub struct ArbitrumPrecompiles<CTX: PrecompilesContextTr> {
    /// Contains precompiles for the current spec.
    pub precompiles: Arc<Precompiles<CTX>>,
    /// Current spec. None means that spec was not set yet.
    pub spec: SpecId,
}

impl<CTX: PrecompilesContextTr> ArbitrumPrecompiles<CTX> {
    /// Returns addresses of the precompiles.
    pub fn warm_addresses(&self) -> Box<impl Iterator<Item = Address>> {
        Box::new(self.precompiles.addresses().cloned())
    }

    /// Returns whether the address is a precompile.
    pub fn contains(&self, address: &Address) -> bool {
        self.precompiles.contains(address)
    }
}

impl<CTX: PrecompilesContextTr> Clone for ArbitrumPrecompiles<CTX> {
    fn clone(&self) -> Self {
        Self { precompiles: self.precompiles.clone(), spec: self.spec }
    }
}

impl<CTX: ArbitrumContextTr> Default for ArbitrumPrecompiles<CTX> {
    fn default() -> Self {
        let spec = SpecId::default();
        let mut precompiles = Precompiles::new(PrecompileSpecId::from_spec_id(spec));

        precompiles.extend([
            // Arbitrum specific precompiles can be added here
            Precompile::Extended(arb_address_table::arb_address_table_precompile::<CTX>()),
            Precompile::Extended(arb_info::arb_info_precompile::<CTX>()),
            Precompile::Extended(arb_wasm_precompile::<CTX>()),
            Precompile::Extended(arb_wasm_cache_precompile::<CTX>()),
            Precompile::Extended(arb_owner::arb_owner_precompile::<CTX>()),
            Precompile::Extended(arb_owner_public::arb_owner_public_precompile::<CTX>()),
        ]);
        Self { precompiles: Arc::new(precompiles), spec }
    }
}

impl<CTX: PrecompilesContextTr> PrecompileProvider<CTX> for ArbitrumPrecompiles<CTX> {
    type Output = InterpreterResult;

    fn set_spec(&mut self, spec: <CTX::Cfg as Cfg>::Spec) -> bool {
        let spec = spec.into();
        // generate new precompiles only on new spec
        if spec == self.spec {
            return false;
        }
        self.precompiles = Arc::new(Precompiles::new(PrecompileSpecId::from_spec_id(spec)));
        self.spec = spec;
        true
    }

    fn run(
        &mut self,
        context: &mut CTX,
        address: &Address,
        inputs: &InputsImpl,
        is_static: bool,
        gas_limit: u64,
    ) -> Result<Option<InterpreterResult>, String> {
        // Get the precompile at the address
        let Some(precompile) = self.precompiles.get(address) else {
            return Ok(None);
        };

        if !is_static
            && inputs.target_address
                != inputs.bytecode_address.expect("bytecode address has to be set")
        {
            return Ok(Some(InterpreterResult {
                result: InstructionResult::Revert,
                output: Bytes::default(),
                gas: Gas::new(gas_limit),
            }));
        }

        // Execute the precompile
        let input_bytes = match &inputs.input {
            CallInput::SharedBuffer(range) => {
                #[allow(clippy::option_if_let_else)]
                if let Some(slice) = context.local().shared_memory_buffer_slice(range.clone()) {
                    slice.to_vec()
                } else {
                    vec![]
                }
            }
            CallInput::Bytes(bytes) => bytes.to_vec(),
        };

        precompile.call(
            context,
            input_bytes.as_slice(),
            address,
            inputs.caller_address,
            inputs.call_value,
            is_static,
            gas_limit,
        )
    }

    fn warm_addresses(&self) -> Box<impl Iterator<Item = Address>> {
        self.warm_addresses()
    }

    fn contains(&self, address: &Address) -> bool {
        self.contains(address)
    }
}
