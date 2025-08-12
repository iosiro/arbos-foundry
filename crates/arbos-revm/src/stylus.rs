use std::num::NonZeroUsize;
use std::vec::Vec;

use arbutil::{evm::EvmData, Bytes20, Bytes32};

use lru::LruCache;
use revm::primitives::FixedBytes;
use revm::{
    context::{Block, Cfg, ContextTr, Transaction},
    interpreter::InputsImpl,
    primitives::{alloy_primitives::U64, keccak256, U256},
};
use stylus::prover::machine::Module;
use stylus::prover::programs::StylusData;
use wasmer_types::lib::std::sync::Mutex;

use crate::api::ArbitrumContextTr;
use crate::chain_config::ArbitrumChainInfoTr;

type ProgramCacheEntry = (Vec<u8>, Module, StylusData);

lazy_static::lazy_static! {
    pub static ref PROGRAM_CACHE: Mutex<LruCache<FixedBytes<32>, ProgramCacheEntry>> = Mutex::new(LruCache::new(NonZeroUsize::new(1024).unwrap()));
}

pub fn build_evm_data<CTX>(context: &mut CTX, input: InputsImpl) -> EvmData
where
    CTX: ArbitrumContextTr,
{
    // find target_address in context.evm.journaled_state.call_stack excluding last
    // if found, set reentrant to true
    // else set reentrant to false
    // let reentrant = if context
    //     .evm
    //     .journaled_state
    //     .call_stack
    //     .iter()
    //     .filter(|&x| *x == self.inputs.target_address)
    //     .count()
    //     > 1
    // {
    //     1
    // } else {
    //     0
    // };
    let reentrant = 0;

    let config_env = context.cfg();
    let arbos_env = context.chain();

    let block_env = context.block();
    let tx_env = context.tx();

    let base_fee = block_env.basefee();

    let evm_data: EvmData = EvmData {
        arbos_version: arbos_env.arbos_version() as u64,
        block_basefee: Bytes32::from(U256::from(base_fee).to_be_bytes()),
        chainid: config_env.chain_id(),
        block_coinbase: Bytes20::try_from(block_env.beneficiary().as_slice()).unwrap(),
        block_gas_limit: U64::wrapping_from(block_env.gas_limit()).to::<u64>(),
        block_number: U64::wrapping_from(block_env.number()).to::<u64>(),
        block_timestamp: U64::wrapping_from(block_env.timestamp()).to::<u64>(),
        contract_address: Bytes20::try_from(input.target_address.as_slice()).unwrap(),
        module_hash: Bytes32::try_from(keccak256(input.target_address.as_slice()).as_slice())
            .unwrap(),
        msg_sender: Bytes20::try_from(input.caller_address.as_slice()).unwrap(),
        msg_value: Bytes32::try_from(input.call_value.to_be_bytes_vec()).unwrap(),
        tx_gas_price: Bytes32::from(
            U256::from(tx_env.effective_gas_price(base_fee as u128)).to_be_bytes(),
        ),
        tx_origin: Bytes20::try_from(tx_env.caller().as_slice()).unwrap(),
        reentrant,
        return_data_len: 0,
        cached: true,
        tracing: true,
    };

    evm_data
}
