//! Cooperative throttling driven by rate-limit response headers and token-bucket algorithms.

use reqwest::header::HeaderMap;
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct TokenBucketConfig {
    pub capacity: f64,
    pub refill_rate_per_sec: f64,
}

impl Default for TokenBucketConfig {
    fn default() -> Self {
        Self {
            capacity: 100.0,
            refill_rate_per_sec: 10.0,
        }
    }
}

#[derive(Debug)]
struct TokenBucketState {
    tokens: f64,
    last_refill: std::time::Instant,
}

#[derive(Clone)]
pub struct TokenBucketLimiter {
    capacity: f64,
    refill_rate_per_sec: f64,
    state: Arc<Mutex<TokenBucketState>>,
}

impl TokenBucketLimiter {
    pub fn new(capacity: f64, refill_rate_per_sec: f64) -> Self {
        Self {
            capacity,
            refill_rate_per_sec,
            state: Arc::new(Mutex::new(TokenBucketState {
                tokens: capacity,
                last_refill: std::time::Instant::now(),
            })),
        }
    }

    pub async fn try_acquire(&self, tokens: f64) -> bool {
        let mut state = self.state.lock().await;
        self.refill(&mut state);
        if state.tokens >= tokens {
            state.tokens -= tokens;
            true
        } else {
            false
        }
    }

    pub async fn acquire(&self, tokens: f64) -> Duration {
        loop {
            let mut state = self.state.lock().await;
            self.refill(&mut state);
            if state.tokens >= tokens {
                state.tokens -= tokens;
                return Duration::ZERO;
            }
            let needed = tokens - state.tokens;
            let wait_secs = needed / self.refill_rate_per_sec;
            let wait_duration = Duration::from_secs_f64(wait_secs);
            drop(state);
            tokio::time::sleep(wait_duration).await;
        }
    }

    pub async fn available_tokens(&self) -> f64 {
        let mut state = self.state.lock().await;
        self.refill(&mut state);
        state.tokens
    }

    fn refill(&self, state: &mut TokenBucketState) {
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(state.last_refill).as_secs_f64();
        let new_tokens = elapsed * self.refill_rate_per_sec;
        state.tokens = (state.tokens + new_tokens).min(self.capacity);
        state.last_refill = now;
    }
}

#[derive(Clone, Default)]
pub struct RpcThrottle {
    blocked_until: Arc<Mutex<Option<tokio::time::Instant>>>,
    bucket: Option<TokenBucketLimiter>,
}

impl RpcThrottle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_with_bucket(capacity: f64, refill_rate_per_sec: f64) -> Self {
        Self {
            blocked_until: Arc::new(Mutex::new(None)),
            bucket: Some(TokenBucketLimiter::new(capacity, refill_rate_per_sec)),
        }
    }

    pub async fn acquire_token(&self, tokens: f64) -> bool {
        if let Some(bucket) = &self.bucket {
            bucket.try_acquire(tokens).await
        } else {
            true
        }
    }

    pub fn bucket(&self) -> Option<&TokenBucketLimiter> {
        self.bucket.as_ref()
    }

    /// Wait until the provider's advertised retry window has elapsed.
    pub async fn wait(&self) {
        let deadline = *self.blocked_until.lock().await;
        if let Some(deadline) = deadline {
            tokio::time::sleep_until(deadline).await;
        }
    }

    /// Update the next permitted request time from standard RPC headers.
    pub async fn observe(&self, headers: &HeaderMap) -> Option<Duration> {
        let delay = retry_delay(headers, SystemTime::now())?;
        *self.blocked_until.lock().await = Some(tokio::time::Instant::now() + delay);
        Some(delay)
    }
}

pub fn retry_delay(headers: &HeaderMap, now: SystemTime) -> Option<Duration> {
    if let Some(seconds) = headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
    {
        return Some(Duration::from_secs(seconds));
    }

    let reset = headers
        .get("x-ratelimit-reset")?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()?;
    let now = now.duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some(Duration::from_secs(reset.saturating_sub(now)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderValue, RETRY_AFTER};

    #[test]
    fn retry_after_takes_precedence() {
        let mut headers = HeaderMap::new();
        headers.insert(RETRY_AFTER, HeaderValue::from_static("7"));
        headers.insert("x-ratelimit-reset", HeaderValue::from_static("999999"));
        assert_eq!(
            retry_delay(&headers, UNIX_EPOCH),
            Some(Duration::from_secs(7))
        );
    }

    #[test]
    fn parses_unix_reset_and_clamps_past_deadlines() {
        let mut headers = HeaderMap::new();
        headers.insert("x-ratelimit-reset", HeaderValue::from_static("110"));
        assert_eq!(
            retry_delay(&headers, UNIX_EPOCH + Duration::from_secs(100)),
            Some(Duration::from_secs(10))
        );
        assert_eq!(
            retry_delay(&headers, UNIX_EPOCH + Duration::from_secs(120)),
            Some(Duration::ZERO)
        );
    }

    #[test]
    fn ignores_invalid_or_missing_headers() {
        assert_eq!(retry_delay(&HeaderMap::new(), UNIX_EPOCH), None);
        let mut headers = HeaderMap::new();
        headers.insert(RETRY_AFTER, HeaderValue::from_static("tomorrow"));
        assert_eq!(retry_delay(&headers, UNIX_EPOCH), None);
    }

    #[tokio::test]
    async fn token_bucket_acquires_and_refills() {
        let limiter = TokenBucketLimiter::new(10.0, 10.0);
        assert!(limiter.try_acquire(5.0).await);
        assert!(limiter.try_acquire(5.0).await);
        assert!(!limiter.try_acquire(1.0).await);

        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(limiter.try_acquire(1.0).await);
    }

    #[tokio::test]
    async fn rpc_throttle_with_bucket() {
        let throttle = RpcThrottle::new_with_bucket(5.0, 5.0);
        assert!(throttle.acquire_token(5.0).await);
        assert!(!throttle.acquire_token(1.0).await);
        assert!(throttle.bucket().is_some());
    }
}
