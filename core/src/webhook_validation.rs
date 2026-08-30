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
    Extension,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

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

    async fn from_request(mut req: Request, state: &S) -> Result<Self, Self::Rejection> {
        // 1. Resolve the shared secret: check Extensions, fallback to environment variable.
        let secret = if let Some(InboundWebhookSecret(s)) = req.extensions().get::<InboundWebhookSecret>().cloned() {
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

/// Verify an HMAC-SHA256 signature.
pub fn verify_signature(secret: &str, timestamp: &str, body: &[u8], signature: &str) -> bool {
    let hex_sig = match signature.strip_prefix("sha256=") {
        Some(h) => h,
        None => return false,
    };

    let sig_bytes = match hex::decode(hex_sig) {
        Ok(b) => b,
        Err(_) => return false,
    };

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts keys of any size");
    mac.update(timestamp.as_bytes());
    mac.update(b".");
    mac.update(body);

    mac.verify_slice(&sig_bytes).is_ok()
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
                format!(
                    "Request timestamp expired (max skew: {MAX_TIMESTAMP_SKEW_SECS}s)"
                ),
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
    use axum::{body::Body, http::Request, routing::post, Router};
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
            .route("/test", post(|ValidatedWebhook(body): ValidatedWebhook| async move {
                assert_eq!(body, BODY);
                StatusCode::OK
            }))
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
            .route("/test", post(|ValidatedWebhook(_): ValidatedWebhook| async move {
                StatusCode::OK
            }))
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
            .route("/test", post(|ValidatedWebhook(_): ValidatedWebhook| async move {
                StatusCode::OK
            }))
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
}
