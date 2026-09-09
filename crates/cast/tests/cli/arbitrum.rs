//! Hermetic coverage for Arbitrum tracing configuration.

use alloy_network::{ReceiptResponse, TransactionBuilder};
use alloy_primitives::address;
use alloy_provider::Provider;
use alloy_rpc_types::TransactionRequest;
use alloy_sol_types::{SolCall, SolValue, sol};
use anvil::NodeConfig;
use foundry_config::stylus::StylusConfig;
use foundry_evm_networks::NetworkConfigs;
use foundry_test_utils::str;

sol! {
    function inkPrice() external view returns (uint32);
}

casttest!(arbitrum_fork_trace_and_replay_apply_stylus_config, async |prj, cmd| {
    let (_api, handle) = anvil::spawn(
        NodeConfig::test()
            .with_networks(NetworkConfigs::with_arbitrum())
            .with_stylus_config(StylusConfig { ink_price: Some(13_579), ..Default::default() }),
    )
    .await;
    let provider = handle.http_provider();
    let query = TransactionRequest::default()
        .with_to(address!("0000000000000000000000000000000000000071"))
        .with_input(inkPriceCall {}.abi_encode());
    let pending = provider
        .send_transaction(
            query
                .clone()
                .with_from(provider.get_accounts().await.unwrap()[0])
                .with_gas_limit(1_000_000)
                .into(),
        )
        .await
        .unwrap();
    let receipt = tokio::time::timeout(std::time::Duration::from_secs(20), pending.get_receipt())
        .await
        .expect("source transaction must be mined before tracing")
        .unwrap();
    assert!(receipt.status());

    prj.update_config(|config| {
        config.stylus.ink_price = Some(24_680);
    });
    cmd.set_current_dir(prj.root());

    cmd.args([
        "call",
        "--trace",
        "--disable-external-identification",
        "--rpc-url",
        &handle.http_endpoint(),
        "0x0000000000000000000000000000000000000071",
        "inkPrice()(uint32)",
    ])
    .assert_success()
    .stderr_eq("")
    .stdout_eq(str![[r#"
Traces:
  [..] 0x0000000000000000000000000000000000000071::inkPrice()
    └─ ← [Return] 0x0000000000000000000000000000000000000000000000000000000000006068


Transaction successfully executed.
[GAS]

"#]]);

    cmd.cast_fuse()
        .args([
            "run",
            &receipt.transaction_hash().to_string(),
            "--disable-external-identification",
            "--rpc-url",
            &handle.http_endpoint(),
        ])
        .assert_success()
        .stderr_eq("Executing previous transactions from the block.\n")
        .stdout_eq(str![[r#"
...
    └─ ← [Return] 0x0000000000000000000000000000000000000000000000000000000000006068
...
Transaction successfully executed.
[GAS]

"#]]);

    // Local tracing overrides must not mutate the fork source.
    let output = provider.call(query.into()).await.unwrap();
    assert_eq!(u32::abi_decode(&output).unwrap(), 13_579);
});
