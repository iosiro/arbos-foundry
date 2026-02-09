use std::{fs, path::PathBuf};

use alloy_primitives::{Address, Bytes, U256, address, hex};
use alloy_sol_types::SolValue;
use arbos_revm::{
    state::program::activate_program,
    stylus_executor::stylus_code,
    utils::{Dictionary, brotli_compress, brotli_decompress, strip_wasm_for_stylus},
};
use foundry_config::fs_permissions::FsAccessKind;
use revm::{
    context::{ContextTr, CreateScheme, JournalTr},
    interpreter::{CallInputs, CallScheme, CreateInputs},
};
use spec::Vm::*;

use crate::{Cheatcode, Cheatcodes, CheatcodesExecutor, CheatsCtxt, Result};

/// Default address of the StylusDeployer contract.
const DEFAULT_STYLUS_DEPLOYER_ADDRESS: Address =
    address!("0xcEcba2F1DC234f70Dd89F2041029807F8D03A990");

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

/// Helper function to deploy stylus contract from artifact code.
/// Matches StylusDeployer.sol behavior: deploys, activates via ARB_WASM precompile, then
/// initializes. Uses CREATE2 scheme if salt specified.
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

    let scheme =
        if let Some(salt) = salt { CreateScheme::Create2 { salt } } else { CreateScheme::Create };

    // StylusDeployer.sol always deploys with 0 value; value is used for initialization only
    let create_value =
        if constructor_args.is_some() { U256::ZERO } else { value.unwrap_or(U256::ZERO) };

    // Use the configured deployer address as the CREATE caller (matching StylusDeployer.sol)
    let deployer_address = ccx
        .state
        .config
        .evm_opts
        .stylus_config
        .deployer_address
        .unwrap_or(DEFAULT_STYLUS_DEPLOYER_ADDRESS);

    let outcome = executor.exec_create(
        CreateInputs {
            caller: deployer_address,
            scheme,
            value: create_value,
            init_code: init_code.into(),
            gas_limit: ccx.gas_limit,
        },
        ccx,
    )?;

    if !outcome.result.result.is_ok() {
        return Err(crate::Error::from(outcome.result.output));
    }

    let address = outcome.address.ok_or_else(|| fmt_err!("contract creation failed"))?;

    // Activate the program
    activate_stylus_program(ccx, address)?;

    if let Some(constructor_args) = constructor_args {
        // cast sig 'stylus_constructor()' => 0x5585258d
        let mut calldata = vec![0x55, 0x85, 0x25, 0x8d];
        calldata.extend_from_slice(constructor_args);

        let outcome = executor.exec_call(
            CallInputs {
                input: revm::interpreter::CallInput::Bytes(calldata.into()),
                return_memory_offset: 0..0,
                gas_limit: outcome.gas().remaining(),
                bytecode_address: address,
                target_address: address,
                caller: ccx.caller,
                value: revm::interpreter::CallValue::Transfer(value.unwrap_or(U256::ZERO)),
                scheme: CallScheme::Call,
                is_static: false,
                known_bytecode: None,
            },
            ccx,
        )?;

        if !outcome.result.result.is_ok() {
            return Err(crate::Error::from(outcome.result.output));
        }
    }

    Ok(address.abi_encode())
}

/// Activates a Stylus program by compiling and storing it directly.
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
