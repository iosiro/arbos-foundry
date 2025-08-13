use crate::ArbitrumSpecId;
use once_cell::race::OnceBox;
use revm::{
    context::{Cfg, ContextTr},
    handler::{EthPrecompiles, PrecompileProvider},
    interpreter::{InputsImpl, InterpreterResult},
    precompile::{secp256r1, Precompiles},
    primitives::Address,
};
use std::{
    boxed::Box,
    ops::{Deref, DerefMut},
    string::String,
};

// Optimism precompile provider
#[derive(Debug, Clone)]
pub struct ArbitrumPrecompiles {
    /// Inner precompile provider is same as Ethereums.
    inner: EthPrecompiles,
    _spec: ArbitrumSpecId,
}

impl ArbitrumPrecompiles {
    /// Create a new [`OpPrecompiles`] with the given precompiles.
    pub fn new(precompiles: &'static Precompiles, spec: ArbitrumSpecId) -> Self {
        Self { inner: EthPrecompiles { precompiles, spec: spec.into_eth_spec() }, _spec: spec }
    }

    /// Create a new precompile provider with the given optimismispec.
    #[inline]
    pub fn new_with_spec(spec: ArbitrumSpecId) -> Self {
        match spec {
            ArbitrumSpecId::ArbosFixRedeemGas
            | ArbitrumSpecId::ArbosStylus
            | ArbitrumSpecId::ArbosStylusFixes
            | ArbitrumSpecId::ArbosStylusChargingFixes => Self::new(bianca(), spec),
        }
    }

    // Precompiles getter.
    #[inline]
    pub fn precompiles(&self) -> &'static Precompiles {
        self.inner.precompiles
    }
}

pub fn bianca() -> &'static Precompiles {
    static INSTANCE: OnceBox<Precompiles> = OnceBox::new();
    INSTANCE.get_or_init(|| {
        let mut precompiles = Precompiles::cancun().clone();
        // EIP-7212: secp256r1 P256verify
        precompiles.extend([secp256r1::P256VERIFY]);
        Box::new(precompiles)
    })
}

impl<CTX: ContextTr> PrecompileProvider<CTX> for ArbitrumPrecompiles {
    type Output = InterpreterResult;

    #[inline]
    fn set_spec(&mut self, _spec: <<CTX as ContextTr>::Cfg as Cfg>::Spec) -> bool {
        // if spec.into() == SpecId::from(self.spec) {
        //     return false;
        // }
        // *self = Self::new_with_spec(ArbitrumSpecId::from(spec.into()));
        true
    }

    #[inline]
    fn run(
        &mut self,
        context: &mut CTX,
        address: &Address,
        inputs: &InputsImpl,
        _is_static: bool,
        gas_limit: u64,
    ) -> Result<Option<Self::Output>, String> {
        self.inner.run(context, address, inputs, _is_static, gas_limit)
    }

    #[inline]
    fn warm_addresses(&self) -> Box<impl Iterator<Item = Address>> {
        self.inner.warm_addresses()
    }

    #[inline]
    fn contains(&self, address: &Address) -> bool {
        self.inner.contains(address)
    }
}

impl Default for ArbitrumPrecompiles {
    fn default() -> Self {
        Self::new_with_spec(ArbitrumSpecId::ArbosStylusChargingFixes)
    }
}

impl Deref for ArbitrumPrecompiles {
    type Target = EthPrecompiles;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for ArbitrumPrecompiles {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
