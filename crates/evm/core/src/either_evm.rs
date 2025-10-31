use alloy_evm::{Database, Evm, EvmEnv, precompiles::PrecompilesMap};
use alloy_primitives::{Address, Bytes};
use arbos_revm::{ArbitrumContext, ArbitrumEvm, config::ArbitrumConfig, precompiles::ArbitrumPrecompiles};
use revm::{ExecuteEvm, Inspector, SystemCallEvm, context::{BlockEnv, TxEnv, result::{EVMError, HaltReason, ResultAndState}}, handler::PrecompileProvider, primitives::hardfork::SpecId};
use revm::InspectEvm;

pub type EitherEvmContext<DB> = ArbitrumContext<DB>;

pub struct EitherEvm<DB: Database, I, P = PrecompilesMap<EitherEvmContext<DB>, ArbitrumPrecompiles<EitherEvmContext<DB>>>> {
    pub inner: ArbitrumEvm<EitherEvmContext<DB>, I, P>,
    pub inspect: bool,
}

impl<DB, I, PRECOMPILE> Evm for EitherEvm<DB, I, PRECOMPILE>
where
    DB: Database,
    I: Inspector<EitherEvmContext<DB>>,
    PRECOMPILE: PrecompileProvider<EitherEvmContext<DB>, Output = revm::interpreter::InterpreterResult>,
{
    type DB = DB;
    type Block = BlockEnv;
    type Config = ArbitrumConfig;
    type Tx = TxEnv;
    type Error = EVMError<DB::Error>;
    type HaltReason = HaltReason;
    type Spec = SpecId;
    type Precompiles = PRECOMPILE;
    type Inspector = I;

    fn block(&self) -> &Self::Block {
        &self.inner.0.block
    }

    fn chain_id(&self) -> u64 {
        self.inner.0.cfg.chain_id
    }

    fn transact_raw(
        &mut self,
        tx: Self::Tx,
    ) -> Result<ResultAndState<Self::HaltReason>, Self::Error> {
        if self.inspect {
            self.inner.0.inspect_tx(tx)
        } else {
            self.inner.0.transact(tx)
        }
    }

    fn transact_system_call(
        &mut self,
        caller: Address,
        contract: Address,
        data: Bytes,
    ) -> Result<ResultAndState<Self::HaltReason>, Self::Error> {
        self.inner.0.system_call_with_caller(caller, contract, data)
    }

    fn finish(self) -> (Self::DB, EvmEnv<Self::Block, Self::Config>) {
        let EitherEvmContext { block: block_env, cfg: cfg_env, journaled_state, .. } = self.inner.0.ctx;

        (journaled_state.database, EvmEnv { block_env, cfg_env })
    }

    fn set_inspector_enabled(&mut self, enabled: bool) {
        self.inspect = enabled;
    }

    fn components(&self) -> (&Self::DB, &Self::Inspector, &Self::Precompiles) {
        (&self.inner.0.ctx.journaled_state.database, &self.inner.0.inspector, &self.inner.0.precompiles)
    }

    fn components_mut(&mut self) -> (&mut Self::DB, &mut Self::Inspector, &mut Self::Precompiles) {
        (
            &mut self.inner.0.ctx.journaled_state.database,
            &mut self.inner.0.inspector,
            &mut self.inner.0.precompiles,
        )
    }
}
