use revm::context::ContextSetters;

use revm::handler::evm::ContextDbError;
use revm::handler::{instructions::InstructionProvider, PrecompileProvider};
use revm::handler::{FrameInitOrResult, ItemOrResult};
use revm::inspector::handler::frame_end;
use revm::inspector::{InspectorEvmTr, InspectorFrame, JournalExt};

use revm::interpreter::{interpreter::EthInterpreter, InterpreterResult};
use revm::Inspector;

use crate::api::ArbitrumContextTr;
use crate::ArbitrumEvm;

impl<CTX, INSP, P, I> ArbitrumEvm<CTX, INSP, P, I> {
    /// Consumed self and returns a new Evm type with given Inspector.
    pub fn with_inspector<OINSP>(self, inspector: OINSP) -> ArbitrumEvm<CTX, OINSP, P, I> {
        ArbitrumEvm(self.0.with_inspector(inspector))
    }

    /// Consumes self and returns a new Evm type with given Precompiles.
    pub fn with_precompiles<OP>(self, precompiles: OP) -> ArbitrumEvm<CTX, INSP, OP, I> {
        ArbitrumEvm(self.0.with_precompiles(precompiles))
    }

    /// Consumes self and returns the inner Inspector.
    pub fn into_inspector(self) -> INSP {
        self.0.into_inspector()
    }
}

impl<CTX, INSP, P, I> InspectorEvmTr for ArbitrumEvm<CTX, INSP, P, I>
where
    CTX: ArbitrumContextTr<Journal: JournalExt> + ContextSetters,
    I: InstructionProvider<Context = CTX, InterpreterTypes = EthInterpreter>,
    P: PrecompileProvider<CTX, Output = InterpreterResult>,
    INSP: Inspector<CTX, I::InterpreterTypes>,
{
    type Inspector = INSP;

    fn inspector(&mut self) -> &mut Self::Inspector {
        &mut self.0.inspector
    }

    fn ctx_inspector(&mut self) -> (&mut Self::Context, &mut Self::Inspector) {
        (&mut self.0.ctx, &mut self.0.inspector)
    }

    fn ctx_inspector_frame(
        &mut self,
    ) -> (&mut Self::Context, &mut Self::Inspector, &mut Self::Frame) {
        (
            &mut self.0.ctx,
            &mut self.0.inspector,
            self.0.frame_stack.get(),
        )
    }

    fn ctx_inspector_frame_instructions(
        &mut self,
    ) -> (
        &mut Self::Context,
        &mut Self::Inspector,
        &mut Self::Frame,
        &mut Self::Instructions,
    ) {
        (
            &mut self.0.ctx,
            &mut self.0.inspector,
            self.0.frame_stack.get(),
            &mut self.0.instruction,
        )
    }

    /// Run the frame from the top of the stack. Returns the frame init or result.
    ///
    /// If frame has returned result it would mark it as finished.
    #[inline]
    fn inspect_frame_run(
        &mut self,
    ) -> Result<FrameInitOrResult<Self::Frame>, ContextDbError<Self::Context>> {
        if let Some(next_action) = self.inspect_frame_run_stylus() {
            let frame = self.0.frame_stack.get();
            let context = &mut self.0.ctx;
            let mut result = frame.process_next_action(context, next_action);

            if let Ok(ItemOrResult::Result(frame_result)) = &mut result {
                let (ctx, inspector, frame) = self.ctx_inspector_frame();
                frame_end(ctx, inspector, frame.frame_input(), frame_result);
                frame.set_finished(true);
            };
            return result;
        }

        self.0.inspect_frame_run()
    }
}
