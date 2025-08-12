use crate::{api::ArbitrumContextTr, ArbitrumHaltReason, ArbitrumTransactionError};
use revm::{
    context::{result::FromStringError, JournalTr},
    handler::{handler::EvmTrError, EthFrame, EvmTr, Handler, MainnetHandler},
    inspector::{InspectorEvmTr, InspectorHandler},
    interpreter::interpreter::EthInterpreter,
    state::EvmState,
    Inspector,
};

pub struct ArbitrumHandler<EVM, ERROR, FRAME> {
    /// Mainnet handler allows us to use functions from the mainnet handler inside optimism handler.
    /// So we dont duplicate the logic
    pub mainnet: MainnetHandler<EVM, ERROR, FRAME>,
    /// Phantom data to avoid type inference issues.
    pub _phantom: core::marker::PhantomData<(EVM, ERROR, FRAME)>,
}

impl<EVM, ERROR, FRAME> ArbitrumHandler<EVM, ERROR, FRAME> {
    pub fn new() -> Self {
        Self {
            mainnet: MainnetHandler::default(),
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<EVM, ERROR, FRAME> Default for ArbitrumHandler<EVM, ERROR, FRAME> {
    fn default() -> Self {
        Self::new()
    }
}

impl<EVM, ERROR> Handler for ArbitrumHandler<EVM, ERROR, EthFrame<EthInterpreter>>
where
    EVM: EvmTr<
        Context: ArbitrumContextTr<Journal: JournalTr<State = EvmState>>,
        Frame = EthFrame<EthInterpreter>,
    >,
    ERROR: EvmTrError<EVM> + From<ArbitrumTransactionError> + FromStringError,
{
    type Evm = EVM;
    type Error = ERROR;
    type HaltReason = ArbitrumHaltReason;
}

impl<EVM, ERROR> InspectorHandler for ArbitrumHandler<EVM, ERROR, EthFrame<EthInterpreter>>
where
    EVM: InspectorEvmTr<
        Context: ArbitrumContextTr<Journal: JournalTr<State = EvmState>>,
        Frame = EthFrame<EthInterpreter>,
        Inspector: Inspector<<<Self as Handler>::Evm as EvmTr>::Context, EthInterpreter>,
    >,
    ERROR: EvmTrError<EVM> + From<ArbitrumTransactionError> + FromStringError,
{
    type IT = EthInterpreter;
}
