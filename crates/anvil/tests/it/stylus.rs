use alloy_primitives::{Address, Bytes, hex};
use alloy_provider::Provider;
use alloy_rpc_types::TransactionRequest;
use anvil::{NodeConfig, spawn};
use arbos_revm::utils::{Dictionary, brotli_compress};

#[tokio::test(flavor = "multi_thread")]
async fn test_stylus_etch_and_call() {
    let config =
        NodeConfig::test().with_networks(foundry_evm_networks::NetworkConfigs::with_arbitrum());
    let (api, handle) = spawn(config).await;
    let provider = handle.http_provider();

    // Build Stylus bytecode: 0xEFF00000 prefix + brotli-compressed WASM.
    let wasm = include_bytes!("../../../../testdata/fixtures/Stylus/foundry_stylus_program.wasm");
    let compressed_wasm = brotli_compress(wasm, 11, 22, Dictionary::Empty).unwrap();
    let mut stylus_bytecode = vec![0xEF, 0xF0, 0x00, 0x00];
    stylus_bytecode.extend_from_slice(&compressed_wasm);

    // Etch the Stylus contract at a chosen address.
    let stylus_addr = Address::with_last_byte(0x42);
    api.anvil_set_code(stylus_addr, stylus_bytecode.into()).await.unwrap();

    // Verify code was set.
    let code = provider.get_code_at(stylus_addr).await.unwrap();
    assert!(!code.is_empty(), "code should be set at address");

    // Call the echo contract — it should return the same data we send.
    let test_data = hex!("deadbeef");
    let tx = TransactionRequest::default()
        .to(stylus_addr)
        .input(Bytes::copy_from_slice(&test_data).into());
    let result = provider.call(tx.into()).await.unwrap();
    assert_eq!(result.as_ref(), &test_data[..], "echo program should return input data");
}
