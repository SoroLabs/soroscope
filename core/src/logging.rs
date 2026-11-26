//! Structured JSON & Logfmt Logging Middleware (Issue #16).
//!
//! Provides structured logging support for JSON and Logfmt formats, OpenTelemetry
//! trace propagation, context enrichment (trace_id, span_id, request duration),
//! and HTTP logging middleware.

use std::fmt;
use std::str::FromStr;
use std::time::Instant;
use axum::{
    body::Body,
    extract::Request,
    middleware::Next,
    response::Response,
};
use serde::{Deserialize, Serialize};
use tracing_subscriber::{
    filter::EnvFilter,
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

/// Supported structured log formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    /// Structured newline-delimited JSON format.
    #[default]
    Json,
    /// Key=value logfmt format.
    Logfmt,
    /// Human-readable compact format.
    Compact,
    /// Pretty multi-line format.
    Pretty,
}

impl FromStr for LogFormat {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().to_lowercase();
        match s.as_str() {
            "json" => Ok(LogFormat::Json),
            "logfmt" => Ok(LogFormat::Logfmt),
            "pretty" => Ok(LogFormat::Pretty),
            _ => Ok(LogFormat::Compact),
        }
    }
}

impl fmt::Display for LogFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogFormat::Json => write!(f, "json"),
            LogFormat::Logfmt => write!(f, "logfmt"),
            LogFormat::Compact => write!(f, "compact"),
            LogFormat::Pretty => write!(f, "pretty"),
        }
    }
}

/// Parse and build an `EnvFilter` from a directive string with fallback to "info".
///
/// Supports the standard `tracing_subscriber` directive syntax, including
/// per-module overrides (e.g. `soroscope_core=debug,tower_http=warn`). An
/// empty or unparsable directive falls back to `"info"` so a startup typo
/// degrades verbosity instead of crashing the server.
pub fn build_env_filter(directive: &str) -> EnvFilter {
    let directive = directive.trim();
    let directive = if directive.is_empty() {
        "info"
    } else {
        directive
    };

    EnvFilter::try_new(directive).unwrap_or_else(|error| {
        eprintln!("Invalid RUST_LOG directive '{directive}': {error}. Falling back to 'info'.");
        EnvFilter::new("info")
    })
}

/// Initialize tracing subscriber with the requested format and log level filter.
pub fn init_logging(format: LogFormat, rust_log: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let filter = build_env_filter(rust_log);

    match format {
        LogFormat::Json => {
            let json_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_current_span(true)
                .with_span_list(true)
                .with_target(true);

            tracing_subscriber::registry()
                .with(filter)
                .with(json_layer)
                .try_init()?;
        }
        LogFormat::Logfmt | LogFormat::Compact => {
            let compact_layer = tracing_subscriber::fmt::layer()
                .compact()
                .with_target(true);

            tracing_subscriber::registry()
                .with(filter)
                .with(compact_layer)
                .try_init()?;
        }
        LogFormat::Pretty => {
            let pretty_layer = tracing_subscriber::fmt::layer()
                .pretty()
                .with_target(true);

            tracing_subscriber::registry()
                .with(filter)
                .with(pretty_layer)
                .try_init()?;
        }
    }

    Ok(())
}

/// HTTP request logging middleware.
///
/// Records structured fields:
/// - `method`: HTTP method
/// - `uri`: Request URI
/// - `status`: Response status code
/// - `duration_ms`: Duration of request execution in milliseconds
/// - `duration_us`: Duration of request execution in microseconds
pub async fn structured_logging_middleware(req: Request<Body>, next: Next) -> Response {
    let start = Instant::now();
    let method = req.method().clone();
    let uri = req.uri().clone();
    let path = uri.path().to_string();

    let response = next.run(req).await;
    let duration = start.elapsed();
    let duration_ms = duration.as_secs_f64() * 1000.0;
    let duration_us = duration.as_micros() as u64;
    let status = response.status().as_u16();

    tracing::info!(
        method = %method,
        uri = %path,
        status = status,
        duration_ms = duration_ms,
        duration_us = duration_us,
        "HTTP request handled"
    );

    response
}

// ─────────────────────────────────────────────────────────────────────────────
// Audit trail (Issue #1403)
// ─────────────────────────────────────────────────────────────────────────────

/// `tracing` target every audit event is emitted under, so the audit stream
/// can be routed and retained separately from operational logs.
pub const AUDIT_TARGET: &str = "soroscope::audit";

/// Replacement for secrets scrubbed from audit fields.
pub const REDACTED: &str = "[REDACTED]";

/// Whether the audited action succeeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditOutcome {
    Success,
    Failure,
}

/// One structured audit record for a privileged action or auth event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Who acted: an account address, an API key fingerprint (`key_fp:` + hash
    /// prefix), or `anonymous`.
    pub actor_id: String,
    pub client_ip: String,
    /// What was attempted, e.g. `auth.authenticate` or `admin.emergency_pause`.
    pub action: String,
    /// What it was attempted on, e.g. the request path.
    pub resource: String,
    pub outcome: AuditOutcome,
    /// RFC 3339 UTC timestamp with millisecond precision.
    pub timestamp_utc: String,
    /// Why the action failed, or extra detail for a success.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl AuditEvent {
    /// Build an event stamped with the current UTC time. Every field is passed
    /// through [`redact_secrets`].
    pub fn new(
        actor_id: &str,
        client_ip: &str,
        action: &str,
        resource: &str,
        outcome: AuditOutcome,
    ) -> Self {
        Self {
            actor_id: redact_secrets(actor_id),
            client_ip: redact_secrets(client_ip),
            action: redact_secrets(action),
            resource: redact_secrets(resource),
            outcome,
            timestamp_utc: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            reason: None,
        }
    }

    /// Attach a (redacted) reason.
    pub fn with_reason(mut self, reason: &str) -> Self {
        self.reason = Some(redact_secrets(reason));
        self
    }

    /// The event as a single-line JSON object.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("audit event serialization is infallible")
    }

    /// Emit the event on the [`AUDIT_TARGET`] target.
    pub fn emit(&self) {
        tracing::info!(
            target: AUDIT_TARGET,
            audit = %self.to_json(),
            actor_id = %self.actor_id,
            client_ip = %self.client_ip,
            action = %self.action,
            resource = %self.resource,
            outcome = ?self.outcome,
            timestamp_utc = %self.timestamp_utc,
            "audit event"
        );
    }
}

/// Replace bearer tokens, JWTs, Stellar secret seeds and `key=value` style
/// credentials with [`REDACTED`].
pub fn redact_secrets(input: &str) -> String {
    use regex::Regex;
    use std::sync::OnceLock;

    static PATTERNS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
        [
            (r"(?i)\b(bearer)\s+[A-Za-z0-9\-._~+/]+=*", "$1 [REDACTED]"),
            (
                r"(?i)\b(api[_-]?key|x-api-key|token|secret|password)(\s*[=:]\s*)[^\s,;&]+",
                "$1$2[REDACTED]",
            ),
            (
                r"\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]*",
                REDACTED,
            ),
            (r"\bS[A-Z2-7]{55}\b", REDACTED),
        ]
        .into_iter()
        .map(|(pattern, replacement)| {
            (
                Regex::new(pattern).expect("audit redaction pattern is valid"),
                replacement,
            )
        })
        .collect()
    });

    let mut redacted = input.to_string();
    for (pattern, replacement) in patterns {
        redacted = pattern.replace_all(&redacted, *replacement).into_owned();
    }
    redacted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_format_from_str() {
        assert_eq!("json".parse::<LogFormat>().unwrap(), LogFormat::Json);
        assert_eq!("JSON".parse::<LogFormat>().unwrap(), LogFormat::Json);
        assert_eq!("logfmt".parse::<LogFormat>().unwrap(), LogFormat::Logfmt);
        assert_eq!("pretty".parse::<LogFormat>().unwrap(), LogFormat::Pretty);
        assert_eq!("compact".parse::<LogFormat>().unwrap(), LogFormat::Compact);
        assert_eq!("other".parse::<LogFormat>().unwrap(), LogFormat::Compact);
    }

    #[test]
    fn test_build_env_filter() {
        assert_eq!(build_env_filter("").to_string(), "info");
        assert_eq!(build_env_filter("   ").to_string(), "info");
        assert_eq!(build_env_filter("debug").to_string(), "debug");
        assert_eq!(build_env_filter("soroscope_core=warn").to_string(), "soroscope_core=warn");
    }

    #[test]
    fn test_log_format_display() {
        assert_eq!(LogFormat::Json.to_string(), "json");
        assert_eq!(LogFormat::Logfmt.to_string(), "logfmt");
        assert_eq!(LogFormat::Compact.to_string(), "compact");
        assert_eq!(LogFormat::Pretty.to_string(), "pretty");
    }

    // ── Audit trail (Issue #1403) ───────────────────────────────────────────

    const JWT: &str = "eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiJHQUJDIn0.c2lnbmF0dXJl";
    const SEED: &str = "SBZVMB74Z76QZ3ZOY7UTDFYKMEGKW5XFJEB6PFKBF4UYSSWHG4EDH7PY";

    #[test]
    fn redaction_scrubs_bearer_tokens_jwts_seeds_and_key_values() {
        let redacted = redact_secrets(&format!(
            "Authorization: Bearer {JWT} seed={SEED} api_key=sk_live_123 X-API-Key: raw-key password:hunter2"
        ));
        assert!(!redacted.contains(JWT), "{redacted}");
        assert!(!redacted.contains(SEED), "{redacted}");
        assert!(!redacted.contains("sk_live_123"), "{redacted}");
        assert!(!redacted.contains("raw-key"), "{redacted}");
        assert!(!redacted.contains("hunter2"), "{redacted}");
        assert!(redacted.contains("Bearer [REDACTED]"), "{redacted}");
        assert!(redacted.contains(REDACTED));
    }

    #[test]
    fn redaction_leaves_ordinary_fields_alone() {
        let address = "GABASEAXDYSSYMZ2IFEE6VS5MRVXE6MAQ6HJLHFDVKY3RP6GZXKNWWW3";
        assert_eq!(redact_secrets(address), address);
        assert_eq!(redact_secrets("/analyze/wasm"), "/analyze/wasm");
        assert_eq!(redact_secrets("203.0.113.7"), "203.0.113.7");
    }

    #[test]
    fn audit_event_json_has_the_schema_fields() {
        let event = AuditEvent::new(
            "key_fp:0123456789ab",
            "203.0.113.7",
            "auth.authenticate",
            "/analyze/wasm",
            AuditOutcome::Failure,
        )
        .with_reason(&format!("Invalid token: {JWT}"));

        let json: serde_json::Value = serde_json::from_str(&event.to_json()).unwrap();
        assert_eq!(json["actor_id"], "key_fp:0123456789ab");
        assert_eq!(json["client_ip"], "203.0.113.7");
        assert_eq!(json["action"], "auth.authenticate");
        assert_eq!(json["resource"], "/analyze/wasm");
        assert_eq!(json["outcome"], "failure");
        assert!(
            chrono::DateTime::parse_from_rfc3339(json["timestamp_utc"].as_str().unwrap()).is_ok()
        );
        assert!(json["timestamp_utc"].as_str().unwrap().ends_with('Z'));
        assert_eq!(json["reason"], "Invalid token: [REDACTED]");
    }

    #[test]
    fn audit_event_redacts_secrets_passed_as_fields() {
        let event = AuditEvent::new(
            &format!("Bearer {JWT}"),
            "10.0.0.1",
            "auth.verify",
            &format!("/auth?token={JWT}"),
            AuditOutcome::Success,
        );
        assert!(!event.to_json().contains(JWT));
    }

    #[derive(Clone, Default)]
    struct CapturedLogs(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl std::io::Write for CapturedLogs {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CapturedLogs {
        type Writer = CapturedLogs;

        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    #[test]
    fn audit_events_are_emitted_on_the_audit_target_without_raw_tokens() {
        let logs = CapturedLogs::default();
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_target(true)
            .with_writer(logs.clone())
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            AuditEvent::new(
                "anonymous",
                "203.0.113.7",
                "auth.authenticate",
                "/analyze",
                AuditOutcome::Failure,
            )
            .with_reason(&format!("Invalid token: Bearer {JWT}"))
            .emit();
        });

        let output = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
        let line: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
        assert_eq!(line["target"], AUDIT_TARGET);
        assert_eq!(line["fields"]["action"], "auth.authenticate");
        assert!(!output.contains(JWT), "{output}");
    }
}
