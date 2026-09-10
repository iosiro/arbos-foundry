use alloy_network::TransactionBuilder;
use alloy_primitives::{B256, U256, address, bytes};
use alloy_provider::Provider;
use alloy_rpc_types::{TransactionRequest, anvil::Forking};
use alloy_sol_types::{SolCall, SolValue, sol};
use anvil::{NodeConfig, spawn};

sol! {
    function arbBlockNumber() external view returns (uint256);
    function arbBlockHash(uint256 number) external view returns (bytes32);
}

#[tokio::test(flavor = "multi_thread")]
async fn arbitrum_fork_hash_domains_survive_reset_and_snapshot() {
    let mut node = NodeConfig::test().with_chain_id(Some(421_614_u64));
    node.stylus_config.arbos_version = Some(61);
    let (source_api, source) = spawn(node).await;
    let reader = address!("1234567890123456789012345678901234567890");
    // Return (BLOCKHASH(calldataload(0)), NUMBER); the source's L1 history is deliberately empty.
    source_api.anvil_set_code(reader, bytes!("600035406000524360205260406000f3")).await.unwrap();
    source_api.anvil_mine(Some(U256::from(3)), None).await.unwrap();
    let source_provider = source.http_provider();
    let block = source_provider.get_block_by_number(2.into()).await.unwrap().unwrap();
    let endpoint =
        foundry_test_utils::rpc::spawn_rpc_proxy_with_l1_block_number(source.http_endpoint(), 1)
            .await;
    let (api, fork) = spawn(NodeConfig::test().with_eth_rpc_url(Some(endpoint.clone()))).await;
    let provider = fork.http_provider();
    let arbsys = address!("0000000000000000000000000000000000000064");

    for (number, expected) in [(2, block.header.hash), (1, block.header.parent_hash)] {
        if number == 1 {
            api.anvil_reset(Some(Forking {
                json_rpc_url: Some(endpoint.clone()),
                block_number: Some(2),
            }))
            .await
            .unwrap();
        }
        let query = TransactionRequest::default()
            .with_to(arbsys)
            .with_input(arbBlockHashCall { number: U256::from(number) }.abi_encode());
        let snapshot = api.evm_snapshot().await.unwrap();
        for after_revert in [false, true] {
            if after_revert {
                api.anvil_mine(Some(U256::from(1)), None).await.unwrap();
                let historical = provider
                    .call(
                        TransactionRequest::default()
                            .with_to(arbsys)
                            .with_input(arbBlockNumberCall {}.abi_encode())
                            .into(),
                    )
                    .block((number + 1).into())
                    .await
                    .unwrap();
                assert_eq!(U256::abi_decode(&historical).unwrap(), U256::from(number + 1));
                assert!(api.evm_revert(snapshot).await.unwrap());
            }
            let result = provider.call(query.clone().into()).latest().await.unwrap();
            assert_eq!(B256::abi_decode(&result).unwrap(), expected);
            let current = provider
                .call(
                    TransactionRequest::default()
                        .with_to(arbsys)
                        .with_input(arbBlockNumberCall {}.abi_encode())
                        .into(),
                )
                .latest()
                .await
                .unwrap();
            let result = provider
                .call(
                    TransactionRequest::default()
                        .with_to(reader)
                        .with_input(U256::from(number).abi_encode())
                        .into(),
                )
                .latest()
                .await
                .unwrap();
            let (hash, l1_number) = <(B256, U256)>::abi_decode(&result).unwrap();
            assert_eq!(hash, B256::ZERO);
            assert_eq!(
                (U256::abi_decode(&current).unwrap(), l1_number),
                (U256::from(number + 1), U256::from(1)),
                "parent={number}, after_revert={after_revert}, RPC head={}",
                provider.get_block_number().await.unwrap()
            );
        }
    }
}
