use revm::{
    Context, Journal,
    context::{BlockEnv, ContextTr, TxEnv}, primitives::hardfork::SpecId,
};

use crate::{config::{ArbitrumConfig, ArbitrumConfigTr}, local_context::{ArbitrumLocalContext, ArbitrumLocalContextTr}};

/// Type alias for the default context type of the ArbitrumEvm.
pub type ArbitrumContext<DB> = Context<BlockEnv, TxEnv, ArbitrumConfig<SpecId>, DB, Journal<DB>, (), ArbitrumLocalContext>;

/// Type alias for Arbitrum context
pub trait ArbitrumContextTr: ContextTr<
    Cfg: ArbitrumConfigTr,
    Local: ArbitrumLocalContextTr,
>
{
}

impl<T> ArbitrumContextTr for T where
    T: ContextTr<
        Cfg: ArbitrumConfigTr,
        Local: ArbitrumLocalContextTr,
    >
{
}
