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
    // Supply the ABI locally so a clean signature cache produces the same decoded trace.
    prj.add_source(
        "ArbWasm.sol",
        "interface ArbWasm { function inkPrice() external view returns (uint32); }",
    );
    cmd.set_current_dir(prj.root());

    cmd.args([
        "call",
        "--trace",
        "--disable-external-identification",
        "--with-local-artifacts",
        "--rpc-url",
        &handle.http_endpoint(),
        "0x0000000000000000000000000000000000000071",
        "inkPrice()(uint32)",
    ])
    .assert_success()
    .stderr_eq("Compiling project to generate artifacts\n")
    .stdout_eq(str![[r#"
...
Traces:
  [..] 0x0000000000000000000000000000000000000071::inkPrice()
    └─ ← [Return] 24680 [2.468e4]


Transaction successfully executed.
[GAS]

"#]]);

    cmd.cast_fuse()
        .args([
            "run",
            &receipt.transaction_hash().to_string(),
            "--disable-external-identification",
            "--with-local-artifacts",
            "--rpc-url",
            &handle.http_endpoint(),
        ])
        .assert_success()
        .stderr_eq("Executing previous transactions from the block.\nCompiling project to generate artifacts\n")
        .stdout_eq(str![[r#"
...
    └─ ← [Return] 24680 [2.468e4]
...
Transaction successfully executed.
[GAS]

"#]]);

    // Local tracing overrides must not mutate the fork source.
    let output = provider.call(query.into()).await.unwrap();
    assert_eq!(u32::abi_decode(&output).unwrap(), 13_579);
});

casttest!(arbitrum_default_fork_execution_and_ethereum_opt_in, async |prj, cmd| {
    // An Ethereum source must not silently change the selected local execution family.
    let (_api, handle) = anvil::spawn(NodeConfig::test().with_chain_id(Some(1_u64))).await;
    let provider = handle.http_provider();
    let query = TransactionRequest::default()
        .with_to(address!("0000000000000000000000000000000000000071"))
        .with_input(inkPriceCall {}.abi_encode());
    assert!(provider.call(query.clone().into()).await.unwrap().is_empty());
    let pending = provider
        .send_transaction(
            query
                .with_from(provider.get_accounts().await.unwrap()[0])
                .with_gas_limit(1_000_000)
                .into(),
        )
        .await
        .unwrap();
    let receipt = tokio::time::timeout(std::time::Duration::from_secs(20), pending.get_receipt())
        .await
        .unwrap()
        .unwrap();
    assert!(receipt.status());
    prj.add_source(
        "ArbWasm.sol",
        "interface ArbWasm { function inkPrice() external view returns (uint32); }",
    );
    cmd.set_current_dir(prj.root());
    for ethereum in [false, true] {
        for replay in [false, true] {
            cmd.cast_fuse();
            if ethereum {
                cmd.env("FOUNDRY_NETWORK", "ethereum");
            }
            if replay {
                cmd.args(["run", &receipt.transaction_hash().to_string()]);
            } else {
                cmd.args([
                    "call",
                    "--trace",
                    "0x0000000000000000000000000000000000000071",
                    "inkPrice()(uint32)",
                ]);
            }
            let output = cmd
                .args([
                    "--disable-external-identification",
                    "--with-local-artifacts",
                    "--rpc-url",
                    &handle.http_endpoint(),
                ])
                .assert_success();
            if ethereum {
                output.stdout_eq(str![[r#"
...
    └─ ← [Stop]
...
Transaction successfully executed.
[GAS]

"#]]);
            } else {
                output.stdout_eq(str![[r#"
...
    └─ ← [Return] 10000 [1e4]
...
Transaction successfully executed.
[GAS]

"#]]);
            }
        }
    }
});
