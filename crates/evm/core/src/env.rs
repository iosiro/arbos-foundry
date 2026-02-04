use crate::context::{FoundryBlockEnv, FoundryCfgEnv, FoundryTxEnv};
use revm::{
    Database, Journal, JournalEntry,
    context::{JournalInner, JournalTr, LocalContextTr},
    primitives::hardfork::SpecId,
};

/// Container type that holds both the configuration and block environment for EVM execution.
///
/// Vendored from `alloy-evm` to remove the dependency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmEnv<Spec = SpecId> {
    /// The configuration environment.
    pub cfg_env: FoundryCfgEnv<Spec>,
    /// The block environment.
    pub block_env: FoundryBlockEnv,
}

impl Default for EvmEnv<SpecId> {
    fn default() -> Self {
        Self { cfg_env: FoundryCfgEnv::default(), block_env: FoundryBlockEnv::default() }
    }
}

impl<Spec: Copy> EvmEnv<Spec> {
    /// Creates a new `EvmEnv` from the given configuration and block environments.
    pub fn new(cfg_env: FoundryCfgEnv<Spec>, block_env: FoundryBlockEnv) -> Self {
        Self { cfg_env, block_env }
    }

    /// Returns a reference to the block environment.
    pub fn block_env(&self) -> &FoundryBlockEnv {
        &self.block_env
    }

    /// Returns a reference to the configuration environment.
    pub fn cfg_env(&self) -> &FoundryCfgEnv<Spec> {
        &self.cfg_env
    }

    /// Returns the spec ID.
    pub fn spec_id(&self) -> Spec {
        self.cfg_env.inner.spec
    }
}

impl<Spec> From<(FoundryCfgEnv<Spec>, FoundryBlockEnv)> for EvmEnv<Spec> {
    fn from((cfg_env, block_env): (FoundryCfgEnv<Spec>, FoundryBlockEnv)) -> Self {
        Self { cfg_env, block_env }
    }
}

/// Helper container type for [`EvmEnv`] and [`FoundryTxEnv`].
#[derive(Clone, Debug, Default)]
pub struct Env {
    pub evm_env: EvmEnv,
    pub tx: FoundryTxEnv,
}

/// Helper container type for [`EvmEnv`] and [`FoundryTxEnv`].
impl Env {
    pub fn default_with_spec_id(spec_id: SpecId) -> Self {
        let mut cfg = FoundryCfgEnv::default();
        cfg.inner.spec = spec_id;

        Self::from(cfg, FoundryBlockEnv::default(), FoundryTxEnv::default())
    }

    pub fn from(cfg: FoundryCfgEnv, block: FoundryBlockEnv, tx: FoundryTxEnv) -> Self {
        Self { evm_env: EvmEnv { cfg_env: cfg, block_env: block }, tx }
    }

    pub fn new_with_spec_id(
        cfg: FoundryCfgEnv,
        block: FoundryBlockEnv,
        tx: FoundryTxEnv,
        spec_id: SpecId,
    ) -> Self {
        let mut cfg = cfg;
        cfg.inner.spec = spec_id;

        Self::from(cfg, block, tx)
    }
}

/// Helper struct with mutable references to the block and cfg environments.
pub struct EnvMut<'a> {
    pub block: &'a mut FoundryBlockEnv,
    pub cfg: &'a mut FoundryCfgEnv,
    pub tx: &'a mut FoundryTxEnv,
}

impl EnvMut<'_> {
    /// Returns a copy of the environment.
    pub fn to_owned(&self) -> Env {
        Env {
            evm_env: EvmEnv { cfg_env: self.cfg.to_owned(), block_env: self.block.to_owned() },
            tx: self.tx.to_owned(),
        }
    }
}

pub trait AsEnvMut {
    fn as_env_mut(&mut self) -> EnvMut<'_>;
}

impl AsEnvMut for EnvMut<'_> {
    fn as_env_mut(&mut self) -> EnvMut<'_> {
        EnvMut { block: self.block, cfg: self.cfg, tx: self.tx }
    }
}

impl AsEnvMut for Env {
    fn as_env_mut(&mut self) -> EnvMut<'_> {
        EnvMut {
            block: &mut self.evm_env.block_env,
            cfg: &mut self.evm_env.cfg_env,
            tx: &mut self.tx,
        }
    }
}

impl<DB: Database, J: JournalTr<Database = DB>, C, L: LocalContextTr> AsEnvMut
    for revm::Context<FoundryBlockEnv, FoundryTxEnv, FoundryCfgEnv, DB, J, C, L>
{
    fn as_env_mut(&mut self) -> EnvMut<'_> {
        EnvMut { block: &mut self.block, cfg: &mut self.cfg, tx: &mut self.tx }
    }
}

pub trait ContextExt {
    type DB: Database;

    fn as_db_env_and_journal(
        &mut self,
    ) -> (&mut Self::DB, &mut JournalInner<JournalEntry>, EnvMut<'_>);
}

impl<DB: Database, C, L: LocalContextTr> ContextExt
    for revm::Context<
        FoundryBlockEnv,
        FoundryTxEnv,
        FoundryCfgEnv,
        DB,
        Journal<DB, JournalEntry>,
        C,
        L,
    >
{
    type DB = DB;

    fn as_db_env_and_journal(
        &mut self,
    ) -> (&mut Self::DB, &mut JournalInner<JournalEntry>, EnvMut<'_>) {
        (
            &mut self.journaled_state.database,
            &mut self.journaled_state.inner,
            EnvMut { block: &mut self.block, cfg: &mut self.cfg, tx: &mut self.tx },
        )
    }
}
