pub mod cache;
pub mod types;

use reqwest::Client;
use serde_json::json;

use types::{ClearinghouseState, SpotClearinghouseState, SpotMeta};

#[derive(Clone)]
pub struct HlClient {
    client: Client,
    api_url: String,
}

impl HlClient {
    pub fn new(api_url: String) -> Self {
        Self {
            client: Client::new(),
            api_url,
        }
    }

    pub async fn get_clearinghouse_state(
        &self,
        user: &str,
    ) -> Result<ClearinghouseState, String> {
        let body = json!({
            "type": "clearinghouseState",
            "user": user,
        });
        let resp = self
            .client
            .post(&self.api_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        resp.json().await.map_err(|e| e.to_string())
    }

    pub async fn get_spot_clearinghouse_state(
        &self,
        user: &str,
    ) -> Result<SpotClearinghouseState, String> {
        let body = json!({
            "type": "spotClearinghouseState",
            "user": user,
        });
        let resp = self
            .client
            .post(&self.api_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        resp.json().await.map_err(|e| e.to_string())
    }

    pub async fn get_spot_meta(&self) -> Result<SpotMeta, String> {
        let body = json!({
            "type": "spotMeta",
        });
        let resp = self
            .client
            .post(&self.api_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        resp.json().await.map_err(|e| e.to_string())
    }

}
