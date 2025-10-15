use revm::{context::{Cfg, ContextTr}, handler::{EthPrecompiles, PrecompileProvider}, interpreter::{InputsImpl, InterpreterResult}, precompile::{Precompile, Precompiles}, primitives::{hardfork::SpecId, Address, HashMap, HashSet}};

use crate::ArbitrumContextTr;

pub struct ArbitrumPrecompiles {
    pub inner: EthPrecompiles,
    // pub spec: ArbitrumSpecId,
}

pub trait CustomPrecompile {
    type Context: ArbitrumContextTr;
    type Output;
    
    /// Run the precompile with the given context and input data.
    fn run(&self, context: &mut Self::Context, input: &InputsImpl, is_static: bool, gas_limit: u64) -> Result<Option<Self::Output> ,String>;
}

impl<CTX> PrecompileProvider<CTX> for ArbitrumPrecompiles
where
    CTX: ArbitrumContextTr,
{
    type Output = InterpreterResult;
    
    fn set_spec(&mut self, spec: <<CTX as ContextTr>::Cfg as Cfg>::Spec) -> bool {
        <EthPrecompiles as PrecompileProvider<CTX>>::set_spec(&mut self.inner, spec)
    }
    
    fn run(&mut self,context: &mut CTX,address: &Address,inputs: &InputsImpl,is_static:bool,gas_limit:u64,) -> Result<Option<Self::Output> ,String>  {
        self.inner.run(context,address,inputs,is_static,gas_limit)
    }

    fn warm_addresses(&self) -> Box<impl Iterator<Item = Address>> {
        self.inner.warm_addresses()
    }
    
    fn contains(&self,address: &Address) -> bool {
        self.inner.contains(address)
    }
}

mod arbos_wasm;