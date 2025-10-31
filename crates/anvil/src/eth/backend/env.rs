use alloy_evm::EvmEnv;
use foundry_evm::{EnvMut, core::AsEnvMut};
use foundry_evm_networks::NetworkConfigs;
use revm::context::{BlockEnv, TxEnv};

type CfgEnv = arbos_revm::config::ArbitrumConfig;

/// Helper container type for [`EvmEnv`] and [`TxEnd`].
#[derive(Clone, Debug, Default)]
pub struct Env {
    pub evm_env: EvmEnv<BlockEnv, CfgEnv>,
    pub tx: TxEnv,
    pub networks: NetworkConfigs,
}

/// Helper container type for [`EvmEnv`] and [`TxEnv`].
impl Env {
    pub fn new(cfg: CfgEnv, block: BlockEnv, tx: TxEnv, networks: NetworkConfigs) -> Self {
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
