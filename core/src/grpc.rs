//! gRPC Protocol Adapter for High-Throughput Event Streaming (Issue #583).
//!
//! Exposes `EventStreamService` — a server-side streaming gRPC service built
//! on [`tonic`] that pushes Soroban contract events over HTTP/2 with Protobuf
//! framing.  This eliminates the latency and serialization overhead of HTTP
//! JSON polling for enterprise indexing consumers.
//!
//! # Architecture
//!
//! ```text
//!  ┌─────────────────────┐       gRPC / HTTP-2        ┌──────────────────┐
//!  │  Enterprise Indexer  │ ◄────────────────────────  │ EventStreamService│
//!  │  (tonic client / any │       Protobuf frames       │                  │
//!  │   gRPC client)       │                             │  SimulationBus   │
//!  └─────────────────────┘                             │  (broadcast rx)  │
//!                                                      └──────────────────┘
//! ```
//!
//! The service subscribes to the existing [`crate::ws::SimulationBus`]
//! broadcast channel.  Inbound `Completed` events are translated into
//! [`ContractEvent`] Protobuf messages and pushed to connected subscribers.
//!
//! # Running the gRPC server
//!
//! The gRPC endpoint is bound on a separate port (default `50051`) so it does
//! not conflict with the existing HTTP/REST server:
//!
//! ```bash
//! GRPC_PORT=50051 RUST_LOG=info cargo run -p soroscope-core
//! ```
//!
//! # Client example (grpcurl)
//!
//! ```bash
//! grpcurl -plaintext -d '{"contract_id":"CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC"}' \
//!   localhost:50051 soroscope.events.v1.EventStreamService/StreamContractEvents
//! ```

use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};
use tonic::{Request, Response, Status};

use crate::ws::{SimulationBus, SimulationEvent};

// Include the tonic-generated code from the compiled proto.
pub mod proto {
    tonic::include_proto!("soroscope.events.v1");
}

use proto::event_stream_service_server::EventStreamService;
pub use proto::event_stream_service_server::EventStreamServiceServer;
use proto::{ContractEvent, StreamContractEventsRequest};

// ── Service implementation ────────────────────────────────────────────────────

/// gRPC service implementation.  Holds a reference to the shared
/// [`SimulationBus`] so it can fan out events to every connected gRPC client.
pub struct EventStreamServiceImpl {
    bus: Arc<SimulationBus>,
}

impl EventStreamServiceImpl {
    /// Create a new service instance backed by the given bus.
    pub fn new(bus: Arc<SimulationBus>) -> Self {
        Self { bus }
    }
}

/// Type alias for the streaming response used by the trait impl.
type EventStream = Pin<Box<dyn Stream<Item = Result<ContractEvent, Status>> + Send + 'static>>;

#[tonic::async_trait]
impl EventStreamService for EventStreamServiceImpl {
    type StreamContractEventsStream = EventStream;

    /// Open a server-side stream of [`ContractEvent`] messages.
    ///
    /// The server subscribes to the internal broadcast bus and translates each
    /// [`SimulationEvent::Completed`] into a [`ContractEvent`].  The stream
    /// remains open until the client disconnects or the server shuts down.
    async fn stream_contract_events(
        &self,
        request: Request<StreamContractEventsRequest>,
    ) -> Result<Response<Self::StreamContractEventsStream>, Status> {
        let params = request.into_inner();
        let contract_id_filter = params.contract_id.clone();
        let event_types_filter: Vec<String> = params
            .event_types_filter
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();

        tracing::info!(
            contract_id = %contract_id_filter,
            start_ledger = params.start_ledger,
            event_types = ?event_types_filter,
            "gRPC client connected to EventStreamService"
        );

        let receiver: broadcast::Receiver<SimulationEvent> = self.bus.subscribe();
        let bus_stream = BroadcastStream::new(receiver);

        let contract_filter = contract_id_filter.clone();
        let output = bus_stream
            // Drop lagged-error items — slow gRPC consumers skip missed events
            // rather than crashing the stream.
            .filter_map(move |item| {
                let cf = contract_filter.clone();
                let etf = event_types_filter.clone();
                async move {
                    match item {
                        Err(_lagged) => {
                            tracing::warn!("gRPC stream lagged; some events were dropped");
                            None
                        }
                        Ok(event) => translate_event(event, &cf, &etf),
                    }
                }
            })
            .map(Ok);

        Ok(Response::new(Box::pin(output)))
    }
}

// ── Translation helpers ───────────────────────────────────────────────────────

/// Convert a [`SimulationEvent`] to a [`ContractEvent`] Protobuf message.
///
/// Returns `None` when the event does not match the caller's filters or is not
/// a contract-event-bearing variant.
fn translate_event(
    event: SimulationEvent,
    contract_id_filter: &str,
    event_types_filter: &[String],
) -> Option<ContractEvent> {
    // Only `Completed` events carry contract resource data that maps to an
    // indexable contract event.  Other bus events (progress, failover, etc.)
    // are internal telemetry and are not exposed over gRPC.
    let (job_id, data, timestamp) = match event {
        SimulationEvent::Completed {
            job_id,
            data,
            timestamp,
        } => (job_id, data, timestamp),
        _ => return None,
    };

    // Apply contract ID filter — empty filter means "all contracts".
    if !contract_id_filter.is_empty() && !job_id.contains(contract_id_filter) {
        return None;
    }

    let event_type = "simulation_completed".to_string();

    // Apply event-type filter — empty filter means "all types".
    if !event_types_filter.is_empty()
        && !event_types_filter
            .iter()
            .any(|f| event_type.contains(f.as_str()))
    {
        return None;
    }

    let topics_json = serde_json::json!([
        {"type": "symbol", "value": "simulation"},
        {"type": "symbol", "value": &event_type}
    ])
    .to_string();

    let value_json = serde_json::json!({
        "cpu_instructions": data.cpu_instructions,
        "ram_bytes": data.ram_bytes,
        "ledger_read_bytes": data.ledger_read_bytes,
        "ledger_write_bytes": data.ledger_write_bytes,
        "transaction_size_bytes": data.transaction_size_bytes,
        "cost_stroops": data.cost_stroops,
    })
    .to_string();

    Some(ContractEvent {
        ledger_sequence: 0, // populated by the indexer pipeline in future
        ledger_close_time: timestamp.timestamp(),
        contract_id: job_id,
        event_type,
        topics_json,
        value_json,
        tx_hash: String::new(),
    })
}

// ── Server startup helper ─────────────────────────────────────────────────────

/// Bind and serve the gRPC endpoint on `addr`.
///
/// This is called from `main` and runs in a separate Tokio task alongside the
/// existing Axum HTTP server.
pub async fn serve(addr: std::net::SocketAddr, bus: Arc<SimulationBus>) {
    let svc = EventStreamServiceServer::new(EventStreamServiceImpl::new(bus));

    tracing::info!(grpc_addr = %addr, "gRPC EventStreamService listening");

    if let Err(e) = tonic::transport::Server::builder()
        .add_service(svc)
        .serve(addr)
        .await
    {
        tracing::error!(error = %e, "gRPC server terminated with error");
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ws::{CompletedPayload, SimulationEvent};
    use chrono::Utc;

    fn make_completed_event(job_id: &str) -> SimulationEvent {
        SimulationEvent::Completed {
            job_id: job_id.to_string(),
            data: CompletedPayload {
                cpu_instructions: 1_000_000,
                ram_bytes: 512_000,
                ledger_read_bytes: 1024,
                ledger_write_bytes: 256,
                transaction_size_bytes: 400,
                cost_stroops: 500,
            },
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn translate_completed_event_no_filter() {
        let event = make_completed_event("contract-abc");
        let result = translate_event(event, "", &[]);
        assert!(result.is_some());
        let ce = result.unwrap();
        assert_eq!(ce.contract_id, "contract-abc");
        assert_eq!(ce.event_type, "simulation_completed");
        assert!(ce.topics_json.contains("simulation"));
        assert!(ce.value_json.contains("cpu_instructions"));
    }

    #[test]
    fn translate_completed_event_matching_filter() {
        let event = make_completed_event("contract-xyz");
        let result = translate_event(event, "contract-xyz", &[]);
        assert!(result.is_some());
    }

    #[test]
    fn translate_completed_event_non_matching_filter() {
        let event = make_completed_event("contract-abc");
        let result = translate_event(event, "contract-xyz", &[]);
        assert!(result.is_none());
    }

    #[test]
    fn translate_progress_event_is_filtered_out() {
        use crate::ws::ProgressPayload;
        let event = SimulationEvent::Progress {
            job_id: "job-1".to_string(),
            data: ProgressPayload {
                percent: 50,
                message: "halfway".to_string(),
            },
            timestamp: Utc::now(),
        };
        let result = translate_event(event, "", &[]);
        assert!(result.is_none(), "Progress events should not be streamed");
    }

    #[test]
    fn translate_event_type_filter_matches() {
        let event = make_completed_event("contract-abc");
        let filter = vec!["simulation_completed".to_string()];
        let result = translate_event(event, "", &filter);
        assert!(result.is_some());
    }

    #[test]
    fn translate_event_type_filter_no_match() {
        let event = make_completed_event("contract-abc");
        let filter = vec!["transfer".to_string()];
        let result = translate_event(event, "", &filter);
        assert!(result.is_none());
    }
}
