use foundry_evm::{
    EnvMut, EvmEnv,
    core::{AsEnvMut, FoundryBlockEnv, FoundryCfgEnv, FoundryTxEnv},
};
use foundry_evm_networks::NetworkConfigs;

/// Helper container type for [`EvmEnv`] and [`FoundryTxEnv`].
#[derive(Clone, Debug, Default)]
pub struct Env {
    pub evm_env: EvmEnv,
    pub tx: FoundryTxEnv,
    pub networks: NetworkConfigs,
}

/// Helper container type for [`EvmEnv`] and [`FoundryTxEnv`].
impl Env {
    pub fn new(
        cfg: FoundryCfgEnv,
        block: FoundryBlockEnv,
        tx: FoundryTxEnv,
        networks: NetworkConfigs,
    ) -> Self {
        Self { evm_env: EvmEnv { cfg_env: cfg, block_env: block }, tx, networks }
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
