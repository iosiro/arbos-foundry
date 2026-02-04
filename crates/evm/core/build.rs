#![allow(clippy::disallowed_macros)]

use std::{env, fs, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir.parent().unwrap().parent().unwrap().parent().unwrap();
    let wat_path = workspace_root.join("testdata/fixtures/Stylus/foundry_stylus_program.wat");
    let wasm_path = workspace_root.join("testdata/fixtures/Stylus/foundry_stylus_program.wasm");
    let wasm_br_path =
        workspace_root.join("testdata/fixtures/Stylus/foundry_stylus_program.wasm.br");

    println!("cargo:rerun-if-changed={}", wat_path.display());

    let wat = fs::read(&wat_path).expect("failed to read wat file");
    let wasm = arbos_revm::utils::wat2wasm(&wat).expect("failed to convert wat to wasm");
    fs::write(&wasm_path, &wasm).expect("failed to write wasm file");

    let wasm_br = arbos_revm::utils::brotli_compress(
        &wasm,
        11,
        22, // DEFAULT_WINDOW_SIZE
        arbos_revm::utils::Dictionary::Empty,
    )
    .expect("failed to compress wasm");
    fs::write(&wasm_br_path, wasm_br).expect("failed to write compressed wasm file");
}
