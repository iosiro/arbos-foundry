use crate::{
    AsEnvMut, Env,
    evm::{BlockEnv, CfgEnv, EvmEnv, TxEnv},
    opts::StylusOpts,
    utils::apply_chain_and_block_specific_env_changes,
};
use alloy_consensus::BlockHeader;
use alloy_primitives::{Address, U256};
use alloy_provider::{Network, Provider, network::BlockResponse};
use alloy_rpc_types::BlockNumberOrTag;
use eyre::WrapErr;
use foundry_common::NON_ARCHIVE_NODE_WARNING;

/// Initializes a REVM block environment based on a forked
/// ethereum provider.
#[allow(clippy::too_many_arguments)]
pub async fn environment<N: Network, P: Provider<N>>(
    provider: &P,
    memory_limit: u64,
    gas_price: Option<u128>,
    override_chain_id: Option<u64>,
    pin_block: Option<u64>,
    origin: Address,
    disable_block_gas_limit: bool,
    enable_tx_gas_limit: bool,
) -> eyre::Result<(Env, N::BlockResponse)> {
    let block_number = if let Some(pin_block) = pin_block {
        pin_block
    } else {
        provider.get_block_number().await.wrap_err("failed to get latest block number")?
    };
    let (fork_gas_price, rpc_chain_id, block) = tokio::try_join!(
        provider.get_gas_price(),
        provider.get_chain_id(),
        provider.get_block_by_number(BlockNumberOrTag::Number(block_number))
    )?;
    let block = if let Some(block) = block {
        block
    } else {
        if let Ok(latest_block) = provider.get_block_number().await {
            // If the `eth_getBlockByNumber` call succeeds, but returns null instead of
            // the block, and the block number is less than equal the latest block, then
            // the user is forking from a non-archive node with an older block number.
            if block_number <= latest_block {
                error!("{NON_ARCHIVE_NODE_WARNING}");
            }
            eyre::bail!(
                "failed to get block for block number: {block_number}; \
                 latest block number: {latest_block}"
            );
        }
        eyre::bail!("failed to get block for block number: {block_number}")
    };

    let cfg = configure_env(
        override_chain_id.unwrap_or(rpc_chain_id),
        memory_limit,
        disable_block_gas_limit,
        enable_tx_gas_limit,
        Some(StylusOpts::default()),
    );

    let mut env = Env {
        evm_env: EvmEnv {
            cfg_env: cfg,
            block_env: BlockEnv {
                number: U256::from(block.header().number()),
                timestamp: U256::from(block.header().timestamp()),
                beneficiary: block.header().beneficiary(),
                difficulty: block.header().difficulty(),
                prevrandao: block.header().mix_hash(),
                basefee: block.header().base_fee_per_gas().unwrap_or_default(),
                gas_limit: block.header().gas_limit(),
                ..Default::default()
            },
        },
        tx: TxEnv {
            caller: origin,
            gas_price: gas_price.unwrap_or(fork_gas_price),
            chain_id: Some(override_chain_id.unwrap_or(rpc_chain_id)),
            gas_limit: block.header().gas_limit() as u64,
            ..Default::default()
        },
    };

    apply_chain_and_block_specific_env_changes::<N>(env.as_env_mut(), &block);

    Ok((env, block))
}

/// Configures the environment for the given chain id and memory limit.
pub fn configure_env(
    chain_id: u64,
    memory_limit: u64,
    disable_block_gas_limit: bool,
    enable_tx_gas_limit: bool,
    stylus: Option<StylusOpts>,
) -> CfgEnv {
    let stylus = stylus.unwrap_or_default();

    let mut cfg = CfgEnv::default();
    cfg.chain_id = chain_id;
    cfg.memory_limit = memory_limit;
    cfg.limit_contract_code_size = Some(usize::MAX);
    // EIP-3607 rejects transactions from senders with deployed code.
    // If EIP-3607 is enabled it can cause issues during fuzz/invariant tests if the caller
    // is a contract. So we disable the check by default.
    cfg.disable_eip3607 = true;
    cfg.disable_eip3541 = true;
    cfg.disable_block_gas_limit = disable_block_gas_limit;
    cfg.disable_nonce_check = true;
    // By default do not enforce transaction gas limits imposed by Osaka (EIP-7825).
    // Users can opt-in to enable these limits by setting `enable_tx_gas_limit` to true.
    if !enable_tx_gas_limit {
        cfg.tx_gas_limit_cap = Some(u64::MAX);
    }

    // Apply Stylus configuration options
    cfg.stylus.arbos_version = stylus.arbos_version;
    cfg.stylus.stylus_version = stylus.stylus_version;
    cfg.stylus.ink_price = stylus.ink_price;
    cfg.stylus.max_stack_depth = stylus.max_stack_depth;
    cfg.stylus.free_pages = stylus.free_pages;
    cfg.stylus.page_gas = stylus.page_gas;
    cfg.stylus.page_ramp = stylus.page_ramp;
    cfg.stylus.page_limit = stylus.page_limit;
    cfg.stylus.min_init_gas = stylus.min_init_gas;
    cfg.stylus.min_cached_init_gas = stylus.min_cached_init_gas;
    cfg.stylus.init_cost_scalar = stylus.init_cost_scalar;
    cfg.stylus.cached_cost_scalar = stylus.cached_cost_scalar;
    cfg.stylus.expiry_days = stylus.expiry_days;
    cfg.stylus.keepalive_days = stylus.keepalive_days;
    cfg.stylus.block_cache_size = stylus.block_cache_size;
    cfg.stylus.max_wasm_size = stylus.max_wasm_size;
    cfg.stylus.debug_mode = stylus.debug_mode;
    cfg.stylus.disable_auto_cache = stylus.disable_auto_cache;
    cfg.stylus.disable_auto_activate = stylus.disable_auto_activate;

    cfg
}
