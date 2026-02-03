//! Foundry precompiles implementation.
//!
//! This module provides `FoundryPrecompiles`, a wrapper around any `PrecompileProvider` that
//! supports dynamic/closure precompiles with priority over the wrapped provider.

use alloy_primitives::{Address, Bytes, U256, address, map::HashMap};
use foundry_evm_networks::ExtendablePrecompiles;
use revm::{
    Context, Database, Journal,
    context::{Cfg, LocalContextTr},
    handler::PrecompileProvider,
    interpreter::{CallInput, CallInputs, Gas, InstructionResult, InterpreterResult},
    precompile::{PrecompileError, PrecompileId, PrecompileResult},
};
use std::sync::Arc;

/// The ECRecover precompile address.
pub const EC_RECOVER: Address = address!("0x0000000000000000000000000000000000000001");

/// The SHA-256 precompile address.
pub const SHA_256: Address = address!("0x0000000000000000000000000000000000000002");

/// The RIPEMD-160 precompile address.
pub const RIPEMD_160: Address = address!("0x0000000000000000000000000000000000000003");

/// The Identity precompile address.
pub const IDENTITY: Address = address!("0x0000000000000000000000000000000000000004");

/// The ModExp precompile address.
pub const MOD_EXP: Address = address!("0x0000000000000000000000000000000000000005");

/// The ECAdd precompile address.
pub const EC_ADD: Address = address!("0x0000000000000000000000000000000000000006");

/// The ECMul precompile address.
pub const EC_MUL: Address = address!("0x0000000000000000000000000000000000000007");

/// The ECPairing precompile address.
pub const EC_PAIRING: Address = address!("0x0000000000000000000000000000000000000008");

/// The Blake2F precompile address.
pub const BLAKE_2F: Address = address!("0x0000000000000000000000000000000000000009");

/// The PointEvaluation precompile address.
pub const POINT_EVALUATION: Address = address!("0x000000000000000000000000000000000000000a");

/// Precompile addresses.
pub const PRECOMPILES: &[Address] = &[
    EC_RECOVER,
    SHA_256,
    RIPEMD_160,
    IDENTITY,
    MOD_EXP,
    EC_ADD,
    EC_MUL,
    EC_PAIRING,
    BLAKE_2F,
    POINT_EVALUATION,
];

/// Input for a precompile call.
#[derive(Debug)]
pub struct PrecompileInput<'a> {
    /// Input data bytes.
    pub data: &'a [u8],
    /// Gas limit.
    pub gas: u64,
    /// Caller address.
    pub caller: Address,
    /// Value sent with the call.
    pub value: U256,
    /// Target address of the call. Would be the same as `bytecode_address` unless it's a
    /// DELEGATECALL.
    pub target_address: Address,
    /// Bytecode address of the call.
    pub bytecode_address: Address,
}

impl<'a> PrecompileInput<'a> {
    /// Returns the calldata of the call.
    pub const fn data(&self) -> &[u8] {
        self.data
    }

    /// Returns the caller address of the call.
    pub const fn caller(&self) -> &Address {
        &self.caller
    }

    /// Returns the gas limit of the call.
    pub const fn gas(&self) -> u64 {
        self.gas
    }

    /// Returns the value of the call.
    pub const fn value(&self) -> &U256 {
        &self.value
    }

    /// Returns the target address of the call.
    pub const fn target_address(&self) -> &Address {
        &self.target_address
    }

    /// Returns the bytecode address of the call.
    pub const fn bytecode_address(&self) -> &Address {
        &self.bytecode_address
    }

    /// Returns whether the call is a direct call, i.e when precompile was called directly and not
    /// via a DELEGATECALL/CALLCODE.
    pub fn is_direct_call(&self) -> bool {
        self.target_address == self.bytecode_address
    }
}

/// Trait for implementing precompiled contracts.
pub trait Precompile: Send + Sync {
    /// Returns precompile ID.
    fn precompile_id(&self) -> &PrecompileId;

    /// Execute the precompile with the given input data, gas limit, and caller address.
    fn call(&self, input: PrecompileInput<'_>) -> PrecompileResult;

    /// Returns whether the precompile is pure.
    ///
    /// A pure precompile has deterministic output based solely on its input.
    /// Non-pure precompiles may produce different outputs for the same input
    /// based on the current state or other external factors.
    ///
    /// # Default
    ///
    /// Returns `true` by default, indicating the precompile is pure
    /// and its results should be cached as this is what most of the precompiles are.
    fn is_pure(&self) -> bool {
        true
    }
}

impl<T: Precompile + ?Sized> Precompile for &T {
    fn precompile_id(&self) -> &PrecompileId {
        (**self).precompile_id()
    }

    fn call(&self, input: PrecompileInput<'_>) -> PrecompileResult {
        (**self).call(input)
    }

    fn is_pure(&self) -> bool {
        (**self).is_pure()
    }
}

impl<T: Precompile + ?Sized> Precompile for Arc<T> {
    fn precompile_id(&self) -> &PrecompileId {
        (**self).precompile_id()
    }

    fn call(&self, input: PrecompileInput<'_>) -> PrecompileResult {
        (**self).call(input)
    }

    fn is_pure(&self) -> bool {
        (**self).is_pure()
    }
}

impl<F> Precompile for (PrecompileId, F)
where
    F: Fn(PrecompileInput<'_>) -> PrecompileResult + Send + Sync,
{
    fn precompile_id(&self) -> &PrecompileId {
        &self.0
    }

    fn call(&self, input: PrecompileInput<'_>) -> PrecompileResult {
        self.1(input)
    }
}

/// A dynamic precompile implementation that can be modified at runtime.
#[derive(Clone)]
pub struct DynPrecompile(Arc<dyn Precompile>);

impl DynPrecompile {
    /// Creates a new [`DynPrecompile`] with the given closure.
    pub fn new<F>(id: PrecompileId, f: F) -> Self
    where
        F: Fn(PrecompileInput<'_>) -> PrecompileResult + Send + Sync + 'static,
    {
        Self(Arc::new((id, f)))
    }

    /// Creates a new [`DynPrecompile`] with the given closure and [`Precompile::is_pure`]
    /// returning `false`.
    pub fn new_stateful<F>(id: PrecompileId, f: F) -> Self
    where
        F: Fn(PrecompileInput<'_>) -> PrecompileResult + Send + Sync + 'static,
    {
        Self(Arc::new(StatefulPrecompile((id, f))))
    }

    /// Flips [`Precompile::is_pure`] to `false`.
    pub fn stateful(self) -> Self {
        Self(Arc::new(StatefulPrecompile(self.0)))
    }
}

impl core::fmt::Debug for DynPrecompile {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DynPrecompile").finish()
    }
}

impl<F> From<F> for DynPrecompile
where
    F: Fn(PrecompileInput<'_>) -> PrecompileResult + Send + Sync + 'static,
{
    fn from(f: F) -> Self {
        Self::new(PrecompileId::Custom("closure".into()), f)
    }
}

impl<F> From<(PrecompileId, F)> for DynPrecompile
where
    F: Fn(PrecompileInput<'_>) -> PrecompileResult + Send + Sync + 'static,
{
    fn from((id, f): (PrecompileId, F)) -> Self {
        Self(Arc::new((id, f)))
    }
}

impl Precompile for DynPrecompile {
    fn precompile_id(&self) -> &PrecompileId {
        self.0.precompile_id()
    }

    fn call(&self, input: PrecompileInput<'_>) -> PrecompileResult {
        self.0.call(input)
    }

    fn is_pure(&self) -> bool {
        self.0.is_pure()
    }
}

/// A wrapper that marks a precompile as stateful (non-pure).
struct StatefulPrecompile<P>(P);

impl<P: Precompile> Precompile for StatefulPrecompile<P> {
    fn precompile_id(&self) -> &PrecompileId {
        self.0.precompile_id()
    }

    fn call(&self, input: PrecompileInput<'_>) -> PrecompileResult {
        self.0.call(input)
    }

    fn is_pure(&self) -> bool {
        false
    }
}

/// A wrapper around a `PrecompileProvider` that supports dynamic/closure precompiles.
///
/// Dynamic precompiles registered via `map_precompile` take priority over the wrapped provider.
///
/// # Example
///
/// ```ignore
/// use foundry_evm::precompiles::FoundryPrecompiles;
/// use revm::handler::EthPrecompiles;
///
/// let eth_precompiles = EthPrecompiles::default();
/// let mut precompiles = FoundryPrecompiles::new(eth_precompiles);
///
/// // Add a custom precompile that overrides the identity precompile
/// precompiles.map_precompile(IDENTITY, |input| {
///     Ok(PrecompileOutput::new(0, Bytes::from("custom")))
/// });
/// ```
pub struct FoundryPrecompiles<P> {
    /// The wrapped precompile provider.
    inner: P,
    /// Dynamic precompiles that take priority over the inner provider.
    dynamic: HashMap<Address, DynPrecompile>,
}

impl<P> FoundryPrecompiles<P> {
    /// Creates a new `FoundryPrecompiles` wrapping the given provider.
    pub fn new(inner: P) -> Self {
        Self { inner, dynamic: HashMap::default() }
    }

    /// Registers a dynamic precompile at the given address.
    ///
    /// This precompile will take priority over any precompile at the same address
    /// in the wrapped provider.
    pub fn map_precompile<F>(&mut self, address: Address, f: F)
    where
        F: Fn(PrecompileInput<'_>) -> PrecompileResult + Send + Sync + 'static,
    {
        self.dynamic.insert(address, DynPrecompile::from(f));
    }

    /// Registers a dynamic precompile with a specific ID at the given address.
    pub fn map_precompile_with_id<F>(&mut self, address: Address, id: PrecompileId, f: F)
    where
        F: Fn(PrecompileInput<'_>) -> PrecompileResult + Send + Sync + 'static,
    {
        self.dynamic.insert(address, DynPrecompile::new(id, f));
    }

    /// Registers a stateful (non-pure) dynamic precompile at the given address.
    pub fn map_stateful_precompile<F>(&mut self, address: Address, id: PrecompileId, f: F)
    where
        F: Fn(PrecompileInput<'_>) -> PrecompileResult + Send + Sync + 'static,
    {
        self.dynamic.insert(address, DynPrecompile::new_stateful(id, f));
    }

    /// Registers a `DynPrecompile` directly at the given address.
    pub fn insert_precompile(&mut self, address: Address, precompile: DynPrecompile) {
        self.dynamic.insert(address, precompile);
    }

    /// Removes a dynamic precompile at the given address.
    ///
    /// This will cause calls to that address to fall through to the inner provider.
    pub fn remove_precompile(&mut self, address: &Address) -> Option<DynPrecompile> {
        self.dynamic.remove(address)
    }

    /// Extends the dynamic precompiles with the given iterator of (address, precompile) pairs.
    pub fn extend<I>(&mut self, precompiles: I)
    where
        I: IntoIterator<Item = (Address, DynPrecompile)>,
    {
        self.dynamic.extend(precompiles);
    }

    /// Returns true if a dynamic precompile is registered at the given address.
    pub fn has_dynamic(&self, address: &Address) -> bool {
        self.dynamic.contains_key(address)
    }

    /// Returns a reference to the inner provider.
    pub fn inner(&self) -> &P {
        &self.inner
    }

    /// Returns a mutable reference to the inner provider.
    pub fn inner_mut(&mut self) -> &mut P {
        &mut self.inner
    }

    /// Consumes self and returns the inner provider.
    pub fn into_inner(self) -> P {
        self.inner
    }

    /// Returns an iterator over the dynamic precompile addresses.
    pub fn dynamic_addresses(&self) -> impl Iterator<Item = &Address> {
        self.dynamic.keys()
    }
}

impl FoundryPrecompiles<revm::handler::EthPrecompiles> {
    /// Returns true if a precompile is registered at the given address.
    ///
    /// This checks both the dynamic precompiles and the inner EthPrecompiles.
    pub fn contains(&self, address: &Address) -> bool {
        self.dynamic.contains_key(address) || self.inner.contains(address)
    }
}

impl<P: Clone> Clone for FoundryPrecompiles<P> {
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone(), dynamic: self.dynamic.clone() }
    }
}

impl<P: std::fmt::Debug> std::fmt::Debug for FoundryPrecompiles<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FoundryPrecompiles")
            .field("inner", &self.inner)
            .field("dynamic_addresses", &self.dynamic.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl<P> ExtendablePrecompiles for FoundryPrecompiles<P> {
    type Precompile = DynPrecompile;

    fn extend<I>(&mut self, precompiles: I)
    where
        I: IntoIterator<Item = (Address, DynPrecompile)>,
    {
        self.dynamic.extend(precompiles);
    }

    fn insert_precompile(&mut self, address: Address, precompile: DynPrecompile) {
        self.dynamic.insert(address, precompile);
    }
}

impl<BLOCK, TX, CFG, DB, CHAIN, L, P>
    PrecompileProvider<Context<BLOCK, TX, CFG, DB, Journal<DB>, CHAIN, L>> for FoundryPrecompiles<P>
where
    BLOCK: revm::context::Block,
    TX: revm::context::Transaction,
    CFG: Cfg,
    DB: Database,
    L: LocalContextTr,
    P: PrecompileProvider<
            Context<BLOCK, TX, CFG, DB, Journal<DB>, CHAIN, L>,
            Output = InterpreterResult,
        >,
{
    type Output = InterpreterResult;

    fn set_spec(&mut self, spec: CFG::Spec) -> bool {
        self.inner.set_spec(spec)
    }

    fn run(
        &mut self,
        context: &mut Context<BLOCK, TX, CFG, DB, Journal<DB>, CHAIN, L>,
        inputs: &CallInputs,
    ) -> Result<Option<Self::Output>, String> {
        // Check dynamic precompiles first (priority)
        if let Some(precompile) = self.dynamic.get(&inputs.bytecode_address) {
            let mut result = InterpreterResult {
                result: InstructionResult::Return,
                gas: Gas::new(inputs.gas_limit),
                output: Bytes::new(),
            };

            let r;
            let input_bytes = match &inputs.input {
                CallInput::SharedBuffer(range) => {
                    if let Some(slice) = context.local.shared_memory_buffer_slice(range.clone()) {
                        r = slice;
                        &*r
                    } else {
                        &[]
                    }
                }
                CallInput::Bytes(bytes) => bytes.as_ref(),
            };

            let precompile_result = precompile.call(PrecompileInput {
                data: input_bytes,
                gas: inputs.gas_limit,
                caller: inputs.caller,
                value: inputs.call_value(),
                target_address: inputs.target_address,
                bytecode_address: inputs.bytecode_address,
            });

            match precompile_result {
                Ok(output) => {
                    let underflow = result.gas.record_cost(output.gas_used);
                    assert!(underflow, "Gas underflow is not possible");
                    result.result = if output.reverted {
                        InstructionResult::Revert
                    } else {
                        InstructionResult::Return
                    };
                    result.output = output.bytes;
                }
                Err(PrecompileError::Fatal(e)) => return Err(e),
                Err(e) => {
                    result.result = if e.is_oog() {
                        InstructionResult::PrecompileOOG
                    } else {
                        InstructionResult::PrecompileError
                    };
                }
            }

            return Ok(Some(result));
        }

        // Fall back to inner provider
        self.inner.run(context, inputs)
    }

    fn warm_addresses(&self) -> Box<impl Iterator<Item = Address>> {
        // Combine addresses from both dynamic and inner provider
        let dynamic_addrs: Vec<Address> = self.dynamic.keys().copied().collect();
        let inner_addrs: Vec<Address> = self.inner.warm_addresses().collect();
        Box::new(dynamic_addrs.into_iter().chain(inner_addrs))
    }

    fn contains(&self, address: &Address) -> bool {
        self.dynamic.contains_key(address) || self.inner.contains(address)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use revm::{
        handler::EthPrecompiles,
        precompile::{PrecompileOutput, PrecompileSpecId, Precompiles},
        primitives::hardfork::SpecId,
    };

    #[test]
    fn test_foundry_precompiles_priority() {
        let spec = SpecId::PRAGUE;
        let eth_precompiles = EthPrecompiles {
            precompiles: Precompiles::new(PrecompileSpecId::from_spec_id(spec)),
            spec,
        };

        let mut precompiles = FoundryPrecompiles::new(eth_precompiles);

        // Verify identity precompile exists in inner
        assert!(precompiles.contains(&IDENTITY));

        // Add a custom precompile at identity address
        precompiles.map_precompile(IDENTITY, |_input| {
            Ok(PrecompileOutput::new(42, Bytes::from_static(b"custom")))
        });

        // Verify dynamic precompile is registered
        assert!(precompiles.has_dynamic(&IDENTITY));

        // Both dynamic and inner should report it as contained
        assert!(precompiles.contains(&IDENTITY));
    }

    #[test]
    fn test_foundry_precompiles_extend() {
        let spec = SpecId::PRAGUE;
        let eth_precompiles = EthPrecompiles {
            precompiles: Precompiles::new(PrecompileSpecId::from_spec_id(spec)),
            spec,
        };

        let mut precompiles = FoundryPrecompiles::new(eth_precompiles);

        let custom_addr = address!("0x0000000000000000000000000000000000000100");
        let precompiles_to_add: Vec<(Address, DynPrecompile)> = vec![(
            custom_addr,
            DynPrecompile::from(|_input: PrecompileInput<'_>| {
                Ok(PrecompileOutput::new(0, Bytes::new()))
            }),
        )];

        precompiles.extend(precompiles_to_add);

        assert!(precompiles.has_dynamic(&custom_addr));
        assert!(precompiles.contains(&custom_addr));
    }

    #[test]
    fn test_foundry_precompiles_remove() {
        let spec = SpecId::PRAGUE;
        let eth_precompiles = EthPrecompiles {
            precompiles: Precompiles::new(PrecompileSpecId::from_spec_id(spec)),
            spec,
        };

        let mut precompiles = FoundryPrecompiles::new(eth_precompiles);

        // Add and then remove
        precompiles.map_precompile(IDENTITY, |_| Ok(PrecompileOutput::new(0, Bytes::new())));
        assert!(precompiles.has_dynamic(&IDENTITY));

        precompiles.remove_precompile(&IDENTITY);
        assert!(!precompiles.has_dynamic(&IDENTITY));

        // Should still be contained via inner
        assert!(precompiles.contains(&IDENTITY));
    }
}
