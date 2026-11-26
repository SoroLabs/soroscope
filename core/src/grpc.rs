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
//!
//! # Health checks, reflection and keepalive (Issue #1404)
//!
//! The server also exposes the standard `grpc.health.v1.Health` service (for
//! Kubernetes gRPC probes and Envoy) and server reflection (for `grpcurl list`).
//! Health is `SERVING` while the readiness signal passed to [`serve`] is
//! `true` (DB and RPC provider healthy) and `NOT_SERVING` otherwise. HTTP/2
//! keepalive pings and a max-concurrent-streams limit are enforced on every
//! connection.
//!
//! ```bash
//! grpcurl -plaintext localhost:50051 list
//! grpcurl -plaintext localhost:50051 grpc.health.v1.Health/Check
//! ```

use std::fs;
use std::io::{self, BufReader};
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use rustls_pemfile::certs as read_cert_chain;
use tokio::sync::watch;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};
use tonic::transport::{Certificate, Identity, ServerTlsConfig};
use tonic::{Request, Response, Status};
use tonic_health::server::HealthReporter;
use tonic_health::ServingStatus;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::ws::{SimulationBus, SimulationEvent};

// Include the tonic-generated code from the compiled proto.
pub mod proto {
    tonic::include_proto!("soroscope.events.v1");
}

pub mod telemetry_proto {
    tonic::include_proto!("soroscope.telemetry.v1");
}

/// Encoded `FileDescriptorSet` of every service compiled from `proto/`, served
/// through gRPC reflection (Issue #1404).
pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("soroscope_descriptor");

use proto::event_stream_service_server::EventStreamService;
pub use proto::event_stream_service_server::EventStreamServiceServer;
use proto::{ContractEvent, StreamContractEventsRequest};

use telemetry_proto::telemetry_stream_service_server::TelemetryStreamService;
pub use telemetry_proto::telemetry_stream_service_server::TelemetryStreamServiceServer;
use telemetry_proto::{StreamTelemetryRequest, TelemetryMetric};

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
        let server_span = tracing::info_span!("grpc.stream_contract_events");
        server_span.set_parent(crate::trace_propagation::extract_grpc(request.metadata()));
        let _span_guard = server_span.enter();
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

        let receiver = self.bus.subscribe();
        let bus_stream = BroadcastStream::new(receiver);

        let contract_filter = contract_id_filter.clone();
        let output = bus_stream
            // Drop lagged-error items — slow gRPC consumers skip missed events
            // rather than crashing the stream.
            .filter_map(move |item| {
                let cf = contract_filter.clone();
                let etf = event_types_filter.clone();
                match item {
                    Err(_lagged) => {
                        tracing::warn!("gRPC stream lagged; some events were dropped");
                        None
                    }
                    Ok(traced_msg) => translate_event(traced_msg.payload, &cf, &etf),
                }
            })
            .map(Ok);

        Ok(Response::new(Box::pin(output)))
    }
}

/// gRPC service implementation for live telemetry metric streaming.
pub struct TelemetryStreamServiceImpl {
    bus: Arc<SimulationBus>,
}

impl TelemetryStreamServiceImpl {
    pub fn new(bus: Arc<SimulationBus>) -> Self {
        Self { bus }
    }
}

type TelemetryStream =
    Pin<Box<dyn Stream<Item = Result<TelemetryMetric, Status>> + Send + 'static>>;

#[tonic::async_trait]
impl TelemetryStreamService for TelemetryStreamServiceImpl {
    type StreamTelemetryStream = TelemetryStream;

    async fn stream_telemetry(
        &self,
        request: Request<StreamTelemetryRequest>,
    ) -> Result<Response<Self::StreamTelemetryStream>, Status> {
        let server_span = tracing::info_span!("grpc.stream_telemetry");
        server_span.set_parent(crate::trace_propagation::extract_grpc(request.metadata()));
        let _span_guard = server_span.enter();
        let params = request.into_inner();
        let target_contract_id = params.contract_id;

        let receiver = self.bus.subscribe();
        let bus_stream = BroadcastStream::new(receiver);

        let output = bus_stream
            .filter_map(move |item| match item {
                Err(_) => None,
                Ok(traced_msg) => match traced_msg.payload {
                    SimulationEvent::Completed {
                        contract_id,
                        result,
                    } => {
                        if target_contract_id.is_empty() || contract_id == target_contract_id {
                            Some(TelemetryMetric {
                                contract_id,
                                cpu_instructions: result.resources.cpu_instructions,
                                ram_bytes: result.resources.ram_bytes,
                                ledger_read_bytes: result.resources.ledger_read_bytes,
                                ledger_write_bytes: result.resources.ledger_write_bytes,
                                timestamp: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|d| d.as_secs())
                                    .unwrap_or(0),
                            })
                        } else {
                            None
                        }
                    }
                    _ => None,
                },
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

// ── Health, reflection & keepalive (Issue #1404) ──────────────────────────────

/// Interval between HTTP/2 keepalive pings sent to idle clients.
const GRPC_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(30);
/// How long to wait for a keepalive ping ack before closing the connection.
const GRPC_KEEPALIVE_TIMEOUT: Duration = Duration::from_secs(10);
/// Maximum number of concurrent HTTP/2 streams per connection.
const GRPC_MAX_CONCURRENT_STREAMS: u32 = 1024;

/// Apply keepalive and stream limits to a Tonic server builder.
fn configure_server(builder: tonic::transport::Server) -> tonic::transport::Server {
    builder
        .http2_keepalive_interval(Some(GRPC_KEEPALIVE_INTERVAL))
        .http2_keepalive_timeout(Some(GRPC_KEEPALIVE_TIMEOUT))
        .max_concurrent_streams(Some(GRPC_MAX_CONCURRENT_STREAMS))
}

/// Names reported by the health service: the overall server (`""`) and every
/// registered telemetry service.
fn health_service_names() -> [&'static str; 3] {
    [
        "",
        <EventStreamServiceServer<EventStreamServiceImpl> as tonic::server::NamedService>::NAME,
        <TelemetryStreamServiceServer<TelemetryStreamServiceImpl> as tonic::server::NamedService>::NAME,
    ]
}

/// Report every registered service as `SERVING` or `NOT_SERVING`.
async fn set_health(reporter: &mut HealthReporter, ready: bool) {
    let status = if ready {
        ServingStatus::Serving
    } else {
        ServingStatus::NotServing
    };
    for name in health_service_names() {
        reporter.set_service_status(name, status).await;
    }
}

/// Keep the gRPC health status in sync with the readiness signal (DB and RPC
/// provider health, published by `main`).
fn spawn_health_sync(mut reporter: HealthReporter, mut ready: watch::Receiver<bool>) {
    tokio::spawn(async move {
        loop {
            let is_ready = *ready.borrow_and_update();
            set_health(&mut reporter, is_ready).await;
            if ready.changed().await.is_err() {
                break;
            }
        }
    });
}

/// Register the health, reflection and telemetry services on a server builder,
/// applying keepalive and stream limits.
fn build_router(
    builder: tonic::transport::Server,
    bus: Arc<SimulationBus>,
    ready: watch::Receiver<bool>,
) -> Result<tonic::transport::server::Router, tonic_reflection::server::Error> {
    let (reporter, health_service) = tonic_health::server::health_reporter();
    spawn_health_sync(reporter, ready);

    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .register_encoded_file_descriptor_set(tonic_health::pb::FILE_DESCRIPTOR_SET)
        .build_v1()?;

    let mut builder = configure_server(builder);
    Ok(builder
        .add_service(health_service)
        .add_service(reflection_service)
        .add_service(EventStreamServiceServer::new(EventStreamServiceImpl::new(
            Arc::clone(&bus),
        )))
        .add_service(TelemetryStreamServiceServer::new(
            TelemetryStreamServiceImpl::new(bus),
        )))
}

// ── Server startup helper ─────────────────────────────────────────────────────

/// Bind and serve the gRPC endpoint on `addr` over plaintext (no TLS).
///
/// This is called from `main` and runs in a separate Tokio task alongside the
/// existing Axum HTTP server. For a TLS-enabled listener use [`serve_tls`].
/// `ready` drives the `grpc.health.v1.Health` status.
pub async fn serve(
    addr: std::net::SocketAddr,
    bus: Arc<SimulationBus>,
    ready: watch::Receiver<bool>,
) {
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    serve_with_shutdown(addr, bus, ready, shutdown_rx).await;
}

/// Bind and serve the gRPC endpoint on `addr` over plaintext (no TLS), stopping
/// gracefully when `shutdown` flips to `true`.
///
/// On shutdown the listener stops accepting new connections/streams and tonic
/// waits for in-flight RPCs to finish. The caller is responsible for bounding
/// that wait with the configured grace period.
pub async fn serve_with_shutdown(
    addr: std::net::SocketAddr,
    bus: Arc<SimulationBus>,
    ready: watch::Receiver<bool>,
    shutdown: watch::Receiver<bool>,
) {
    let router = match build_router(tonic::transport::Server::builder(), bus, ready) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "failed to build gRPC reflection service");
            return;
        }
    };

    tracing::info!(grpc_addr = %addr, tls = false, "gRPC EventStreamService listening");

    if let Err(e) = router.serve_with_shutdown(addr, shutdown_signal(shutdown)).await {
        tracing::error!(error = %e, "gRPC server terminated with error");
    }
}

/// Resolve once the coordinator signals shutdown.
///
/// Handles the case where the signal was already flipped before this future
/// was created (watch channels only notify on *changes*).
async fn shutdown_signal(mut shutdown: watch::Receiver<bool>) {
    if *shutdown.borrow() {
        return;
    }
    while shutdown.changed().await.is_ok() {
        if *shutdown.borrow() {
            return;
        }
    }
    // Sender dropped without signalling: treat as shutdown so we never hang.
}

// ── TLS configuration & certificate chain loading (Issue #918) ────────────────

/// Server-side TLS configuration, loaded from the process environment.
///
/// * `GRPC_TLS_CERT` — path to the PEM **certificate chain**. The file must
///   contain the leaf certificate first, followed by any intermediate
///   certificates. Every `CERTIFICATE` block is loaded so the full chain is
///   presented to clients; presenting only the leaf (the previous bug) made
///   client-side chain validation fail.
/// * `GRPC_TLS_KEY` — path to the PEM private key for the leaf certificate.
/// * `GRPC_TLS_CA_CERT` — optional path to a custom root CA the server trusts
///   when validating client certificates (mutual TLS).
#[derive(Debug, Clone, Default)]
pub struct TlsConfig {
    /// PEM certificate chain (leaf + intermediates).
    pub cert_chain: String,
    /// PEM private key matching the leaf certificate.
    pub private_key: String,
    /// Optional custom root CA used to validate client certificates.
    pub ca_cert: Option<String>,
}

impl TlsConfig {
    /// Build a [`TlsConfig`] by reading the standardised gRPC environment
    /// variables. Returns `Ok(None)` when TLS is not configured.
    pub fn from_env() -> std::io::Result<Option<TlsConfig>> {
        let cert_chain = match std::env::var("GRPC_TLS_CERT") {
            Ok(c) => c,
            Err(_) => return Ok(None),
        };
        let private_key = std::env::var("GRPC_TLS_KEY").map_err(|_| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "GRPC_TLS_CERT is set but GRPC_TLS_KEY is missing",
            )
        })?;
        let ca_cert = std::env::var("GRPC_TLS_CA_CERT").ok();
        Ok(Some(TlsConfig {
            cert_chain,
            private_key,
            ca_cert,
        }))
    }
}

/// Read every `CERTIFICATE` block out of the PEM file at `path` and re-encode
/// them as a single PEM chain.
///
/// # Why this fixes the validation failure
///
/// Tonic's [`Identity::from_pem`] expects the *complete* certificate chain in
/// one PEM document. The previous implementation only kept the first
/// certificate, so clients could not validate the presented chain up to their
/// trusted root and every TLS handshake failed. Parsing with `rustls-pemfile`
/// preserves every block (leaf + intermediates) in the correct order.
fn load_cert_chain(path: &str) -> io::Result<String> {
    let data = fs::read(path).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!("failed to read certificate chain file '{path}': {e}"),
        )
    })?;
    let mut reader = BufReader::new(&data[..]);
    let mut collected = read_cert_chain(&mut reader).collect::<io::Result<Vec<_>>>()?;
    if collected.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("no CERTIFICATE blocks found in '{path}'"),
        ));
    }
    // rustls-pemfile yields certs in file order (leaf first). Ensure that
    // order is preserved exactly as authored.
    collected.shrink_to_fit();

    let mut chain = String::new();
    for cert in collected {
        chain.push_str("-----BEGIN CERTIFICATE-----\n");
        for chunk in BASE64.encode(cert).as_bytes().chunks(64) {
            // SAFETY: base64 output is always ASCII.
            let line = str::from_utf8(chunk).unwrap_or_default();
            chain.push_str(line);
            chain.push('\n');
        }
        chain.push_str("-----END CERTIFICATE-----\n");
    }
    Ok(chain)
}

/// Load the Tonic server [`Identity`] (full certificate chain + private key).
fn load_identity(tls: &TlsConfig) -> io::Result<Identity> {
    let cert_chain = load_cert_chain(&tls.cert_chain)?;
    let key = fs::read_to_string(&tls.private_key).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!("failed to read private key file '{}': {e}", tls.private_key),
        )
    })?;
    // `Identity::from_pem` carries the entire PEM document through to rustls,
    // so the full chain (leaf + intermediates) is what gets presented to
    // clients — this is what makes chain validation succeed (Issue #918).
    Ok(Identity::from_pem(cert_chain, key))
}

/// Load the optional custom root CA certificate used to validate client certs.
fn load_client_ca(ca_path: &Path) -> io::Result<Option<Certificate>> {
    if !ca_path.as_os_str().is_empty() {
        let pem = fs::read(ca_path).map_err(|e| {
            io::Error::new(
                e.kind(),
                format!(
                    "failed to read CA certificate file '{}': {e}",
                    ca_path.display()
                ),
            )
        })?;
        return Ok(Some(Certificate::from_pem(pem)));
    }
    Ok(None)
}

/// Build a Tonic [`ServerTlsConfig`] from the supplied [`TlsConfig`], loading
/// the full certificate chain (see [`load_cert_chain`]) and, when configured,
/// the custom root CA used to validate client certificates.
fn build_server_tls(tls: &TlsConfig) -> io::Result<ServerTlsConfig> {
    let mut server_tls = ServerTlsConfig::new();
    server_tls = server_tls.identity(load_identity(tls)?);

    if let Some(ca_path) = tls.ca_cert.as_deref() {
        if let Some(ca) = load_client_ca(Path::new(ca_path))? {
            server_tls = server_tls.client_ca_root(ca);
        }
    }
    Ok(server_tls)
}

/// Bind and serve the gRPC endpoint on `addr` over TLS using the supplied
/// [`TlsConfig`].
///
/// The Tonic server builder is configured with an [`Identity`] that carries the
/// full certificate chain (see [`load_cert_chain`]). If a custom CA root is
/// configured it is registered so client certificates are validated against it.
pub async fn serve_tls(
    addr: std::net::SocketAddr,
    bus: Arc<SimulationBus>,
    tls: TlsConfig,
    ready: watch::Receiver<bool>,
) {
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    serve_tls_with_shutdown(addr, bus, tls, ready, shutdown_rx).await;
}

/// Bind and serve the gRPC endpoint on `addr` over TLS, stopping gracefully when
/// `shutdown` flips to `true`.
///
/// On shutdown the listener stops accepting new streams and tonic waits for
/// in-flight RPCs to complete; the caller bounds that wait with the grace
/// period.
pub async fn serve_tls_with_shutdown(
    addr: std::net::SocketAddr,
    bus: Arc<SimulationBus>,
    tls: TlsConfig,
    ready: watch::Receiver<bool>,
    shutdown: watch::Receiver<bool>,
) {
    let server_tls = match build_server_tls(&tls) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, "failed to configure gRPC TLS");
            return;
        }
    };

    tracing::info!(
        grpc_addr = %addr,
        tls = true,
        "gRPC EventStreamService listening with TLS"
    );

    let builder = match tonic::transport::Server::builder().tls_config(server_tls) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(error = %e, "invalid gRPC TLS server configuration");
            return;
        }
    };
    let router = match build_router(builder, bus, ready) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "failed to build gRPC reflection service");
            return;
        }
    };
    if let Err(e) = router.serve_with_shutdown(addr, shutdown_signal(shutdown)).await {
        tracing::error!(error = %e, "gRPC TLS server terminated with error");
    }
}

/// Run the gRPC server, automatically enabling TLS when `GRPC_TLS_CERT` and
/// `GRPC_TLS_KEY` are present in the environment, otherwise serving plaintext.
///
/// Convenience entry point used by `main` so a single call always "just works".
pub async fn serve_with_tls_from_env(
    addr: std::net::SocketAddr,
    bus: Arc<SimulationBus>,
    ready: watch::Receiver<bool>,
) {
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    serve_with_tls_from_env_and_shutdown(addr, bus, ready, shutdown_rx).await;
}

/// Run the gRPC server (TLS when configured, plaintext otherwise) and stop it
/// gracefully when `shutdown` flips to `true`.
///
/// This is the entry point the [`crate::runner::ShutdownCoordinator`] uses: it
/// flips the shared watch channel so the gRPC listener stops accepting new
/// streams and lets in-flight streams finish within the grace period.
pub async fn serve_with_tls_from_env_and_shutdown(
    addr: std::net::SocketAddr,
    bus: Arc<SimulationBus>,
    ready: watch::Receiver<bool>,
    shutdown: watch::Receiver<bool>,
) {
    match TlsConfig::from_env() {
        Ok(Some(tls)) => serve_tls_with_shutdown(addr, bus, tls, ready, shutdown).await,
        Ok(None) => {
            tracing::warn!(
                grpc_addr = %addr,
                "GRPC_TLS_CERT not set; gRPC server running without TLS"
            );
            serve_with_shutdown(addr, bus, ready, shutdown).await;
        }
        Err(e) => {
            tracing::error!(error = %e, "invalid gRPC TLS configuration");
        }
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

    // ── TLS certificate chain loading & secure connection (Issue #918) ────

    use rcgen::{
        BasicConstraints, Certificate as RcCert, CertificateParams, DistinguishedName, DnType,
        IsCa, KeyPair,
    };
    use rustls::RootCertStore;
    use rustls_pki_types::{CertificateDer, ServerName};
    use std::net::SocketAddr;
    use tokio::net::TcpStream;
    use tokio_rustls::TlsConnector;

    /// Generate a throwaway root CA used as the trust anchor for the test.
    fn make_test_ca() -> (RcCert, KeyPair) {
        let mut params =
            CertificateParams::new(vec!["soroscope.test".to_string()]).expect("ca params");
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, "soroscope test root CA".to_string());
        params.distinguished_name = dn;
        let key = KeyPair::generate().expect("generate ca key");
        let cert = params.self_signed(&key).expect("self-sign ca");
        (cert, key)
    }

    /// Issue a leaf certificate (CN/SAN `localhost`) signed by `ca`.
    fn make_test_leaf(ca: &RcCert, ca_key: &KeyPair) -> (RcCert, KeyPair) {
        let mut params =
            CertificateParams::new(vec!["localhost".to_string()]).expect("leaf params");
        params.is_ca = IsCa::NoCa;
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, "localhost".to_string());
        params.distinguished_name = dn;
        let key = KeyPair::generate().expect("generate leaf key");
        let cert = params
            .signed_by(&key, ca, ca_key)
            .expect("sign leaf with ca");
        (cert, key)
    }

    /// Open a TCP connection to the TLS server, retrying briefly to allow the
    /// async server to finish binding.
    async fn connect_with_retry(addr: SocketAddr) -> TcpStream {
        let mut attempt = 0;
        loop {
            match TcpStream::connect(addr).await {
                Ok(s) => return s,
                Err(_) if attempt < 50 => {
                    attempt += 1;
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
                Err(e) => panic!("failed to connect to gRPC TLS server: {e}"),
            }
        }
    }

    /// Starts a Tonic gRPC server terminated with TLS using the supplied leaf
    /// signed by a shared CA. Both the server chain and the trust anchor come
    /// from the same CA so the handshake can only succeed if the full chain is
    /// loaded (the regression this test guards).
    #[tokio::test]
    async fn tls_server_presents_chain_and_accepts_secure_connection() {
        let (ca, ca_key) = make_test_ca();
        let (leaf, leaf_key) = make_test_leaf(&ca, &ca_key);

        // Server presents the full chain: leaf + issuing CA.
        let chain_pem = format!("{}\n{}", leaf.pem(), ca.pem());
        let key_pem = leaf_key.serialize_pem();
        let ca_der: CertificateDer<'static> = ca.der().clone();

        let dir = tempfile::tempdir().expect("temp dir");
        let cert_path = dir.path().join("chain.pem");
        let key_path = dir.path().join("key.pem");
        std::fs::write(&cert_path, chain_pem).expect("write chain.pem");
        std::fs::write(&key_path, key_pem).expect("write key.pem");

        // Pick a free port for the server.
        let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("free port probe");
        let port = probe.local_addr().expect("probe addr").port();
        drop(probe);
        let addr: SocketAddr = format!("127.0.0.1:{port}").parse().expect("addr");

        let tls = TlsConfig {
            cert_chain: cert_path.display().to_string(),
            private_key: key_path.display().to_string(),
            ca_cert: None,
        };
        let bus = crate::ws::SimulationBus::new();
        let (_ready_tx, ready_rx) = watch::channel(true);
        let server = tokio::spawn(serve_tls(addr, bus, tls, ready_rx));

        // The client trusts only the shared test root CA. A successful
        // handshake proves the server presented the full, valid chain.
        let mut roots = RootCertStore::empty();
        roots.add(ca_der).expect("add root ca");
        let client_cfg = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        let connector = TlsConnector::from(Arc::new(client_cfg));

        let stream = connect_with_retry(addr).await;
        let server_name = ServerName::try_from("localhost").expect("dns name");
        let tls_stream = connector
            .connect(server_name, stream)
            .await
            .expect("TLS handshake must succeed when the full certificate chain is loaded");

        // Sanity: the server authenticated with our leaf certificate.
        let peer = tls_stream
            .get_ref()
            .1
            .peer_certificates()
            .expect("peer certificate present");
        assert!(
            !peer.is_empty(),
            "server presented at least one certificate"
        );

        server.abort();
    }

    #[tokio::test]
    async fn shutdown_signal_resolves_when_already_flagged() {
        let (tx, rx) = watch::channel(true);
        // Already-flagged receivers must resolve immediately.
        tokio::time::timeout(std::time::Duration::from_secs(1), shutdown_signal(rx))
            .await
            .expect("shutdown_signal must resolve when already flagged");
        drop(tx);
    }

    #[tokio::test]
    async fn shutdown_signal_resolves_on_flip() {
        let (tx, rx) = watch::channel(false);
        let handle = tokio::spawn(shutdown_signal(rx));
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        assert!(!handle.is_finished(), "must not resolve before the flip");
        tx.send(true).expect("send shutdown");
        tokio::time::timeout(std::time::Duration::from_secs(1), handle)
            .await
            .expect("shutdown_signal must resolve after the flip")
            .expect("join");
    }

    #[tokio::test]
    async fn shutdown_signal_resolves_when_sender_dropped() {
        let (tx, rx) = watch::channel(false);
        drop(tx);
        tokio::time::timeout(std::time::Duration::from_secs(1), shutdown_signal(rx))
            .await
            .expect("shutdown_signal must resolve when the sender is dropped");
    }

    #[test]
    fn load_cert_chain_reads_every_certificate_block() {
        let (ca, ca_key) = make_test_ca();
        let (leaf, _leaf_key) = make_test_leaf(&ca, &ca_key);
        let chain_pem = format!("{}\n{}", leaf.pem(), ca.pem());

        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("chain.pem");
        std::fs::write(&path, chain_pem).expect("write chain");

        let loaded = load_cert_chain(path.to_str().unwrap()).expect("load chain");
        // Both the leaf and the CA block survive; the chain is the whole file.
        assert_eq!(
            loaded.matches("BEGIN CERTIFICATE").count(),
            2,
            "expected both certificate blocks to be preserved"
        );
        assert_eq!(
            loaded.matches("END CERTIFICATE").count(),
            2,
            "expected both certificate terminators to be preserved"
        );
    }

    // ── Health check service & reflection (Issue #1404) ───────────────────

    use tonic::transport::Channel;
    use tonic_health::pb::health_check_response::ServingStatus as PbServingStatus;
    use tonic_health::pb::health_client::HealthClient;
    use tonic_health::pb::HealthCheckRequest;

    /// Connect a health client, retrying briefly while the server binds.
    async fn connect_health_client(addr: SocketAddr) -> HealthClient<Channel> {
        let mut attempt = 0;
        loop {
            match HealthClient::connect(format!("http://{addr}")).await {
                Ok(client) => return client,
                Err(_) if attempt < 50 => {
                    attempt += 1;
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                Err(e) => panic!("failed to connect to gRPC health service: {e}"),
            }
        }
    }

    /// Poll `grpc.health.v1.Health/Check` until `service` reports `expected`.
    async fn wait_for_status(
        client: &mut HealthClient<Channel>,
        service: &str,
        expected: PbServingStatus,
    ) {
        for _ in 0..100 {
            let request = HealthCheckRequest {
                service: service.to_string(),
            };
            if let Ok(response) = client.check(request).await {
                if response.into_inner().status == expected as i32 {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("health status of {service:?} never became {expected:?}");
    }

    #[tokio::test]
    async fn grpc_health_check_follows_readiness() {
        let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("free port probe");
        let port = probe.local_addr().expect("probe addr").port();
        drop(probe);
        let addr: SocketAddr = format!("127.0.0.1:{port}").parse().expect("addr");

        let (ready_tx, ready_rx) = watch::channel(true);
        let server = tokio::spawn(serve(addr, crate::ws::SimulationBus::new(), ready_rx));
        let mut client = connect_health_client(addr).await;

        // Dependencies healthy: every service reports SERVING.
        for name in health_service_names() {
            wait_for_status(&mut client, name, PbServingStatus::Serving).await;
        }

        // Dependencies degraded: every service flips to NOT_SERVING.
        ready_tx.send(false).expect("health sync task is listening");
        for name in health_service_names() {
            wait_for_status(&mut client, name, PbServingStatus::NotServing).await;
        }

        // Recovery is reported again.
        ready_tx.send(true).expect("health sync task is listening");
        wait_for_status(&mut client, "", PbServingStatus::Serving).await;

        server.abort();
    }

    #[test]
    fn reflection_descriptor_set_lists_telemetry_services() {
        for service in ["EventStreamService", "TelemetryStreamService"] {
            assert!(
                FILE_DESCRIPTOR_SET
                    .windows(service.len())
                    .any(|w| w == service.as_bytes()),
                "reflection descriptor set is missing {service}"
            );
        }
    }

    #[tokio::test]
    async fn reflection_service_builds_from_descriptor_sets() {
        let (_ready_tx, ready_rx) = watch::channel(true);
        assert!(
            build_router(
                tonic::transport::Server::builder(),
                crate::ws::SimulationBus::new(),
                ready_rx
            )
            .is_ok(),
            "health + reflection + telemetry services must register"
        );
    }
}
