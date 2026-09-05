use alloy_network::{ReceiptResponse, TransactionBuilder};
use alloy_primitives::{Address, address};
use alloy_provider::Provider;
use alloy_rpc_types::TransactionRequest;
use alloy_sol_types::{SolCall, SolValue, sol};
use anvil::{NodeConfig, spawn};
use foundry_evm_networks::NetworkConfigs;

sol! {
    function stylusVersion() external view returns (uint16);
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
