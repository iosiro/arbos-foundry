use revm::{
    Context, Journal,
    context::{BlockEnv, ContextTr, TxEnv},
};

use crate::config::{ArbitrumConfig, ArbitrumConfigTr};

pub type ArbitrumChainInfo = ();
pub type ArbitrumLocalContext = revm::context::LocalContext;

/// Type alias for the default context type of the ArbitrumEvm.
pub type ArbitrumContext<DB> = Context<
    BlockEnv,
    TxEnv,
    ArbitrumConfig,
    DB,
    Journal<DB>,
    ArbitrumChainInfo,
    ArbitrumLocalContext,
>;

/// Type alias for Arbitrum context
pub trait ArbitrumContextTr: ContextTr<Cfg: ArbitrumConfigTr> {}

impl<T> ArbitrumContextTr for T where T: ContextTr<Cfg: ArbitrumConfigTr> {}
