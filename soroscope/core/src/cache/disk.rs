//! Disk-backed L2 contract state cache using Sled.
//!
//! Each cached payload is wrapped in a [`DiskCacheEntry`] that records the
//! ledger sequence at which the entry was written. Reads reject entries
//! older than `max_ledger_age` ledgers behind the caller's view of the
//! current ledger and lazily evict them; a background sweep
//! ([`DiskCache::evict_stale`]) removes stale entries in bulk when a new
//! ledger lands.
//!
//! The store is keyed by arbitrary byte slices so callers can decide the
//! key shape — today `SimulationCache` uses the hex-SHA256 of
//! `(contract_id, function_name, args)`.

use super::CacheError;
use serde::{Deserialize, Serialize};
use sled::Db;
use std::path::{Path, PathBuf};

/// Tunable parameters for [`DiskCache`].
///
/// Exposed as a struct (rather than just loose args) so the builder call
/// site in `main.rs` reads cleanly when more knobs are added — size caps,
/// compaction cadence, etc. are planned follow-ups.
#[derive(Debug, Clone)]
pub struct DiskCacheConfig {
    /// Directory Sled will create / open. The directory is created if
    /// missing.
    pub path: PathBuf,
    /// Maximum number of ledgers an entry may trail the current ledger
    /// before it is treated as stale.
    pub max_ledger_age: u32,
}

impl DiskCacheConfig {
    pub fn new(path: impl Into<PathBuf>, max_ledger_age: u32) -> Self {
        Self {
            path: path.into(),
            max_ledger_age,
        }
    }
}

/// Serialised wrapper persisted to disk. Stored alongside the payload so
/// staleness checks don't need an out-of-band index.
#[derive(Debug, Serialize, Deserialize)]
struct DiskCacheEntry {
    written_at_ledger: u32,
    payload: Vec<u8>,
}

/// Sled-backed L2 cache.
///
/// Cheap to clone (`sled::Db` is `Arc`-backed internally), safe to share
/// across async tasks. All methods are synchronous because Sled's I/O is
/// fast on local disk; callers that care about runtime isolation are
/// expected to wrap the store in `tokio::task::spawn_blocking` themselves.
#[derive(Clone)]
pub struct DiskCache {
    db: Db,
    max_ledger_age: u32,
}

impl DiskCache {
    /// Open (or create) the Sled database at the given path.
    pub fn open(config: DiskCacheConfig) -> Result<Self, CacheError> {
        let db = sled::open(&config.path)?;
        Ok(Self {
            db,
            max_ledger_age: config.max_ledger_age,
        })
    }

    /// Convenience constructor for tests and ad-hoc call sites.
    pub fn open_at(path: &Path, max_ledger_age: u32) -> Result<Self, CacheError> {
        Self::open(DiskCacheConfig::new(path.to_path_buf(), max_ledger_age))
    }

    /// Look up `key` and return the raw payload if it is still within the
    /// ledger-age window. Stale entries are evicted as a side effect so
    /// the next caller doesn't re-read them.
    ///
    /// Errors are swallowed into `None` — an L2 failure must never break
    /// the request path; callers treat it as a miss and the fetch path
    /// still runs.
    pub fn get(&self, key: &[u8], current_ledger: u32) -> Option<Vec<u8>> {
        let raw = match self.db.get(key) {
            Ok(Some(ivec)) => ivec,
            Ok(None) => return None,
            Err(e) => {
                tracing::warn!(error = %e, "disk cache read failed, treating as miss");
                return None;
            }
        };

        let entry: DiskCacheEntry = match serde_json::from_slice(raw.as_ref()) {
            Ok(entry) => entry,
            Err(e) => {
                // Corrupt / schema-mismatched entry — drop it so the next
                // write can replace it.
                tracing::warn!(error = %e, "disk cache entry failed to deserialise, evicting");
                let _ = self.db.remove(key);
                return None;
            }
        };

        if self.is_stale(current_ledger, entry.written_at_ledger) {
            let _ = self.db.remove(key);
            return None;
        }

        Some(entry.payload)
    }

    /// Insert `payload` tagged with `current_ledger`. Returns an error on
    /// backend or serialisation failure — the caller decides whether to
    /// propagate it or log-and-continue.
    pub fn set(&self, key: &[u8], payload: Vec<u8>, current_ledger: u32) -> Result<(), CacheError> {
        let entry = DiskCacheEntry {
            written_at_ledger: current_ledger,
            payload,
        };
        let raw = serde_json::to_vec(&entry)?;
        self.db.insert(key, raw)?;
        Ok(())
    }

    /// Remove a specific key. Primarily exercised by tests; production
    /// eviction happens via ledger-advance and ledger-age checks.
    pub fn remove(&self, key: &[u8]) -> Result<(), CacheError> {
        self.db.remove(key)?;
        Ok(())
    }

    /// Sweep the store and remove every entry whose write ledger trails
    /// `current_ledger` by more than `max_ledger_age`. Called from the
    /// background ledger-watch loop on every new ledger.
    ///
    /// Returns the number of entries removed.
    pub fn evict_stale(&self, current_ledger: u32) -> u64 {
        let mut removed = 0u64;
        for kv in self.db.iter() {
            let (key, value) = match kv {
                Ok(pair) => pair,
                Err(e) => {
                    tracing::warn!(error = %e, "disk cache iter failed mid-sweep");
                    break;
                }
            };
            match serde_json::from_slice::<DiskCacheEntry>(value.as_ref()) {
                Ok(entry) => {
                    if self.is_stale(current_ledger, entry.written_at_ledger)
                        && self.db.remove(&key).is_ok()
                    {
                        removed += 1;
                    }
                }
                Err(_) => {
                    // Corrupt entry — remove it on sight.
                    if self.db.remove(&key).is_ok() {
                        removed += 1;
                    }
                }
            }
        }
        if removed > 0 {
            tracing::debug!(
                current_ledger,
                removed,
                "evicted stale entries from disk cache"
            );
        }
        removed
    }

    /// Number of entries currently present. Useful for tests and metrics.
    pub fn len(&self) -> usize {
        self.db.len()
    }

    /// Report whether the store holds any entries.
    pub fn is_empty(&self) -> bool {
        self.db.is_empty()
    }

    /// Flush any in-memory Sled buffers to disk. Called before drop in
    /// tests that simulate a crash / restart boundary.
    pub fn flush(&self) -> Result<(), CacheError> {
        self.db.flush()?;
        Ok(())
    }

    /// Entry is stale when `current_ledger - written_at_ledger >
    /// max_ledger_age`. `saturating_sub` protects against
    /// out-of-order / future-dated writes — those are treated as fresh.
    fn is_stale(&self, current_ledger: u32, written_at_ledger: u32) -> bool {
        current_ledger.saturating_sub(written_at_ledger) > self.max_ledger_age
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn open_cache(dir: &Path, max_age: u32) -> DiskCache {
        DiskCache::open_at(dir, max_age).expect("open disk cache")
    }

    #[test]
    fn set_and_get_roundtrip() {
        let dir = tempdir().unwrap();
        let cache = open_cache(dir.path(), 100);
        cache.set(b"key1", b"value1".to_vec(), 1000).unwrap();
        assert_eq!(cache.get(b"key1", 1000), Some(b"value1".to_vec()));
    }

    #[test]
    fn miss_returns_none() {
        let dir = tempdir().unwrap();
        let cache = open_cache(dir.path(), 100);
        assert_eq!(cache.get(b"missing", 42), None);
    }

    #[test]
    fn disk_cache_survives_restart() {
        let dir = tempdir().unwrap();
        {
            let cache = open_cache(dir.path(), 100);
            cache.set(b"key1", b"value1".to_vec(), 1000).unwrap();
            cache.flush().unwrap();
        }
        // Reopen the same directory — simulates a process restart.
        let cache2 = open_cache(dir.path(), 100);
        assert_eq!(cache2.get(b"key1", 1050), Some(b"value1".to_vec()));
    }

    #[test]
    fn stale_entries_are_evicted_on_read() {
        let dir = tempdir().unwrap();
        let cache = open_cache(dir.path(), 50);
        cache.set(b"old", b"data".to_vec(), 1000).unwrap();
        // current_ledger - written_at_ledger = 51 > max_ledger_age (50)
        assert_eq!(cache.get(b"old", 1051), None);
        // And the key is physically gone — next get with a "fresh" ledger
        // view must still miss because the entry was evicted.
        assert_eq!(cache.get(b"old", 1000), None);
    }

    #[test]
    fn entries_at_boundary_are_still_fresh() {
        let dir = tempdir().unwrap();
        let cache = open_cache(dir.path(), 50);
        cache.set(b"edge", b"data".to_vec(), 1000).unwrap();
        // Exactly max_ledger_age behind — still fresh.
        assert_eq!(cache.get(b"edge", 1050), Some(b"data".to_vec()));
    }

    #[test]
    fn evict_stale_sweeps_all_old_entries() {
        let dir = tempdir().unwrap();
        let cache = open_cache(dir.path(), 10);
        cache.set(b"a", b"A".to_vec(), 100).unwrap();
        cache.set(b"b", b"B".to_vec(), 105).unwrap();
        cache.set(b"c", b"C".to_vec(), 120).unwrap();
        // At ledger 130: a (30 old) and b (25 old) are stale, c (10 old) is fresh.
        let removed = cache.evict_stale(130);
        assert_eq!(removed, 2);
        assert_eq!(cache.get(b"a", 130), None);
        assert_eq!(cache.get(b"b", 130), None);
        assert_eq!(cache.get(b"c", 130), Some(b"C".to_vec()));
    }

    #[test]
    fn set_overwrites_previous_entry() {
        let dir = tempdir().unwrap();
        let cache = open_cache(dir.path(), 100);
        cache.set(b"k", b"v1".to_vec(), 100).unwrap();
        cache.set(b"k", b"v2".to_vec(), 200).unwrap();
        assert_eq!(cache.get(b"k", 200), Some(b"v2".to_vec()));
    }

    #[test]
    fn future_dated_writes_are_not_stale() {
        // If a write ledger is ahead of the current ledger (clock skew,
        // reordered reads) the saturating-sub guard treats it as fresh.
        let dir = tempdir().unwrap();
        let cache = open_cache(dir.path(), 10);
        cache.set(b"k", b"v".to_vec(), 200).unwrap();
        assert_eq!(cache.get(b"k", 100), Some(b"v".to_vec()));
    }
}
