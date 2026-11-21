//! HMAC-SHA256 signature validation for inbound webhook calls.
//!
//! # Security design
//!
//! Each inbound request must carry three headers:
//!   - `x-soroscope-signature` — `sha256=<hex>`, the HMAC-SHA256 over
//!     `<timestamp_bytes>.<raw_body_bytes>` using the pre-shared secret.
//!   - `x-soroscope-timestamp` — Unix timestamp (seconds) as an ASCII integer.
//!   - `x-soroscope-delivery` — opaque delivery UUID (validated for presence only).
//!
//! **Replay protection:** timestamps older than [`MAX_TIMESTAMP_SKEW_SECS`] are
//! rejected. This bounds the window in which a stolen signature can be reused.
//!
//! **Constant-time comparison:** signature bytes are compared with
//! [`hmac::Mac::verify_slice`], which is constant-time by construction and avoids
//! timing-based forgery.
//!
//! # Complexity
//!
//! - Time: `O(n)` in the byte length of the request body — one HMAC pass.
//! - Space: `O(1)` extra allocation beyond the body that Axum already holds.
//!
//! # Usage
//!
//! Wrap any inbound route with [`ValidatedWebhook`] as the last extractor:
//!
//! ```rust,ignore
//! async fn handler(ValidatedWebhook(body): ValidatedWebhook) -> impl IntoResponse {
//!     // `body` is the raw, authenticated request bytes.
//! }
//! ```

use axum::{
    async_trait,
    body::Bytes,
    extract::{FromRequest, Request},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use url::{Host, Url};

use crate::webhooks::{DELIVERY_HEADER, SIGNATURE_HEADER, TIMESTAMP_HEADER};

type HmacSha256 = Hmac<Sha256>;

/// Maximum allowed clock skew (seconds) between the timestamp in the request
/// header and the server's wall clock. Requests older than this are rejected
/// to prevent replay attacks.
///
/// Five minutes is the industry-standard window.
pub const MAX_TIMESTAMP_SKEW_SECS: u64 = 300;

// ─────────────────────────────────────────────────────────────────────────────
// Shared secret state
// ─────────────────────────────────────────────────────────────────────────────

/// Server-side pre-shared secret injected as an Axum [`Extension`].
#[derive(Clone)]
pub struct InboundWebhookSecret(pub Arc<String>);

// ─────────────────────────────────────────────────────────────────────────────
// Extractor
// ─────────────────────────────────────────────────────────────────────────────

/// Axum extractor that validates an inbound webhook request's HMAC-SHA256
/// signature before the handler runs.
pub struct ValidatedWebhook(pub Bytes);

#[async_trait]
impl<S> FromRequest<S> for ValidatedWebhook
where
    S: Send + Sync,
{
    type Rejection = WebhookValidationError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        // 1. Resolve the shared secret: check Extensions, fallback to environment variable.
        let secret = if let Some(InboundWebhookSecret(s)) =
            req.extensions().get::<InboundWebhookSecret>().cloned()
        {
            (*s).clone()
        } else if let Ok(s) = std::env::var("SOROSCOPE_INBOUND_WEBHOOK_SECRET") {
            if s.len() < 32 {
                return Err(WebhookValidationError::MissingSecret);
            }
            s
        } else {
            return Err(WebhookValidationError::MissingSecret);
        };

        // 2. Read required header values.
        let timestamp_val = req
            .headers()
            .get(TIMESTAMP_HEADER)
            .and_then(|v| v.to_str().ok())
            .ok_or(WebhookValidationError::MissingHeader(TIMESTAMP_HEADER))?
            .to_owned();

        let signature_val = req
            .headers()
            .get(SIGNATURE_HEADER)
            .or_else(|| req.headers().get("x-signature-256"))
            .or_else(|| req.headers().get("X-Signature-256"))
            .and_then(|v| v.to_str().ok())
            .ok_or(WebhookValidationError::MissingHeader(SIGNATURE_HEADER))?
            .to_owned();

        req.headers()
            .get(DELIVERY_HEADER)
            .ok_or(WebhookValidationError::MissingHeader(DELIVERY_HEADER))?;

        // 3. Validate timestamp freshness.
        let request_ts: u64 = timestamp_val
            .parse()
            .map_err(|_| WebhookValidationError::InvalidTimestamp)?;

        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if now_secs.saturating_sub(request_ts) > MAX_TIMESTAMP_SKEW_SECS
            || request_ts.saturating_sub(now_secs) > MAX_TIMESTAMP_SKEW_SECS
        {
            return Err(WebhookValidationError::TimestampExpired);
        }

        // 4. Buffer the body.
        let body = Bytes::from_request(req, state)
            .await
            .map_err(|_| WebhookValidationError::BodyReadError)?;

        // 5. Verify HMAC-SHA256 signature in constant time.
        if !verify_signature(&secret, &timestamp_val, &body, &signature_val) {
            return Err(WebhookValidationError::InvalidSignature);
        }

        Ok(Self(body))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Core verification logic
// ─────────────────────────────────────────────────────────────────────────────

/// Compute an HMAC-SHA256 signature for a payload.
pub fn generate_signature(secret: &str, timestamp: &str, body: &[u8]) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts keys of any size");
    mac.update(timestamp.as_bytes());
    mac.update(b".");
    mac.update(body);
    format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
}

/// Verify an HMAC-SHA256 signature.
pub fn verify_signature(secret: &str, timestamp: &str, body: &[u8], signature: &str) -> bool {
    let hex_sig = match signature.strip_prefix("sha256=") {
        Some(h) => h,
        None => signature,
    };

    let sig_bytes = match hex::decode(hex_sig) {
        Ok(b) => b,
        Err(_) => return false,
    };

    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts keys of any size");
    mac.update(timestamp.as_bytes());
    mac.update(b".");
    mac.update(body);

    mac.verify_slice(&sig_bytes).is_ok()
}

// ─────────────────────────────────────────────────────────────────────────────
// Outbound target URL validation (SSRF protection)
// ─────────────────────────────────────────────────────────────────────────────

/// Validate a user-supplied outbound webhook target URL to prevent SSRF.
///
/// # Rules
///
/// - The URL must parse and use `http` or `https`. Any other scheme
///   (e.g. `file://`, `ftp://`, `gopher://`) is rejected.
/// - In production (`allow_localhost_in_dev == false`) only `https://` is
///   accepted. `http://` is permitted only when `allow_localhost_in_dev` is
///   `true` (local development).
/// - The host must not resolve to a loopback, link-local, private (RFC1918 /
///   IPv6 unique-local), or unspecified address, and must not be `localhost`.
///   This blocks cloud metadata endpoints such as `169.254.169.254` and
///   internal LAN addresses.
///
/// Note: this validates the literal host only. Callers must additionally
/// disable redirect following on the outbound HTTP client so that an external
/// server cannot 302-redirect into an internal network.
pub fn validate_webhook_target_url(
    url: &str,
    allow_localhost_in_dev: bool,
) -> Result<Url, WebhookValidationError> {
    let parsed = Url::parse(url)
        .map_err(|e| WebhookValidationError::InvalidTargetUrl(format!("invalid URL: {e}")))?;

    match parsed.scheme() {
        "https" => {}
        "http" if allow_localhost_in_dev => {}
        "http" => {
            return Err(WebhookValidationError::InvalidTargetUrl(
                "http:// is not allowed; use https://".to_string(),
            ));
        }
        other => {
            return Err(WebhookValidationError::InvalidTargetUrl(format!(
                "unsupported scheme '{other}'; only http(s) is allowed"
            )));
        }
    }

    let host = parsed
        .host()
        .ok_or_else(|| WebhookValidationError::InvalidTargetUrl("URL has no host".to_string()))?;

    let blocked = match host {
        Host::Domain(domain) => {
            let domain = domain.to_ascii_lowercase();
            domain == "localhost" || domain.ends_with(".localhost")
        }
        Host::Ipv4(ip) => is_blocked_ipv4(ip),
        Host::Ipv6(ip) => is_blocked_ipv6(ip),
    };

    if blocked {
        return Err(WebhookValidationError::InvalidTargetUrl(
            "target host resolves to a loopback, link-local, private, or unspecified address"
                .to_string(),
        ));
    }

    Ok(parsed)
}

/// Returns `true` for loopback, link-local, private (RFC1918), and unspecified
/// IPv4 addresses.
fn is_blocked_ipv4(ip: Ipv4Addr) -> bool {
    ip.is_loopback()          // 127.0.0.0/8
        || ip.is_link_local() // 169.254.0.0/16 (includes 169.254.169.254)
        || ip.is_private()    // 10/8, 172.16/12, 192.168/16
        || ip.is_unspecified() // 0.0.0.0
        || ip.is_broadcast()  // 255.255.255.255
}

/// Returns `true` for loopback, link-local (fe80::/10), unique-local
/// (fc00::/7), and unspecified IPv6 addresses.
fn is_blocked_ipv6(ip: Ipv6Addr) -> bool {
    ip.is_loopback()      // ::1
        || ip.is_unspecified() // ::
        || is_ipv6_unique_local(ip) // fc00::/7
        || is_ipv6_link_local(ip) // fe80::/10
        || ip
            .to_ipv4_mapped()
            .map(|v4| is_blocked_ipv4(v4))
            .unwrap_or(false)
}

/// `fc00::/7` — IPv6 unique-local addresses.
fn is_ipv6_unique_local(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

/// `fe80::/10` — IPv6 link-local addresses.
fn is_ipv6_link_local(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
}

/// Convenience helper for callers that already hold an [`IpAddr`].
pub fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_blocked_ipv4(v4),
        IpAddr::V6(v6) => is_blocked_ipv6(v6),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Error type
// ─────────────────────────────────────────────────────────────────────────────

/// Errors that can occur during inbound webhook signature validation.
#[derive(Debug)]
pub enum WebhookValidationError {
    /// A required header was absent.
    MissingHeader(&'static str),
    /// The timestamp header value could not be parsed as a `u64`.
    InvalidTimestamp,
    /// The timestamp is outside the skew window.
    TimestampExpired,
    /// The HMAC signature verification failed.
    InvalidSignature,
    /// The request body could not be read.
    BodyReadError,
    /// The pre-shared secret is not configured.
    MissingSecret,
    /// The outbound webhook target URL failed SSRF validation.
    InvalidTargetUrl(String),
}

impl IntoResponse for WebhookValidationError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::MissingHeader(name) => (
                StatusCode::BAD_REQUEST,
                format!("Missing required header: {name}"),
            ),
            Self::InvalidTimestamp => (
                StatusCode::BAD_REQUEST,
                "Invalid timestamp header value".to_string(),
            ),
            Self::TimestampExpired => (
                StatusCode::UNAUTHORIZED,
                format!("Request timestamp expired (max skew: {MAX_TIMESTAMP_SKEW_SECS}s)"),
            ),
            Self::InvalidSignature => (
                StatusCode::UNAUTHORIZED,
                "Webhook signature verification failed".to_string(),
            ),
            Self::BodyReadError => (
                StatusCode::BAD_REQUEST,
                "Failed to read request body".to_string(),
            ),
            Self::MissingSecret => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Inbound webhook secret not configured".to_string(),
            ),
            Self::InvalidTargetUrl(reason) => (
                StatusCode::BAD_REQUEST,
                format!("Invalid webhook target URL: {reason}"),
            ),
        };

        (status, message).into_response()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit and Integration tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::webhooks::sign;
    use axum::{body::Body, http::Request, routing::post, Extension, Router};
    use std::time::{SystemTime, UNIX_EPOCH};
    use tower::ServiceExt;

    fn now_str() -> String {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string()
    }

    const SECRET: &str = "a-secret-that-is-at-least-thirty-two-bytes";
    const BODY: &[u8] = br#"{"event":"transfer","amount":"42"}"#;

    #[test]
    fn valid_signature_is_accepted() {
        let ts = now_str();
        let sig = format!("sha256={}", sign(SECRET, &ts, BODY));
        assert!(verify_signature(SECRET, &ts, BODY, &sig));
    }

    #[test]
    fn signature_with_wrong_body_is_rejected() {
        let ts = now_str();
        let sig = format!("sha256={}", sign(SECRET, &ts, BODY));
        assert!(!verify_signature(SECRET, &ts, b"tampered", &sig));
    }

    #[test]
    fn signature_with_wrong_timestamp_is_rejected() {
        let ts = now_str();
        let sig = format!("sha256={}", sign(SECRET, &ts, BODY));
        assert!(!verify_signature(SECRET, "0", BODY, &sig));
    }

    #[test]
    fn signature_with_wrong_secret_is_rejected() {
        let ts = now_str();
        let sig = format!("sha256={}", sign(SECRET, &ts, BODY));
        let wrong_secret = "wrong-secret-that-is-at-least-thirty-two-bytes";
        assert!(!verify_signature(wrong_secret, &ts, BODY, &sig));
    }

    #[tokio::test]
    async fn extractor_accepts_valid_signature() {
        let secret = Arc::new(SECRET.to_string());
        let app = Router::new()
            .route(
                "/test",
                post(|ValidatedWebhook(body): ValidatedWebhook| async move {
                    assert_eq!(body, BODY);
                    StatusCode::OK
                }),
            )
            .layer(Extension(InboundWebhookSecret(secret)));

        let ts = now_str();
        let sig = format!("sha256={}", sign(SECRET, &ts, BODY));

        let req = Request::builder()
            .method("POST")
            .uri("/test")
            .header(TIMESTAMP_HEADER, &ts)
            .header(SIGNATURE_HEADER, &sig)
            .header(DELIVERY_HEADER, "some-uuid")
            .body(Body::from(BODY))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn extractor_rejects_missing_headers() {
        let secret = Arc::new(SECRET.to_string());
        let app = Router::new()
            .route(
                "/test",
                post(|ValidatedWebhook(_): ValidatedWebhook| async move { StatusCode::OK }),
            )
            .layer(Extension(InboundWebhookSecret(secret)));

        let ts = now_str();
        let sig = format!("sha256={}", sign(SECRET, &ts, BODY));

        // Missing signature header
        let req = Request::builder()
            .method("POST")
            .uri("/test")
            .header(TIMESTAMP_HEADER, &ts)
            .header(DELIVERY_HEADER, "some-uuid")
            .body(Body::from(BODY))
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // Missing delivery header
        let req = Request::builder()
            .method("POST")
            .uri("/test")
            .header(TIMESTAMP_HEADER, &ts)
            .header(SIGNATURE_HEADER, &sig)
            .body(Body::from(BODY))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn extractor_rejects_expired_timestamp() {
        let secret = Arc::new(SECRET.to_string());
        let app = Router::new()
            .route(
                "/test",
                post(|ValidatedWebhook(_): ValidatedWebhook| async move { StatusCode::OK }),
            )
            .layer(Extension(InboundWebhookSecret(secret)));

        let expired_ts = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - MAX_TIMESTAMP_SKEW_SECS
            - 10)
            .to_string();

        let sig = format!("sha256={}", sign(SECRET, &expired_ts, BODY));

        let req = Request::builder()
            .method("POST")
            .uri("/test")
            .header(TIMESTAMP_HEADER, &expired_ts)
            .header(SIGNATURE_HEADER, &sig)
            .header(DELIVERY_HEADER, "some-uuid")
            .body(Body::from(BODY))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn extractor_accepts_x_signature_256_header() {
        let secret = Arc::new(SECRET.to_string());
        let app = Router::new()
            .route(
                "/test",
                post(|ValidatedWebhook(body): ValidatedWebhook| async move {
                    assert_eq!(body, BODY);
                    StatusCode::OK
                }),
            )
            .layer(Extension(InboundWebhookSecret(secret)));

        let ts = now_str();
        let sig = generate_signature(SECRET, &ts, BODY);

        let req = Request::builder()
            .method("POST")
            .uri("/test")
            .header(TIMESTAMP_HEADER, &ts)
            .header("X-Signature-256", &sig)
            .header(DELIVERY_HEADER, "some-uuid")
            .body(Body::from(BODY))
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[test]
    fn hmac_known_test_vector_verification() {
        let secret = "test-secret-key-32-bytes-long!!";
        let ts = "1700000000";
        let body = b"hello world";
        let generated = generate_signature(secret, ts, body);
        assert!(verify_signature(secret, ts, body, &generated));
        assert!(!verify_signature(secret, ts, b"tampered", &generated));
    }

    // ── SSRF target URL validation ──────────────────────────────────────────

    #[test]
    fn accepts_valid_https_endpoint() {
        let url = validate_webhook_target_url("https://example.com/webhook", false)
            .expect("valid https endpoint should be accepted");
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("example.com"));
    }

    #[test]
    fn rejects_cloud_metadata_ip() {
        let err = validate_webhook_target_url("http://169.254.169.254/latest/meta-data/", true)
            .expect_err("cloud metadata IP must be blocked");
        assert!(matches!(err, WebhookValidationError::InvalidTargetUrl(_)));

        let err = validate_webhook_target_url("https://169.254.169.254/latest/meta-data/", false)
            .expect_err("cloud metadata IP must be blocked even over https");
        assert!(matches!(err, WebhookValidationError::InvalidTargetUrl(_)));
    }

    #[test]
    fn rejects_private_lan_ips() {
        for target in [
            "https://10.0.0.1/hook",
            "https://172.16.0.1/hook",
            "https://172.31.255.255/hook",
            "https://192.168.1.1/hook",
            "https://127.0.0.1/hook",
            "https://0.0.0.0/hook",
        ] {
            let err = validate_webhook_target_url(target, false)
                .unwrap_or_else(|_| panic!("{target} must be blocked"));
            let _ = err;
        }
    }

    #[test]
    fn rejects_private_lan_ips_returns_error() {
        for target in [
            "https://10.0.0.1/hook",
            "https://172.16.0.1/hook",
            "https://192.168.1.1/hook",
            "https://127.0.0.1/hook",
            "https://[::1]/hook",
            "https://[fc00::1]/hook",
            "https://[fe80::1]/hook",
            "https://[::]/hook",
        ] {
            assert!(
                validate_webhook_target_url(target, false).is_err(),
                "{target} must be blocked"
            );
        }
    }

    #[test]
    fn rejects_localhost_hostname() {
        assert!(validate_webhook_target_url("https://localhost/hook", false).is_err());
        assert!(validate_webhook_target_url("https://LOCALHOST/hook", false).is_err());
        assert!(validate_webhook_target_url("https://foo.localhost/hook", false).is_err());
    }

    #[test]
    fn rejects_non_http_schemes() {
        for target in [
            "file:///etc/passwd",
            "ftp://example.com/hook",
            "gopher://example.com/hook",
            "javascript:alert(1)",
        ] {
            assert!(
                validate_webhook_target_url(target, false).is_err(),
                "{target} must be rejected"
            );
        }
    }

    #[test]
    fn rejects_http_in_production() {
        assert!(validate_webhook_target_url("http://example.com/hook", false).is_err());
    }

    #[test]
    fn allows_http_in_dev_mode() {
        assert!(validate_webhook_target_url("http://example.com/hook", true).is_ok());
    }

    #[test]
    fn dev_mode_still_blocks_private_ips() {
        assert!(validate_webhook_target_url("http://127.0.0.1/hook", true).is_err());
        assert!(validate_webhook_target_url("http://169.254.169.254/hook", true).is_err());
    }

    #[test]
    fn rejects_ipv4_mapped_ipv6_loopback() {
        assert!(validate_webhook_target_url("https://[::ffff:127.0.0.1]/hook", false).is_err());
    }
}
