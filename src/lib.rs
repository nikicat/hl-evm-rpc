pub mod config;
pub mod evm;
pub mod hl;
pub mod rpc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use tower_http::cors::CorsLayer;

use evm::address;
use rpc::AppState;

const BUILD_VERSION: &str = match option_env!("BUILD_VERSION") {
    Some(v) => v,
    None => "dev",
};

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(wallet_page).post(rpc::handle_rpc))
        .route("/send", get(send_page))
        .route("/version", get(version))
        .route("/health", get(health))
        .route("/tokens", get(tokens))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn wallet_page() -> Html<&'static str> {
    Html(include_str!("wallet.html"))
}

async fn send_page() -> Html<&'static str> {
    Html(include_str!("send.html"))
}

async fn version() -> &'static str {
    BUILD_VERSION
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
                        "tokenId": t.token_id,
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
