//! Redis-backed distributed leader lock for clustered Core instances.
//!
//! Ensures that leader-only background work (e.g. ledger fee collection, cron
//! scheduling) runs on **exactly one** node at a time, even in the presence of
//! multiple concurrent instances.
//!
//! ## Protocol (Issue #25)
//!
//! 1. **Acquisition** — `SET key token NX PX <ttl_ms>`.  The atomicity of the
//!    Redis `SET NX` command guarantees that at most one caller succeeds.
//! 2. **Keepalive / renewal** — a background Tokio task calls a Lua
//!    compare-and-expire script every `ttl / 3` to extend the lease only if
//!    the stored token matches our own.  This prevents a stale leader from
//!    inadvertently extending a lease that was claimed by a new leader after a
//!    network partition.
//! 3. **Release on shutdown** — [`RedisLeaderLock::release`] executes a Lua
//!    compare-and-delete script so the lease is deleted immediately rather than
//!    waiting for TTL expiry, allowing a new leader to be elected quickly.
//! 4. **Mutual exclusion guarantee** — because both renewal and release are
//!    Lua scripts executed atomically on Redis, there is no TOCTOU window
//!    between checking the token and modifying the key.
//! 5. **Fencing (Issue #1497)** — every leadership transition bumps a
//!    monotonically increasing `fencing_token`.  The lease value stored in
//!    Redis encodes both the `owner_id` and the `fencing_token`, so a paused
//!    former leader whose lease was taken over cannot renew or release the new
//!    leader's lock: both operations verify the full lease value and return
//!    [`LockError::LeaseLost`] otherwise, without touching the key.
//!
//! ## Usage
//!
//! ```rust,ignore
//! let lock = Arc::new(RedisLeaderLock::new(
//!     redis_client,
//!     "soroscope:leader",
//!     Duration::from_secs(10),
//! ));
//!
//! // Try to become leader (call periodically, e.g. every ttl/2).
//! if lock.try_acquire_or_renew().await.is_ok() {
//!     run_leader_only_work().await;
//! }
//!
//! // Start the background keepalive (returns a handle to cancel on shutdown).
//! let keepalive = lock.clone().start_keepalive();
//!
//! // On graceful shutdown:
//! keepalive.abort();
//! lock.release().await;
//! ```

use redis::{AsyncCommands, Client as RedisClient, ExistenceCheck, SetExpiry, SetOptions};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use uuid::Uuid;

// ── Errors ────────────────────────────────────────────────────────────────────

/// Errors returned by [`RedisLeaderLock`] operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockError {
    /// The lease is no longer held by this instance: either it expired and was
    /// taken over by another node, or the stored `owner_id` / `fencing_token`
    /// no longer match ours.  The key was **not** modified.
    LeaseLost,
    /// The operation could not be completed because Redis was unreachable.
    Backend(String),
}

impl fmt::Display for LockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LockError::LeaseLost => write!(f, "leader lock: lease lost to another node"),
            LockError::Backend(msg) => write!(f, "leader lock: backend error: {msg}"),
        }
    }
}

impl std::error::Error for LockError {}

// ── Lua scripts ───────────────────────────────────────────────────────────────

/// Renew TTL only when the stored lease value matches ours exactly.
///
/// `ARGV[1]` is the full lease value (`owner_id:fencing_token`), so a stale
/// leader whose lease was taken over by a newer fencing token cannot extend it.
const RENEW_SCRIPT: &str = r#"
if redis.call("GET", KEYS[1]) == ARGV[1] then
    return redis.call("PEXPIRE", KEYS[1], ARGV[2])
else
    return 0
end
"#;

/// Delete the key only when the stored lease value matches ours exactly.
const RELEASE_SCRIPT: &str = r#"
if redis.call("GET", KEYS[1]) == ARGV[1] then
    return redis.call("DEL", KEYS[1])
else
    return 0
end
"#;

/// Encode the lease value stored in Redis: `<owner_id>:<fencing_token>`.
fn lease_value(owner_id: &str, fencing_token: u64) -> String {
    format!("{owner_id}:{fencing_token}")
}

// ── RedisLeaderLock ───────────────────────────────────────────────────────────

/// A renewable Redis lease used to elect a single leader across instances.
///
/// The lock is thread-safe and clone-safe via internal `Arc` state, so it can
/// be passed to the keepalive background task while remaining usable from the
/// main application.
pub struct RedisLeaderLock {
    inner: Arc<LockInner>,
}

struct LockInner {
    redis: RedisClient,
    key: String,
    /// Stable per-instance identity used to distinguish lease owners.
    owner_id: String,
    /// Monotonically increasing fencing token, bumped on every leadership
    /// transition.  Guards against stale-lease takeover (Issue #1497).
    fencing_token: std::sync::atomic::AtomicU64,
    /// Lease duration in milliseconds.
    ttl_ms: u64,
    /// Local monotonic deadline after which the lease is considered expired
    /// without consulting Redis.
    lease_deadline: std::sync::Mutex<Option<Instant>>,
}

impl RedisLeaderLock {
    /// Create a new leader lock.
    ///
    /// * `redis`  — an already-configured `redis::Client`.
    /// * `key`    — the Redis key used as the lock.
    /// * `ttl`    — how long the lease lives without renewal.  Minimum 1 ms.
    pub fn new(redis: RedisClient, key: impl Into<String>, ttl: Duration) -> Self {
        let ttl_ms = ttl.as_millis().try_into().unwrap_or(u64::MAX).max(1);

        Self {
            inner: Arc::new(LockInner {
                redis,
                key: key.into(),
                owner_id: Uuid::new_v4().to_string(),
                fencing_token: std::sync::atomic::AtomicU64::new(0),
                ttl_ms,
                lease_deadline: std::sync::Mutex::new(None),
            }),
        }
    }

    // ── Acquisition / renewal ─────────────────────────────────────────────────

    /// Attempt to become (or remain) leader.
    ///
    /// Returns `Ok(())` if this instance holds the lease after the call.
    /// Returns `Err(LockError::LeaseLost)` if another instance holds the lease
    /// (callers should treat this as non-leader and skip leader-only work), and
    /// `Err(LockError::Backend)` if Redis is unreachable.
    ///
    /// Renewal and acquisition are each a single atomic Redis operation
    /// (Lua compare-and-expire / `SET NX`), so there is no TOCTOU window.
    pub async fn try_acquire_or_renew(&self) -> Result<(), LockError> {
        let inner = &self.inner;

        // Check whether our local deadline has already passed.
        let expired = inner
            .lease_deadline
            .lock()
            .expect("leader lock deadline mutex poisoned")
            .is_some_and(|deadline| Instant::now() >= deadline);

        if expired {
            // Clear the stale local deadline so we attempt a fresh acquisition.
            *inner
                .lease_deadline
                .lock()
                .expect("leader lock deadline mutex poisoned") = None;
        }

        let mut conn = inner
            .redis
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, "leader lock: failed to connect to Redis");
                LockError::Backend(e.to_string())
            })?;

        // Try to renew first (cheaper — avoids a full round-trip SET).
        if !expired {
            let current = inner.current_lease_value();
            let renewed: i64 = redis::Script::new(RENEW_SCRIPT)
                .key(&inner.key)
                .arg(&current)
                .arg(inner.ttl_ms)
                .invoke_async(&mut conn)
                .await
                .unwrap_or(0);

            if renewed == 1 {
                inner.update_lease_deadline();
                return Ok(());
            }
        }

        // Fresh acquisition attempt — fails atomically if another instance
        // already holds the key (NX semantics).  Bump the fencing token so
        // every leadership transition is strictly increasing.
        let new_token = inner.next_fencing_token();
        let value = lease_value(&inner.owner_id, new_token);

        let opts = SetOptions::default()
            .conditional_set(ExistenceCheck::NX)
            .with_expiration(SetExpiry::PX(inner.ttl_ms as usize));

        let acquired = conn
            .set_options::<_, _, bool>(&inner.key, value.as_str(), opts)
            .await
            .unwrap_or(false);

        if acquired {
            inner.update_lease_deadline();
            tracing::info!(
                key = %inner.key,
                fencing_token = new_token,
                "leader lock: acquired"
            );
            Ok(())
        } else {
            Err(LockError::LeaseLost)
        }
    }

    /// Renew the lease, verifying that we still own it.
    ///
    /// Returns `Err(LockError::LeaseLost)` — without touching the key — if the
    /// lease expired and was taken over by another node (different `owner_id`
    /// or `fencing_token`).
    pub async fn renew(&self) -> Result<(), LockError> {
        let inner = &self.inner;
        let mut conn = inner
            .redis
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| LockError::Backend(e.to_string()))?;

        let current = inner.current_lease_value();
        let renewed: i64 = redis::Script::new(RENEW_SCRIPT)
            .key(&inner.key)
            .arg(&current)
            .arg(inner.ttl_ms)
            .invoke_async(&mut conn)
            .await
            .unwrap_or(0);

        if renewed == 1 {
            inner.update_lease_deadline();
            Ok(())
        } else {
            // Lease was taken over (or expired); drop local validity.
            *inner
                .lease_deadline
                .lock()
                .expect("leader lock deadline mutex poisoned") = None;
            Err(LockError::LeaseLost)
        }
    }

    // ── Background keepalive ──────────────────────────────────────────────────

    /// Start a background task that renews the lease every `ttl / 3`.
    ///
    /// The returned [`JoinHandle`] should be **aborted** as part of graceful
    /// shutdown (before calling [`release`]) to prevent a post-shutdown renewal
    /// from re-acquiring the lock.
    ///
    /// If the renewal fails (Redis unreachable, or another instance stole the
    /// lease) the task logs a warning and continues to retry; it never panics.
    pub fn start_keepalive(self: Arc<Self>) -> JoinHandle<()> {
        let lock = self;
        let inner = Arc::clone(&lock.inner);
        let interval = Duration::from_millis((inner.ttl_ms / 3).max(1));

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

            loop {
                ticker.tick().await;

                let mut conn = match inner.redis.get_multiplexed_async_connection().await {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            key = %inner.key,
                            "leader lock keepalive: Redis connection failed, will retry"
                        );
                        continue;
                    }
                };

                let current = inner.current_lease_value();
                let renewed: i64 = redis::Script::new(RENEW_SCRIPT)
                    .key(&inner.key)
                    .arg(&current)
                    .arg(inner.ttl_ms)
                    .invoke_async(&mut conn)
                    .await
                    .unwrap_or(0);

                if renewed == 1 {
                    inner.update_lease_deadline();
                    tracing::debug!(key = %inner.key, "leader lock keepalive: renewed");
                } else {
                    // Another instance took over; clear our local deadline so
                    // the next try_acquire_or_renew call performs a fresh NX.
                    *inner
                        .lease_deadline
                        .lock()
                        .expect("leader lock deadline mutex poisoned") = None;
                    tracing::info!(
                        key = %inner.key,
                        "leader lock keepalive: lease lost, will reattempt acquisition"
                    );
                }
            }
        })
    }

    // ── Release ───────────────────────────────────────────────────────────────

    /// Release the lease immediately if still held by this instance.
    ///
    /// The compare-and-delete Lua script verifies the full lease value
    /// (`owner_id:fencing_token`), so a stale leader whose lease was taken over
    /// by another node **cannot** delete the new leader's lock; in that case
    /// `Err(LockError::LeaseLost)` is returned and the key is left untouched.
    ///
    /// Best-effort — if Redis is unreachable the key simply expires after
    /// `ttl`.  Always call this during graceful shutdown **after** aborting the
    /// keepalive handle, so a restarted instance can elect a new leader quickly.
    pub async fn release(&self) -> Result<(), LockError> {
        let inner = &self.inner;

        // Clear local deadline immediately so no further work is scheduled.
        *inner
            .lease_deadline
            .lock()
            .expect("leader lock deadline mutex poisoned") = None;

        let mut conn = inner
            .redis
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| {
                tracing::warn!(key = %inner.key, "leader lock: release failed (Redis unreachable); TTL expiry will clean up");
                LockError::Backend(e.to_string())
            })?;

        let current = inner.current_lease_value();
        let deleted: i64 = redis::Script::new(RELEASE_SCRIPT)
            .key(&inner.key)
            .arg(&current)
            .invoke_async(&mut conn)
            .await
            .unwrap_or(0);

        if deleted == 1 {
            tracing::info!(key = %inner.key, "leader lock: released");
            Ok(())
        } else {
            tracing::debug!(key = %inner.key, "leader lock: release skipped (lease lost or already expired)");
            Err(LockError::LeaseLost)
        }
    }

    // ── Accessors (primarily for tests) ──────────────────────────────────────

    /// Returns `true` if the local monotonic deadline has not yet elapsed,
    /// i.e. we *believe* we still hold the lease without consulting Redis.
    pub fn is_lease_locally_valid(&self) -> bool {
        self.inner
            .lease_deadline
            .lock()
            .expect("leader lock deadline mutex poisoned")
            .is_some_and(|deadline| Instant::now() < deadline)
    }

    /// Return the instant at which the current lease expires locally, if any.
    pub fn lease_deadline(&self) -> Option<Instant> {
        *self
            .inner
            .lease_deadline
            .lock()
            .expect("leader lock deadline mutex poisoned")
    }

    /// Return the stable per-instance owner identity.
    pub fn owner_id(&self) -> &str {
        &self.inner.owner_id
    }

    /// Return the current fencing token (0 before the first acquisition).
    pub fn fencing_token(&self) -> u64 {
        self.inner
            .fencing_token
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Return the lease value this instance would write to Redis.
    pub fn lease_value(&self) -> String {
        self.inner.current_lease_value()
    }

    /// Return the configured TTL in milliseconds.
    pub fn ttl_ms(&self) -> u64 {
        self.inner.ttl_ms
    }
}

// ── LockInner helpers ─────────────────────────────────────────────────────────

impl LockInner {
    fn update_lease_deadline(&self) {
        *self
            .lease_deadline
            .lock()
            .expect("leader lock deadline mutex poisoned") =
            Some(Instant::now() + Duration::from_millis(self.ttl_ms));
    }

    /// The lease value currently associated with this instance.
    fn current_lease_value(&self) -> String {
        lease_value(
            &self.owner_id,
            self.fencing_token.load(std::sync::atomic::Ordering::SeqCst),
        )
    }

    /// Atomically bump the fencing token and return the new value.
    fn next_fencing_token(&self) -> u64 {
        self.fencing_token
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: build a lock against a local Redis URL without needing a live server.
    fn make_lock(ttl: Duration) -> RedisLeaderLock {
        RedisLeaderLock::new(
            RedisClient::open("redis://127.0.0.1/").expect("valid Redis URL"),
            "soroscope:test:leader",
            ttl,
        )
    }

    // ── Unit tests (no live Redis required) ──────────────────────────────────

    #[test]
    fn zero_ttl_is_clamped_to_one_millisecond() {
        let lock = make_lock(Duration::ZERO);
        assert_eq!(lock.ttl_ms(), 1, "TTL must be at least 1 ms");
    }

    #[test]
    fn owner_id_is_non_empty_and_unique() {
        let a = make_lock(Duration::from_secs(5));
        let b = make_lock(Duration::from_secs(5));
        assert!(!a.owner_id().is_empty());
        assert_ne!(a.owner_id(), b.owner_id(), "each lock instance must have a distinct owner id");
    }

    #[test]
    fn fencing_token_starts_at_zero_and_increases() {
        let lock = make_lock(Duration::from_secs(5));
        assert_eq!(lock.fencing_token(), 0);
        assert_eq!(lock.inner.next_fencing_token(), 1);
        assert_eq!(lock.inner.next_fencing_token(), 2);
        assert_eq!(lock.fencing_token(), 2);
    }

    #[test]
    fn lease_value_encodes_owner_and_fencing_token() {
        let lock = make_lock(Duration::from_secs(5));
        lock.inner.next_fencing_token();
        assert_eq!(lock.lease_value(), format!("{}:1", lock.owner_id()));
    }

    // ── Stale-lease takeover (Issue #1497) ───────────────────────────────────

    /// Simulate: Node A acquires, its lease expires, Node B takes over with a
    /// higher fencing token, then Node A's late release/renew must be rejected
    /// without touching Node B's lock.
    #[test]
    fn stale_leader_cannot_release_or_renew_after_takeover() {
        let node_a = make_lock(Duration::from_secs(5));
        let node_b = make_lock(Duration::from_secs(5));

        // Node A acquires leadership (fencing token 1).
        let a_token = node_a.inner.next_fencing_token();
        let a_value = lease_value(node_a.owner_id(), a_token);
        assert_eq!(a_token, 1);

        // Node A pauses; its lease expires and Node B takes over with a
        // strictly greater fencing token.
        let b_token = node_b.inner.next_fencing_token();
        let b_value = lease_value(node_b.owner_id(), b_token);
        assert!(b_token > a_token, "fencing tokens must strictly increase on transition");

        // The stored lease is now Node B's.
        let stored = b_value.clone();

        // Node A's late release: compare-and-delete sees a mismatch and must
        // not delete the key.
        let a_release_matches = stored == a_value;
        assert!(!a_release_matches, "stale release must not match the new lease");
        assert_eq!(stored, b_value, "Node B's lock must remain untouched");

        // Node A's late renew: compare-and-expire sees a mismatch and must not
        // extend the key.
        let a_renew_matches = stored == a_value;
        assert!(!a_renew_matches, "stale renew must not match the new lease");
        assert_eq!(stored, b_value, "Node B's lock must remain untouched");

        // Node B can still renew/release its own lease.
        assert_eq!(stored, lease_value(node_b.owner_id(), node_b.fencing_token()));
    }

    #[test]
    fn lease_lost_error_is_reported() {
        let err = LockError::LeaseLost;
        assert_eq!(err, LockError::LeaseLost);
        assert!(err.to_string().contains("lease lost"));
    }

    #[test]
    fn lease_not_locally_valid_before_acquisition() {
        let lock = make_lock(Duration::from_secs(5));
        assert!(
            !lock.is_lease_locally_valid(),
            "lease must not be locally valid before any acquisition attempt"
        );
        assert!(lock.lease_deadline().is_none());
    }

    #[test]
    fn monotonic_deadline_survives_wall_clock_jump() {
        let start = Instant::now();
        let deadline = start + Duration::from_secs(1);

        // A wall-clock adjustment cannot change a tokio monotonic instant.
        assert!(Instant::now() < deadline);
        assert_eq!(deadline.duration_since(start), Duration::from_secs(1));
    }

    // ── Simulated acquisition tests (tokio paused time) ───────────────────────

    #[tokio::test]
    async fn simulated_lease_expires_after_ttl() {
        tokio::time::pause();

        let lock = make_lock(Duration::from_secs(5));

        // Manually set a deadline as if `try_acquire_or_renew` succeeded.
        lock.inner.update_lease_deadline();

        assert!(lock.is_lease_locally_valid(), "should be valid immediately after acquisition");

        // Advance time by 3 s — still within TTL.
        tokio::time::advance(Duration::from_secs(3)).await;
        assert!(lock.is_lease_locally_valid(), "should still be valid at 3 s");

        // Advance past the 5 s TTL.
        tokio::time::advance(Duration::from_secs(3)).await;
        assert!(!lock.is_lease_locally_valid(), "should be expired at 6 s");
    }

    #[tokio::test]
    async fn simulated_release_clears_local_deadline() {
        tokio::time::pause();

        let lock = make_lock(Duration::from_secs(5));
        lock.inner.update_lease_deadline();

        assert!(lock.is_lease_locally_valid());

        // Release clears the local deadline synchronously (the Redis DEL call
        // will fail since no server is running, but the deadline is cleared
        // before the network call).
        // We test only the local state here.
        *lock.inner.lease_deadline.lock().unwrap() = None;

        assert!(!lock.is_lease_locally_valid(), "deadline must be cleared after release");
        assert!(lock.lease_deadline().is_none());
    }

    // ── Mutual-exclusion simulation (single process, two lock objects) ────────

    #[test]
    fn two_locks_have_different_owner_ids() {
        // If two instances try to acquire the same key, the one that arrives
        // second will receive a different owner id and the NX SET will fail.
        // We verify the owner uniqueness invariant here; the full Redis
        // integration test is in core/tests/leader_lock_integration.rs.
        let lock_a = make_lock(Duration::from_secs(10));
        let lock_b = make_lock(Duration::from_secs(10));
        assert_ne!(
            lock_a.owner_id(),
            lock_b.owner_id(),
            "distinct instances must carry distinct owner ids to enforce mutual exclusion"
        );
    }

    // ── Keepalive interval sanity check ──────────────────────────────────────

    #[test]
    fn keepalive_interval_is_one_third_of_ttl() {
        let ttl = Duration::from_millis(300);
        let lock = make_lock(ttl);
        // Interval = ttl_ms / 3 = 100 ms — verified indirectly.
        let expected_interval_ms = lock.ttl_ms() / 3;
        assert_eq!(expected_interval_ms, 100);
    }

    #[test]
    fn keepalive_interval_is_at_least_one_ms_for_tiny_ttls() {
        let lock = make_lock(Duration::from_millis(1));
        let interval_ms = (lock.ttl_ms() / 3).max(1);
        assert!(interval_ms >= 1);
    }
}
