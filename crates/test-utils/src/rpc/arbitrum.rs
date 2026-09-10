use axum::{Json, Router, routing::post};
use serde_json::{Value, json};

/// Spawns an RPC proxy with an independently controlled Arbitrum L1 block number.
///
/// RPC block numbers, hashes, parent hashes, and state remain unchanged. Both single and batched
/// block responses are rewritten so fork discovery and ancestry use the same L1/L2 separation.
pub async fn spawn_rpc_proxy_with_l1_block_number(endpoint: String, l1_number: u64) -> String {
    let client = reqwest::Client::new();
    let router = Router::new().route(
        "/",
        post(move |Json(request): Json<Value>| {
            let client = client.clone();
            let endpoint = endpoint.clone();
            async move {
                let mut response = client
                    .post(endpoint)
                    .json(&request)
                    .send()
                    .await
                    .unwrap()
                    .json::<Value>()
                    .await
                    .unwrap();
                let responses = match &mut response {
                    Value::Array(items) => items.as_mut_slice(),
                    single => std::slice::from_mut(single),
                };
                for response in responses {
                    if let Some(block) = response.get_mut("result").and_then(Value::as_object_mut)
                        && block.contains_key("number")
                        && block.contains_key("transactions")
                    {
                        block.insert("l1BlockNumber".into(), json!(format!("{l1_number:#x}")));
                    }
                }
                Json(response)
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    format!("http://{address}")
}
