use alloy_evm::precompiles::{DynPrecompile, Precompile, PrecompileInput, PrecompilesMap};
use alloy_network::{ReceiptResponse, TransactionBuilder};
use alloy_primitives::{Address, Bytes, U256, address, bytes};
use alloy_provider::Provider;
use alloy_rpc_types::{Authorization, TransactionRequest, anvil::Forking};
use alloy_signer::SignerSync;
use alloy_sol_types::{SolCall, SolValue, sol};
use anvil::{NodeConfig, PrecompileFactory, eth::error::BlockchainError, spawn};
use foundry_config::stylus::StylusConfig;
use foundry_evm_networks::NetworkConfigs;
use revm::precompile::PrecompileOutput;
use serde_json::json;
use std::time::Duration;

sol! {
    function stylusVersion() external view returns (uint16);
    function inkPrice() external view returns (uint32);
    function becomeChainOwner() external;
    function setInkPrice(uint32 price) external;
    function arbBlockNumber() external view returns (uint256);
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
async fn arbitrum_memory_reset_reinitializes_local_stylus_parameters() {
    let (api, handle) = spawn(
        NodeConfig::test()
            .with_networks(NetworkConfigs::with_arbitrum())
            .with_stylus_config(StylusConfig { ink_price: Some(24_680), ..Default::default() }),
    )
    .await;
    let provider = handle.http_provider();
    let query =
        TransactionRequest::default().with_to(ARB_WASM).with_input(inkPriceCall {}.abi_encode());
    assert_eq!(
        u32::abi_decode(&provider.call(query.clone().into()).await.unwrap()).unwrap(),
        24_680
    );

    api.anvil_reset(None).await.unwrap();
    let output = provider.call(query.into()).await.unwrap();
    assert_eq!(u32::abi_decode(&output).unwrap(), 24_680);
}

#[tokio::test(flavor = "multi_thread")]
async fn arbitrum_fork_reset_reapplies_overrides_and_restores_local_state() {
    let (_source_api, source) = spawn(
        NodeConfig::test()
            .with_chain_id(Some(421_614_u64))
            .with_networks(NetworkConfigs::with_arbitrum())
            .with_stylus_config(StylusConfig {
                arbos_version: Some(40),
                ink_price: Some(13_579),
                ..Default::default()
            }),
    )
    .await;
    let endpoint = source.http_endpoint();
    let (api, handle) = spawn(
        NodeConfig::test()
            .with_eth_rpc_url(Some(endpoint.clone()))
            .with_stylus_config(StylusConfig { ink_price: Some(24_680), ..Default::default() }),
    )
    .await;
    let provider = handle.http_provider();
    let ink_query =
        TransactionRequest::default().with_to(ARB_WASM).with_input(inkPriceCall {}.abi_encode());
    let version_query = TransactionRequest::default()
        .with_to(ARB_WASM)
        .with_input(stylusVersionCall {}.abi_encode());

    api.anvil_reset(Some(Forking { json_rpc_url: Some(endpoint), block_number: None }))
        .await
        .unwrap();
    assert_eq!(
        u32::abi_decode(&provider.call(ink_query.clone().into()).await.unwrap()).unwrap(),
        24_680
    );
    assert_eq!(
        u16::abi_decode(&provider.call(version_query.clone().into()).await.unwrap()).unwrap(),
        2
    );
    let remote = source.http_provider().call(ink_query.clone().into()).await.unwrap();
    assert_eq!(u32::abi_decode(&remote).unwrap(), 13_579, "the source must remain unchanged");

    api.anvil_reset(None).await.unwrap();
    assert_eq!(u32::abi_decode(&provider.call(ink_query.into()).await.unwrap()).unwrap(), 24_680);
    assert_eq!(u16::abi_decode(&provider.call(version_query.into()).await.unwrap()).unwrap(), 3);
}

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

#[tokio::test(flavor = "multi_thread")]
async fn arbitrum_signature_impersonation_overrides_ecrecover() {
    let (api, handle) =
        spawn(NodeConfig::test().with_networks(NetworkConfigs::with_arbitrum())).await;
    let provider = handle.http_provider();
    let expected = address!("1234567890123456789012345678901234567890");
    api.anvil_impersonate_signature(Bytes::from(vec![0x11; 65]), expected).await.unwrap();

    let mut input = vec![0; 128];
    input[63..].fill(0x11);
    let output = provider
        .call(
            TransactionRequest::default()
                .with_to(address!("0000000000000000000000000000000000000001"))
                .with_input(input)
                .into(),
        )
        .await
        .unwrap();
    assert_eq!(output.as_ref(), expected.abi_encode());

    // Installing a tool override must not replace the remaining ArbOS provider.
    let output = provider
        .call(
            TransactionRequest::default()
                .with_to(ARB_WASM)
                .with_input(stylusVersionCall {}.abi_encode())
                .into(),
        )
        .await
        .unwrap();
    assert_eq!(u16::abi_decode(&output).unwrap(), 3);
}

const CUSTOM_PRECOMPILE: Address = address!("0000000000000000000000000000000000123456");
const LOOKUP_PRECOMPILE: Address = address!("0000000000000000000000000000000000234567");

#[derive(Debug)]
struct ArbitrumTestPrecompiles;

fn echo_precompile() -> DynPrecompile {
    DynPrecompile::from(|input: PrecompileInput<'_>| {
        Ok(PrecompileOutput::new(321, Bytes::copy_from_slice(input.data), input.reservoir))
    })
}

impl PrecompileFactory for ArbitrumTestPrecompiles {
    fn precompiles(&self) -> Vec<(Address, DynPrecompile)> {
        vec![(CUSTOM_PRECOMPILE, echo_precompile()), (ARB_WASM, echo_precompile())]
    }

    fn install(&self, precompiles: &mut PrecompilesMap) {
        precompiles.extend_precompiles(self.precompiles());
        precompiles
            .apply_precompile(&address!("0000000000000000000000000000000000000002"), |_| None);
        precompiles.map_precompile(
            &address!("0000000000000000000000000000000000000004"),
            |identity| {
                DynPrecompile::from(move |input: PrecompileInput<'_>| {
                    let mut output = identity.call(input)?;
                    let mut bytes = output.bytes.to_vec();
                    bytes.push(0xab);
                    output.bytes = bytes.into();
                    Ok(output)
                })
            },
        );
        precompiles.set_precompile_lookup(|address: &Address| {
            (*address == LOOKUP_PRECOMPILE).then(echo_precompile)
        });
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn arbitrum_installs_explicit_and_lookup_precompiles() {
    let (_api, handle) = spawn(
        NodeConfig::test()
            .with_networks(NetworkConfigs::with_arbitrum())
            .with_precompile_factory(ArbitrumTestPrecompiles),
    )
    .await;
    let provider = handle.http_provider();
    let input = Bytes::from_static(b"custom precompile");
    for address in [CUSTOM_PRECOMPILE, LOOKUP_PRECOMPILE, ARB_WASM] {
        let output = provider
            .call(TransactionRequest::default().with_to(address).with_input(input.clone()).into())
            .await
            .unwrap();
        assert_eq!(output, input, "override at {address}");
    }
    let output = provider
        .call(
            TransactionRequest::default()
                .with_to(address!("0000000000000000000000000000000000000004"))
                .with_input(input.clone())
                .into(),
        )
        .await
        .unwrap();
    let mut expected = input.to_vec();
    expected.push(0xab);
    assert_eq!(output.as_ref(), expected);
    let removed_output = provider
        .call(
            TransactionRequest::default()
                .with_to(address!("0000000000000000000000000000000000000002"))
                .with_input(input)
                .into(),
        )
        .await
        .unwrap();
    assert!(removed_output.is_empty(), "removed SHA256 precompile must not be restored");
}

#[tokio::test(flavor = "multi_thread")]
async fn arbitrum_simulation_rejects_precompile_relocation() {
    let (api, _handle) =
        spawn(NodeConfig::test().with_networks(NetworkConfigs::with_arbitrum())).await;
    let mut payload = json!({"blockStateCalls": [{"calls": [{
        "to": ARB_WASM,
        "input": Bytes::from(stylusVersionCall {}.abi_encode())
    }]}]});
    let result =
        api.simulate_v1(serde_json::from_value(payload.clone()).unwrap(), None).await.unwrap();
    assert!(result[0].calls[0].status);
    assert_eq!(u16::abi_decode(&result[0].calls[0].return_data).unwrap(), 3);

    payload["blockStateCalls"][0]["stateOverrides"] = json!({
        "0x0000000000000000000000000000000000000004": {
            "movePrecompileToAddress": CUSTOM_PRECOMPILE
        }
    });
    let error = api.simulate_v1(serde_json::from_value(payload).unwrap(), None).await.unwrap_err();
    let BlockchainError::RpcError(error) = error else {
        panic!("unexpected simulation error: {error}");
    };
    assert_eq!(error.message, "precompile moves are not supported on this network");
}

#[tokio::test(flavor = "multi_thread")]
async fn arbitrum_blob_simulation_does_not_use_ethereum_executor() {
    let (api, _handle) =
        spawn(NodeConfig::test().with_networks(NetworkConfigs::with_arbitrum())).await;
    let payload = json!({
        "validation": false,
        "blockStateCalls": [{"calls": [{
            "to": ARB_WASM,
            "input": Bytes::from(stylusVersionCall {}.abi_encode()),
            "type": "0x3",
            "maxFeePerBlobGas": "0x0",
            "blobVersionedHashes": [format!("0x01{}", "00".repeat(31))]
        }]}]
    });
    // Arbitrum must validate this using its own handler instead of running the invalid ArbOS
    // placeholder bytecode in the Ethereum blob-simulation handler.
    let error = api.simulate_v1(serde_json::from_value(payload).unwrap(), None).await.unwrap_err();
    assert!(error.to_string().to_lowercase().contains("blob"), "{error}");
}

#[tokio::test(flavor = "multi_thread")]
async fn arbitrum_estimated_gas_covers_transfer_and_precompile() {
    assert_arbitrum_estimates(None).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn arbitrum_estimated_gas_respects_base_fee_below_nitro_minimum() {
    assert_arbitrum_estimates(Some(50_000_000)).await;
}

async fn assert_arbitrum_estimates(base_fee: Option<u64>) {
    let (_api, handle) = spawn(
        NodeConfig::test()
            .with_chain_id(Some(42_161_u64))
            .with_base_fee(base_fee)
            .with_networks(NetworkConfigs::with_arbitrum()),
    )
    .await;
    let provider = handle.http_provider();
    let sender = provider.get_accounts().await.unwrap()[0];
    let requests = [
        TransactionRequest::default()
            .with_from(sender)
            .with_to(address!("0000000000000000000000000000000000123456"))
            .with_value(U256::from(1)),
        TransactionRequest::default()
            .with_from(sender)
            .with_to(address!("0000000000000000000000000000000000000064"))
            .with_input(arbBlockNumberCall {}.abi_encode()),
    ];
    for request in requests {
        let estimate = provider.estimate_gas(request.clone().into()).await.unwrap();
        assert!(estimate > 21_000, "the L1 poster allowance must be included");
        let receipt = tokio::time::timeout(Duration::from_secs(20), async {
            provider.send_transaction(request.into()).await.unwrap().get_receipt().await.unwrap()
        })
        .await
        .expect("an automatically estimated transaction must be mined, not dropped");
        assert!(receipt.status());
        assert!(receipt.gas_used > 21_000, "mining must still charge the actual poster fee");
        assert!(receipt.gas_used <= estimate, "estimate {estimate}, actual {}", receipt.gas_used);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn arbitrum_estimated_gas_covers_authorization_envelope() {
    let (api, handle) = spawn(
        NodeConfig::test()
            .with_chain_id(Some(42_161_u64))
            .with_networks(NetworkConfigs::with_arbitrum()),
    )
    .await;
    let provider = handle.http_provider();
    let wallets = handle.dev_wallets().collect::<Vec<_>>();
    let delegate = address!("0000000000000000000000000000000000123456");
    api.anvil_set_code(delegate, bytes!("600160005500")).await.unwrap();
    let authorizations = wallets
        .iter()
        .skip(1)
        .take(4)
        .map(|wallet| {
            let authorization =
                Authorization { chain_id: U256::from(42_161), address: delegate, nonce: 0 };
            let signature = wallet.sign_hash_sync(&authorization.signature_hash()).unwrap();
            authorization.into_signed(signature)
        })
        .collect();
    let request = TransactionRequest {
        authorization_list: Some(authorizations),
        transaction_type: Some(4),
        ..Default::default()
    }
    .with_from(wallets[0].address())
    .with_to(wallets[1].address());
    let estimate = provider.estimate_gas(request.clone().into()).await.unwrap();
    let receipt = tokio::time::timeout(Duration::from_secs(20), async {
        provider.send_transaction(request.into()).await.unwrap().get_receipt().await.unwrap()
    })
    .await
    .expect("authorization bytes must be included in the poster allowance");
    assert!(receipt.status());
    assert!(receipt.gas_used <= estimate, "estimate {estimate}, actual {}", receipt.gas_used);
    assert_eq!(
        provider.get_storage_at(wallets[1].address(), U256::ZERO).await.unwrap(),
        U256::from(1)
    );
}
