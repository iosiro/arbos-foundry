use std::ops::{Deref, DerefMut};

use crate::ArbitrumContextTr;
use revm::{
    Database, Inspector,
    context::{ContextError, ContextSetters, ContextTr, Evm, FrameStack},
    handler::{
        EthFrame, EvmTr, FrameInitOrResult, FrameResult, FrameTr, ItemOrResult, PrecompileProvider,
        instructions::{EthInstructions, InstructionProvider},
    },
    interpreter::{InterpreterResult, interpreter::EthInterpreter, interpreter_action::FrameInit},
};

pub struct ArbitrumEvm<CTX, INSP, P, I = EthInstructions<EthInterpreter, CTX>, F = EthFrame>(
    pub Evm<CTX, INSP, I, P, F>,
);

impl<CTX, I, INSP, P, F> ArbitrumEvm<CTX, INSP, P, I, F> {
    /// Create a new EVM instance with a given context, inspector, instruction set, and precompile
    /// provider.
    pub fn new_with_inspector(ctx: CTX, inspector: INSP, instruction: I, precompiles: P) -> Self {
        Self(Evm { ctx, inspector, instruction, precompiles, frame_stack: FrameStack::new() })
    }
}

impl<CTX, INSP, P, I, F> Deref for ArbitrumEvm<CTX, INSP, P, I, F>
where
    CTX: ArbitrumContextTr + ContextSetters,
    INSP: Inspector<CTX, I::InterpreterTypes>,
    I: InstructionProvider<Context = CTX, InterpreterTypes = EthInterpreter>,
    P: PrecompileProvider<CTX, Output = InterpreterResult>,
{
    type Target = Evm<CTX, INSP, I, P, F>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<CTX, INSP, P, I, F> DerefMut for ArbitrumEvm<CTX, INSP, P, I, F>
where
    CTX: ArbitrumContextTr + ContextSetters,
    INSP: Inspector<CTX, I::InterpreterTypes>,
    I: InstructionProvider<Context = CTX, InterpreterTypes = EthInterpreter>,
    P: PrecompileProvider<CTX, Output = InterpreterResult>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<CTX, INSP, P, I> EvmTr for ArbitrumEvm<CTX, INSP, P, I, EthFrame<EthInterpreter>>
where
    CTX: ArbitrumContextTr,
    I: InstructionProvider<Context = CTX, InterpreterTypes = EthInterpreter>,
    P: PrecompileProvider<CTX, Output = InterpreterResult>,
{
    type Context = CTX;
    type Instructions = I;
    type Precompiles = P;
    type Frame = EthFrame<EthInterpreter>;

    fn ctx(&mut self) -> &mut Self::Context {
        &mut self.0.ctx
    }

    fn ctx_ref(&self) -> &Self::Context {
        &self.0.ctx
    }

    fn ctx_instructions(&mut self) -> (&mut Self::Context, &mut Self::Instructions) {
        (&mut self.0.ctx, &mut self.0.instruction)
    }

    fn ctx_precompiles(&mut self) -> (&mut Self::Context, &mut Self::Precompiles) {
        (&mut self.0.ctx, &mut self.0.precompiles)
    }

    fn frame_stack(&mut self) -> &mut FrameStack<Self::Frame> {
        &mut self.0.frame_stack
    }

    fn frame_init(
        &mut self,
        frame_input: <Self::Frame as FrameTr>::FrameInit,
    ) -> Result<
        ItemOrResult<&mut Self::Frame, <Self::Frame as FrameTr>::FrameResult>,
        ContextError<<<Self::Context as ContextTr>::Db as Database>::Error>,
    > {
        self.0.frame_init(frame_input)
    }

    fn frame_run(
        &mut self,
    ) -> Result<
        FrameInitOrResult<Self::Frame>,
        ContextError<<<Self::Context as ContextTr>::Db as Database>::Error>,
    > {
        if let Some(action) = self.frame_run_stylus() {
            let frame = self.0.frame_stack.get();
            let context = &mut self.0.ctx;
            return frame.process_next_action(context, action).inspect(|i| {
                if i.is_result() {
                    frame.set_finished(true);
                }
            });
        }

        self.0.frame_run()
    }

    fn frame_return_result(
        &mut self,
        result: <Self::Frame as FrameTr>::FrameResult,
    ) -> Result<
        Option<<Self::Frame as FrameTr>::FrameResult>,
        ContextError<<<Self::Context as ContextTr>::Db as Database>::Error>,
    > {
        self.0.frame_return_result(result)
    }
}

impl<CTX, INSP, P, I> ArbitrumEvm<CTX, INSP, P, I>
where
    CTX: ArbitrumContextTr,
    I: InstructionProvider<Context = CTX, InterpreterTypes = EthInterpreter>,
    P: PrecompileProvider<CTX, Output = InterpreterResult>,
{
    /// Executes the main frame processing loop.
    ///
    /// This loop manages the frame stack, processing each frame until execution completes.
    /// For each iteration:
    /// 1. Calls the current frame
    /// 2. Handles the returned frame input or result
    /// 3. Creates new frames or propagates results as needed
    #[inline]
    pub(crate) fn run_exec_loop(
        &mut self,
        first_frame_input: FrameInit,
    ) -> Result<FrameResult, ContextError<<<CTX as ContextTr>::Db as Database>::Error>> {
        let res = self.frame_init(first_frame_input)?;

        if let ItemOrResult::Result(frame_result) = res {
            return Ok(frame_result);
        }

        loop {
            let call_or_result = self.frame_run()?;

            let result = match call_or_result {
                ItemOrResult::Item(init) => {
                    match self.frame_init(init)? {
                        ItemOrResult::Item(_) => {
                            continue;
                        }
                        // Do not pop the frame since no new frame was created
                        ItemOrResult::Result(result) => result,
                    }
                }
                ItemOrResult::Result(result) => result,
            };

            if let Some(result) = self.frame_return_result(result)? {
                return Ok(result);
            }
        }
    }
}
