//! Arbitrum builder trait [`ArbitrumBuilder`] used to build [`ArbitrumEvm`].

use revm::{
    context::{Block, Cfg, Evm, FrameStack, Transaction},
    context_interface::JournalTr,
    handler::instructions::EthInstructions,
    interpreter::interpreter::EthInterpreter,
    Context, Database,
};

use crate::{chain_config::ArbitrumChainInfoTr, ArbitrumEvm, ArbitrumPrecompiles};

/// Type alias for default ArbitrumEvm
pub type DefaultArbitrumEvm<CTX, INSP = ()> =
    ArbitrumEvm<CTX, INSP, ArbitrumPrecompiles, EthInstructions<EthInterpreter, CTX>>;

/// Trait that allows for optimism OpEvm to be built.
pub trait ArbitrumBuilder: Sized {
    /// Type of the context.
    type Context;

    /// Build the arbitrum.
    fn build_arbitrum(self) -> ArbitrumEvm<Self::Context, (), ArbitrumPrecompiles>;

    /// Build the arbitrum with an inspector.
    fn build_arbitrum_with_inspector<INSP>(
        self,
        inspector: INSP,
    ) -> ArbitrumEvm<
        Self::Context,
        INSP,
        ArbitrumPrecompiles,
        EthInstructions<EthInterpreter, Self::Context>,
    >;
}

impl<BLOCK, TX, CFG, DB, JOURNAL, CHAIN> ArbitrumBuilder
    for Context<BLOCK, TX, CFG, DB, JOURNAL, CHAIN>
where
    BLOCK: Block,
    TX: Transaction,
    DB: Database,
    CFG: Cfg,
    JOURNAL: JournalTr<Database = DB>,
    CHAIN: ArbitrumChainInfoTr,
{
    type Context = Self;

    fn build_arbitrum(
        self,
    ) -> ArbitrumEvm<
        Self::Context,
        (),
        ArbitrumPrecompiles,
        EthInstructions<EthInterpreter, Self::Context>,
    > {
        ArbitrumEvm(Evm {
            ctx: self,
            inspector: (),
            instruction: EthInstructions::default(),
            precompiles: ArbitrumPrecompiles::default(),
            frame_stack: FrameStack::default(),
        })
    }

    fn build_arbitrum_with_inspector<INSP>(
        self,
        inspector: INSP,
    ) -> ArbitrumEvm<
        Self::Context,
        INSP,
        ArbitrumPrecompiles,
        EthInstructions<EthInterpreter, Self::Context>,
    > {
        ArbitrumEvm(Evm {
            ctx: self,
            inspector,
            instruction: EthInstructions::default(),
            precompiles: ArbitrumPrecompiles::default(),
            frame_stack: FrameStack::default(),
        })
    }
}
