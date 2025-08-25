use revm::{
    Context, Journal,
    context::{BlockEnv, CfgEnv, ContextTr, TxEnv},
};

use crate::chain::{ArbitrumChainInfo, ArbitrumChainInfoTr};

/// Type alias for the default context type of the ArbitrumEvm.
pub type ArbitrumContext<DB> = Context<BlockEnv, TxEnv, CfgEnv, DB, Journal<DB>, ArbitrumChainInfo>;

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
