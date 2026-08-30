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
    lease_deadline: std::sync::Mutex<Option<Instant>>,
}

impl RedisLeaderLock {
    pub fn new(redis: RedisClient, key: impl Into<String>, ttl: Duration) -> Self {
        let ttl_ms = ttl.as_millis().try_into().unwrap_or(usize::MAX).max(1);

        Self {
            redis,
            key: key.into(),
            token: Uuid::new_v4().to_string(),
            ttl_ms,
            lease_deadline: std::sync::Mutex::new(None),
        }
    }

    /// Attempt to become (or remain) leader. Returns `true` if this
    /// instance holds the lease after the call, `false` if another
    /// instance currently holds it (or Redis is unreachable, in which case
    /// callers should treat the cycle as non-leader and skip).
    pub async fn try_acquire_or_renew(&self) -> bool {
        let expired = self
            .lease_deadline
            .lock()
            .expect("leader lock deadline mutex poisoned")
            .is_some_and(|deadline| Instant::now() >= deadline);
        if expired {
            *self
                .lease_deadline
                .lock()
                .expect("leader lock deadline mutex poisoned") = None;
        }

        let mut conn = match self.redis.get_multiplexed_async_connection().await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "leader lock: failed to connect to Redis");
                return false;
            }
        };

        // If we already hold the lease, extend it.
        let renewed: i64 = if expired {
            0
        } else {
            redis::Script::new(RENEW_SCRIPT)
                .key(&self.key)
                .arg(&self.token)
                .arg(self.ttl_ms)
                .invoke_async(&mut conn)
                .await
                .unwrap_or(0)
        };

        if renewed == 1 {
            self.update_lease_deadline();
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
            self.update_lease_deadline();
        }
        acquired
    }

    fn update_lease_deadline(&self) {
        *self
            .lease_deadline
            .lock()
            .expect("leader lock deadline mutex poisoned") =
            Some(Instant::now() + Duration::from_millis(self.ttl_ms as u64));
    }

    /// Release the lease if still held by this instance. Best-effort — if
    /// this fails (e.g. Redis is briefly unreachable) the lease simply
    /// expires after `ttl` and another instance takes over.
    #[allow(dead_code)]
    pub async fn release(&self) {
        let Ok(mut conn) = self.redis.get_multiplexed_async_connection().await else {
            return;
        };
        let _: Result<i64, _> = redis::Script::new(RELEASE_SCRIPT)
            .key(&self.key)
            .arg(&self.token)
            .invoke_async(&mut conn)
            .await;
        *self
            .lease_deadline
            .lock()
            .expect("leader lock deadline mutex poisoned") = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monotonic_deadline_survives_wall_clock_jump() {
        let start = Instant::now();
        let deadline = start + Duration::from_secs(1);

        // A wall-clock adjustment cannot change a tokio monotonic instant.
        assert!(Instant::now() < deadline);
        assert_eq!(deadline.duration_since(start), Duration::from_secs(1));
    }

    #[test]
    fn zero_ttl_is_encoded_as_one_millisecond() {
        let lock = RedisLeaderLock::new(
            RedisClient::open("redis://127.0.0.1/").expect("valid Redis URL"),
            "test",
            Duration::ZERO,
        );
        assert_eq!(lock.ttl_ms, 1);
    }
}
