use std::{fs, path::PathBuf};

use alloy_primitives::{Address, Bytes, FixedBytes, U256, hex};
use alloy_sol_types::{SolCall, SolValue, sol};
use arbos_revm::{
    state::program::activate_program,
    stylus_executor::stylus_code,
    utils::{Dictionary, brotli_compress, brotli_decompress, strip_wasm_for_stylus},
};
use foundry_config::fs_permissions::FsAccessKind;
use foundry_evm_core::constants::{DEFAULT_STYLUS_DEPLOYER, DEFAULT_STYLUS_DEPLOYER_RUNTIME_CODE};
use revm::{
    bytecode::Bytecode,
    context::{ContextTr, JournalTr},
    interpreter::{CallInputs, CallScheme},
    primitives::KECCAK_EMPTY,
};
use spec::Vm::*;

use crate::{Cheatcode, Cheatcodes, CheatcodesExecutor, CheatsCtxt, Result};

/// Generous activation fee budget (1 ETH). StylusDeployer sends `msg.value - initValue` to
/// ARB_WASM for activation and refunds excess to `msg.sender`.
const ACTIVATION_FEE_BUDGET: U256 = U256::from_limbs([1_000_000_000_000_000_000u64, 0, 0, 0]);

sol! {
    /// StylusDeployer.deploy function ABI
    function deploy(bytes initCode, bytes constructorArgs, uint256 value, bytes32 salt)
        external payable returns (address);
}

impl Cheatcode for deployStylusCode_0Call {
    fn apply_full(&self, ccx: &mut CheatsCtxt, executor: &mut dyn CheatcodesExecutor) -> Result {
        let Self { artifactPath: path } = self;
        deploy_stylus_code(ccx, executor, path, None, None, None)
    }
}

impl Cheatcode for deployStylusCode_1Call {
    fn apply_full(&self, ccx: &mut CheatsCtxt, executor: &mut dyn CheatcodesExecutor) -> Result {
        let Self { artifactPath: path, constructorArgs: args } = self;
        deploy_stylus_code(ccx, executor, path, Some(args), None, None)
    }
}

impl Cheatcode for deployStylusCode_2Call {
    fn apply_full(&self, ccx: &mut CheatsCtxt, executor: &mut dyn CheatcodesExecutor) -> Result {
        let Self { artifactPath: path, value } = self;
        deploy_stylus_code(ccx, executor, path, None, Some(*value), None)
    }
}

impl Cheatcode for deployStylusCode_3Call {
    fn apply_full(&self, ccx: &mut CheatsCtxt, executor: &mut dyn CheatcodesExecutor) -> Result {
        let Self { artifactPath: path, constructorArgs: args, value } = self;
        deploy_stylus_code(ccx, executor, path, Some(args), Some(*value), None)
    }
}

impl Cheatcode for deployStylusCode_4Call {
    fn apply_full(&self, ccx: &mut CheatsCtxt, executor: &mut dyn CheatcodesExecutor) -> Result {
        let Self { artifactPath: path, salt } = self;
        deploy_stylus_code(ccx, executor, path, None, None, Some((*salt).into()))
    }
}

impl Cheatcode for deployStylusCode_5Call {
    fn apply_full(&self, ccx: &mut CheatsCtxt, executor: &mut dyn CheatcodesExecutor) -> Result {
        let Self { artifactPath: path, constructorArgs: args, salt } = self;
        deploy_stylus_code(ccx, executor, path, Some(args), None, Some((*salt).into()))
    }
}

impl Cheatcode for deployStylusCode_6Call {
    fn apply_full(&self, ccx: &mut CheatsCtxt, executor: &mut dyn CheatcodesExecutor) -> Result {
        let Self { artifactPath: path, value, salt } = self;
        deploy_stylus_code(ccx, executor, path, None, Some(*value), Some((*salt).into()))
    }
}

impl Cheatcode for deployStylusCode_7Call {
    fn apply_full(&self, ccx: &mut CheatsCtxt, executor: &mut dyn CheatcodesExecutor) -> Result {
        let Self { artifactPath: path, constructorArgs: args, value, salt } = self;
        deploy_stylus_code(ccx, executor, path, Some(args), Some(*value), Some((*salt).into()))
    }
}

impl Cheatcode for getStylusCodeCall {
    fn apply(&self, state: &mut Cheatcodes) -> Result {
        let Self { artifactPath: path } = self;
        get_stylus_code(state, path)
    }
}

impl Cheatcode for getStylusInitCodeCall {
    fn apply(&self, state: &mut Cheatcodes) -> Result {
        let Self { artifactPath: path } = self;
        get_stylus_init_code(state, path)
    }
}

impl Cheatcode for brotliCompressCall {
    fn apply(&self, _state: &mut Cheatcodes) -> Result {
        let Self { data } = self;
        compress_brotli(data)
    }
}

impl Cheatcode for brotliDecompressCall {
    fn apply(&self, _state: &mut Cheatcodes) -> Result {
        let Self { compressed } = self;
        decompress_brotli(compressed)
    }
}

/// Deploys a Stylus contract via the StylusDeployer contract.
///
/// Delegates to the StylusDeployer contract which atomically handles:
/// 1. Deploy compressed bytecode via CREATE/CREATE2
/// 2. Activate via ARB_WASM precompile
/// 3. Call constructor if args provided
///
/// This produces a single CALL transaction when broadcasting, ensuring on-chain
/// replay deploys a fully activated contract.
fn deploy_stylus_code(
    ccx: &mut CheatsCtxt,
    executor: &mut dyn CheatcodesExecutor,
    path: &str,
    constructor_args: Option<&Bytes>,
    value: Option<U256>,
    salt: Option<U256>,
) -> Result {
    let compressed_bytecode = get_stylus_bytecode(ccx.state, path)?;
    let init_code = get_init_code_of_empty_constructor(compressed_bytecode.to_vec());

    // Build constructor calldata: stylus_constructor() selector + args, or empty
    let constructor_calldata = if let Some(args) = constructor_args {
        // cast sig 'stylus_constructor()' => 0x5585258d
        let mut calldata = vec![0x55, 0x85, 0x25, 0x8d];
        calldata.extend_from_slice(args);
        Bytes::from(calldata)
    } else {
        Bytes::new()
    };

    let init_value = value.unwrap_or(U256::ZERO);

    // salt: None → bytes32(0) → StylusDeployer uses CREATE
    // salt: Some(val) → bytes32(val) → StylusDeployer uses CREATE2
    let salt_bytes: FixedBytes<32> =
        if let Some(s) = salt { FixedBytes::from(s.to_be_bytes::<32>()) } else { FixedBytes::ZERO };

    // ABI-encode the deploy(bytes,bytes,uint256,bytes32) call
    let deploy_calldata =
        deployCall::new((init_code.into(), constructor_calldata, init_value, salt_bytes))
            .abi_encode();

    // Use the configured deployer address or the well-known default
    let deployer_address =
        ccx.state.config.evm_opts.stylus_config.deployer_address.unwrap_or(DEFAULT_STYLUS_DEPLOYER);

    // If broadcast is active, set call_from_code so this exec_call is captured
    let is_broadcasting = ccx.state.broadcast.is_some();
    if let Some(broadcast) = &mut ccx.state.broadcast {
        broadcast.call_from_code = true;
    }

    // Temporarily clear caller's code so StylusDeployer can refund excess ETH.
    // The refund uses a low-level call that requires the recipient to accept ETH;
    // contracts without receive()/fallback() payable would reject it.
    let caller = ccx.caller;
    let caller_code = ccx.ecx.journal_mut().code(caller).ok().unwrap_or_default().data;
    if !caller_code.is_empty() {
        ccx.ecx.journaled_state.set_code_with_hash(caller, Bytecode::default(), KECCAK_EMPTY);
    }

    // Record balance before to compute actual activation fee afterwards.
    // When broadcasting, the inspector reroutes the transfer to broadcast.new_origin (the
    // signer EOA), so we must track *that* account's balance — not ccx.caller (the script).
    let balance_account =
        if let Some(broadcast) = &ccx.state.broadcast { broadcast.new_origin } else { caller };
    let balance_before = ccx.ecx.journaled_state.load_account(balance_account)?.data.info.balance;

    // Ensure StylusDeployer is available. When broadcasting, the deployer must exist
    // on-chain — etching it locally would produce a simulation that can't replay on-chain.
    // For local tests/scripts (no broadcast), deploy on-demand to avoid polluting initial
    // EVM state which would shift deterministic fuzz sequences for unrelated invariant tests.
    let deployer_account = ccx.ecx.journaled_state.load_account(deployer_address)?;
    if deployer_account.data.info.code.as_ref().is_none_or(|code| code.is_empty()) {
        if is_broadcasting {
            return Err(fmt_err!(
                "StylusDeployer not found at {deployer_address}. \
                 Deploy it on-chain or set a custom address via `stylus.deployer_address` in foundry.toml"
            ));
        }
        ccx.ecx.journaled_state.set_code_with_hash(
            deployer_address,
            Bytecode::new_raw(Bytes::from_static(DEFAULT_STYLUS_DEPLOYER_RUNTIME_CODE)),
            KECCAK_EMPTY,
        );
    }

    // Call StylusDeployer with a generous activation fee budget (1 ETH).
    // The deployer sends `msg.value - initValue` to ARB_WASM for activation
    // and refunds any excess back to msg.sender.
    let total_value = init_value + ACTIVATION_FEE_BUDGET;
    let outcome = executor.exec_call(
        CallInputs {
            input: revm::interpreter::CallInput::Bytes(deploy_calldata.into()),
            return_memory_offset: 0..0,
            gas_limit: ccx.gas_limit,
            bytecode_address: deployer_address,
            target_address: deployer_address,
            caller,
            value: revm::interpreter::CallValue::Transfer(total_value),
            scheme: CallScheme::Call,
            is_static: false,
            known_bytecode: None,
        },
        ccx,
    );

    // Restore caller's code before handling the result
    if !caller_code.is_empty() {
        ccx.ecx.journaled_state.set_code(caller, Bytecode::new_raw(caller_code));
    }

    let outcome = outcome?;
    if !outcome.result.result.is_ok() {
        return Err(crate::Error::from(outcome.result.output));
    }

    // Compute actual activation data fee from balance change of the paying account.
    // Net cost = init_value + data_fee (budget minus refund).
    let balance_after = ccx.ecx.journaled_state.load_account(balance_account)?.data.info.balance;
    let total_spent = balance_before.saturating_sub(balance_after);
    let data_fee = total_spent.saturating_sub(init_value);

    // For broadcast, set the tx value to init_value + estimated fee with 20% buffer
    // instead of the full 1 ETH budget used during simulation.
    if is_broadcasting
        && let Some(last_tx) = ccx.state.broadcastable_transactions.back_mut()
        && let Some(tx) = last_tx.transaction.as_unsigned_mut()
    {
        let buffered_fee = data_fee * U256::from(120) / U256::from(100);
        tx.value = Some(init_value + buffered_fee);
    }

    // StylusDeployer returns the deployed address as ABI-encoded address
    let output = &outcome.result.output;
    if output.len() < 32 {
        return Err(fmt_err!("StylusDeployer returned invalid output"));
    }
    let address = Address::from_slice(&output[12..32]);

    Ok(address.abi_encode())
}

/// Activates a Stylus program by compiling and storing it directly.
/// Kept for potential future use (e.g., `vm.etch` scenarios).
#[allow(dead_code)]
fn activate_stylus_program(ccx: &mut CheatsCtxt, program_address: Address) -> Result<()> {
    let code_hash = ccx
        .ecx
        .journal_mut()
        .code_hash(program_address)
        .map_err(|e| fmt_err!("failed to get code hash: {:?}", e))?
        .data;

    let bytecode = ccx.ecx.journal_mut().code(program_address).ok().unwrap_or_default().data;

    let wasm_bytecode = match stylus_code(&bytecode) {
        Ok(Some(code)) => code,
        Ok(None) => return Err(fmt_err!("program is not a Stylus WASM contract")),
        Err(err) => {
            return Err(fmt_err!(
                "failed to decode Stylus bytecode: {}",
                String::from_utf8_lossy(&err)
            ));
        }
    };

    activate_program(ccx.ecx, code_hash, &wasm_bytecode, true)
        .map_err(|e| fmt_err!("failed to activate program: {e}"))?;

    Ok(())
}

/// Returns the compressed and prefixed Stylus bytecode from a WASM artifact file.
///
/// Can parse following input formats:
/// - `path/to/artifact.wasm` - uncompressed WASM
/// - `path/to/artifact.wasm.br` - pre-compressed WASM
///
/// This function returns raw bytecode suitable for use with external deployment contracts like
/// StylusDeployer, or for wrapping in init code for direct CREATE/CREATE2 deployment.
fn get_stylus_bytecode(state: &Cheatcodes, path: &str) -> Result<Bytes> {
    let path = if path.ends_with(".wasm") || path.ends_with(".wasm.br") {
        PathBuf::from(path)
    } else {
        bail!("invalid artifact path")
    };

    let path = state.config.ensure_path_allowed(path, FsAccessKind::Read)?;
    let artifact = fs::read(path)?;

    // if it starts with WASM magic number, we assume it's a Stylus artifact and we first
    // need to compress it and then prefix it with Stylus discriminant
    let artifact = if artifact.starts_with(&[0x00, 0x61, 0x73, 0x6d]) {
        // Strip user metadata and dangling reference types
        let artifact = strip_wasm_for_stylus(&artifact)
            .map_err(|e| fmt_err!("failed to strip WASM for Stylus: {}", e))?;

        // Compress the artifact
        brotli_compress(&artifact, 11, 22, Dictionary::Empty)
            .map_err(|_| fmt_err!("failed to compress stylus artifact"))?
    } else {
        artifact
    };

    // If the artifact is already prefixed with Stylus discriminant, we can return it as is.
    // Otherwise, we need to prefix it with Stylus discriminant and compression level.
    let artifact = if artifact.starts_with(arbos_revm::constants::STYLUS_DISCRIMINANT) {
        // If the artifact is already prefixed with Stylus discriminant, we can return it as is.
        artifact
    } else {
        // If not, we need to prefix it with Stylus discriminant and compression level.
        [arbos_revm::constants::STYLUS_DISCRIMINANT, &[0], artifact.as_ref()].concat()
    };

    Ok(Bytes::from(artifact))
}

fn get_init_code_of_empty_constructor(bytecode: Vec<u8>) -> Vec<u8> {
    let mut init = Vec::with_capacity(2 + bytecode.len() + 4);

    // step 1: fixed header
    init.extend_from_slice(hex!("608060405234801561001057600080fd5b50").as_slice());

    // step 2: push the bytecode length (always 2 bytes)
    init.extend_from_slice(&[0x61]);

    let bytecode_len = bytecode.len();

    // always returns at least 32 bytes
    let bytecode_len_bytes = bytecode_len.to_be_bytes();

    // can safely ignore the first 30 bytes as length is always less than 2^16
    init.extend_from_slice(&bytecode_len_bytes[bytecode_len_bytes.len() - 2..]);

    // step 3: fixed footer
    init.extend_from_slice(hex!("806100206000396000f3fe").as_slice());

    // push the bytecode itself
    init.extend_from_slice(&bytecode);

    init
}

/// Returns the compressed and prefixed Stylus bytecode (runtime code) for a contract.
/// This applies the same compression and prefixing logic as deployStylusCode, but returns
/// the raw bytecode without the init code wrapper. Suitable for use with `vm.etch`.
fn get_stylus_code(state: &Cheatcodes, path: &str) -> Result {
    let bytecode = get_stylus_bytecode(state, path)?;
    Ok(bytecode.abi_encode())
}

/// Returns init code for a Stylus contract suitable for CREATE/CREATE2 or the StylusDeployer.
/// Wraps the compressed bytecode in EVM init code using `get_init_code_of_empty_constructor`.
fn get_stylus_init_code(state: &Cheatcodes, path: &str) -> Result {
    let bytecode = get_stylus_bytecode(state, path)?;
    let init_code = get_init_code_of_empty_constructor(bytecode.to_vec());
    Ok(Bytes::from(init_code).abi_encode())
}

/// Compresses the given data using Brotli compression.
/// Uses quality 11 and window size 22, without dictionary.
fn compress_brotli(data: &Bytes) -> Result {
    let compressed = brotli_compress(data, 11, 22, Dictionary::Empty)
        .map_err(|_| fmt_err!("brotli compression failed"))?;
    Ok(Bytes::from(compressed).abi_encode())
}

/// Decompresses Brotli-compressed data.
fn decompress_brotli(compressed: &Bytes) -> Result {
    let decompressed = brotli_decompress(compressed, Dictionary::Empty)
        .map_err(|_| fmt_err!("brotli decompression failed"))?;
    Ok(Bytes::from(decompressed).abi_encode())
}
