mod benchmarks;
mod errors;

use crate::errors::AppError;
use axum::{
    extract::Json,
    routing::{get, post},
    Router,
};
use config::{Config, ConfigError};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;
use tower_http::cors::{Any, CorsLayer};
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

#[derive(Debug, Deserialize)]
struct AppConfig {
    server_port: u16,
    rust_log: String,
    soroban_rpc_url: String,
}

fn load_config() -> Result<AppConfig, ConfigError> {
    // Load .env file if present
    dotenvy::dotenv().ok();

    let settings = Config::builder()
        .add_source(config::Environment::default())
        .set_default("server_port", 8080)?
        .set_default("rust_log", "info")?
        .set_default("soroban_rpc_url", "https://soroban-testnet.stellar.org")?
        .build()?;

    settings.try_deserialize()
}

#[derive(Deserialize, ToSchema)]
struct AnalyzeRequest {
    #[schema(example = "0x1234...")]
    contract_id: String,
    #[schema(example = "invoke")]
    function_name: String,
}

#[derive(Serialize, ToSchema)]
struct ResourceReport {
    #[schema(example = 1000)]
    cpu_instructions: u64,
    #[schema(example = 2048)]
    memory_bytes: u64,
    #[schema(example = 512)]
    ledger_read_bytes: u64,
    #[schema(example = 256)]
    ledger_write_bytes: u64,
}

#[utoipa::path(
    post,
    path = "/analyze",
    request_body = AnalyzeRequest,
    responses(
        (status = 200, description = "Resource analysis successful", body = ResourceReport),
        (status = 500, description = "Analysis failed")
    ),
    tag = "Analysis"
)]
async fn analyze(Json(payload): Json<AnalyzeRequest>) -> Result<Json<ResourceReport>, AppError> {
    // Placeholder implementation
    let report = ResourceReport {
        cpu_instructions: 1000,
        memory_bytes: 2048,
        ledger_read_bytes: 512,
        ledger_write_bytes: 256,
    };
    Ok(Json(report))
}

#[derive(OpenApi)]
#[openapi(
    paths(analyze),
    components(schemas(AnalyzeRequest, ResourceReport)),
    tags(
        (name = "Analysis", description = "Soroban contract resource analysis endpoints")
    ),
    info(
        title = "SoroScope API",
        version = "0.1.0",
        description = "API for analyzing Soroban smart contract resource consumption"
    )
)]
struct ApiDoc;

#[tokio::main]
async fn main() {
    // Load configuration
    let config = load_config().expect("Failed to load configuration");
    println!("SoroScope initialized with config: {:?}", config);

    // CLI Argument Handling
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 && args[1] == "benchmark" {
        println!("Starting SoroScope Benchmark...");
        let possible_paths = vec![
            "target/wasm32-unknown-unknown/release/soroban_token_contract.wasm",
            "../target/wasm32-unknown-unknown/release/soroban_token_contract.wasm",
        ];

        let mut wasm_path = None;
        for p in possible_paths {
            let path = PathBuf::from(p);
            if path.exists() {
                wasm_path = Some(path);
                break;
            }
        }

        if let Some(path) = wasm_path {
            if let Err(e) = benchmarks::run_token_benchmark(path) {
                eprintln!("Benchmark failed: {}", e);
            }
        } else {
            eprintln!("Could not find soroban_token_contract.wasm. Make sure to build the contract first.");
        }
        return;
    }

    // Default Web Server with Swagger UI
    println!("SoroScope API Server starting...");
    println!("Run with 'benchmark' argument to profile token contract.");

    let cors = CorsLayer::new().allow_origin(Any);

    let app = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route("/", get(|| async { "Hello from SoroScope! Usage: cargo run -p soroscope-core -- benchmark" }))
        .route(
            "/error",
            get(|| async { Err::<&str, AppError>(AppError::BadRequest("Test error".to_string())) }),
        )
        .route("/analyze", post(analyze))
        .layer(cors);

    let bind_addr = format!("0.0.0.0:{}", config.server_port);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .unwrap();

    println!("SoroScope API running on http://{}", listener.local_addr().unwrap());
    println!("Swagger UI available at http://{}/swagger-ui", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}
