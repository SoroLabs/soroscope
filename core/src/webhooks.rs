//! Asynchronous, signed webhook delivery for contract event subscriptions.
//!
//! Event ingestion can enqueue a [`ContractEvent`] without waiting for an
//! external endpoint. The worker fans the event out to matching subscriptions,
//! signs each request, and retries transient failures with bounded exponential
//! backoff.
//!
//! Deliveries that exhaust their retries are persisted in a dead-letter queue
//! ([`WebhookDlq`], `webhook_dlq` table) with full diagnostics, and can be
//! inspected and replayed (#1399).

use crate::contract_registry::{verify_bytecode, ContractBytecodeSource, ContractRegister};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
use thiserror::Error;
use tokio::{
    sync::{mpsc, RwLock},
    task::JoinHandle,
};
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

pub const SIGNATURE_HEADER: &str = "x-soroscope-signature";
pub const DELIVERY_HEADER: &str = "x-soroscope-delivery";
pub const TIMESTAMP_HEADER: &str = "x-soroscope-timestamp";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ContractSubscription {
    pub id: Uuid,
    pub contract_id: String,
    /// Empty means every event emitted by the contract.
    pub event_types: Vec<String>,
    pub callback_url: String,
    /// Kept server-side and never serialized into delivery payloads.
    #[serde(skip_serializing)]
    pub signing_secret: String,
    pub active: bool,
}

impl ContractSubscription {
    pub fn new(
        contract_id: impl Into<String>,
        event_types: Vec<String>,
        callback_url: impl Into<String>,
        signing_secret: impl Into<String>,
    ) -> Result<Self, WebhookError> {
        let contract_id = contract_id.into();
        let callback_url = callback_url.into();
        let signing_secret = signing_secret.into();
        if contract_id.trim().is_empty() {
            return Err(WebhookError::InvalidSubscription(
                "contract_id cannot be empty".into(),
            ));
        }
        crate::webhook_validation::validate_webhook_target_url(
            &callback_url,
            cfg!(debug_assertions),
        )
        .map_err(|error| WebhookError::InvalidSubscription(error.to_string()))?;
        if signing_secret.len() < 32 {
            return Err(WebhookError::InvalidSubscription(
                "signing_secret must contain at least 32 bytes".into(),
            ));
        }

        Ok(Self {
            id: Uuid::new_v4(),
            contract_id,
            event_types,
            callback_url,
            signing_secret,
            active: true,
        })
    }

    fn matches(&self, event: &ContractEvent) -> bool {
        self.active
            && self.contract_id == event.contract_id
            && (self.event_types.is_empty()
                || self
                    .event_types
                    .iter()
                    .any(|kind| kind == &event.event_type))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ContractEvent {
    pub id: Uuid,
    pub contract_id: String,
    pub event_type: String,
    pub ledger: u32,
    pub payload: Value,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
struct DeliveryPayload<'a> {
    delivery_id: Uuid,
    subscription_id: Uuid,
    event: &'a ContractEvent,
}

#[derive(Clone, Default)]
pub struct SubscriptionRegistry {
    subscriptions: Arc<RwLock<HashMap<Uuid, ContractSubscription>>>,
}

impl SubscriptionRegistry {
    pub async fn insert(&self, subscription: ContractSubscription) -> Uuid {
        let id = subscription.id;
        self.subscriptions.write().await.insert(id, subscription);
        id
    }

    pub async fn remove(&self, id: Uuid) -> Option<ContractSubscription> {
        self.subscriptions.write().await.remove(&id)
    }

    pub async fn get(&self, id: Uuid) -> Option<ContractSubscription> {
        self.subscriptions.read().await.get(&id).cloned()
    }

    pub async fn matching(&self, event: &ContractEvent) -> Vec<ContractSubscription> {
        self.subscriptions
            .read()
            .await
            .values()
            .filter(|subscription| subscription.matches(event))
            .cloned()
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct WebhookConfig {
    pub queue_capacity: usize,
    pub request_timeout: Duration,
    pub max_attempts: u32,
    pub retry_base: Duration,
    pub retry_max: Duration,
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            queue_capacity: 1_024,
            request_timeout: Duration::from_secs(10),
            max_attempts: 5,
            retry_base: Duration::from_secs(1),
            retry_max: Duration::from_secs(60),
        }
    }
}

impl WebhookConfig {
    fn retry_delay(&self, completed_attempts: u32) -> Duration {
        let exponent = completed_attempts.saturating_sub(1).min(31);
        self.retry_base
            .saturating_mul(2_u32.saturating_pow(exponent))
            .min(self.retry_max)
    }
}

#[derive(Debug, Error)]
pub enum WebhookError {
    #[error("invalid webhook subscription: {0}")]
    InvalidSubscription(String),
    #[error("webhook queue is closed")]
    QueueClosed,
    #[error("failed to serialize webhook payload: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("webhook delivery failed after {attempts} attempts: {reason}")]
    DeliveryFailed {
        attempts: u32,
        reason: String,
        /// HTTP status of the last attempt, if the endpoint answered at all.
        last_status: Option<u16>,
    },
    #[error("event rejected by contract bytecode verification: {0}")]
    BytecodeUnverified(String),
    #[error("webhook dead-letter queue storage error: {0}")]
    Storage(String),
    #[error("dead-letter entry {0} not found")]
    DlqEntryNotFound(Uuid),
    #[error("subscription {0} no longer exists")]
    SubscriptionNotFound(Uuid),
}

impl From<rusqlite::Error> for WebhookError {
    fn from(error: rusqlite::Error) -> Self {
        WebhookError::Storage(error.to_string())
    }
}

#[derive(Clone)]
pub struct WebhookSender {
    tx: mpsc::Sender<ContractEvent>,
}

impl WebhookSender {
    pub async fn enqueue(&self, event: ContractEvent) -> Result<(), WebhookError> {
        self.tx
            .send(event)
            .await
            .map_err(|_| WebhookError::QueueClosed)
    }
}

/// Entry point for the event ingestion pipeline: verifies a contract event's
/// live WASM bytecode hash against the whitelisted [`ContractRegister`]
/// before allowing it into the dispatch queue, so a proxy contract spoofing
/// a verified `contract_id` cannot have its events indexed.
#[derive(Clone)]
pub struct EventIngestor<S: ContractBytecodeSource + Clone> {
    registry: ContractRegister,
    source: S,
    sender: WebhookSender,
}

impl<S: ContractBytecodeSource + Clone> EventIngestor<S> {
    pub fn new(registry: ContractRegister, source: S, sender: WebhookSender) -> Self {
        Self {
            registry,
            source,
            sender,
        }
    }

    pub async fn ingest(&self, event: ContractEvent) -> Result<(), WebhookError> {
        verify_bytecode(&self.source, &self.registry, &event.contract_id)
            .await
            .map_err(|e| WebhookError::BytecodeUnverified(e.to_string()))?;
        self.sender.enqueue(event).await
    }
}

pub struct WebhookWorker {
    pub sender: WebhookSender,
    pub task: JoinHandle<()>,
}

impl WebhookWorker {
    pub fn start(registry: SubscriptionRegistry, config: WebhookConfig) -> Self {
        Self::start_with_client(registry, config, Client::new(), None)
    }

    /// Like [`Self::start`], but deliveries that exhaust their retries are
    /// stored in `dlq` instead of being dropped (#1399).
    pub fn start_with_dlq(
        registry: SubscriptionRegistry,
        config: WebhookConfig,
        dlq: WebhookDlq,
    ) -> Self {
        Self::start_with_client(registry, config, Client::new(), Some(dlq))
        Self::start_with_client(registry, config, build_outbound_client(&config))
    }

    fn start_with_client(
        registry: SubscriptionRegistry,
        config: WebhookConfig,
        client: Client,
        dlq: Option<WebhookDlq>,
    ) -> Self {
        let (tx, mut rx) = mpsc::channel(config.queue_capacity);
        let task = tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                for subscription in registry.matching(&event).await {
                    deliver_or_dead_letter(&client, &config, &subscription, &event, dlq.as_ref())
                        .await;
                }
            }
        });

        Self {
            sender: WebhookSender { tx },
            task,
        }
    }
}

/// Deliver `event` to `subscription`; if retries are exhausted, log it and
/// persist it in `dlq` when one is configured.
async fn deliver_or_dead_letter(
    client: &Client,
    config: &WebhookConfig,
    subscription: &ContractSubscription,
    event: &ContractEvent,
    dlq: Option<&WebhookDlq>,
) {
    let delivery_id = Uuid::new_v4();
    let Err(error) = deliver_with_id(client, config, subscription, event, delivery_id).await else {
        return;
    };
    tracing::error!(
        subscription_id = %subscription.id,
        event_id = %event.id,
        error = %error,
        "contract event webhook delivery exhausted retries"
    );

    let (
        Some(dlq),
        WebhookError::DeliveryFailed {
            attempts,
            reason,
            last_status,
        },
    ) = (dlq, error)
    else {
        return;
    };
    let payload = match serde_json::to_value(DeliveryPayload {
        delivery_id,
        subscription_id: subscription.id,
        event,
    }) {
        Ok(payload) => payload,
        Err(error) => {
            tracing::error!(delivery_id = %delivery_id, error = %error, "webhook DLQ payload not serialisable");
            return;
        }
    };
    let entry = DlqEntry {
        id: delivery_id,
        subscription_id: subscription.id,
        event_id: event.id,
        target_url: subscription.callback_url.clone(),
        payload,
        last_status,
        error: reason,
        attempts,
        failed_at: Utc::now(),
    };
    if let Err(error) = dlq.insert(&entry) {
        tracing::error!(delivery_id = %delivery_id, error = %error, "failed to store webhook in DLQ");
    }
/// Builds the HTTP client used for outbound webhook deliveries.
///
/// Redirects are never followed: an attacker-controlled endpoint could
/// otherwise answer with a `302` pointing at an internal address (for example
/// the cloud metadata service at `169.254.169.254`) and bypass the
/// registration-time SSRF checks.
fn build_outbound_client(config: &WebhookConfig) -> Client {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(config.request_timeout)
        .build()
        .expect("failed to build webhook HTTP client")
}

async fn deliver(
    client: &Client,
    config: &WebhookConfig,
    subscription: &ContractSubscription,
    event: &ContractEvent,
) -> Result<(), WebhookError> {
    deliver_with_id(client, config, subscription, event, Uuid::new_v4()).await
}

async fn deliver_with_id(
    client: &Client,
    config: &WebhookConfig,
    subscription: &ContractSubscription,
    event: &ContractEvent,
    delivery_id: Uuid,
) -> Result<(), WebhookError> {
    let body = serde_json::to_vec(&DeliveryPayload {
        delivery_id,
        subscription_id: subscription.id,
        event,
    })?;
    let mut last_error = "delivery was not attempted".to_string();
    let mut last_status = None;
    let mut attempts = 0;

    for attempt in 1..=config.max_attempts.max(1) {
        attempts = attempt;
        let timestamp = Utc::now().timestamp().to_string();
        let signature = sign(&subscription.signing_secret, &timestamp, &body);
        let response = client
            .post(subscription.callback_url.clone())
            .header("content-type", "application/json")
            .header(DELIVERY_HEADER, delivery_id.to_string())
            .header(TIMESTAMP_HEADER, &timestamp)
            .header(SIGNATURE_HEADER, format!("sha256={signature}"))
            .timeout(config.request_timeout)
            .body(body.clone())
            .send()
            .await;

        match response {
            Ok(response) if response.status().is_success() => return Ok(()),
            Ok(response) => {
                let status = response.status();
                last_error = format!("HTTP {status}");
                last_status = Some(status.as_u16());
                if !is_retryable(status) {
                    break;
                }
            }
            Err(error) => {
                last_error = error.to_string();
                last_status = None;
            }
        }

        if attempt < config.max_attempts {
            tokio::time::sleep(config.retry_delay(attempt)).await;
        }
    }

    Err(WebhookError::DeliveryFailed {
        attempts,
        reason: last_error,
        last_status,
    })
}

// ─── Dead-letter queue (#1399) ───────────────────────────────────────────────

/// A webhook delivery that exhausted its retries.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct DlqEntry {
    /// The original delivery id, reused when the entry is replayed.
    pub id: Uuid,
    pub subscription_id: Uuid,
    pub event_id: Uuid,
    pub target_url: String,
    /// The delivery body that was sent.
    pub payload: Value,
    /// HTTP status of the last attempt; `None` when the endpoint never answered.
    pub last_status: Option<u16>,
    pub error: String,
    pub attempts: u32,
    pub failed_at: DateTime<Utc>,
}

/// Result of replaying a dead-letter entry.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ReplayOutcome {
    pub id: Uuid,
    /// HTTP status returned by the endpoint, if it answered.
    pub status: Option<u16>,
    /// True when the endpoint answered 2xx and the entry was removed.
    pub delivered: bool,
}

/// SQLite-backed store for deliveries that exhausted their retries.
///
/// Signing secrets are never stored here: a replay re-signs with the
/// subscription's current secret.
#[derive(Clone)]
pub struct WebhookDlq {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl WebhookDlq {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, WebhookError> {
        Self::init(rusqlite::Connection::open(path)?)
    }

    pub fn in_memory() -> Result<Self, WebhookError> {
        Self::init(rusqlite::Connection::open_in_memory()?)
    }

    fn init(conn: rusqlite::Connection) -> Result<Self, WebhookError> {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS webhook_dlq (
                id TEXT PRIMARY KEY,
                subscription_id TEXT NOT NULL,
                event_id TEXT NOT NULL,
                target_url TEXT NOT NULL,
                payload TEXT NOT NULL,
                last_status INTEGER,
                error TEXT NOT NULL,
                attempts INTEGER NOT NULL,
                failed_at TEXT NOT NULL
            );
            ",
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, rusqlite::Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn insert(&self, entry: &DlqEntry) -> Result<(), WebhookError> {
        self.conn().execute(
            "INSERT OR REPLACE INTO webhook_dlq
                (id, subscription_id, event_id, target_url, payload, last_status, error, attempts, failed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                entry.id.to_string(),
                entry.subscription_id.to_string(),
                entry.event_id.to_string(),
                entry.target_url,
                serde_json::to_string(&entry.payload)?,
                entry.last_status,
                entry.error,
                entry.attempts,
                entry.failed_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Every entry, oldest failure first.
    pub fn list(&self) -> Result<Vec<DlqEntry>, WebhookError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, subscription_id, event_id, target_url, payload, last_status, error, attempts, failed_at
             FROM webhook_dlq ORDER BY failed_at",
        )?;
        let rows = stmt.query_map([], Self::row_to_entry)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(WebhookError::from)
    }

    pub fn get(&self, id: Uuid) -> Result<Option<DlqEntry>, WebhookError> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, subscription_id, event_id, target_url, payload, last_status, error, attempts, failed_at
             FROM webhook_dlq WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map([id.to_string()], Self::row_to_entry)?;
        rows.next().transpose().map_err(WebhookError::from)
    }

    /// Remove an entry; returns whether it existed.
    pub fn remove(&self, id: Uuid) -> Result<bool, WebhookError> {
        let removed = self
            .conn()
            .execute("DELETE FROM webhook_dlq WHERE id = ?1", [id.to_string()])?;
        Ok(removed > 0)
    }

    fn row_to_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<DlqEntry> {
        fn invalid(
            index: usize,
            error: impl std::error::Error + Send + Sync + 'static,
        ) -> rusqlite::Error {
            rusqlite::Error::FromSqlConversionFailure(
                index,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        }
        let uuid = |index: usize| -> rusqlite::Result<Uuid> {
            Uuid::parse_str(&row.get::<_, String>(index)?).map_err(|e| invalid(index, e))
        };
        Ok(DlqEntry {
            id: uuid(0)?,
            subscription_id: uuid(1)?,
            event_id: uuid(2)?,
            target_url: row.get(3)?,
            payload: serde_json::from_str(&row.get::<_, String>(4)?).map_err(|e| invalid(4, e))?,
            last_status: row.get(5)?,
            error: row.get(6)?,
            attempts: row.get(7)?,
            failed_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                .map_err(|e| invalid(8, e))?
                .with_timezone(&Utc),
        })
    }

    /// Re-send a dead-lettered delivery with a fresh timestamp and HMAC-SHA256
    /// signature. A 2xx answer removes the entry; anything else keeps it with
    /// the new status, error and attempt count.
    pub async fn replay(
        &self,
        id: Uuid,
        registry: &SubscriptionRegistry,
        client: &Client,
        config: &WebhookConfig,
    ) -> Result<ReplayOutcome, WebhookError> {
        let mut entry = self.get(id)?.ok_or(WebhookError::DlqEntryNotFound(id))?;
        let subscription = registry
            .get(entry.subscription_id)
            .await
            .ok_or(WebhookError::SubscriptionNotFound(entry.subscription_id))?;

        let body = serde_json::to_vec(&entry.payload)?;
        let timestamp = Utc::now().timestamp().to_string();
        let signature = crate::webhook_validation::generate_signature(
            &subscription.signing_secret,
            &timestamp,
            &body,
        );
        let response = client
            .post(subscription.callback_url.clone())
            .header("content-type", "application/json")
            .header(DELIVERY_HEADER, entry.id.to_string())
            .header(TIMESTAMP_HEADER, &timestamp)
            .header(SIGNATURE_HEADER, signature)
            .timeout(config.request_timeout)
            .body(body)
            .send()
            .await;

        entry.attempts = entry.attempts.saturating_add(1);
        match response {
            Ok(response) if response.status().is_success() => {
                self.remove(id)?;
                return Ok(ReplayOutcome {
                    id,
                    status: Some(response.status().as_u16()),
                    delivered: true,
                });
            }
            Ok(response) => {
                let status = response.status();
                entry.last_status = Some(status.as_u16());
                entry.error = format!("HTTP {status}");
            }
            Err(error) => {
                entry.last_status = None;
                entry.error = error.to_string();
            }
        }
        entry.failed_at = Utc::now();
        self.insert(&entry)?;
        Ok(ReplayOutcome {
            id,
            status: entry.last_status,
            delivered: false,
        })
    }
}

fn is_retryable(status: StatusCode) -> bool {
    status.is_server_error()
        || status == StatusCode::REQUEST_TIMEOUT
        || status == StatusCode::TOO_MANY_REQUESTS
}

pub fn sign(secret: &str, timestamp: &str, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts signing keys of any size");
    mac.update(timestamp.as_bytes());
    mac.update(b".");
    mac.update(body);
    hex::encode(mac.finalize().into_bytes())
}

pub fn verify(secret: &str, timestamp: &str, body: &[u8], signature: &str) -> bool {
    let Some(signature) = signature.strip_prefix("sha256=") else {
        return false;
    };
    let Ok(signature) = hex::decode(signature) else {
        return false;
    };
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts signing keys of any size");
    mac.update(timestamp.as_bytes());
    mac.update(b".");
    mac.update(body);
    mac.verify_slice(&signature).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Bytes, extract::State, http::HeaderMap, routing::post, Router};
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn event(event_type: &str) -> ContractEvent {
        ContractEvent {
            id: Uuid::nil(),
            contract_id: "CABC".into(),
            event_type: event_type.into(),
            ledger: 123,
            payload: json!({"amount": "42"}),
            occurred_at: Utc::now(),
        }
    }

    #[derive(Clone)]
    struct FakeBytecodeSource {
        wasm_by_contract: std::collections::HashMap<String, Vec<u8>>,
    }

    impl ContractBytecodeSource for FakeBytecodeSource {
        async fn fetch_wasm(&self, contract_id: &str) -> Result<Vec<u8>, String> {
            self.wasm_by_contract
                .get(contract_id)
                .cloned()
                .ok_or_else(|| format!("no wasm registered for {contract_id}"))
        }
    }

    fn hash_hex(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::digest(bytes))
    }

    #[tokio::test]
    async fn ingestor_enqueues_events_whose_bytecode_matches_the_whitelist() {
        let wasm = b"verified-wasm".to_vec();
        let source = FakeBytecodeSource {
            wasm_by_contract: std::collections::HashMap::from([("CABC".to_string(), wasm.clone())]),
        };
        let registry = ContractRegister::new(std::collections::HashMap::from([(
            "CABC".to_string(),
            hash_hex(&wasm),
        )]));
        let (tx, mut rx) = mpsc::channel(1);
        let ingestor = EventIngestor::new(registry, source, WebhookSender { tx });

        ingestor.ingest(event("transfer")).await.unwrap();
        assert!(rx.recv().await.is_some());
    }

    #[tokio::test]
    async fn ingestor_rejects_events_from_an_unwhitelisted_or_spoofed_contract() {
        let source = FakeBytecodeSource {
            wasm_by_contract: std::collections::HashMap::from([(
                "CABC".to_string(),
                b"proxy-wasm".to_vec(),
            )]),
        };
        let registry = ContractRegister::new(std::collections::HashMap::from([(
            "CABC".to_string(),
            hash_hex(b"real-wasm"),
        )]));
        let (tx, mut rx) = mpsc::channel(1);
        let ingestor = EventIngestor::new(registry, source, WebhookSender { tx });

        let err = ingestor.ingest(event("transfer")).await.unwrap_err();
        assert!(matches!(err, WebhookError::BytecodeUnverified(_)));
        rx.close();
        assert!(rx.recv().await.is_none());
    }

    #[test]
    fn signature_round_trip_detects_tampering() {
        let secret = "a-secret-that-is-at-least-thirty-two-bytes";
        let body = br#"{"event":"transfer"}"#;
        let signature = format!("sha256={}", sign(secret, "1700000000", body));

        assert!(verify(secret, "1700000000", body, &signature));
        assert!(!verify(secret, "1700000001", body, &signature));
        assert!(!verify(secret, "1700000000", b"changed", &signature));
    }

    #[tokio::test]
    async fn registry_filters_by_contract_event_and_active_state() {
        let registry = SubscriptionRegistry::default();
        let transfer = ContractSubscription::new(
            "CABC",
            vec!["transfer".into()],
            "https://example.com/events",
            "a-secret-that-is-at-least-thirty-two-bytes",
        )
        .unwrap();
        let mut inactive = ContractSubscription::new(
            "CABC",
            Vec::new(),
            "https://example.com/all",
            "another-secret-that-is-at-least-32-bytes",
        )
        .unwrap();
        inactive.active = false;
        registry.insert(transfer.clone()).await;
        registry.insert(inactive).await;

        assert_eq!(registry.matching(&event("transfer")).await, vec![transfer]);
        assert!(registry.matching(&event("mint")).await.is_empty());
    }

    #[test]
    fn exponential_backoff_is_capped() {
        let config = WebhookConfig {
            retry_base: Duration::from_secs(2),
            retry_max: Duration::from_secs(10),
            ..WebhookConfig::default()
        };
        assert_eq!(config.retry_delay(1), Duration::from_secs(2));
        assert_eq!(config.retry_delay(2), Duration::from_secs(4));
        assert_eq!(config.retry_delay(3), Duration::from_secs(8));
        assert_eq!(config.retry_delay(4), Duration::from_secs(10));
    }

    #[test]
    fn exponential_backoff_sequence_matches_specification() {
        let config = WebhookConfig::default();
        assert_eq!(config.retry_delay(1), Duration::from_secs(1));
        assert_eq!(config.retry_delay(2), Duration::from_secs(2));
        assert_eq!(config.retry_delay(3), Duration::from_secs(4));
        assert_eq!(config.retry_delay(4), Duration::from_secs(8));
        assert_eq!(config.retry_delay(5), Duration::from_secs(16));
    }

    #[test]
    fn rejects_weak_subscription_configuration() {
        let result = ContractSubscription::new("", Vec::new(), "https://example.com", "short");
        assert!(matches!(result, Err(WebhookError::InvalidSubscription(_))));
    }

    #[test]
    fn rejects_ssrf_callback_urls_at_registration() {
        let secret = "a-secret-that-is-at-least-thirty-two-bytes";
        let blocked = [
            "http://127.0.0.1:8080/admin",
            "http://169.254.169.254/latest/meta-data/",
            "http://10.0.0.5/hook",
            "http://172.16.0.1/hook",
            "http://192.168.1.10/hook",
            "http://localhost/hook",
            "http://[::1]/hook",
            "file:///etc/passwd",
        ];
        for url in blocked {
            let result = ContractSubscription::new("CABC", Vec::new(), url, secret);
            assert!(
                matches!(result, Err(WebhookError::InvalidSubscription(_))),
                "expected {url} to be rejected"
            );
        }
    }

    #[test]
    fn accepts_public_https_callback_url() {
        let subscription = ContractSubscription::new(
            "CABC",
            Vec::new(),
            "https://example.com/events",
            "a-secret-that-is-at-least-thirty-two-bytes",
        )
        .unwrap();
        assert_eq!(subscription.callback_url, "https://example.com/events");
    }

    #[test]
    fn outbound_client_does_not_follow_redirects() {
        let client = build_outbound_client(&WebhookConfig::default());
        assert_eq!(client.redirect_policy(), &reqwest::redirect::Policy::none());
    }

    #[tokio::test]
    async fn transient_failure_is_retried_with_a_valid_signature() {
        #[derive(Clone)]
        struct TestState {
            attempts: Arc<AtomicUsize>,
            secret: Arc<String>,
        }

        async fn receiver(
            State(state): State<TestState>,
            headers: HeaderMap,
            body: Bytes,
        ) -> StatusCode {
            let timestamp = headers
                .get(TIMESTAMP_HEADER)
                .and_then(|value| value.to_str().ok())
                .unwrap();
            let signature = headers
                .get(SIGNATURE_HEADER)
                .and_then(|value| value.to_str().ok())
                .unwrap();
            assert!(verify(&state.secret, timestamp, &body, signature));
            assert!(headers.get(DELIVERY_HEADER).is_some());

            if state.attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                StatusCode::SERVICE_UNAVAILABLE
            } else {
                StatusCode::NO_CONTENT
            }
        }

        let attempts = Arc::new(AtomicUsize::new(0));
        let secret = Arc::new("a-secret-that-is-at-least-thirty-two-bytes".to_string());
        let state = TestState {
            attempts: attempts.clone(),
            secret: secret.clone(),
        };
        let app = Router::new()
            .route("/events", post(receiver))
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let subscription = ContractSubscription::new(
            "CABC",
            Vec::new(),
            format!("http://{address}/events"),
            secret.as_str(),
        )
        .unwrap();
        let config = WebhookConfig {
            max_attempts: 3,
            retry_base: Duration::from_millis(1),
            retry_max: Duration::from_millis(1),
            ..WebhookConfig::default()
        };

        deliver(&Client::new(), &config, &subscription, &event("transfer"))
            .await
            .unwrap();
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        server.abort();
    }

    // ── Dead-letter queue (#1399) ────────────────────────────────────────────

    const SECRET: &str = "a-secret-that-is-at-least-thirty-two-bytes";

    #[derive(Clone)]
    struct ReceiverState {
        status: StatusCode,
        hits: Arc<AtomicUsize>,
        signatures_valid: Arc<AtomicUsize>,
    }

    /// An endpoint answering `status`, counting requests with a valid signature.
    async fn spawn_receiver(status: StatusCode) -> (String, ReceiverState, JoinHandle<()>) {
        async fn receiver(
            State(state): State<ReceiverState>,
            headers: HeaderMap,
            body: Bytes,
        ) -> StatusCode {
            state.hits.fetch_add(1, Ordering::SeqCst);
            let timestamp = headers.get(TIMESTAMP_HEADER).and_then(|v| v.to_str().ok());
            let signature = headers.get(SIGNATURE_HEADER).and_then(|v| v.to_str().ok());
            if let (Some(timestamp), Some(signature)) = (timestamp, signature) {
                if verify(SECRET, timestamp, &body, signature) {
                    state.signatures_valid.fetch_add(1, Ordering::SeqCst);
                }
            }
            state.status
        }

        let state = ReceiverState {
            status,
            hits: Arc::new(AtomicUsize::new(0)),
            signatures_valid: Arc::new(AtomicUsize::new(0)),
        };
        let app = Router::new()
            .route("/events", post(receiver))
            .with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (format!("http://{address}/events"), state, server)
    }

    fn fast_retries(max_attempts: u32) -> WebhookConfig {
        WebhookConfig {
            max_attempts,
            retry_base: Duration::from_millis(1),
            retry_max: Duration::from_millis(1),
            ..WebhookConfig::default()
        }
    }

    #[tokio::test]
    async fn webhooks_exhausted_delivery_is_stored_in_dlq() {
        let (url, receiver, server) = spawn_receiver(StatusCode::SERVICE_UNAVAILABLE).await;
        let subscription =
            ContractSubscription::new("CABC", Vec::new(), url.clone(), SECRET).unwrap();
        let dlq = WebhookDlq::in_memory().unwrap();
        let event = event("transfer");

        deliver_or_dead_letter(
            &Client::new(),
            &fast_retries(3),
            &subscription,
            &event,
            Some(&dlq),
        )
        .await;

        assert_eq!(receiver.hits.load(Ordering::SeqCst), 3);
        let entries = dlq.list().unwrap();
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.subscription_id, subscription.id);
        assert_eq!(entry.event_id, event.id);
        assert_eq!(entry.target_url, url);
        assert_eq!(entry.last_status, Some(503));
        assert_eq!(entry.attempts, 3);
        assert!(entry.error.contains("503"));
        assert_eq!(entry.payload["event"]["event_type"], "transfer");
        server.abort();
    }

    #[tokio::test]
    async fn webhooks_unreachable_endpoint_is_dead_lettered_without_status() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let subscription = ContractSubscription::new(
            "CABC",
            Vec::new(),
            format!("http://{address}/events"),
            SECRET,
        )
        .unwrap();
        let dlq = WebhookDlq::in_memory().unwrap();

        deliver_or_dead_letter(
            &Client::new(),
            &fast_retries(2),
            &subscription,
            &event("transfer"),
            Some(&dlq),
        )
        .await;

        let entries = dlq.list().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].last_status, None);
        assert_eq!(entries[0].attempts, 2);
    }

    #[tokio::test]
    async fn webhooks_successful_delivery_is_not_dead_lettered() {
        let (url, _receiver, server) = spawn_receiver(StatusCode::NO_CONTENT).await;
        let subscription = ContractSubscription::new("CABC", Vec::new(), url, SECRET).unwrap();
        let dlq = WebhookDlq::in_memory().unwrap();

        deliver_or_dead_letter(
            &Client::new(),
            &fast_retries(3),
            &subscription,
            &event("transfer"),
            Some(&dlq),
        )
        .await;

        assert!(dlq.list().unwrap().is_empty());
        server.abort();
    }

    /// Dead-letter one delivery to `url` and return the entry id.
    async fn dead_letter_one(
        registry: &SubscriptionRegistry,
        dlq: &WebhookDlq,
        url: String,
    ) -> Uuid {
        let subscription = ContractSubscription::new("CABC", Vec::new(), url, SECRET).unwrap();
        registry.insert(subscription.clone()).await;
        let (failing_url, _receiver, server) = spawn_receiver(StatusCode::BAD_GATEWAY).await;
        let mut failing = subscription.clone();
        failing.callback_url = failing_url;
        deliver_or_dead_letter(
            &Client::new(),
            &fast_retries(1),
            &failing,
            &event("transfer"),
            Some(dlq),
        )
        .await;
        server.abort();

        // Replays go to the subscription's registered callback, `url`.
        dlq.list().unwrap().pop().unwrap().id
    }

    #[tokio::test]
    async fn webhooks_replay_resigns_dispatches_and_removes_on_2xx() {
        let (url, receiver, server) = spawn_receiver(StatusCode::OK).await;
        let registry = SubscriptionRegistry::default();
        let dlq = WebhookDlq::in_memory().unwrap();
        let id = dead_letter_one(&registry, &dlq, url).await;

        let outcome = dlq
            .replay(id, &registry, &Client::new(), &WebhookConfig::default())
            .await
            .unwrap();

        assert!(outcome.delivered);
        assert_eq!(outcome.status, Some(200));
        assert_eq!(receiver.hits.load(Ordering::SeqCst), 1);
        assert_eq!(receiver.signatures_valid.load(Ordering::SeqCst), 1);
        assert!(dlq.get(id).unwrap().is_none());
        server.abort();
    }

    #[tokio::test]
    async fn webhooks_failed_replay_keeps_entry_with_updated_diagnostics() {
        let (url, _receiver, server) = spawn_receiver(StatusCode::INTERNAL_SERVER_ERROR).await;
        let registry = SubscriptionRegistry::default();
        let dlq = WebhookDlq::in_memory().unwrap();
        let id = dead_letter_one(&registry, &dlq, url).await;

        let outcome = dlq
            .replay(id, &registry, &Client::new(), &WebhookConfig::default())
            .await
            .unwrap();

        assert!(!outcome.delivered);
        assert_eq!(outcome.status, Some(500));
        let entry = dlq.get(id).unwrap().unwrap();
        assert_eq!(entry.last_status, Some(500));
        assert_eq!(entry.attempts, 2);
        server.abort();
    }

    #[tokio::test]
    async fn webhooks_replay_of_unknown_entry_fails() {
        let dlq = WebhookDlq::in_memory().unwrap();
        let id = Uuid::new_v4();

        let error = dlq
            .replay(
                id,
                &SubscriptionRegistry::default(),
                &Client::new(),
                &WebhookConfig::default(),
            )
            .await
            .unwrap_err();

        assert!(matches!(error, WebhookError::DlqEntryNotFound(missing) if missing == id));
    }
}
