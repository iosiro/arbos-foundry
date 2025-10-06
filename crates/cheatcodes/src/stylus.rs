use std::{fs, ops::Range, path::PathBuf};

use alloy_primitives::{Bytes, U256, hex};
use alloy_sol_types::SolValue;
use foundry_config::fs_permissions::FsAccessKind;
use revm::{
    context::CreateScheme,
    interpreter::{CallInputs, CallScheme, CreateInputs},
};
use spec::Vm::*;
use wasm_encoder::{Module, RawSection};
use wasmparser::{Parser, Payload};

use crate::{Cheatcode, Cheatcodes, CheatcodesExecutor, CheatsCtxt, Result};

// pub const PROJECT_HASH_SECTION_NAME: &str = "project_hash";

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
    let path = if path.ends_with(".wasm") {
        PathBuf::from(path)
    } else {
        bail!("invalid artifact path")
    };

    let path = state.config.ensure_path_allowed(path, FsAccessKind::Read)?;
    let wasm = fs::read(path)?;


    // We convert the WASM from binary to text and back to binary as this trick removes any dangling
    // mentions of reference types in the wasm body, which are not yet supported by Arbitrum chain backends.
    let wat_str = if let Ok(wat_str) = wasmprinter::print_bytes(&wasm) {
        wat_str
    } else {
        bail!("failed to convert WASM to WAT")
    };

    let wasm = if let Ok(wasm) = stylus::wasmer::wat2wasm(wat_str.as_bytes()) {
        wasm
    } else {
        bail!("failed to convert WAT to WASM")
    };

    // let wasm = if let Some(project_hash) = project_hash_section(&wasm) {
    //     add_custom_section(&wasm, project_hash[0..32].try_into().unwrap())
    // } else {
    //    wasm.to_vec()
    // };

    let wasm = strip_user_metadata(&wasm)?;

    let wasm = if compress {
        // Compress the artifact if it is a Stylus artifact
        if let Ok(compressed) =
            stylus::brotli::compress(&wasm, 11, 22, stylus::brotli::Dictionary::Empty)
        {
            compressed
        } else {
            bail!("failed to compress stylus artifact")
        }
    } else {
        wasm
    };

    let wasm = [arbos_revm::constants::STYLUS_DISCRIMINANT, &[arbos_revm::constants::STYLUS_EOF_NO_DICT], wasm.as_ref()].concat();

    // add init code
    let artifact = get_init_code_of_empty_constructor(wasm);

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

// pub fn project_hash_section(wasm_file_bytes: &[u8]) -> Option<&[u8]> {
//     let parser = wasmparser::Parser::new(0);
//     for payload in parser.parse_all(wasm_file_bytes) {
//         if let Ok(payload) = payload {
//             if let wasmparser::Payload::CustomSection(reader) = payload {
//                 if reader.name() == PROJECT_HASH_SECTION_NAME {
//                     return Some(reader.data());
//                 }
//             }
//         }
//     }
//     None
// }

// fn add_custom_section(wasm_file_bytes: &[u8], project_hash: [u8; 32]) -> Vec<u8> {
//     let mut bytes = vec![];
//     bytes.extend_from_slice(wasm_file_bytes);
//     wasm_gen::write_custom_section(&mut bytes, PROJECT_HASH_SECTION_NAME, &project_hash);
//     bytes
// }

fn strip_user_metadata(wasm_file_bytes: &[u8]) -> Result<Vec<u8>> {
    let mut module = Module::new();
    // Parse the input WASM and iterate over the sections
    let parser = Parser::new(0);
    for payload in parser.parse_all(wasm_file_bytes) {
        match payload {
            Ok(Payload::CustomSection { .. }) => {
                // Skip custom sections to remove sensitive metadata
                debug!("stripped custom section from user wasm to remove any sensitive data");
            }
            Ok(Payload::UnknownSection { .. }) => {
                // Skip unknown sections that might not be sensitive
                debug!("stripped unknown section from user wasm to remove any sensitive data");
            }
            Ok(item) => {
                // Handle other sections as normal.
                if let Some(section) = item.as_section() {
                    let (id, range): (u8, Range<usize>) = section;
                    let data_slice = &wasm_file_bytes[range.start..range.end]; // Start at the beginning of the range
                    let raw_section = RawSection {
                        id,
                        data: data_slice,
                    };
                    module.section(&raw_section);
                }
            },
            Err(e) => {
                bail!("error parsing wasm: {}", e);
            }
        }
    }
    // Return the stripped WASM binary
    Ok(module.finish())
}