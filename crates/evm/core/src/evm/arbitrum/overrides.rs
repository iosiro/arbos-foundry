//! Explicit tool-provided overrides layered over the context-aware ArbOS precompiles.

use std::borrow::Cow;

use alloy_evm::{
    Database, EvmInternals,
    precompiles::{Precompile, PrecompileInput, PrecompilesMap},
};
use alloy_primitives::{Address, map::AddressSet};
use arbos_revm::{ArbitrumContext, precompiles::ArbitrumPrecompileProvider};
use revm::{
    context::Context,
    handler::{PrecompileProvider, precompile_output_to_interpreter_result},
    interpreter::{CallInputs, InterpreterResult},
    precompile::Precompiles,
    primitives::hardfork::SpecId,
};

/// Keeps ArbOS execution authoritative unless a caller explicitly overrides an address.
pub struct ArbitrumPrecompileOverrides<DB: Database> {
    inner: ArbitrumPrecompileProvider<ArbitrumContext<DB>>,
    overrides: PrecompilesMap,
    removed: AddressSet,
    addresses: AddressSet,
}

impl<DB: Database> ArbitrumPrecompileOverrides<DB> {
    pub fn new(spec: SpecId) -> Self {
        let inner = ArbitrumPrecompileProvider::new(spec);
        let addresses = inner.warm_addresses().collect();
        Self {
            inner,
            overrides: PrecompilesMap::new(Cow::Owned(Precompiles::default())),
            removed: AddressSet::default(),
            addresses,
        }
    }

    pub fn set_overrides(&mut self, overrides: PrecompilesMap, removed: AddressSet) {
        self.overrides = overrides;
        self.removed = removed;
        self.refresh_addresses();
    }

    fn refresh_addresses(&mut self) {
        self.addresses =
            self.inner.warm_addresses().filter(|address| !self.removed.contains(address)).collect();
        self.addresses.extend(self.overrides.addresses().copied());
    }
}

impl<DB: Database> PrecompileProvider<ArbitrumContext<DB>> for ArbitrumPrecompileOverrides<DB> {
    type Output = InterpreterResult;

    fn set_spec(&mut self, spec: SpecId) -> bool {
        let changed = self.inner.set_spec(spec);
        if changed {
            self.refresh_addresses();
        }
        changed
    }

    fn run(
        &mut self,
        context: &mut ArbitrumContext<DB>,
        inputs: &CallInputs,
    ) -> Result<Option<InterpreterResult>, String> {
        let Some(precompile) = self.overrides.get(&inputs.bytecode_address) else {
            if self.removed.contains(&inputs.bytecode_address) {
                return Ok(None);
            }
            return self.inner.run(context, inputs);
        };

        let Context { block, tx, cfg, journaled_state, local, .. } = context;
        let output = precompile
            .call(PrecompileInput {
                data: inputs.input.as_bytes_local(local).as_ref(),
                gas: inputs.gas_limit,
                reservoir: inputs.reservoir,
                caller: inputs.caller,
                value: inputs.call_value(),
                is_static: inputs.is_static,
                internals: EvmInternals::new(journaled_state, block, cfg, tx),
                target_address: inputs.target_address,
                bytecode_address: inputs.bytecode_address,
            })
            .map_err(|err| err.to_string())?;
        Ok(Some(precompile_output_to_interpreter_result(output, inputs.gas_limit)))
    }

    fn warm_addresses(&self) -> &AddressSet {
        &self.addresses
    }

    fn contains(&self, address: &Address) -> bool {
        self.overrides.get(address).is_some()
            || (!self.removed.contains(address) && self.inner.contains(address))
    }
}
