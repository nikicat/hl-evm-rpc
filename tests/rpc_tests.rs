use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};
use tokio::net::TcpListener;

// ── Mock HL API ──────────────────────────────────────────────────────

/// Spin up a mock HL Info API on a random port. Returns its base URL.
async fn start_mock_hl() -> String {
    let app = Router::new().route("/info", post(mock_hl_handler));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}/info")
}

async fn mock_hl_handler(Json(body): Json<Value>) -> Json<Value> {
    let req_type = body["type"].as_str().unwrap_or("");
    match req_type {
        "clearinghouseState" => Json(json!({
            "marginSummary": {
                "accountValue": "1234.56"
            },
            "assetPositions": []
        })),
        "spotClearinghouseState" => Json(json!({
            "balances": [
                {"coin": "USDC", "total": "500.12345678"},
                {"coin": "PURR", "total": "1000.0"}
            ]
        })),
        "spotMeta" => Json(json!({
            "tokens": [
                {
                    "index": 0,
                    "name": "USDC",
                    "fullName": "USD Coin",
                    "weiDecimals": 8
                },
                {
                    "index": 1,
                    "name": "PURR",
                    "fullName": "Purrfect Token",
                    "weiDecimals": 18
                }
            ]
        })),
        _ => Json(json!({"error": "unknown type"})),
    }
}

// ── Test RPC server ──────────────────────────────────────────────────

/// Start the actual RPC proxy pointed at our mock HL API. Returns the RPC base URL.
async fn start_rpc_server(hl_api_url: &str) -> String {
    use hl_evm_rpc::hl::HlClient;
    use hl_evm_rpc::hl::cache::CachedHlClient;
    use hl_evm_rpc::rpc::AppState;

    let hl = CachedHlClient::new(HlClient::new(hl_api_url.to_string()));
    let state = AppState {
        hl,
        chain_id: 18508,
    };

    let app = hl_evm_rpc::build_router(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

/// Send a single JSON-RPC request and return the parsed response.
async fn rpc_call(url: &str, method: &str, params: Value) -> Value {
    let client = reqwest::Client::new();
    let body = json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": 1
    });
    client
        .post(url)
        .json(&body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

/// Send a batch JSON-RPC request.
async fn rpc_batch(url: &str, requests: Vec<Value>) -> Vec<Value> {
    let client = reqwest::Client::new();
    let resp: Value = client
        .post(url)
        .json(&requests)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    resp.as_array().unwrap().clone()
}

// ── Fixture ──────────────────────────────────────────────────────────

async fn setup() -> String {
    let hl_url = start_mock_hl().await;
    start_rpc_server(&hl_url).await
}

// Must have non-zero bytes in first 16 bytes so it's NOT a synthetic token address.
const TEST_ADDR: &str = "0xabcd000000000000000000000000000000000001";

// ── Tests ────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_eth_chain_id() {
    let url = setup().await;
    let resp = rpc_call(&url, "eth_chainId", json!([])).await;
    assert_eq!(resp["result"], "0x484c");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_net_version() {
    let url = setup().await;
    let resp = rpc_call(&url, "net_version", json!([])).await;
    assert_eq!(resp["result"], "18508");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_eth_block_number() {
    let url = setup().await;
    let resp = rpc_call(&url, "eth_blockNumber", json!([])).await;
    let hex_str = resp["result"].as_str().unwrap();
    assert!(hex_str.starts_with("0x"));
    let block = u64::from_str_radix(&hex_str[2..], 16).unwrap();
    // Should be approximately current unix timestamp (within 5 seconds)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!((block as i64 - now as i64).unsigned_abs() < 5);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_eth_get_balance() {
    let url = setup().await;
    let resp = rpc_call(&url, "eth_getBalance", json!([TEST_ADDR, "latest"])).await;
    let hex_str = resp["result"].as_str().unwrap();
    // 1234.56 * 10^18 = 1234560000000000000000
    // In hex: 0x42E1387D7B3A400000... let's just verify it decodes to the right value
    let bytes = hex::decode(&hex_str[2..]).unwrap();
    assert_eq!(bytes.len(), 32);
    let val = u128::from_be_bytes(bytes[16..].try_into().unwrap());
    assert_eq!(val, 1234560000000000000000u128);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_eth_gas_price() {
    let url = setup().await;
    let resp = rpc_call(&url, "eth_gasPrice", json!([])).await;
    assert_eq!(resp["result"], "0x0");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_eth_max_priority_fee_per_gas() {
    let url = setup().await;
    let resp = rpc_call(&url, "eth_maxPriorityFeePerGas", json!([])).await;
    assert_eq!(resp["result"], "0x0");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_eth_get_transaction_count() {
    let url = setup().await;
    let resp = rpc_call(&url, "eth_getTransactionCount", json!([])).await;
    assert_eq!(resp["result"], "0x0");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_eth_estimate_gas() {
    let url = setup().await;
    let resp = rpc_call(&url, "eth_estimateGas", json!([])).await;
    assert_eq!(resp["result"], "0x0");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_eth_get_logs() {
    let url = setup().await;
    let resp = rpc_call(&url, "eth_getLogs", json!([{}])).await;
    assert_eq!(resp["result"], json!([]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_eth_syncing() {
    let url = setup().await;
    let resp = rpc_call(&url, "eth_syncing", json!([])).await;
    assert_eq!(resp["result"], false);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_web3_client_version() {
    let url = setup().await;
    let resp = rpc_call(&url, "web3_clientVersion", json!([])).await;
    assert_eq!(resp["result"], "hl-evm-rpc/0.1.0");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_eth_send_raw_transaction_error() {
    let url = setup().await;
    let resp = rpc_call(&url, "eth_sendRawTransaction", json!(["0xdeadbeef"])).await;
    assert!(resp["error"].is_object());
    assert_eq!(resp["error"]["code"], -32000);
    assert!(resp["error"]["message"]
        .as_str()
        .unwrap()
        .contains("read-only"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_unknown_method() {
    let url = setup().await;
    let resp = rpc_call(&url, "eth_nonExistentMethod", json!([])).await;
    assert_eq!(resp["error"]["code"], -32601);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_eth_get_code_synthetic_address() {
    let url = setup().await;
    // Token index 0 → address 0x...00000100 (offset 0x100)
    let addr = "0x0000000000000000000000000000000000000100";
    let resp = rpc_call(&url, "eth_getCode", json!([addr, "latest"])).await;
    assert_eq!(resp["result"], "0x01");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_eth_get_code_non_synthetic_address() {
    let url = setup().await;
    let resp = rpc_call(&url, "eth_getCode", json!([TEST_ADDR, "latest"])).await;
    assert_eq!(resp["result"], "0x");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_eth_get_block_by_number() {
    let url = setup().await;
    let resp = rpc_call(&url, "eth_getBlockByNumber", json!(["latest", false])).await;
    let block = &resp["result"];
    assert!(block["number"].as_str().unwrap().starts_with("0x"));
    assert!(block["timestamp"].as_str().unwrap().starts_with("0x"));
    assert_eq!(block["gasUsed"], "0x0");
    assert_eq!(block["transactions"], json!([]));
}

// ── eth_call ERC-20 tests ────────────────────────────────────────────

/// Build an eth_call params array targeting a synthetic token address with given calldata.
fn eth_call_params(token_index: u32, calldata: &str) -> Value {
    let val = token_index + 0x100;
    let addr = format!("0x{:0>40x}", val);
    json!([{"to": addr, "data": calldata}, "latest"])
}

/// Build balanceOf(address) calldata.
fn balance_of_calldata(owner: &str) -> String {
    let owner_hex = owner.strip_prefix("0x").unwrap_or(owner);
    format!("0x70a08231{:0>64}", owner_hex)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_eth_call_balance_of_usdc() {
    let url = setup().await;
    let calldata = balance_of_calldata(TEST_ADDR);
    let resp = rpc_call(&url, "eth_call", eth_call_params(0, &calldata)).await;
    let hex_str = resp["result"].as_str().unwrap();
    let bytes = hex::decode(&hex_str[2..]).unwrap();
    let val = u128::from_be_bytes(bytes[16..].try_into().unwrap());
    // 500.12345678 * 10^8 = 50012345678
    assert_eq!(val, 50012345678u128);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_eth_call_balance_of_purr() {
    let url = setup().await;
    let calldata = balance_of_calldata(TEST_ADDR);
    let resp = rpc_call(&url, "eth_call", eth_call_params(1, &calldata)).await;
    let hex_str = resp["result"].as_str().unwrap();
    let bytes = hex::decode(&hex_str[2..]).unwrap();
    let val = u128::from_be_bytes(bytes[16..].try_into().unwrap());
    // 1000.0 * 10^18
    assert_eq!(val, 1000_000000000000000000u128);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_eth_call_symbol() {
    let url = setup().await;
    // symbol() selector = 0x95d89b41
    let resp = rpc_call(&url, "eth_call", eth_call_params(0, "0x95d89b41")).await;
    let hex_str = resp["result"].as_str().unwrap();
    let bytes = hex::decode(&hex_str[2..]).unwrap();
    // ABI string: offset(32) + length(32) + data
    let len = u64::from_be_bytes(bytes[56..64].try_into().unwrap()) as usize;
    let symbol = std::str::from_utf8(&bytes[64..64 + len]).unwrap();
    assert_eq!(symbol, "USDC");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_eth_call_name() {
    let url = setup().await;
    // name() selector = 0x06fdde03
    let resp = rpc_call(&url, "eth_call", eth_call_params(0, "0x06fdde03")).await;
    let hex_str = resp["result"].as_str().unwrap();
    let bytes = hex::decode(&hex_str[2..]).unwrap();
    let len = u64::from_be_bytes(bytes[56..64].try_into().unwrap()) as usize;
    let name = std::str::from_utf8(&bytes[64..64 + len]).unwrap();
    assert_eq!(name, "USD Coin");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_eth_call_decimals() {
    let url = setup().await;
    // decimals() selector = 0x313ce567
    let resp = rpc_call(&url, "eth_call", eth_call_params(0, "0x313ce567")).await;
    let hex_str = resp["result"].as_str().unwrap();
    let bytes = hex::decode(&hex_str[2..]).unwrap();
    let decimals = bytes[31];
    assert_eq!(decimals, 8); // USDC weiDecimals = 8
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_eth_call_decimals_purr() {
    let url = setup().await;
    let resp = rpc_call(&url, "eth_call", eth_call_params(1, "0x313ce567")).await;
    let hex_str = resp["result"].as_str().unwrap();
    let bytes = hex::decode(&hex_str[2..]).unwrap();
    let decimals = bytes[31];
    assert_eq!(decimals, 18); // PURR weiDecimals = 18
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_eth_call_total_supply() {
    let url = setup().await;
    // totalSupply() selector = 0x18160ddd
    let resp = rpc_call(&url, "eth_call", eth_call_params(0, "0x18160ddd")).await;
    let hex_str = resp["result"].as_str().unwrap();
    let bytes = hex::decode(&hex_str[2..]).unwrap();
    assert_eq!(bytes.len(), 32);
    // Should be 10^28
    let expected =
        hex::decode("0000000000000000000000000000000000000000204fce5e3e25026110000000")
            .unwrap();
    assert_eq!(bytes, expected);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_eth_call_non_synthetic_address() {
    let url = setup().await;
    // Call to a non-synthetic address should return 0x
    let calldata = balance_of_calldata(TEST_ADDR);
    let resp = rpc_call(
        &url,
        "eth_call",
        json!([{"to": TEST_ADDR, "data": calldata}, "latest"]),
    )
    .await;
    assert_eq!(resp["result"], "0x");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_eth_call_unknown_selector() {
    let url = setup().await;
    // Unknown selector on a synthetic address → 0x
    let resp = rpc_call(&url, "eth_call", eth_call_params(0, "0xdeadbeef")).await;
    assert_eq!(resp["result"], "0x");
}

// ── Batch request test ───────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_batch_request() {
    let url = setup().await;
    let requests = vec![
        json!({"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}),
        json!({"jsonrpc":"2.0","method":"net_version","params":[],"id":2}),
        json!({"jsonrpc":"2.0","method":"web3_clientVersion","params":[],"id":3}),
    ];
    let results = rpc_batch(&url, requests).await;
    assert_eq!(results.len(), 3);
    assert_eq!(results[0]["result"], "0x484c");
    assert_eq!(results[1]["result"], "18508");
    assert_eq!(results[2]["result"], "hl-evm-rpc/0.1.0");
}

// ── Helper endpoint tests ────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_health_endpoint() {
    let url = setup().await;
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{url}/health"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_tokens_endpoint() {
    let url = setup().await;
    let client = reqwest::Client::new();
    let resp: Value = client
        .get(format!("{url}/tokens"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let tokens = resp.as_array().unwrap();
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0]["symbol"], "USDC");
    assert_eq!(tokens[0]["decimals"], 8);
    assert_eq!(
        tokens[0]["address"],
        "0x0000000000000000000000000000000000000100"
    );
    assert_eq!(tokens[1]["symbol"], "PURR");
    assert_eq!(tokens[1]["decimals"], 18);
}

// ── Edge cases ───────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_malformed_request() {
    let url = setup().await;
    let client = reqwest::Client::new();
    let resp: Value = client
        .post(&url)
        .json(&json!({"garbage": true}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(resp["error"].is_object());
    assert_eq!(resp["error"]["code"], -32700);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_get_balance_invalid_address() {
    let url = setup().await;
    let resp = rpc_call(&url, "eth_getBalance", json!(["0xinvalid", "latest"])).await;
    assert!(resp["error"].is_object());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_eth_call_missing_to() {
    let url = setup().await;
    // No 'to' field = contract creation simulation → return 0x
    let resp = rpc_call(&url, "eth_call", json!([{"data": "0x70a08231"}, "latest"])).await;
    assert_eq!(resp["result"], "0x");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_eth_call_short_data() {
    let url = setup().await;
    // Only 2 bytes of data, less than 4-byte selector — revm executes, inspector returns empty
    let resp = rpc_call(&url, "eth_call", eth_call_params(0, "0xab")).await;
    assert_eq!(resp["result"], "0x");
}
