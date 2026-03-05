use serde::Deserialize;

/// Response from `clearinghouseState` — perps account state.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClearinghouseState {
    pub margin_summary: MarginSummary,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MarginSummary {
    pub account_value: String,
}

/// Response from `spotClearinghouseState` — spot balances.
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SpotClearinghouseState {
    pub balances: Vec<SpotBalance>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SpotBalance {
    pub coin: String,
    pub total: String,
}

/// Response from `spotMeta` — token metadata.
#[derive(Debug, Deserialize, Clone)]
pub struct SpotMeta {
    pub tokens: Vec<SpotTokenInfo>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SpotTokenInfo {
    pub index: u32,
    pub name: String,
    pub full_name: Option<String>,
    pub wei_decimals: u32,
    pub token_id: Option<String>,
}
