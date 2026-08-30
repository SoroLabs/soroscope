pub mod disk;

pub use disk::{DiskCache, DiskCacheConfig};

use crate::simulation::SimulationResult;
use moka::future::Cache;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sled::{Db, Tree};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

const CACHE_TTL_SECS: u64 = 3_600;
const CACHE_MAX_CAPACITY: u64 = 1_000;

#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry<T> {
    data: T,
    ledger_sequence: u64,
    timestamp: u64,
}

pub struct SimulationCache {
    l1: Cache<String, SimulationResult>,
    l2: Tree,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl SimulationCache {
    pub fn new(db: &Db) -> Arc<Self> {
        let l1 = Cache::builder()
            .max_capacity(CACHE_MAX_CAPACITY)
            .time_to_live(Duration::from_secs(CACHE_TTL_SECS))
            .build();

        let l2 = db
            .open_tree("simulation_results")
            .expect("Failed to open simulation_results tree");

        Arc::new(Self {
            l1,
            l2,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        })
    }

    pub fn generate_key(contract_id: &str, function_name: &str, args: &[String]) -> String {
        let args_json = serde_json::to_string(args).unwrap_or_else(|_| "[]".to_string());
        let input = format!("{}{}{}", contract_id, function_name, args_json);
        let digest = Sha256::digest(input.as_bytes());
        hex::encode(digest)
    }

    pub async fn get(&self, key: &str) -> Option<SimulationResult> {
        if let Some(result) = self.l1.get(key).await {
            self.hits.fetch_add(1, Ordering::Relaxed);
            tracing::debug!(cache.key = %key, "Cache HIT (L1)");
            return Some(result);
        }

        if let Ok(Some(bytes)) = self.l2.get(key) {
            if let Ok(entry) = serde_json::from_slice::<CacheEntry<SimulationResult>>(&bytes) {
                self.l1.insert(key.to_string(), entry.data.clone()).await;
                self.hits.fetch_add(1, Ordering::Relaxed);
                tracing::debug!(cache.key = %key, "Cache HIT (L2)");
                return Some(entry.data);
            }
        }

        self.misses.fetch_add(1, Ordering::Relaxed);
        tracing::debug!(cache.key = %key, "Cache MISS");
        None
    }

    pub async fn set(&self, key: String, result: SimulationResult) {
        let entry = CacheEntry {
            ledger_sequence: result.latest_ledger,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            data: result.clone(),
        };

        if let Ok(bytes) = serde_json::to_vec(&entry) {
            let _ = self.l2.insert(&key, bytes);
        }
        self.l1.insert(key, result).await;
    }

    pub fn log_stats(&self) {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;
        let hit_rate_pct = hits
            .checked_mul(100)
            .and_then(|v| v.checked_div(total))
            .unwrap_or(0);
        tracing::info!(
            cache.hits = hits,
            cache.misses = misses,
            cache.total = total,
            cache.hit_rate_pct = hit_rate_pct,
            "Cache statistics"
        );
    }

    pub fn hit_count(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    pub fn miss_count(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }
}

pub struct ContractCache {
    wasm_tree: Tree,
    ledger_tree: Tree,
}

impl ContractCache {
    pub fn new(db: &Db) -> Self {
        let wasm_tree = db
            .open_tree("wasm_bytes")
            .expect("Failed to open wasm_bytes tree");
        let ledger_tree = db
            .open_tree("ledger_entries")
            .expect("Failed to open ledger_entries tree");
        Self {
            wasm_tree,
            ledger_tree,
        }
    }

    pub fn get_wasm(&self, hash_hex: &str) -> Option<Vec<u8>> {
        self.wasm_tree
            .get(hash_hex)
            .ok()
            .flatten()
            .map(|v| v.to_vec())
    }

    pub fn set_wasm(&self, hash_hex: String, wasm_bytes: Vec<u8>) {
        let _ = self.wasm_tree.insert(hash_hex, wasm_bytes);
    }

    pub fn get_ledger_entry(&self, key_64: &str, current_ledger: u64) -> Option<Vec<u8>> {
        if let Ok(Some(bytes)) = self.ledger_tree.get(key_64) {
            if let Ok(entry) = serde_json::from_slice::<CacheEntry<Vec<u8>>>(&bytes) {
                if entry.ledger_sequence >= current_ledger {
                    return Some(entry.data);
                }
            }
        }
        None
    }

    pub fn set_ledger_entry(&self, key_64: String, entry_bytes: Vec<u8>, ledger_sequence: u64) {
        let entry = CacheEntry {
            data: entry_bytes,
            ledger_sequence,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        if let Ok(bytes) = serde_json::to_vec(&entry) {
            let _ = self.ledger_tree.insert(key_64, bytes);
        }
    }
}

/// Errors surfaced by the cache subsystem.
///
/// These bubble up from Sled's disk store, JSON (de)serialisation, and
/// I/O when opening a backing directory. The main service converts them
/// into HTTP 500 via the `AppError` layer; callers inside the cache path
/// normally treat L2 errors as misses and log-and-continue rather than
/// failing the whole simulation.
#[derive(Error, Debug)]
pub enum CacheError {
    #[error("disk cache I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("disk cache backend error: {0}")]
    Backend(#[from] sled::Error),

    #[error("cache payload (de)serialisation error: {0}")]
    Serialization(#[from] serde_json::Error),
}
