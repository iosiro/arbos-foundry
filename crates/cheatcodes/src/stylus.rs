use std::{fs, path::PathBuf};

use alloy_primitives::{Address, Bytes, U256, address, hex};
use alloy_sol_types::SolValue;
use arbos_revm::utils::{Dictionary, brotli_compress, brotli_decompress, strip_wasm_for_stylus};
use foundry_config::fs_permissions::FsAccessKind;
use foundry_evm_core::{env::FoundryContextExt, evm::FoundryEvmNetwork};
use revm::{
    context::{ContextTr, JournalTr},
    context_interface::CreateScheme,
    interpreter::{CallInput, CallInputs, CallScheme, CallValue, CreateInputs},
};
use spec::Vm::*;

use crate::{
    Cheatcode, Cheatcodes, CheatcodesExecutor, CheatsCtxt, Result,
    inspector::{exec_call, exec_create},
};

const DEFAULT_STYLUS_DEPLOYER_ADDRESS: Address =
    address!("0xcEcba2F1DC234f70Dd89F2041029807F8D03A990");

macro_rules! impl_deploy_stylus {
    ($call:ident, |$this:ident| $args:expr) => {
        impl Cheatcode for $call {
            fn apply_full<FEN: FoundryEvmNetwork>(
                &self,
                ccx: &mut CheatsCtxt<'_, '_, FEN>,
                executor: &mut dyn CheatcodesExecutor<FEN>,
            ) -> Result {
                let $this = self;
                let (path, constructor_args, value, salt) = $args;
                deploy_stylus_code(ccx, executor, path, constructor_args, value, salt)
            }
        }
    };
}

impl_deploy_stylus!(deployStylusCode_0Call, |this| (this.artifactPath.as_str(), None, None, None));
impl_deploy_stylus!(deployStylusCode_1Call, |this| (
    this.artifactPath.as_str(),
    Some(&this.constructorArgs),
    None,
    None
));
impl_deploy_stylus!(deployStylusCode_2Call, |this| (
    this.artifactPath.as_str(),
    None,
    Some(this.value),
    None
));
impl_deploy_stylus!(deployStylusCode_3Call, |this| (
    this.artifactPath.as_str(),
    Some(&this.constructorArgs),
    Some(this.value),
    None
));
impl_deploy_stylus!(deployStylusCode_4Call, |this| (
    this.artifactPath.as_str(),
    None,
    None,
    Some(U256::from_be_bytes(this.salt.0))
));
impl_deploy_stylus!(deployStylusCode_5Call, |this| (
    this.artifactPath.as_str(),
    Some(&this.constructorArgs),
    None,
    Some(U256::from_be_bytes(this.salt.0))
));
impl_deploy_stylus!(deployStylusCode_6Call, |this| (
    this.artifactPath.as_str(),
    None,
    Some(this.value),
    Some(U256::from_be_bytes(this.salt.0))
));
impl_deploy_stylus!(deployStylusCode_7Call, |this| (
    this.artifactPath.as_str(),
    Some(&this.constructorArgs),
    Some(this.value),
    Some(U256::from_be_bytes(this.salt.0))
));

impl Cheatcode for getStylusCodeCall {
    fn apply<FEN: FoundryEvmNetwork>(&self, state: &mut Cheatcodes<FEN>) -> Result {
        get_stylus_code(state, &self.artifactPath)
    }
}

impl Cheatcode for getStylusInitCodeCall {
    fn apply<FEN: FoundryEvmNetwork>(&self, state: &mut Cheatcodes<FEN>) -> Result {
        get_stylus_init_code(state, &self.artifactPath)
    }
}

impl Cheatcode for brotliCompressCall {
    fn apply<FEN: FoundryEvmNetwork>(&self, _state: &mut Cheatcodes<FEN>) -> Result {
        let compressed = brotli_compress(&self.data, 11, 22, Dictionary::Empty)
            .map_err(|_| fmt_err!("Brotli compression failed"))?;
        Ok(Bytes::from(compressed).abi_encode())
    }
}

impl Cheatcode for brotliDecompressCall {
    fn apply<FEN: FoundryEvmNetwork>(&self, _state: &mut Cheatcodes<FEN>) -> Result {
        let decompressed = brotli_decompress(&self.compressed, Dictionary::Empty)
            .map_err(|_| fmt_err!("Brotli decompression failed"))?;
        Ok(Bytes::from(decompressed).abi_encode())
    }
}

fn deploy_stylus_code<FEN: FoundryEvmNetwork>(
    ccx: &mut CheatsCtxt<'_, '_, FEN>,
    executor: &mut dyn CheatcodesExecutor<FEN>,
    path: &str,
    constructor_args: Option<&Bytes>,
    value: Option<U256>,
    salt: Option<U256>,
) -> Result {
    if ccx.state.config.evm_opts.stylus_config.disable_stylus_deployment {
        return Err(fmt_err!("Stylus deployment is disabled by configuration"));
    }

    let bytecode = get_stylus_bytecode(ccx.state, path)?;
    let init_code = get_init_code(bytecode.as_ref())?;
    let scheme = salt.map_or(CreateScheme::Create, |salt| CreateScheme::Create2 { salt });
    let create_value =
        if constructor_args.is_some() { U256::ZERO } else { value.unwrap_or_default() };
    let caller = ccx
        .state
        .config
        .evm_opts
        .stylus_config
        .deployer_address
        .unwrap_or(DEFAULT_STYLUS_DEPLOYER_ADDRESS);

    let created = exec_create(
        executor,
        CreateInputs::new(caller, scheme, create_value, init_code.into(), ccx.gas_limit, 0),
        ccx,
    )?;
    if !created.result.result.is_ok() {
        return Err(crate::Error::from(created.result.output));
    }
    let address = created.address.ok_or_else(|| fmt_err!("Stylus contract creation failed"))?;
    ccx.ecx.activate_stylus_program(address)?;

    if let Some(args) = constructor_args {
        let account = ccx.ecx.journal_mut().load_account_with_code(address)?;
        let known_bytecode =
            (account.info.code_hash, account.info.code.clone().unwrap_or_default());
        let mut calldata = Vec::with_capacity(4 + args.len());
        calldata.extend_from_slice(&[0x55, 0x85, 0x25, 0x8d]);
        calldata.extend_from_slice(args);
        let called = exec_call(
            executor,
            CallInputs {
                input: CallInput::Bytes(calldata.into()),
                return_memory_offset: 0..0,
                gas_limit: created.gas().remaining(),
                reservoir: 0,
                bytecode_address: address,
                target_address: address,
                caller: ccx.caller,
                value: CallValue::Transfer(value.unwrap_or_default()),
                scheme: CallScheme::Call,
                is_static: false,
                known_bytecode,
                charged_new_account_state_gas: false,
            },
            ccx,
        )?;
        if !called.result.result.is_ok() {
            return Err(crate::Error::from(called.result.output));
        }
    }

    Ok(address.abi_encode())
}

fn get_stylus_bytecode<FEN: FoundryEvmNetwork>(
    state: &Cheatcodes<FEN>,
    path: &str,
) -> Result<Bytes> {
    if !path.ends_with(".wasm") && !path.ends_with(".wasm.br") {
        bail!("Stylus artifact must end in .wasm or .wasm.br")
    }
    let path = state.config.ensure_path_allowed(PathBuf::from(path), FsAccessKind::Read)?;
    let artifact = fs::read(path)?;
    let compressed = if artifact.starts_with(b"\0asm") {
        let stripped = strip_wasm_for_stylus(&artifact)
            .map_err(|err| fmt_err!("failed to strip WASM for Stylus: {err}"))?;
        brotli_compress(&stripped, 11, 22, Dictionary::Empty)
            .map_err(|_| fmt_err!("failed to compress Stylus artifact"))?
    } else {
        artifact
    };
    if compressed.starts_with(arbos_revm::constants::STYLUS_DISCRIMINANT) {
        return Ok(compressed.into());
    }
    Ok([arbos_revm::constants::STYLUS_DISCRIMINANT, &[0], compressed.as_slice()].concat().into())
}

fn get_init_code(bytecode: &[u8]) -> Result<Vec<u8>> {
    let length = u16::try_from(bytecode.len())
        .map_err(|_| fmt_err!("compressed Stylus bytecode exceeds 65535 bytes"))?;
    let mut init = Vec::with_capacity(32 + bytecode.len());
    init.extend_from_slice(&hex!("608060405234801561001057600080fd5b50"));
    init.push(0x61);
    init.extend_from_slice(&length.to_be_bytes());
    init.extend_from_slice(&hex!("806100206000396000f3fe"));
    init.extend_from_slice(bytecode);
    Ok(init)
}

fn get_stylus_code<FEN: FoundryEvmNetwork>(state: &Cheatcodes<FEN>, path: &str) -> Result {
    Ok(get_stylus_bytecode(state, path)?.abi_encode())
}

fn get_stylus_init_code<FEN: FoundryEvmNetwork>(state: &Cheatcodes<FEN>, path: &str) -> Result {
    let bytecode = get_stylus_bytecode(state, path)?;
    Ok(Bytes::from(get_init_code(&bytecode)?).abi_encode())
}
