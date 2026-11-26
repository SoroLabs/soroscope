//! Tiered read-through cache for contract profiles (#1397).
//!
//! Reads walk L1 (bounded in-memory LRU, per instance) → L2 (shared
//! persistent store, normally Redis) → loader. A loaded value is written to
//! both tiers, and an L2 hit is promoted into L1, so horizontally scaled core
//! instances share one L2 while each keeps its own hot L1.
//!
//! Entries are keyed by [`ProfileKey`] — `(wasm_hash, protocol_version,
//! ledger_bucket)` — so a profile computed under one protocol version or
//! ledger window is never served for another.
//!
//! Concurrent misses for the same key are coalesced (singleflight): only one
//! caller runs the loader, the others wait for its result.

use super::CacheError;
use crate::metrics::AppMetrics;
use async_trait::async_trait;
use moka::future::Cache;
use prometheus::IntCounter;
use redis::AsyncCommands;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Cache key for a contract profile.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProfileKey {
    pub wasm_hash: String,
    pub protocol_version: u32,
    pub ledger_bucket: u64,
}

impl ProfileKey {
    pub fn new(wasm_hash: impl Into<String>, protocol_version: u32, ledger_bucket: u64) -> Self {
        Self {
            wasm_hash: wasm_hash.into(),
            protocol_version,
            ledger_bucket,
        }
    }

    /// Key for `ledger_sequence`, bucketed into windows of `bucket_size` ledgers.
    pub fn for_ledger(
        wasm_hash: impl Into<String>,
        protocol_version: u32,
        ledger_sequence: u64,
        bucket_size: u64,
    ) -> Self {
        Self::new(
            wasm_hash,
            protocol_version,
            ledger_sequence / bucket_size.max(1),
        )
    }

    /// String form used in both tiers.
    pub fn cache_key(&self) -> String {
        format!(
            "soroscope:profile:{}:{}:{}",
            self.wasm_hash, self.protocol_version, self.ledger_bucket
        )
    }
}

/// Persistent L2 backend shared by every core instance.
#[async_trait]
pub trait ProfileStore: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError>;
    async fn set(&self, key: &str, value: Vec<u8>, ttl: Duration) -> Result<(), CacheError>;
    async fn delete(&self, key: &str) -> Result<(), CacheError>;
}

/// Redis-backed L2.
pub struct RedisProfileStore {
    client: redis::Client,
}

impl RedisProfileStore {
    pub fn new(client: redis::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl ProfileStore for RedisProfileStore {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        Ok(conn.get::<_, Option<Vec<u8>>>(key).await?)
    }

    async fn set(&self, key: &str, value: Vec<u8>, ttl: Duration) -> Result<(), CacheError> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        conn.set_ex::<_, _, ()>(key, value, ttl.as_secs().max(1))
            .await?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), CacheError> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        conn.del::<_, ()>(key).await?;
        Ok(())
    }
}

/// Process-local L2, for single-instance deployments and tests. Ignores TTL.
#[derive(Default)]
pub struct InMemoryProfileStore {
    entries: RwLock<HashMap<String, Vec<u8>>>,
}

impl InMemoryProfileStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ProfileStore for InMemoryProfileStore {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        Ok(self.entries.read().await.get(key).cloned())
    }

    async fn set(&self, key: &str, value: Vec<u8>, _ttl: Duration) -> Result<(), CacheError> {
        self.entries.write().await.insert(key.to_string(), value);
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), CacheError> {
        self.entries.write().await.remove(key);
        Ok(())
    }
}

/// L1 + L2 read-through cache with singleflight loading.
pub struct TieredProfileCache<V> {
    l1: Cache<String, V>,
    l2: Arc<dyn ProfileStore>,
    ttl: Duration,
    l1_hits: IntCounter,
    l2_hits: IntCounter,
    misses: IntCounter,
}

impl<V> TieredProfileCache<V>
where
    V: Clone + Serialize + DeserializeOwned + Send + Sync + 'static,
{
    /// `l1_capacity` bounds the in-memory tier; `ttl` applies to both tiers.
    ///
    /// Hit/miss counters are local until [`Self::with_metrics`] binds them to
    /// the exported Prometheus registry.
    pub fn new(l2: Arc<dyn ProfileStore>, l1_capacity: u64, ttl: Duration) -> Self {
        let l1 = Cache::builder()
            .max_capacity(l1_capacity)
            .time_to_live(ttl)
            .build();
        Self {
            l1,
            l2,
            ttl,
            l1_hits: local_counter("cache_l1_hits_total"),
            l2_hits: local_counter("cache_l2_hits_total"),
            misses: local_counter("cache_misses_total"),
        }
    }

    /// Report hits and misses through the exported `cache_*_total` counters.
    pub fn with_metrics(mut self, metrics: &AppMetrics) -> Self {
        self.l1_hits = metrics.cache_l1_hits_total.clone();
        self.l2_hits = metrics.cache_l2_hits_total.clone();
        self.misses = metrics.cache_misses_total.clone();
        self
    }

    /// Return the profile for `key`, running `load` only when neither tier has
    /// it. Concurrent callers missing on the same key share a single `load`.
    ///
    /// A failed load is not cached; every waiter receives the same error.
    pub async fn get_or_load<F, Fut, E>(&self, key: &ProfileKey, load: F) -> Result<V, Arc<E>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<V, E>>,
        E: Send + Sync + 'static,
    {
        let cache_key = key.cache_key();
        if let Some(value) = self.l1.get(&cache_key).await {
            self.l1_hits.inc();
            return Ok(value);
        }

        self.l1
            .try_get_with(cache_key.clone(), async {
                if let Some(value) = self.read_l2(&cache_key).await {
                    self.l2_hits.inc();
                    return Ok(value);
                }
                self.misses.inc();
                let value = load().await?;
                self.write_l2(&cache_key, &value).await;
                Ok(value)
            })
            .await
    }

    /// Drop `key` from this instance's L1 and from the shared L2. Other
    /// instances stop serving their L1 copy once its TTL expires.
    pub async fn invalidate(&self, key: &ProfileKey) {
        let cache_key = key.cache_key();
        self.l1.invalidate(&cache_key).await;
        if let Err(e) = self.l2.delete(&cache_key).await {
            tracing::warn!(cache.key = %cache_key, error = %e, "L2 profile cache delete failed");
        }
    }

    pub fn l1_hit_count(&self) -> u64 {
        self.l1_hits.get()
    }

    pub fn l2_hit_count(&self) -> u64 {
        self.l2_hits.get()
    }

    pub fn miss_count(&self) -> u64 {
        self.misses.get()
    }

    /// L2 errors and undecodable payloads are treated as misses.
    async fn read_l2(&self, cache_key: &str) -> Option<V> {
        match self.l2.get(cache_key).await {
            Ok(Some(bytes)) => match serde_json::from_slice(&bytes) {
                Ok(value) => Some(value),
                Err(e) => {
                    tracing::warn!(cache.key = %cache_key, error = %e, "Undecodable L2 profile entry");
                    None
                }
            },
            Ok(None) => None,
            Err(e) => {
                tracing::warn!(cache.key = %cache_key, error = %e, "L2 profile cache read failed");
                None
            }
        }
    }

    async fn write_l2(&self, cache_key: &str, value: &V) {
        let bytes = match serde_json::to_vec(value) {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::warn!(cache.key = %cache_key, error = %e, "Profile not serialisable for L2");
                return;
            }
        };
        if let Err(e) = self.l2.set(cache_key, bytes, self.ttl).await {
            tracing::warn!(cache.key = %cache_key, error = %e, "L2 profile cache write failed");
        }
    }
}

/// Counter that is not registered anywhere, used until metrics are bound.
fn local_counter(name: &str) -> IntCounter {
    IntCounter::new(name, "tiered profile cache counter").expect("valid counter name")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn cache(store: Arc<dyn ProfileStore>) -> TieredProfileCache<u64> {
        TieredProfileCache::new(store, 100, Duration::from_secs(60))
    }

    #[test]
    fn cache_key_separates_protocol_versions_and_ledger_buckets() {
        let base = ProfileKey::new("abc", 22, 7);
        assert_ne!(base.cache_key(), ProfileKey::new("abc", 21, 7).cache_key());
        assert_ne!(base.cache_key(), ProfileKey::new("abc", 22, 8).cache_key());
        assert_eq!(
            ProfileKey::for_ledger("abc", 22, 1_234, 100),
            ProfileKey::new("abc", 22, 12)
        );
    }

    #[tokio::test]
    async fn cache_miss_loads_once_then_hits_l1() {
        let cache = cache(Arc::new(InMemoryProfileStore::new()));
        let key = ProfileKey::new("abc", 22, 0);
        let loads = AtomicUsize::new(0);

        for _ in 0..3 {
            let value = cache
                .get_or_load(&key, || async {
                    loads.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, String>(42)
                })
                .await
                .unwrap();
            assert_eq!(value, 42);
        }

        assert_eq!(loads.load(Ordering::SeqCst), 1);
        assert_eq!(cache.miss_count(), 1);
        assert_eq!(cache.l1_hit_count(), 2);
        assert_eq!(cache.l2_hit_count(), 0);
    }

    #[tokio::test]
    async fn cache_l2_hit_is_served_without_loading() {
        let store = Arc::new(InMemoryProfileStore::new());
        let key = ProfileKey::new("abc", 22, 0);
        store
            .set(
                &key.cache_key(),
                serde_json::to_vec(&7u64).unwrap(),
                Duration::from_secs(60),
            )
            .await
            .unwrap();
        // A fresh instance has an empty L1 but shares the L2.
        let cache = cache(store);

        let value = cache
            .get_or_load(&key, || async {
                Err::<u64, _>("loader must not run".to_string())
            })
            .await
            .unwrap();

        assert_eq!(value, 7);
        assert_eq!(cache.l2_hit_count(), 1);
        assert_eq!(cache.miss_count(), 0);
    }

    #[tokio::test]
    async fn cache_coalesces_concurrent_identical_requests() {
        let cache = Arc::new(cache(Arc::new(InMemoryProfileStore::new())));
        let loads = Arc::new(AtomicUsize::new(0));
        let key = ProfileKey::new("hot-contract", 22, 0);

        let handles: Vec<_> = (0..16)
            .map(|_| {
                let cache = Arc::clone(&cache);
                let loads = Arc::clone(&loads);
                let key = key.clone();
                tokio::spawn(async move {
                    cache
                        .get_or_load(&key, || async move {
                            loads.fetch_add(1, Ordering::SeqCst);
                            tokio::time::sleep(Duration::from_millis(50)).await;
                            Ok::<_, String>(99)
                        })
                        .await
                        .unwrap()
                })
            })
            .collect();
        for handle in handles {
            assert_eq!(handle.await.unwrap(), 99);
        }

        assert_eq!(loads.load(Ordering::SeqCst), 1);
        assert_eq!(cache.miss_count(), 1);
    }

    #[tokio::test]
    async fn cache_does_not_store_failed_loads() {
        let cache = cache(Arc::new(InMemoryProfileStore::new()));
        let key = ProfileKey::new("abc", 22, 0);

        let first = cache
            .get_or_load(&key, || async { Err::<u64, _>("rpc down".to_string()) })
            .await;
        assert_eq!(first.unwrap_err().as_str(), "rpc down");

        let second = cache
            .get_or_load(&key, || async { Ok::<_, String>(5) })
            .await
            .unwrap();
        assert_eq!(second, 5);
        assert_eq!(cache.miss_count(), 2);
    }

    #[tokio::test]
    async fn cache_invalidate_clears_both_tiers() {
        let store = Arc::new(InMemoryProfileStore::new());
        let cache = cache(store.clone());
        let key = ProfileKey::new("abc", 22, 0);
        cache
            .get_or_load(&key, || async { Ok::<_, String>(1) })
            .await
            .unwrap();

        cache.invalidate(&key).await;

        assert!(store.get(&key.cache_key()).await.unwrap().is_none());
        let reloaded = cache
            .get_or_load(&key, || async { Ok::<_, String>(2) })
            .await
            .unwrap();
        assert_eq!(reloaded, 2);
    }

    #[tokio::test]
    async fn cache_counters_are_exported_through_app_metrics() {
        let metrics = AppMetrics::new().unwrap();
        let cache = cache(Arc::new(InMemoryProfileStore::new())).with_metrics(&metrics);
        let key = ProfileKey::new("abc", 22, 0);

        for _ in 0..2 {
            cache
                .get_or_load(&key, || async { Ok::<_, String>(1) })
                .await
                .unwrap();
        }

        assert_eq!(metrics.cache_misses_total.get(), 1);
        assert_eq!(metrics.cache_l1_hits_total.get(), 1);
        assert_eq!(metrics.cache_l2_hits_total.get(), 0);
    }
}
