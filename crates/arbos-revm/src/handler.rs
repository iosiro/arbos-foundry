use crate::ArbitrumContextTr;
use revm::{
    Inspector,
    context::{
        JournalTr,
        result::{FromStringError, HaltReason},
    },
    handler::{EthFrame, EvmTr, Handler, MainnetHandler, handler::EvmTrError},
    inspector::{InspectorEvmTr, InspectorHandler},
    interpreter::interpreter::EthInterpreter,
    state::EvmState,
};

pub struct ArbitrumHandler<EVM, ERROR, FRAME> {
    /// Mainnet handler allows us to use functions from the mainnet handler inside optimism
    /// handler. So we dont duplicate the logic
    pub mainnet: MainnetHandler<EVM, ERROR, FRAME>,
    /// Phantom data to avoid type inference issues.
    pub _phantom: core::marker::PhantomData<(EVM, ERROR, FRAME)>,
}

impl<EVM, ERROR, FRAME> ArbitrumHandler<EVM, ERROR, FRAME> {
    pub fn new() -> Self {
        Self { mainnet: MainnetHandler::default(), _phantom: core::marker::PhantomData }
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
    ERROR: EvmTrError<EVM> + FromStringError,
{
    type Evm = EVM;
    type Error = ERROR;
    type HaltReason = HaltReason;
}

impl<EVM, ERROR> InspectorHandler for ArbitrumHandler<EVM, ERROR, EthFrame<EthInterpreter>>
where
    EVM: InspectorEvmTr<
            Context: ArbitrumContextTr<Journal: JournalTr<State = EvmState>>,
            Frame = EthFrame<EthInterpreter>,
            Inspector: Inspector<<<Self as Handler>::Evm as EvmTr>::Context, EthInterpreter>,
        >,
    ERROR: EvmTrError<EVM> + FromStringError,
{
    type IT = EthInterpreter;
}
