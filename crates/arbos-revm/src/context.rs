use revm::{
    Context, Journal,
    context::{BlockEnv, CfgEnv, ContextTr, TxEnv},
};

use crate::{chain::{ArbitrumChainInfo, ArbitrumChainInfoTr}, local_context::{ArbitrumLocalContext, ArbitrumLocalContextTr}};

/// Type alias for the default context type of the ArbitrumEvm.
pub type ArbitrumContext<DB> = Context<BlockEnv, TxEnv, CfgEnv, DB, Journal<DB>, ArbitrumChainInfo, ArbitrumLocalContext>;

/// Type alias for Arbitrum context
pub trait ArbitrumContextTr: ContextTr<
    // Cfg: Cfg<Spec = ArbitrumSpecId>,
    Chain: ArbitrumChainInfoTr,
    Local: ArbitrumLocalContextTr,
>
{
}

impl<T> ArbitrumContextTr for T where
    T: ContextTr<
        //Cfg: Cfg<Spec = ArbitrumSpecId>,
        Chain: ArbitrumChainInfoTr,
        Local: ArbitrumLocalContextTr,
    >
{
}
