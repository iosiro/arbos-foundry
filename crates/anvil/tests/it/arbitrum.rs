use alloy_network::{ReceiptResponse, TransactionBuilder};
use alloy_primitives::{Address, address};
use alloy_provider::Provider;
use alloy_rpc_types::TransactionRequest;
use alloy_sol_types::{SolCall, SolValue, sol};
use anvil::{NodeConfig, spawn};
use foundry_config::stylus::StylusConfig;
use foundry_evm_networks::NetworkConfigs;

sol! {
    function stylusVersion() external view returns (uint16);
    function inkPrice() external view returns (uint32);
    function becomeChainOwner() external;
    function setInkPrice(uint32 price) external;
}

#[tokio::test(flavor = "multi_thread")]
async fn arbitrum_snapshot_restores_owner_updated_stylus_parameters() {
    let (api, handle) = spawn(
        NodeConfig::test().with_networks(NetworkConfigs::with_arbitrum()).with_stylus_config(
            StylusConfig { ink_price: Some(11_000), debug_mode_stylus: true, ..Default::default() },
        ),
    )
    .await;
    let provider = handle.http_provider();
    let sender = provider.get_accounts().await.unwrap()[0];
    let receipt = provider
        .send_transaction(
            TransactionRequest::default()
                .from(sender)
                .with_to(address!("00000000000000000000000000000000000000ff"))
                .with_input(becomeChainOwnerCall {}.abi_encode())
                .with_gas_limit(1_000_000)
                .into(),
        )
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();
    assert!(receipt.status());

    let snapshot = api.evm_snapshot().await.unwrap();
    let receipt = provider
        .send_transaction(
            TransactionRequest::default()
                .from(sender)
                .with_to(address!("0000000000000000000000000000000000000070"))
                .with_input(setInkPriceCall { price: 23_000 }.abi_encode())
                .with_gas_limit(1_000_000)
                .into(),
        )
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();
    assert!(receipt.status());
    let query =
        TransactionRequest::default().with_to(ARB_WASM).with_input(inkPriceCall {}.abi_encode());
    let output = provider.call(query.clone().into()).await.unwrap();
    assert_eq!(u32::abi_decode(&output).unwrap(), 23_000);

    assert!(api.evm_revert(snapshot).await.unwrap());
    let output = provider.call(query.into()).await.unwrap();
    assert_eq!(u32::abi_decode(&output).unwrap(), 11_000);
}

#[tokio::test(flavor = "multi_thread")]
async fn arbitrum_fork_preserves_remote_state_and_applies_local_stylus_override() {
    let (_source_api, source) = spawn(
        NodeConfig::test()
            .with_chain_id(Some(421_614_u64))
            .with_networks(NetworkConfigs::with_arbitrum())
            .with_stylus_config(StylusConfig {
                arbos_version: Some(59),
                ink_price: Some(13_579),
                ..Default::default()
            }),
    )
    .await;
    let (_fork_api, fork) = spawn(
        NodeConfig::test()
            .with_eth_rpc_url(Some(source.http_endpoint()))
            .with_stylus_config(StylusConfig { ink_price: Some(24_680), ..Default::default() }),
    )
    .await;
    let query =
        TransactionRequest::default().with_to(ARB_WASM).with_input(inkPriceCall {}.abi_encode());
    let remote = source.http_provider().call(query.clone().into()).await.unwrap();
    let local = fork.http_provider().call(query.into()).await.unwrap();
    assert_eq!(u32::abi_decode(&remote).unwrap(), 13_579);
    assert_eq!(u32::abi_decode(&local).unwrap(), 24_680);
    assert_eq!(fork.http_provider().get_chain_id().await.unwrap(), 421_614);
}

const ARB_WASM: Address = address!("0000000000000000000000000000000000000071");

#[tokio::test(flavor = "multi_thread")]
async fn arbitrum_mode_initializes_arbos_and_routes_calls_to_arbos_revm() {
    let (_api, handle) =
        spawn(NodeConfig::test().with_networks(NetworkConfigs::with_arbitrum())).await;
    let provider = handle.http_provider();

    let request = TransactionRequest::default()
        .with_to(ARB_WASM)
        .with_input(stylusVersionCall {}.abi_encode());
    let output = provider.call(request.clone().into()).await.unwrap();
    assert_eq!(u16::abi_decode(&output).unwrap(), 3);

    let sender = provider.get_accounts().await.unwrap()[0];
    let receipt = provider
        .send_transaction(request.from(sender).with_gas_limit(1_000_000).into())
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();
    assert!(receipt.status());
    assert!(receipt.gas_used > 0);
}
