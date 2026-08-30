//! WebSocket streaming for simulation progress (Issue #105).
//!
//! # Design
//!
//! A lightweight pub/sub bus ([`SimulationBus`]) wraps a Tokio
//! [`broadcast`] channel.  Any part of the application that holds an
//! [`Arc<SimulationBus>`] can publish [`SimulationEvent`]s.  The
//! [`JobWorker`](crate::jobs::JobWorker) is the primary publisher; the
//! WebSocket upgrade handler ([`ws_handler`]) is the primary consumer.
//!
//! ## Client protocol
//!
//! Connect with:
//! ```
//! GET /ws/jobs/<job_id>
//! Upgrade: websocket
//! ```
//!
//! The server streams newline-delimited JSON frames until the job
//! reaches a terminal state (`completed` / `failed` / `cancelled`), at
//! which point it sends the final event and closes the connection.
//!
//! ### Event shape
//! ```json
//! {
//!   "event": "progress",
//!   "job_id": "550e8400-e29b-41d4-a716-446655440000",
//!   "data": { "percent": 30, "message": "Running simulation" },
//!   "timestamp": "2026-04-25T12:00:00Z"
//! }
//! ```
//!
//! `event` can be: `progress` | `provider_failover` | `consensus_check`
//! | `completed` | `failed`.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::IntoResponse,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::jobs::JobId;

// ── Channel capacity ─────────────────────────────────────────────────────────

/// Number of events that can be buffered per broadcast channel slot before
/// slow consumers are forced to drop events via `RecvError::Lagged`.
const BUS_CAPACITY: usize = 256;

// ── Event types ──────────────────────────────────────────────────────────────

/// Progress update emitted at each stage of job execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressPayload {
    /// Completion percentage (0–100).
    pub percent: i32,
    /// Human-readable status message.
    pub message: String,
}

/// Emitted when the engine fails over to a different RPC provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderFailoverPayload {
    /// Provider that failed / was tripped.
    pub from_provider: String,
    /// Provider now being used.
    pub to_provider: String,
    /// Reason for the failover (e.g. "timeout", "http_error").
    pub reason: String,
}

/// Emitted once per consensus quorum check (only in `consensus` mode).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusCheckPayload {
    /// Whether all sampled providers agreed on resources + ledger changes.
    pub agreement: bool,
    /// Providers that were queried.
    pub providers: Vec<String>,
    /// Optional human-readable mismatch summary.
    pub detail: Option<String>,
}

/// Emitted when a job finishes successfully — carries the full resource report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedPayload {
    pub cpu_instructions: u64,
    pub ram_bytes: u64,
    pub ledger_read_bytes: u64,
    pub ledger_write_bytes: u64,
    pub transaction_size_bytes: u64,
    pub cost_stroops: u64,
}

/// Emitted when a job fails.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedPayload {
    pub error: String,
    pub error_type: String,
}

/// All events that can be published on the [`SimulationBus`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SimulationEvent {
    Progress {
        job_id: String,
        data: ProgressPayload,
        timestamp: DateTime<Utc>,
    },
    ProviderFailover {
        job_id: String,
        data: ProviderFailoverPayload,
        timestamp: DateTime<Utc>,
    },
    ConsensusCheck {
        job_id: String,
        data: ConsensusCheckPayload,
        timestamp: DateTime<Utc>,
    },
    Completed {
        job_id: String,
        data: CompletedPayload,
        timestamp: DateTime<Utc>,
    },
    Failed {
        job_id: String,
        data: FailedPayload,
        timestamp: DateTime<Utc>,
    },
}

impl SimulationEvent {
    /// Returns the `job_id` string embedded in every event variant.
    pub fn job_id(&self) -> &str {
        match self {
            Self::Progress { job_id, .. } => job_id,
            Self::ProviderFailover { job_id, .. } => job_id,
            Self::ConsensusCheck { job_id, .. } => job_id,
            Self::Completed { job_id, .. } => job_id,
            Self::Failed { job_id, .. } => job_id,
        }
    }

    /// Returns `true` if this event signals the end of a job.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed { .. } | Self::Failed { .. })
    }
}

// ── Bus ──────────────────────────────────────────────────────────────────────

/// Application-wide pub/sub bus for simulation events.
///
/// Clone the bus cheaply via [`Arc`]; call [`SimulationBus::publish`] from any
/// async context and [`SimulationBus::subscribe`] to get a receiver.
#[derive(Clone)]
pub struct SimulationBus {
    sender: broadcast::Sender<SimulationEvent>,
}

impl SimulationBus {
    /// Create a new bus with the default channel capacity.
    pub fn new() -> Arc<Self> {
        let (sender, _) = broadcast::channel(BUS_CAPACITY);
        Arc::new(Self { sender })
    }

    /// Publish an event.  Returns the number of active subscribers that
    /// received it (0 if nobody is listening, which is perfectly fine).
    pub fn publish(&self, event: SimulationEvent) -> usize {
        self.sender.send(event).unwrap_or(0)
    }

    /// Subscribe to the bus.  The returned receiver will lag (and skip events)
    /// if it cannot keep up with the publication rate.
    pub fn subscribe(&self) -> broadcast::Receiver<SimulationEvent> {
        self.sender.subscribe()
    }

    // ── Convenience constructors ─────────────────────────────────────────

    pub fn progress(job_id: &JobId, percent: i32, message: impl Into<String>) -> SimulationEvent {
        SimulationEvent::Progress {
            job_id: job_id.to_string(),
            data: ProgressPayload {
                percent,
                message: message.into(),
            },
            timestamp: Utc::now(),
        }
    }

    pub fn provider_failover(
        job_id: &JobId,
        from_provider: impl Into<String>,
        to_provider: impl Into<String>,
        reason: impl Into<String>,
    ) -> SimulationEvent {
        SimulationEvent::ProviderFailover {
            job_id: job_id.to_string(),
            data: ProviderFailoverPayload {
                from_provider: from_provider.into(),
                to_provider: to_provider.into(),
                reason: reason.into(),
            },
            timestamp: Utc::now(),
        }
    }

    pub fn consensus_check(
        job_id: &JobId,
        agreement: bool,
        providers: Vec<String>,
        detail: Option<String>,
    ) -> SimulationEvent {
        SimulationEvent::ConsensusCheck {
            job_id: job_id.to_string(),
            data: ConsensusCheckPayload {
                agreement,
                providers,
                detail,
            },
            timestamp: Utc::now(),
        }
    }

    pub fn completed(
        job_id: &JobId,
        resources: &crate::simulation::SorobanResources,
        cost_stroops: u64,
    ) -> SimulationEvent {
        SimulationEvent::Completed {
            job_id: job_id.to_string(),
            data: CompletedPayload {
                cpu_instructions: resources.cpu_instructions,
                ram_bytes: resources.ram_bytes,
                ledger_read_bytes: resources.ledger_read_bytes,
                ledger_write_bytes: resources.ledger_write_bytes,
                transaction_size_bytes: resources.transaction_size_bytes,
                cost_stroops,
            },
            timestamp: Utc::now(),
        }
    }

    pub fn failed(
        job_id: &JobId,
        error: impl Into<String>,
        error_type: impl Into<String>,
    ) -> SimulationEvent {
        SimulationEvent::Failed {
            job_id: job_id.to_string(),
            data: FailedPayload {
                error: error.into(),
                error_type: error_type.into(),
            },
            timestamp: Utc::now(),
        }
    }
}

impl Default for SimulationBus {
    fn default() -> Self {
        let (sender, _) = broadcast::channel(BUS_CAPACITY);
        Self { sender }
    }
}

// ── Axum extractor alias ─────────────────────────────────────────────────────

/// Shared state slice required by the WebSocket handler.
/// The handler accesses the bus through the main [`AppState`](crate::AppState).
pub struct WsState {
    pub bus: Arc<SimulationBus>,
}

// ── WebSocket handler ─────────────────────────────────────────────────────────

/// Upgrade handler for `GET /ws/jobs/:job_id`.
///
/// Clients connect with a standard WebSocket handshake; the server streams
/// JSON-serialised [`SimulationEvent`]s until the job reaches a terminal state
/// or the client disconnects.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(job_id): Path<String>,
    State(state): State<Arc<crate::AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, job_id, state))
}

async fn handle_socket(mut socket: WebSocket, job_id: String, state: Arc<crate::AppState>) {
    tracing::info!(job_id = %job_id, "WebSocket client connected");

    let mut rx = state.simulation_bus.subscribe();

    loop {
        tokio::select! {
            // Receive next event from the bus
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        // Only forward events belonging to the requested job
                        if event.job_id() != job_id {
                            continue;
                        }

                        let is_terminal = event.is_terminal();

                        let json = match serde_json::to_string(&event) {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::error!(
                                    job_id = %job_id,
                                    error = %e,
                                    "Failed to serialise SimulationEvent"
                                );
                                continue;
                            }
                        };

                        if socket.send(Message::Text(json)).await.is_err() {
                            // Client disconnected
                            break;
                        }

                        if is_terminal {
                            // Close gracefully after the terminal event
                            let _ = socket.send(Message::Close(None)).await;
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(
                            job_id = %job_id,
                            skipped = n,
                            "WebSocket consumer lagged — events were skipped"
                        );
                        // Keep going; the client just missed some progress ticks
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        // Bus was dropped — server shutting down
                        break;
                    }
                }
            }

            // Echo / ping handling: consume incoming messages from the client
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Ping(payload))) => {
                        let _ = socket.send(Message::Pong(payload)).await;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {} // ignore text/binary frames from the client
                }
            }
        }
    }

    tracing::info!(job_id = %job_id, "WebSocket client disconnected");
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bus_publish_and_receive() {
        let bus = SimulationBus::new();
        let mut rx = bus.subscribe();

        let fake_id = JobId::new();
        let event = SimulationBus::progress(&fake_id, 42, "halfway there");
        bus.publish(event);

        let received = rx.recv().await.expect("should receive event");
        assert_eq!(received.job_id(), fake_id.to_string());
        assert!(!received.is_terminal());
    }

    #[tokio::test]
    async fn terminal_events_are_identified_correctly() {
        let fake_id = JobId::new();

        let progress = SimulationBus::progress(&fake_id, 50, "running");
        assert!(!progress.is_terminal());

        let failed = SimulationBus::failed(&fake_id, "oops", "NetworkError");
        assert!(failed.is_terminal());

        let resources = crate::simulation::SorobanResources {
            cpu_instructions: 1000,
            ram_bytes: 2048,
            ledger_read_bytes: 128,
            ledger_write_bytes: 64,
            transaction_size_bytes: 512,
        };
        let completed = SimulationBus::completed(&fake_id, &resources, 500);
        assert!(completed.is_terminal());
    }

    #[tokio::test]
    async fn event_json_round_trips() {
        let fake_id = JobId::new();
        let event =
            SimulationBus::provider_failover(&fake_id, "primary-node", "backup-node", "timeout");
        let json = serde_json::to_string(&event).expect("serialise");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");
        assert_eq!(parsed["event"], "provider_failover");
        assert_eq!(parsed["data"]["from_provider"], "primary-node");
    }

    #[tokio::test]
    async fn consensus_check_event_serialises() {
        let fake_id = JobId::new();
        let event = SimulationBus::consensus_check(
            &fake_id,
            true,
            vec![
                "node-a".to_string(),
                "node-b".to_string(),
                "node-c".to_string(),
            ],
            None,
        );
        let json = serde_json::to_string(&event).expect("serialise");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");
        assert_eq!(parsed["event"], "consensus_check");
        assert_eq!(parsed["data"]["agreement"], true);
    }

    #[tokio::test]
    async fn no_subscribers_does_not_panic() {
        let bus = SimulationBus::new();
        let fake_id = JobId::new();
        // publish with zero subscribers — should silently return 0
        let n = bus.publish(SimulationBus::progress(&fake_id, 10, "start"));
        assert_eq!(n, 0);
    }
}
