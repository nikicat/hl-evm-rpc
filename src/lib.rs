pub mod config;
pub mod evm;
pub mod hl;
pub mod rpc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;
use tower_http::cors::CorsLayer;

use evm::address;
use rpc::AppState;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", post(rpc::handle_rpc))
        .route("/health", get(health))
        .route("/tokens", get(tokens))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn health() -> StatusCode {
    StatusCode::OK
}

async fn tokens(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    match state.hl.get_spot_meta().await {
        Ok(meta) => {
            let tokens: Vec<_> = meta
                .tokens
                .iter()
                .map(|t| {
                    let addr = address::token_index_to_addr(t.index);
                    json!({
                        "address": address::addr_to_hex(&addr),
                        "symbol": t.name,
                        "name": t.full_name.as_deref().unwrap_or(&t.name),
                        "decimals": t.wei_decimals,
                        "index": t.index,
                    })
                })
                .collect();
            (StatusCode::OK, Json(json!(tokens)))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e})),
        ),
    }
}
