use std::{
    fmt::{self, Debug},
    sync::Arc,
};

use revm::{
    context::ContextTr,
    interpreter::{Gas, InstructionResult, InterpreterResult},
    precompile::{PrecompileError, PrecompileId, PrecompileSpecId},
    primitives::{Address, Bytes, HashMap, HashSet, SHORT_ADDRESS_CAP, U256, short_address},
};

pub trait PrecompilesContextTr: ContextTr {}

impl<T> PrecompilesContextTr for T where T: ContextTr {}

/// The [`PrecompileProvider`] for Arbitrum precompiles.
#[derive(Clone, Debug)]
pub struct Precompiles<CTX: PrecompilesContextTr> {
    /// Precompiles
    inner: HashMap<Address, Precompile<CTX>>,
    /// Addresses of precompiles.
    addresses: HashSet<Address>,
    /// Optimized addresses filter.
    optimized_access: Vec<Option<Precompile<CTX>>>,
    /// `true` if all precompiles are short addresses.
    all_short_addresses: bool,
}

/// Precompile.
pub struct ExtendedPrecompile<CTX: PrecompilesContextTr> {
    /// Unique identifier.
    id: PrecompileId,
    /// Precompile address.
    address: Address,
    /// Precompile implementation.
    fn_: Arc<ExtendedPrecompileFn<CTX>>,
}

impl<CTX: PrecompilesContextTr> Clone for ExtendedPrecompile<CTX> {
    fn clone(&self) -> Self {
        Self { id: self.id.clone(), address: self.address, fn_: self.fn_.clone() }
    }
}

impl<CTX: PrecompilesContextTr> Debug for ExtendedPrecompile<CTX> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExtendedPrecompile")
            .field("id", &self.id)
            .field("address", &self.address)
            .finish()
    }
}

impl<CTX: PrecompilesContextTr> ExtendedPrecompile<CTX> {
    pub fn new(id: PrecompileId, address: Address, fn_: ExtendedPrecompileFn<CTX>) -> Self {
        Self { id, address, fn_: Arc::new(fn_) }
    }
    /// Returns the precompile id.
    #[inline]
    pub fn id(&self) -> &PrecompileId {
        &self.id
    }

    /// Returns the precompile address.
    #[inline]
    pub fn address(&self) -> &Address {
        &self.address
    }
}

pub type ExtendedPrecompileFn<CTX> = fn(
    context: &mut CTX,
    input: &[u8],
    target_address: &Address,
    caller_address: Address,
    call_value: U256,
    is_static: bool,
    gas_limit: u64,
) -> Result<Option<InterpreterResult>, String>;

#[derive(Debug)]
pub enum Precompile<CTX: PrecompilesContextTr> {
    Simple(revm::precompile::Precompile),
    Extended(ExtendedPrecompile<CTX>),
}

// Manual implementation of Clone for Precompile<CTX>
impl<CTX: PrecompilesContextTr> Clone for Precompile<CTX> {
    fn clone(&self) -> Self {
        match self {
            Self::Simple(p) => Self::Simple(p.clone()),
            Self::Extended(p) => Self::Extended(p.clone()),
        }
    }
}

impl<CTX: PrecompilesContextTr> Precompile<CTX> {
    /// Returns the precompile address.
    #[inline]
    pub fn address(&self) -> &Address {
        match self {
            Self::Simple(p) => p.address(),
            Self::Extended(p) => &p.address,
        }
    }

    /// Calls the precompile.
    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub fn call(
        &self,
        context: &mut CTX,
        input: &[u8],
        target_address: &Address,
        caller_address: Address,
        call_value: U256,
        is_static: bool,
        gas_limit: u64,
    ) -> Result<Option<InterpreterResult>, String> {
        match self {
            Self::Simple(p) => {
                let precompile_result = p.execute(input, gas_limit);

                let mut result = InterpreterResult {
                    result: InstructionResult::Return,
                    gas: Gas::new(gas_limit),
                    output: Bytes::new(),
                };

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
                };

                Ok(Some(result))
            }
            Self::Extended(p) => p.execute(
                context,
                input,
                target_address,
                caller_address,
                call_value,
                is_static,
                gas_limit,
            ),
        }
    }
}

impl<CTX: PrecompilesContextTr> ExtendedPrecompile<CTX> {
    #[allow(clippy::too_many_arguments)]
    pub fn execute(
        &self,
        context: &mut CTX,
        input: &[u8],
        target_address: &Address,
        caller_address: Address,
        call_value: U256,
        is_static: bool,
        gas_limit: u64,
    ) -> Result<Option<InterpreterResult>, String> {
        (self.fn_)(context, input, target_address, caller_address, call_value, is_static, gas_limit)
    }
}

impl<CTX: PrecompilesContextTr> Default for Precompiles<CTX> {
    fn default() -> Self {
        Self {
            inner: HashMap::default(),
            addresses: HashSet::default(),
            optimized_access: vec![None; SHORT_ADDRESS_CAP],
            all_short_addresses: true,
        }
    }
}

impl<CTX: PrecompilesContextTr> Precompiles<CTX> {
    pub fn new(_spec: PrecompileSpecId) -> Self {
        let mut precompiles = Self::default();
        precompiles.extend(
            [
                // Homestead
                revm::precompile::secp256k1::ECRECOVER,
                revm::precompile::hash::SHA256,
                revm::precompile::hash::RIPEMD160,
                revm::precompile::identity::FUN,
                // Byzantium
                revm::precompile::modexp::BYZANTIUM,
                revm::precompile::bn254::add::BYZANTIUM,
                revm::precompile::bn254::mul::BYZANTIUM,
                revm::precompile::bn254::pair::BYZANTIUM,
                // Istanbul
                revm::precompile::bn254::add::ISTANBUL,
                revm::precompile::bn254::mul::ISTANBUL,
                revm::precompile::bn254::pair::ISTANBUL,
                revm::precompile::blake2::FUN,
                // Berlin
                revm::precompile::modexp::BERLIN,
                // Cancun
                revm::precompile::kzg_point_evaluation::POINT_EVALUATION,
                // Osaka
                revm::precompile::modexp::OSAKA,
                revm::precompile::secp256r1::P256VERIFY_OSAKA,
            ]
            .map(|p| Precompile::<CTX>::Simple(p)),
        );

        // Prague
        precompiles.extend(
            revm::precompile::bls12_381::precompiles().map(|p| Precompile::<CTX>::Simple(p)),
        );

        precompiles
    }

    /// Returns an iterator over the precompiles addresses.
    #[inline]
    pub fn addresses(&self) -> impl ExactSizeIterator<Item = &Address> {
        self.inner.keys()
    }

    /// Consumes the type and returns all precompile addresses.
    #[inline]
    pub fn into_addresses(self) -> impl ExactSizeIterator<Item = Address> {
        self.inner.into_keys()
    }

    /// Is the given address a precompile.
    #[inline]
    pub fn contains(&self, address: &Address) -> bool {
        self.inner.contains_key(address)
    }

    /// Returns the precompile for the given address.
    #[inline]
    pub fn get(&self, address: &Address) -> Option<&Precompile<CTX>> {
        if let Some(short_address) = short_address(address) {
            return self.optimized_access[short_address].as_ref();
        }
        self.inner.get(address)
    }

    /// Returns the precompile for the given address.
    #[inline]
    pub fn get_mut(&mut self, address: &Address) -> Option<&mut Precompile<CTX>> {
        self.inner.get_mut(address)
    }

    /// Is the precompiles list empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns the number of precompiles.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns the precompiles addresses as a set.
    pub fn addresses_set(&self) -> &HashSet<Address> {
        &self.addresses
    }

    /// Extends the precompiles with the given precompiles.
    ///
    /// Other precompiles with overwrite existing precompiles.
    #[inline]
    pub fn extend(&mut self, other: impl IntoIterator<Item = Precompile<CTX>>) {
        let items: Vec<Precompile<CTX>> = other.into_iter().collect::<Vec<_>>();
        for item in &items {
            if let Some(short_address) = short_address(item.address()) {
                self.optimized_access[short_address] = Some(item.clone());
            } else {
                self.all_short_addresses = false;
            }
        }

        self.addresses.extend(items.iter().map(|p| *p.address()));
        self.inner.extend(items.into_iter().map(|p| (*p.address(), p.clone())));
    }

    /// Returns complement of `other` in `self`.
    ///
    /// Two entries are considered equal if the precompile addresses are equal.
    pub fn difference(&self, other: &Self) -> Self {
        let Self { inner, .. } = self;

        let inner = inner
            .iter()
            .filter(|(a, _)| !other.inner.contains_key(*a))
            .map(|(a, p)| (*a, p.clone()))
            .collect::<HashMap<_, _>>();

        let mut precompiles = Self::default();
        precompiles.extend(inner.into_iter().map(|p| p.1));
        precompiles
    }

    /// Returns intersection of `self` and `other`.
    ///
    /// Two entries are considered equal if the precompile addresses are equal.
    pub fn intersection(&self, other: &Self) -> Self {
        let Self { inner, .. } = self;

        let inner = inner
            .iter()
            .filter(|(a, _)| other.inner.contains_key(*a))
            .map(|(a, p)| (*a, p.clone()))
            .collect::<HashMap<_, _>>();

        let mut precompiles = Self::default();
        precompiles.extend(inner.into_iter().map(|p| p.1));
        precompiles
    }
}
