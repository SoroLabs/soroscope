//! Redis-backed lease lock for electing a single leader among multiple
//! Core instances, so leader-only background work (e.g. ledger fee
//! collection) runs on exactly one instance at a time and doesn't race on
//! shared state.
//!
//! This is a simplified, single-Redis-node lease lock: `SET key token NX
//! PX` to acquire, a Lua compare-and-expire script to renew, and a
//! compare-and-delete Lua script to release — the same primitives the
//! Redlock algorithm builds on, sized down to the one `redis_url` this
//! service already connects to for its job queue.

use redis::{AsyncCommands, Client as RedisClient, ExistenceCheck, SetExpiry, SetOptions};
use std::sync::Mutex as StdMutex;
use std::time::Duration;
use tokio::time::Instant;
use uuid::Uuid;

const RENEW_SCRIPT: &str = r#"
if redis.call("GET", KEYS[1]) == ARGV[1] then
    return redis.call("PEXPIRE", KEYS[1], ARGV[2])
else
    return 0
end
"#;

const RELEASE_SCRIPT: &str = r#"
if redis.call("GET", KEYS[1]) == ARGV[1] then
    return redis.call("DEL", KEYS[1])
else
    return 0
end
"#;

/// A renewable Redis lease used to elect a single leader across instances.
pub struct RedisLeaderLock {
    redis: RedisClient,
    key: String,
    token: String,
    ttl_ms: usize,
    last_acquired_at: StdMutex<Option<Instant>>,
}

impl RedisLeaderLock {
    pub fn new(redis: RedisClient, key: impl Into<String>, ttl: Duration) -> Self {
        Self {
            redis,
            key: key.into(),
            token: Uuid::new_v4().to_string(),
            ttl_ms: ttl.as_millis() as usize,
            last_acquired_at: StdMutex::new(None),
        }
    }

    /// Returns the monotonic instant when the lease was last successfully acquired or renewed locally.
    pub fn last_acquired_instant(&self) -> Option<Instant> {
        *self.last_acquired_at.lock().unwrap()
    }

    /// Checks whether the lease is locally considered valid based on the monotonic system clock.
    pub fn is_lease_locally_valid(&self) -> bool {
        if let Some(acquired_at) = self.last_acquired_instant() {
            acquired_at.elapsed() < Duration::from_millis(self.ttl_ms as u64)
        } else {
            false
        }
    }

    /// Attempt to become (or remain) leader. Returns `true` if this
    /// instance holds the lease after the call, `false` if another
    /// instance currently holds it (or Redis is unreachable, in which case
    /// callers should treat the cycle as non-leader and skip).
    pub async fn try_acquire_or_renew(&self) -> bool {
        let mut conn = match self.redis.get_multiplexed_async_connection().await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "leader lock: failed to connect to Redis");
                return false;
            }
        };

        // If we already hold the lease, extend it.
        let renewed: i64 = redis::Script::new(RENEW_SCRIPT)
            .key(&self.key)
            .arg(&self.token)
            .arg(self.ttl_ms)
            .invoke_async(&mut conn)
            .await
            .unwrap_or(0);

        if renewed == 1 {
            *self.last_acquired_at.lock().unwrap() = Some(Instant::now());
            return true;
        }

        // Otherwise try to claim it fresh (fails if another instance holds it).
        let opts = SetOptions::default()
            .conditional_set(ExistenceCheck::NX)
            .with_expiration(SetExpiry::PX(self.ttl_ms));

        let acquired = conn
            .set_options::<_, _, bool>(&self.key, self.token.as_str(), opts)
            .await
            .unwrap_or(false);

        if acquired {
            *self.last_acquired_at.lock().unwrap() = Some(Instant::now());
        }

        acquired
    }

    /// Release the lease if still held by this instance. Best-effort — if
    /// this fails (e.g. Redis is briefly unreachable) the lease simply
    /// expires after `ttl` and another instance takes over.
    #[allow(dead_code)]
    pub async fn release(&self) {
        *self.last_acquired_at.lock().unwrap() = None;
        let Ok(mut conn) = self.redis.get_multiplexed_async_connection().await else {
            return;
        };
        let _: Result<i64, _> = redis::Script::new(RELEASE_SCRIPT)
            .key(&self.key)
            .arg(&self.token)
            .invoke_async(&mut conn)
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lease_local_validity_and_expiry() {
        let client = redis::Client::open("redis://127.0.0.1/").unwrap();
        let lock = RedisLeaderLock::new(client, "test_key", Duration::from_millis(100));

        assert!(!lock.is_lease_locally_valid());
        assert!(lock.last_acquired_instant().is_none());

        // Simulate acquisition
        *lock.last_acquired_at.lock().unwrap() = Some(Instant::now());
        assert!(lock.is_lease_locally_valid());
        assert!(lock.last_acquired_instant().is_some());
    }

    #[tokio::test]
    async fn test_clock_jump_stability() {
        tokio::time::pause();
        let client = redis::Client::open("redis://127.0.0.1/").unwrap();
        let lock = RedisLeaderLock::new(client, "test_key", Duration::from_secs(5));

        *lock.last_acquired_at.lock().unwrap() = Some(Instant::now());
        assert!(lock.is_lease_locally_valid());

        // Advance simulated monotonic time by 2 seconds — should still be valid
        tokio::time::advance(Duration::from_secs(2)).await;
        assert!(lock.is_lease_locally_valid());

        // Advance simulated time past TTL — should expire
        tokio::time::advance(Duration::from_secs(4)).await;
        assert!(!lock.is_lease_locally_valid());
    }
}
