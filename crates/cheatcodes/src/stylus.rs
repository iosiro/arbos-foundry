use std::{fs, path::PathBuf};

use alloy_primitives::{Bytes, U256, hex};
use alloy_sol_types::SolValue;
use foundry_config::fs_permissions::FsAccessKind;
use revm::{
    context::CreateScheme,
    interpreter::{CallInputs, CallScheme, CreateInputs},
};
use spec::Vm::*;

use crate::{Cheatcode, Cheatcodes, CheatcodesExecutor, CheatsCtxt, Result};

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

/// Helper function to deploy stylus contract from artifact code.
/// Uses CREATE2 scheme if salt specified.
fn deploy_stylus_code(
    ccx: &mut CheatsCtxt,
    executor: &mut dyn CheatcodesExecutor,
    path: &str,
    constructor_args: Option<&Bytes>,
    value: Option<U256>,
    salt: Option<U256>,
) -> Result {
    let bytecode = get_artifact_code(ccx.state, path, false)?.to_vec();

    let scheme =
        if let Some(salt) = salt { CreateScheme::Create2 { salt } } else { CreateScheme::Create };

    let create_value = if constructor_args.is_some() {
        // if constructor args are provided, we need to deploy the contract with value
        U256::ZERO
    } else {
        // if no constructor args, we can deploy without value
        value.unwrap_or(U256::ZERO)
    };

    let outcome = executor.exec_create(
        CreateInputs {
            caller: ccx.caller,
            scheme,
            value: create_value,
            init_code: bytecode.into(),
            gas_limit: ccx.gas_limit,
        },
        ccx,
    )?;

    if !outcome.result.result.is_ok() {
        return Err(crate::Error::from(outcome.result.output));
    }

    let address = outcome.address.ok_or_else(|| fmt_err!("contract creation failed"))?;

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
            },
            ccx,
        )?;

        if !outcome.result.result.is_ok() {
            return Err(crate::Error::from(outcome.result.output));
        }
    }

    Ok(address.abi_encode())
}

/// Returns the path to the json artifact depending on the input
///
/// Can parse following input formats:
/// - `path/to/artifact.wasm`
/// - `path/to/artifact.wasm.br`
fn get_artifact_code(state: &Cheatcodes, path: &str, compress: bool) -> Result<Bytes> {
    let path = if path.ends_with(".wasm") || path.ends_with(".wasm.br") {
        PathBuf::from(path)
    } else {
        bail!("invalid artifact path")
    };

    let path = state.config.ensure_path_allowed(path, FsAccessKind::Read)?;
    let artifact = fs::read(path)?;

    // if it starts with WASM magic number, we assume it's a Stylus artifact and we first
    // need to compress it and then prefix it with Stylus discriminant
    let artifact = if artifact.starts_with(&[0x00, 0x61, 0x73, 0x6d]) && compress {
        // Compress the artifact if it is a Stylus artifact
        if let Ok(compressed) =
            stylus::brotli::compress(&artifact, 11, 22, stylus::brotli::Dictionary::StylusProgram)
        {
            compressed
        } else {
            bail!("failed to compress stylus artifact")
        }
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
        [arbos_revm::constants::STYLUS_DISCRIMINANT, &[compress as u8], artifact.as_ref()].concat()
    };

    // add init code
    let artifact = get_init_code_of_empty_constructor(artifact);

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
