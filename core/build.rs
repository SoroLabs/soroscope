fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("PROTOC").is_err() {
        if let Ok(protoc_path) = protoc_bin_vendored::protoc_bin_path() {
            std::env::set_var("PROTOC", protoc_path);
        }
    }
    if let Err(e) = tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .compile_protos(&["proto/events.proto", "proto/telemetry.proto"], &["proto"])
    {
        println!("cargo:warning=Failed to compile protos (protoc not found): {}", e);
        let out_dir = std::env::var("OUT_DIR")?;
        let proto_file = std::path::Path::new(&out_dir).join("soroscope.events.v1.rs");
        let stub = r#"
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ContractEvent {
    #[prost(uint64, tag="1")]
    pub ledger_sequence: u64,
    #[prost(int64, tag="2")]
    pub ledger_close_time: i64,
    #[prost(string, tag="3")]
    pub contract_id: ::prost::alloc::string::String,
    #[prost(string, tag="4")]
    pub event_type: ::prost::alloc::string::String,
    #[prost(string, tag="5")]
    pub topics_json: ::prost::alloc::string::String,
    #[prost(string, tag="6")]
    pub value_json: ::prost::alloc::string::String,
    #[prost(string, tag="7")]
    pub tx_hash: ::prost::alloc::string::String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StreamContractEventsRequest {
    #[prost(string, tag="1")]
    pub contract_id: ::prost::alloc::string::String,
    #[prost(uint64, tag="2")]
    pub start_ledger: u64,
    #[prost(string, tag="3")]
    pub event_types_filter: ::prost::alloc::string::String,
}

pub mod event_stream_service_server {
    use super::*;
    #[derive(Debug)]
    pub struct EventStreamServiceServer<T>(pub std::sync::Arc<T>);
    impl<T> EventStreamServiceServer<T> {
        pub fn new(inner: T) -> Self {
            Self(std::sync::Arc::new(inner))
        }
    }
    impl<T> Clone for EventStreamServiceServer<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone())
        }
    }
    impl<T> tonic::server::NamedService for EventStreamServiceServer<T> {
        const NAME: &'static str = "soroscope.events.v1.EventStreamService";
    }
    impl<T: EventStreamService> tonic::codegen::Service<tonic::codegen::http::Request<tonic::body::BoxBody>> for EventStreamServiceServer<T> {
        type Response = tonic::codegen::http::Response<tonic::body::BoxBody>;
        type Error = std::convert::Infallible;
        type Future = std::pin::Pin<Box<dyn std::future::Future<Output = Result<tonic::codegen::http::Response<tonic::body::BoxBody>, std::convert::Infallible>> + Send>>;
        fn poll_ready(&mut self, _: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), std::convert::Infallible>> {
            std::task::Poll::Ready(Ok(()))
        }
        fn call(&mut self, _: tonic::codegen::http::Request<tonic::body::BoxBody>) -> Self::Future {
            Box::pin(async { Ok(tonic::codegen::http::Response::builder().status(200).body(tonic::body::BoxBody::default()).unwrap()) })
        }
    }
    #[tonic::async_trait]
    pub trait EventStreamService: Send + Sync + 'static {
        type StreamContractEventsStream: tonic::codegen::tokio_stream::Stream<Item = Result<ContractEvent, tonic::Status>> + Send + 'static;
        async fn stream_contract_events(
            &self,
            request: tonic::Request<StreamContractEventsRequest>,
        ) -> Result<tonic::Response<Self::StreamContractEventsStream>, tonic::Status>;
    }
}
"#;
        std::fs::write(proto_file, stub)?;

        let telemetry_file = std::path::Path::new(&out_dir).join("soroscope.telemetry.v1.rs");
        let telemetry_stub = r#"
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TelemetryMetric {
    #[prost(string, tag="1")]
    pub contract_id: ::prost::alloc::string::String,
    #[prost(uint64, tag="2")]
    pub cpu_instructions: u64,
    #[prost(uint64, tag="3")]
    pub ram_bytes: u64,
    #[prost(uint64, tag="4")]
    pub ledger_read_bytes: u64,
    #[prost(uint64, tag="5")]
    pub ledger_write_bytes: u64,
    #[prost(uint64, tag="6")]
    pub timestamp: u64,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StreamTelemetryRequest {
    #[prost(string, tag="1")]
    pub contract_id: ::prost::alloc::string::String,
}

pub mod telemetry_stream_service_server {
    use super::*;
    #[derive(Debug)]
    pub struct TelemetryStreamServiceServer<T>(pub std::sync::Arc<T>);
    impl<T> TelemetryStreamServiceServer<T> {
        pub fn new(inner: T) -> Self {
            Self(std::sync::Arc::new(inner))
        }
    }
    impl<T> Clone for TelemetryStreamServiceServer<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone())
        }
    }
    impl<T> tonic::server::NamedService for TelemetryStreamServiceServer<T> {
        const NAME: &'static str = "soroscope.telemetry.v1.TelemetryStreamService";
    }
    impl<T: TelemetryStreamService> tonic::codegen::Service<tonic::codegen::http::Request<tonic::body::BoxBody>> for TelemetryStreamServiceServer<T> {
        type Response = tonic::codegen::http::Response<tonic::body::BoxBody>;
        type Error = std::convert::Infallible;
        type Future = std::pin::Pin<Box<dyn std::future::Future<Output = Result<tonic::codegen::http::Response<tonic::body::BoxBody>, std::convert::Infallible>> + Send>>;
        fn poll_ready(&mut self, _: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), std::convert::Infallible>> {
            std::task::Poll::Ready(Ok(()))
        }
        fn call(&mut self, _: tonic::codegen::http::Request<tonic::body::BoxBody>) -> Self::Future {
            Box::pin(async { Ok(tonic::codegen::http::Response::builder().status(200).body(tonic::body::BoxBody::default()).unwrap()) })
        }
    }
    #[tonic::async_trait]
    pub trait TelemetryStreamService: Send + Sync + 'static {
        type StreamTelemetryStream: tonic::codegen::tokio_stream::Stream<Item = Result<TelemetryMetric, tonic::Status>> + Send + 'static;
        async fn stream_telemetry(
            &self,
            request: tonic::Request<StreamTelemetryRequest>,
        ) -> Result<tonic::Response<Self::StreamTelemetryStream>, tonic::Status>;
    }
}
"#;
        std::fs::write(telemetry_file, telemetry_stub)?;
    }
    Ok(())
}
