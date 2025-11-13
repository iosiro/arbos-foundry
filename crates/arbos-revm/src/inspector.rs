use revm::context::{ContextError, ContextSetters, ContextTr};

use revm::{
    Database,
    handler::{
        EvmTr, FrameInitOrResult, FrameResult, ItemOrResult, PrecompileProvider,
        evm::ContextDbError, instructions::InstructionProvider,
    },
    inspector::{InspectorEvmTr, InspectorFrame, JournalExt, handler::frame_end},
    interpreter::interpreter_action::FrameInit,
};

use revm::{
    Inspector,
    interpreter::{InterpreterResult, interpreter::EthInterpreter},
};

use crate::{ArbitrumContextTr, ArbitrumEvm};

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
        (&mut self.0.ctx, &mut self.0.inspector, self.0.frame_stack.get())
    }

    fn ctx_inspector_frame_instructions(
        &mut self,
    ) -> (&mut Self::Context, &mut Self::Inspector, &mut Self::Frame, &mut Self::Instructions) {
        (&mut self.0.ctx, &mut self.0.inspector, self.0.frame_stack.get(), &mut self.0.instruction)
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

impl<CTX, INSP, P, I> ArbitrumEvm<CTX, INSP, P, I>
where
    CTX: ArbitrumContextTr,
    CTX::Journal: JournalExt,
    I: InstructionProvider<Context = CTX, InterpreterTypes = EthInterpreter>,
    P: PrecompileProvider<CTX, Output = InterpreterResult>,
    CTX: ContextSetters,
    INSP: Inspector<CTX>,
{
    /// Run inspection on execution loop.
    ///
    /// It will call:
    /// * [`Inspector::call`],[`Inspector::create`] to inspect call, create and eofcreate.
    /// * [`Inspector::call_end`],[`Inspector::create_end`] to inspect call, create and eofcreate
    ///   end.
    /// * [`Inspector::initialize_interp`] to inspect initialized interpreter.
    pub(crate) fn inspect_run_exec_loop(
        &mut self,
        first_frame_input: FrameInit,
    ) -> Result<FrameResult, ContextError<<<CTX as ContextTr>::Db as Database>::Error>> {
        let res = self.inspect_frame_init(first_frame_input)?;

        if let ItemOrResult::Result(frame_result) = res {
            return Ok(frame_result);
        }

        loop {
            let call_or_result = self.inspect_frame_run()?;

            let result = match call_or_result {
                ItemOrResult::Item(init) => {
                    match self.inspect_frame_init(init)? {
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
