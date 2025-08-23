//! Implementation of the [`ExecuteEvm`] trait for the [`ArbitrumEvm`].
use crate::{
    ArbitrumHaltReason, ArbitrumTransactionError, chain_config::ArbitrumChainInfoTr,
    evm::ArbitrumEvm, handler::ArbitrumHandler,
};
use revm::{
    DatabaseCommit, ExecuteCommitEvm, ExecuteEvm,
    context::{ContextSetters, result::ExecResultAndState},
    context_interface::{
        ContextTr, Database, JournalTr,
        result::{EVMError, ExecutionResult},
    },
    handler::{
        EthFrame, Handler, PrecompileProvider, SystemCallTx, instructions::EthInstructions,
        system_call::SystemCallEvm,
    },
    inspector::{InspectCommitEvm, InspectEvm, Inspector, InspectorHandler, JournalExt},
    interpreter::{InterpreterResult, interpreter::EthInterpreter},
    primitives::{Address, Bytes},
    state::EvmState,
};

/// Type alias for Arbitrum context
pub trait ArbitrumContextTr: ContextTr<
    // Cfg: Cfg<Spec = ArbitrumSpecId>,
    Chain: ArbitrumChainInfoTr,
>
{
}

impl<T> ArbitrumContextTr for T where
    T: ContextTr<
        //Cfg: Cfg<Spec = ArbitrumSpecId>,
        Chain: ArbitrumChainInfoTr,
    >
{
}

/// Type alias for the error type of the ArbitrumEvm.
pub type ArbitrumError<CTX> =
    EVMError<<<CTX as ContextTr>::Db as Database>::Error, ArbitrumTransactionError>;

impl<CTX, INSP, PRECOMPILE> ExecuteEvm
    for ArbitrumEvm<CTX, INSP, PRECOMPILE, EthInstructions<EthInterpreter, CTX>>
where
    CTX: ArbitrumContextTr<Journal: JournalTr<State = EvmState>> + ContextSetters,
    PRECOMPILE: PrecompileProvider<CTX, Output = InterpreterResult>,
{
    type Tx = <CTX as ContextTr>::Tx;
    type Block = <CTX as ContextTr>::Block;
    type State = EvmState;
    type Error = ArbitrumError<CTX>;
    type ExecutionResult = ExecutionResult<ArbitrumHaltReason>;

    fn set_block(&mut self, block: Self::Block) {
        self.0.ctx.set_block(block);
    }

    fn transact_one(&mut self, tx: Self::Tx) -> Result<Self::ExecutionResult, Self::Error> {
        self.0.ctx.set_tx(tx);
        let mut h = ArbitrumHandler::<_, _, EthFrame<EthInterpreter>>::new();
        h.run(self)
    }

    fn finalize(&mut self) -> Self::State {
        self.0.ctx.journal_mut().finalize()
    }

    fn replay(
        &mut self,
    ) -> Result<ExecResultAndState<Self::ExecutionResult, Self::State>, Self::Error> {
        let mut h = ArbitrumHandler::<_, _, EthFrame<EthInterpreter>>::new();
        h.run(self).map(|result| {
            let state = self.finalize();
            ExecResultAndState::new(result, state)
        })
    }
}

impl<CTX, INSP, PRECOMPILE> ExecuteCommitEvm
    for ArbitrumEvm<CTX, INSP, PRECOMPILE, EthInstructions<EthInterpreter, CTX>>
where
    CTX: ArbitrumContextTr<Db: DatabaseCommit, Journal: JournalTr<State = EvmState>>
        + ContextSetters,
    PRECOMPILE: PrecompileProvider<CTX, Output = InterpreterResult>,
{
    fn commit(&mut self, state: Self::State) {
        self.0.ctx.db_mut().commit(state);
    }
}

impl<CTX, INSP, PRECOMPILE> InspectEvm
    for ArbitrumEvm<CTX, INSP, PRECOMPILE, EthInstructions<EthInterpreter, CTX>>
where
    CTX: ArbitrumContextTr<Journal: JournalTr<State = EvmState> + JournalExt> + ContextSetters,
    INSP: Inspector<CTX, EthInterpreter>,
    PRECOMPILE: PrecompileProvider<CTX, Output = InterpreterResult>,
{
    type Inspector = INSP;

    fn set_inspector(&mut self, inspector: Self::Inspector) {
        self.0.inspector = inspector;
    }

    fn inspect_one_tx(&mut self, tx: Self::Tx) -> Result<Self::ExecutionResult, Self::Error> {
        self.0.ctx.set_tx(tx);
        let mut h = ArbitrumHandler::<_, _, EthFrame<EthInterpreter>>::new();
        h.inspect_run(self)
    }
}

impl<CTX, INSP, PRECOMPILE> InspectCommitEvm
    for ArbitrumEvm<CTX, INSP, PRECOMPILE, EthInstructions<EthInterpreter, CTX>>
where
    CTX: ArbitrumContextTr<Journal: JournalTr<State = EvmState> + JournalExt, Db: DatabaseCommit>
        + ContextSetters,
    INSP: Inspector<CTX, EthInterpreter>,
    PRECOMPILE: PrecompileProvider<CTX, Output = InterpreterResult>,
{
}

impl<CTX, INSP, PRECOMPILE> SystemCallEvm
    for ArbitrumEvm<CTX, INSP, PRECOMPILE, EthInstructions<EthInterpreter, CTX>>
where
    CTX: ArbitrumContextTr<Tx: SystemCallTx, Journal: JournalTr<State = EvmState>> + ContextSetters,
    PRECOMPILE: PrecompileProvider<CTX, Output = InterpreterResult>,
{
    fn transact_system_call_with_caller(
        &mut self,
        caller: Address,
        system_contract_address: Address,
        data: Bytes,
    ) -> Result<Self::ExecutionResult, Self::Error> {
        self.0.ctx.set_tx(CTX::Tx::new_system_tx_with_caller(
            caller,
            system_contract_address,
            data,
        ));
        let mut h = ArbitrumHandler::<_, _, EthFrame<EthInterpreter>>::new();
        h.run_system_call(self)
    }
}
