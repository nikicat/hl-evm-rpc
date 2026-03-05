pub mod methods;
pub mod types;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde_json::Value;

use crate::hl::cache::CachedHlClient;
use types::{JsonRpcRequest, JsonRpcResponse};

#[derive(Clone)]
pub struct AppState {
    pub hl: CachedHlClient,
    pub chain_id: u64,
}

pub async fn handle_rpc(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    if let Some(arr) = body.as_array() {
        // Batch request
        let mut results = Vec::with_capacity(arr.len());
        for item in arr {
            let resp = dispatch_single(item, &state).await;
            results.push(serde_json::to_value(resp).unwrap());
        }
        (StatusCode::OK, Json(Value::Array(results)))
    } else {
        // Single request
        let resp = dispatch_single(&body, &state).await;
        (StatusCode::OK, Json(serde_json::to_value(resp).unwrap()))
    }
}

async fn dispatch_single(body: &Value, state: &AppState) -> JsonRpcResponse {
    let req: JsonRpcRequest = match serde_json::from_value(body.clone()) {
        Ok(r) => r,
        Err(e) => {
            return JsonRpcResponse::err(Value::Null, -32700, format!("parse error: {e}"));
        }
    };

    let id = req.id.clone();

    match methods::dispatch(&req.method, &req.params, state.chain_id, &state.hl).await {
        Ok(result) => JsonRpcResponse::ok(id, result),
        Err((code, msg)) => {
            eprintln!("[rpc] ERROR method={} code={code} msg={msg}", req.method);
            JsonRpcResponse::err(id, code, msg)
        }
    }
}
