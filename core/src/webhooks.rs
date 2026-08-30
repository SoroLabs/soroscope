//! Asynchronous, signed webhook delivery for contract event subscriptions.
//!
//! Event ingestion can enqueue a [`ContractEvent`] without waiting for an
//! external endpoint. The worker fans the event out to matching subscriptions,
//! signs each request, and retries transient failures with bounded exponential
//! backoff.

use crate::contract_registry::{verify_bytecode, ContractBytecodeSource, ContractRegister};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use std::{collections::HashMap, sync::Arc, time::Duration};
use thiserror::Error;
use reqwest::{Client, StatusCode};
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
        if !(callback_url.starts_with("http://") || callback_url.starts_with("https://")) {
            return Err(WebhookError::InvalidSubscription(
                "callback_url must use http or https".into(),
            ));
        }
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
    DeliveryFailed { attempts: u32, reason: String },
    #[error("event rejected by contract bytecode verification: {0}")]
    BytecodeUnverified(String),
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
        Self::start_with_client(registry, config, Client::new())
    }

    fn start_with_client(
        registry: SubscriptionRegistry,
        config: WebhookConfig,
        client: Client,
    ) -> Self {
        let (tx, mut rx) = mpsc::channel(config.queue_capacity);
        let task = tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                for subscription in registry.matching(&event).await {
                    if let Err(error) = deliver(&client, &config, &subscription, &event).await {
                        tracing::error!(
                            subscription_id = %subscription.id,
                            event_id = %event.id,
                            error = %error,
                            "contract event webhook delivery exhausted retries"
                        );
                    }
                }
            }
        });

        Self {
            sender: WebhookSender { tx },
            task,
        }
    }
}

async fn deliver(
    client: &Client,
    config: &WebhookConfig,
    subscription: &ContractSubscription,
    event: &ContractEvent,
) -> Result<(), WebhookError> {
    let delivery_id = Uuid::new_v4();
    let body = serde_json::to_vec(&DeliveryPayload {
        delivery_id,
        subscription_id: subscription.id,
        event,
    })?;
    let mut last_error = "delivery was not attempted".to_string();
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
                if !is_retryable(status) {
                    break;
                }
            }
            Err(error) => last_error = error.to_string(),
        }

        if attempt < config.max_attempts {
            tokio::time::sleep(config.retry_delay(attempt)).await;
        }
    }

    Err(WebhookError::DeliveryFailed {
        attempts,
        reason: last_error,
    })
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
    fn rejects_weak_subscription_configuration() {
        let result = ContractSubscription::new(
            "",
            Vec::new(),
            "https://example.com",
            "short",
        );
        assert!(matches!(result, Err(WebhookError::InvalidSubscription(_))));
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
}
