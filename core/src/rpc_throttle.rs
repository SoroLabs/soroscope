//! Cooperative throttling driven by rate-limit response headers.

use reqwest::header::HeaderMap;
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;

#[derive(Clone, Default)]
pub struct RpcThrottle {
    blocked_until: Arc<Mutex<Option<tokio::time::Instant>>>,
}

impl RpcThrottle {
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
}
