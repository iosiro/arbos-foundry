#![allow(clippy::disallowed_macros)]

use std::{env, fs, path::PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let workspace = manifest.parent().unwrap().parent().unwrap().parent().unwrap();
    let fixture = workspace.join("testdata/fixtures/Stylus/foundry_stylus_program.wat");
    let output = fixture.with_extension("wasm");
    println!("cargo:rerun-if-changed={}", fixture.display());
    let source = fs::read(&fixture).expect("failed to read Stylus WAT fixture");
    let wasm = wat::parse_bytes(&source).expect("failed to compile Stylus WAT fixture");
    fs::write(output, wasm).expect("failed to write Stylus WASM fixture");
}
