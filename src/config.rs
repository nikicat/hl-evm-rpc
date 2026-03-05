use std::env;
use std::net::SocketAddr;

#[derive(Clone, Debug)]
pub struct Config {
    pub listen_addr: SocketAddr,
    pub hl_api_url: String,
    pub chain_id: u64,
    pub log_level: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            listen_addr: env::var("LISTEN_ADDR")
                .unwrap_or_else(|_| "127.0.0.1:8545".into())
                .parse()
                .expect("invalid LISTEN_ADDR"),
            hl_api_url: env::var("HL_API_URL")
                .unwrap_or_else(|_| "https://api.hyperliquid.xyz/info".into()),
            chain_id: env::var("CHAIN_ID")
                .unwrap_or_else(|_| "18508".into())
                .parse()
                .expect("invalid CHAIN_ID"),
            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".into()),
        }
    }
}
