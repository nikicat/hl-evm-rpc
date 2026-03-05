use std::sync::Arc;
use std::time::Duration;

use moka::future::Cache;

use super::HlClient;
use super::types::{ClearinghouseState, SpotClearinghouseState, SpotMeta};

#[derive(Clone)]
pub struct CachedHlClient {
    inner: HlClient,
    spot_meta_cache: Cache<(), Arc<SpotMeta>>,
    clearinghouse_cache: Cache<String, Arc<ClearinghouseState>>,
    spot_clearinghouse_cache: Cache<String, Arc<SpotClearinghouseState>>,
}

impl CachedHlClient {
    pub fn new(inner: HlClient) -> Self {
        Self {
            inner,
            spot_meta_cache: Cache::builder()
                .time_to_live(Duration::from_secs(300))
                .max_capacity(1)
                .build(),
            clearinghouse_cache: Cache::builder()
                .time_to_live(Duration::from_secs(10))
                .max_capacity(1000)
                .build(),
            spot_clearinghouse_cache: Cache::builder()
                .time_to_live(Duration::from_secs(10))
                .max_capacity(1000)
                .build(),
        }
    }

    pub async fn get_spot_meta(&self) -> Result<Arc<SpotMeta>, String> {
        if let Some(cached) = self.spot_meta_cache.get(&()) .await {
            return Ok(cached);
        }
        let meta = self.inner.get_spot_meta().await?;
        let arc = Arc::new(meta);
        self.spot_meta_cache.insert((), arc.clone()).await;
        Ok(arc)
    }

    pub async fn get_clearinghouse_state(
        &self,
        user: &str,
    ) -> Result<Arc<ClearinghouseState>, String> {
        let key = user.to_lowercase();
        if let Some(cached) = self.clearinghouse_cache.get(&key).await {
            return Ok(cached);
        }
        let state = self.inner.get_clearinghouse_state(user).await?;
        let arc = Arc::new(state);
        self.clearinghouse_cache.insert(key, arc.clone()).await;
        Ok(arc)
    }

    pub async fn get_spot_clearinghouse_state(
        &self,
        user: &str,
    ) -> Result<Arc<SpotClearinghouseState>, String> {
        let key = user.to_lowercase();
        if let Some(cached) = self.spot_clearinghouse_cache.get(&key).await {
            return Ok(cached);
        }
        let state = self.inner.get_spot_clearinghouse_state(user).await?;
        let arc = Arc::new(state);
        self.spot_clearinghouse_cache
            .insert(key, arc.clone())
            .await;
        Ok(arc)
    }
}
