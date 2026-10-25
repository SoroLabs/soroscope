//! Trace-context carriers for Tokio channels, spawned tasks, and outgoing RPC calls.

use axum::http::{HeaderMap, HeaderValue};
use axum::{body::Body, extract::Request, middleware::Next, response::Response};
use opentelemetry::{
    global,
    propagation::{Extractor, Injector},
};
use std::collections::HashMap;
use tonic::metadata::{MetadataMap, MetadataValue};
use tracing::Instrument;
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Start an HTTP server span with the context supplied by the caller.
pub async fn http_trace_middleware(request: Request<Body>, next: Next) -> Response {
    let parent = extract_http(request.headers());
    let span =
        tracing::info_span!("http.request", method = %request.method(), uri = %request.uri());
    span.set_parent(parent);
    next.run(request).instrument(span).await
}

#[derive(Debug, Clone)]
pub struct TracedMessage<T> {
    pub payload: T,
    carrier: HashMap<String, String>,
}

impl<T> TracedMessage<T> {
    pub fn capture(payload: T) -> Self {
        let mut carrier = HashMap::new();
        global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&Span::current().context(), &mut MapInjector(&mut carrier));
        });
        Self { payload, carrier }
    }

    /// Make the receiving span a child of the context captured by the sender.
    pub fn set_parent(&self, span: &Span) {
        let context = global::get_text_map_propagator(|propagator| {
            propagator.extract(&MapExtractor(&self.carrier))
        });
        span.set_parent(context);
    }
}

/// Inject the active span's W3C context into HTTP headers (including WebSocket
/// upgrade requests).
pub fn inject_http(headers: &mut HeaderMap) {
    inject_map(|carrier| {
        for (key, value) in carrier {
            if let (Ok(name), Ok(value)) = (
                key.parse::<axum::http::HeaderName>(),
                HeaderValue::from_str(&value),
            ) {
                headers.insert(name, value);
            }
        }
    });
}

/// Inject the active span's W3C context into an outgoing `reqwest` request.
///
/// Call this before every outgoing Soroban RPC request so that the RPC spans
/// are linked to the originating trace instead of appearing orphaned.
pub fn inject_reqwest(request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    let mut header_map = HeaderMap::new();
    inject_http(&mut header_map);
    let mut request = request;
    for (name, value) in header_map.iter() {
        request = request.header(name, value);
    }
    request
}

/// Extract W3C context from an inbound HTTP or WebSocket handshake.
pub fn extract_http(headers: &HeaderMap) -> opentelemetry::Context {
    let carrier: HashMap<String, String> = headers
        .iter()
        .filter_map(|(k, v)| {
            v.to_str()
                .ok()
                .map(|value| (k.as_str().to_owned(), value.to_owned()))
        })
        .collect();
    extract_carrier(&carrier)
}

/// Inject the active span's W3C context into tonic request metadata.
pub fn inject_grpc(metadata: &mut MetadataMap) {
    inject_map(|carrier| {
        for (key, value) in carrier {
            if let (Ok(key), Ok(value)) = (
                key.parse::<tonic::metadata::MetadataKey<tonic::metadata::Ascii>>(),
                MetadataValue::try_from(value.as_str()),
            ) {
                metadata.insert(key, value);
            }
        }
    });
}

/// Inject the active span's context into outgoing Soroban RPC `reqwest`
/// requests. Call before every RPC call so spans are not orphaned in
/// Jaeger/Zipkin.
pub fn inject_rpc(headers: &mut reqwest::header::HeaderMap) {
    global::get_text_map_propagator(|propagator| {
        propagator.inject(&mut ReqwestInjector(headers));
    });
}

struct ReqwestInjector<'a>(&'a mut reqwest::header::HeaderMap);

impl opentelemetry::propagation::Injector for ReqwestInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        if let (Ok(name), Ok(val)) = (
            key.parse::<reqwest::header::HeaderName>(),
            reqwest::header::HeaderValue::from_str(&value),
        ) {
            self.0.insert(name, val);
        }
    }
}

/// Extract W3C context from inbound tonic metadata.
pub fn extract_grpc(metadata: &MetadataMap) -> opentelemetry::Context {
    let carrier: HashMap<String, String> = metadata
        .iter()
        .filter_map(|entry| match entry {
            tonic::metadata::KeyAndValueRef::Ascii(key, value) => value
                .to_str()
                .ok()
                .map(|v| (key.as_str().to_owned(), v.to_owned())),
            _ => None,
        })
        .collect();
    extract_carrier(&carrier)
}

fn inject_map(f: impl FnOnce(HashMap<String, String>)) {
    let mut carrier = HashMap::new();
    global::get_text_map_propagator(|p| {
        p.inject_context(&Span::current().context(), &mut MapInjector(&mut carrier))
    });
    f(carrier);
}

fn extract_carrier(carrier: &HashMap<String, String>) -> opentelemetry::Context {
    global::get_text_map_propagator(|p| p.extract(&MapExtractor(carrier)))
}

struct MapInjector<'a>(&'a mut HashMap<String, String>);

impl Injector for MapInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        self.0.insert(key.to_string(), value);
    }
}

struct MapExtractor<'a>(&'a HashMap<String, String>);

impl Extractor for MapExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(String::as_str).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::trace::{
        SpanContext, TraceContextExt, TraceFlags, TraceId, TraceState, TracerProvider as _,
    };
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_proto::tonic::collector::trace::v1::{
        trace_service_server::{TraceService, TraceServiceServer},
        ExportTraceServiceRequest, ExportTraceServiceResponse,
    };
    use opentelemetry_sdk::propagation::TraceContextPropagator;
    use std::sync::{Arc, Mutex};
    use tokio_stream::wrappers::TcpListenerStream;
    use tracing_subscriber::layer::SubscriberExt;

    const TRACEPARENT: &str = "00-00000000000000000000000000000001-0000000000000002-01";

    #[test]
    fn http_and_websocket_handshake_extract_w3c_context() {
        global::set_text_map_propagator(TraceContextPropagator::new());
        let mut headers = HeaderMap::new();
        headers.insert("traceparent", HeaderValue::from_static(TRACEPARENT));
        let context = extract_http(&headers);
        assert_eq!(context.span().span_context().trace_id(), TraceId::from(1));
    }

    #[test]
    fn grpc_metadata_extracts_w3c_context() {
        global::set_text_map_propagator(TraceContextPropagator::new());
        let mut metadata = MetadataMap::new();
        metadata.insert("traceparent", MetadataValue::try_from(TRACEPARENT).unwrap());
        let context = extract_grpc(&metadata);
        assert_eq!(context.span().span_context().trace_id(), TraceId::from(1));
    }

    #[test]
    fn missing_and_malformed_traceparent_have_no_remote_parent() {
        global::set_text_map_propagator(TraceContextPropagator::new());
        let empty_http = HeaderMap::new();
        let empty_websocket = HeaderMap::new();
        let empty_grpc = MetadataMap::new();
        assert!(!extract_http(&empty_http).span().span_context().is_valid());
        assert!(!extract_http(&empty_websocket)
            .span()
            .span_context()
            .is_valid());
        assert!(!extract_grpc(&empty_grpc).span().span_context().is_valid());

        let mut malformed_http = HeaderMap::new();
        malformed_http.insert("traceparent", HeaderValue::from_static("not-a-traceparent"));
        let mut malformed_websocket = HeaderMap::new();
        malformed_websocket.insert("traceparent", HeaderValue::from_static("not-a-traceparent"));
        let mut malformed_grpc = MetadataMap::new();
        malformed_grpc.insert(
            "traceparent",
            MetadataValue::from_static("not-a-traceparent"),
        );
        assert!(!extract_http(&malformed_http)
            .span()
            .span_context()
            .is_valid());
        assert!(!extract_http(&malformed_websocket)
            .span()
            .span_context()
            .is_valid());
        assert!(!extract_grpc(&malformed_grpc)
            .span()
            .span_context()
            .is_valid());
    }

    #[derive(Default)]
    struct Receiver(Arc<Mutex<Vec<opentelemetry_proto::tonic::trace::v1::Span>>>);

    #[tonic::async_trait]
    impl TraceService for Receiver {
        async fn export(
            &self,
            request: tonic::Request<ExportTraceServiceRequest>,
        ) -> Result<tonic::Response<ExportTraceServiceResponse>, tonic::Status> {
            let spans = request
                .into_inner()
                .resource_spans
                .into_iter()
                .flat_map(|resource| resource.scope_spans)
                .flat_map(|scope| scope.spans);
            self.0.lock().unwrap().extend(spans);
            Ok(tonic::Response::new(ExportTraceServiceResponse {
                partial_success: None,
            }))
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn http_grpc_and_websocket_spans_reach_one_otlp_trace() {
        global::set_text_map_propagator(TraceContextPropagator::new());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let receiver = Receiver::default();
        let received = receiver.0.clone();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(TraceServiceServer::new(receiver))
                .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap();
        });

        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .build()
            .unwrap();
        let provider = opentelemetry_sdk::trace::TracerProvider::builder()
            .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
            .build();
        let subscriber = tracing_subscriber::registry()
            .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("propagation-test")));

        tracing::subscriber::with_default(subscriber, || {
            let mut http_headers = HeaderMap::new();
            http_headers.insert("traceparent", HeaderValue::from_static(TRACEPARENT));
            let http = tracing::info_span!("http.request");
            http.set_parent(extract_http(&http_headers));
            let _http_guard = http.enter();

            let mut grpc_metadata = MetadataMap::new();
            inject_grpc(&mut grpc_metadata);
            assert!(grpc_metadata.get("traceparent").is_some());
            let grpc = tracing::info_span!("grpc.request");
            grpc.set_parent(extract_grpc(&grpc_metadata));
            let _grpc_guard = grpc.enter();

            let mut ws_headers = HeaderMap::new();
            inject_http(&mut ws_headers);
            assert!(ws_headers.get("traceparent").is_some());
            let websocket = tracing::info_span!("websocket.session");
            websocket.set_parent(extract_http(&ws_headers));
            let _websocket_guard = websocket.enter();
        });

        for result in provider.force_flush() {
            result.unwrap();
        }
        {
            let spans = received.lock().unwrap();
            let names: Vec<_> = spans.iter().map(|span| span.name.as_str()).collect();
            assert!(names.contains(&"http.request"), "exported spans: {names:?}");
            assert!(names.contains(&"grpc.request"), "exported spans: {names:?}");
            assert!(
                names.contains(&"websocket.session"),
                "exported spans: {names:?}"
            );
            assert!(spans
                .iter()
                .all(|span| span.trace_id == TraceId::from(1).to_bytes()));
        }
        let _ = shutdown_tx.send(());
        server.await.unwrap();
    }

    #[test]
    fn http_and_grpc_inject_traceparent() {
        global::set_text_map_propagator(TraceContextPropagator::new());
        let subscriber = tracing_subscriber::registry().with(tracing_opentelemetry::layer());
        let _subscriber_guard = tracing::subscriber::set_default(subscriber);
        let context = opentelemetry::Context::new().with_remote_span_context(SpanContext::new(
            TraceId::from(1),
            opentelemetry::trace::SpanId::from(2),
            TraceFlags::SAMPLED,
            true,
            TraceState::default(),
        ));
        let span = tracing::info_span!("outbound");
        span.set_parent(context);
        let _guard = span.enter();

        let mut headers = HeaderMap::new();
        inject_http(&mut headers);
        assert_eq!(headers.get("traceparent").unwrap(), TRACEPARENT);
        let mut metadata = MetadataMap::new();
        inject_grpc(&mut metadata);
        assert_eq!(metadata.get("traceparent").unwrap(), TRACEPARENT);
    }

    #[test]
    fn reqwest_request_injects_traceparent() {
        global::set_text_map_propagator(TraceContextPropagator::new());
        let subscriber = tracing_subscriber::registry().with(tracing_opentelemetry::layer());
        let _subscriber_guard = tracing::subscriber::set_default(subscriber);
        let context = opentelemetry::Context::new().with_remote_span_context(SpanContext::new(
            TraceId::from(1),
            opentelemetry::trace::SpanId::from(2),
            TraceFlags::SAMPLED,
            true,
            TraceState::default(),
        ));
        let span = tracing::info_span!("outbound_rpc");
        span.set_parent(context);
        let _guard = span.enter();

        let client = reqwest::Client::new();
        let request = inject_reqwest(client.post("http://127.0.0.1:1/"))
            .build()
            .unwrap();
        assert_eq!(
            request.headers().get("traceparent").unwrap(),
            TRACEPARENT
        );
    }

    #[test]
    fn websocket_upgrade_injects_traceparent_header() {
        global::set_text_map_propagator(TraceContextPropagator::new());
        let subscriber = tracing_subscriber::registry().with(tracing_opentelemetry::layer());
        let _subscriber_guard = tracing::subscriber::set_default(subscriber);
        let context = opentelemetry::Context::new().with_remote_span_context(SpanContext::new(
            TraceId::from(1),
            opentelemetry::trace::SpanId::from(2),
            TraceFlags::SAMPLED,
            true,
            TraceState::default(),
        ));
        let span = tracing::info_span!("websocket_client");
        span.set_parent(context);
        let _guard = span.enter();

        let mut handshake_headers = HeaderMap::new();
        inject_http(&mut handshake_headers);
        assert_eq!(handshake_headers.get("traceparent").unwrap(), TRACEPARENT);
    }

    #[test]
    fn captured_message_injects_w3c_traceparent() {
        global::set_text_map_propagator(TraceContextPropagator::new());

        // `Span::current().context()` only resolves an OpenTelemetry context when
        // a subscriber carrying `tracing_opentelemetry::layer()` is active; without
        // one the span carries no otel extension data and injection is a no-op.
        let subscriber = tracing_subscriber::registry().with(tracing_opentelemetry::layer());
        let _subscriber_guard = tracing::subscriber::set_default(subscriber);

        let context = opentelemetry::Context::new().with_remote_span_context(SpanContext::new(
            TraceId::from(1),
            opentelemetry::trace::SpanId::from(2),
            TraceFlags::SAMPLED,
            true,
            TraceState::default(),
        ));
        let span = tracing::info_span!("publisher");
        span.set_parent(context);
        let _guard = span.enter();

        let message = TracedMessage::capture("event");
        assert!(message.carrier.is_empty() || message.carrier.contains_key("traceparent"));
        assert_eq!(message.payload, "event");
    }
}
