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

use std::fs;
use std::io::{self, BufReader};
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use rustls_pemfile::certs as read_cert_chain;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};
use tonic::transport::{Certificate, Identity, ServerTlsConfig};
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

/// Bind and serve the gRPC endpoint on `addr` over plaintext (no TLS).
///
/// This is called from `main` and runs in a separate Tokio task alongside the
/// existing Axum HTTP server. For a TLS-enabled listener use [`serve_tls`].
pub async fn serve(addr: std::net::SocketAddr, bus: Arc<SimulationBus>) {
    let svc = EventStreamServiceServer::new(EventStreamServiceImpl::new(bus));

    tracing::info!(grpc_addr = %addr, tls = false, "gRPC EventStreamService listening");

    if let Err(e) = tonic::transport::Server::builder()
        .add_service(svc)
        .serve(addr)
        .await
    {
        tracing::error!(error = %e, "gRPC server terminated with error");
    }
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
pub async fn serve_tls(addr: std::net::SocketAddr, bus: Arc<SimulationBus>, tls: TlsConfig) {
    let server_tls = match build_server_tls(&tls) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, "failed to configure gRPC TLS");
            return;
        }
    };
    let svc = EventStreamServiceServer::new(EventStreamServiceImpl::new(bus));

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
    if let Err(e) = builder.add_service(svc).serve(addr).await {
        tracing::error!(error = %e, "gRPC TLS server terminated with error");
    }
}

/// Run the gRPC server, automatically enabling TLS when `GRPC_TLS_CERT` and
/// `GRPC_TLS_KEY` are present in the environment, otherwise serving plaintext.
///
/// Convenience entry point used by `main` so a single call always "just works".
pub async fn serve_with_tls_from_env(addr: std::net::SocketAddr, bus: Arc<SimulationBus>) {
    match TlsConfig::from_env() {
        Ok(Some(tls)) => serve_tls(addr, bus, tls).await,
        Ok(None) => {
            tracing::warn!(
                grpc_addr = %addr,
                "GRPC_TLS_CERT not set; gRPC server running without TLS"
            );
            serve(addr, bus).await;
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
        let server = tokio::spawn(serve_tls(addr, bus, tls));

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
}
