#![allow(dead_code)]

mod auth;
mod benchmarks;
mod cache;
mod comparison;
mod errors;
pub mod fee_analytics;
pub mod fee_collector;
pub mod fee_store;
mod gas_golfing;
pub mod insights;
mod jobs;
mod merkle_tree;
mod parser;
mod routing;
pub mod rpc_provider;
mod runner;
mod simulation;
mod simulation_service;
mod wasm_branch_analysis;
mod ws;
mod merkle_tree;

use crate::cache::{ContractCache, SimulationCache};
use crate::comparison::{CompareMode, RegressionFlag, RegressionReport, ResourceDelta};
use crate::errors::AppError;
use crate::merkle_tree::MerkleTree;
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use simulation_service::{AnalysisResult, SimulationMetric, SimulationService};
use std::env;
use std::path::PathBuf;
use std::sync::Arc;

// CLI Argument Handling
use crate::fee_analytics::{FeeAnalyticsEngine, MarketConditions, ModelBreakdown};
use crate::fee_collector::{FeeCollector, FeeCollectorConfig};
use crate::fee_store::FeeStore;
use crate::gas_golfing::{GasGolfingAnalyzer, GasGolfingReport};
use crate::insights::InsightsEngine;
use crate::jobs::{JobQueue, JobQueueConfig, JobWorker};
use crate::rpc_provider::{ProviderRegistry, RegistryConfig, RegistrySnapshot, RpcProvider};
use crate::simulation::{SimulationEngine, SimulationMode, SimulationResult};
use crate::ws::SimulationBus;
use axum::{
    extract::{Json, Multipart, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    middleware,
    response::IntoResponse,
    routing::{get, post},
    Extension, Router,
};
use config::{Config, ConfigError};
use prometheus::{Encoder, HistogramVec, IntCounterVec, Opts, Registry, TextEncoder};
use serde::{Deserialize, Serialize};
use simulation_service::{AnalysisResult, SimulationMetric, SimulationService};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AppConfig {
    /// Port for the HTTP server
    server_port: u16,
    /// Rust log level (e.g., "info", "debug")
    rust_log: String,
    /// Primary RPC URL — used as a single-provider fallback when
    /// `RPC_PROVIDERS` is not set.
    soroban_rpc_url: String,
    /// Optional RSA Private Key PEM for RS256 JWTs. If missing, a dev key is generated.
    jwt_private_key: Option<String>,
    /// Stellar network passphrase
    network_passphrase: String,
    /// Redis URL reserved for the distributed cache migration (issue #65).
    /// Unused in the MVP in-memory implementation — present so the config
    /// surface is stable when Redis is wired in.
    redis_url: String,
    /// JSON-encoded array of RPC provider objects.  Example:
    /// ```json
    /// [
    ///   {"name":"stellar-testnet","url":"https://soroban-testnet.stellar.org"},
    ///   {"name":"blockdaemon","url":"https://soroban.blockdaemon.com","auth_header":"X-API-Key","auth_value":"KEY"}
    /// ]
    /// ```
    /// When empty or absent the engine falls back to `soroban_rpc_url`.
    #[serde(default)]
    rpc_providers: String,
    /// Stable node identifier used for gossip snapshots.
    #[serde(default)]
    registry_instance_id: String,
    /// Public base URL announced to peers, e.g. `https://api-a.example.com`.
    #[serde(default)]
    registry_public_url: String,
    /// Seed peers as a JSON array or comma-separated list of base URLs.
    #[serde(default)]
    registry_seed_peers: String,
    /// Health-check interval in seconds (default 30).
    #[serde(default = "default_health_check_interval")]
    health_check_interval_secs: u64,
    /// Gossip sync interval in seconds (default 30).
    #[serde(default = "default_gossip_interval_secs")]
    gossip_interval_secs: u64,
    /// Simulation timeout in seconds (default 30).
    #[serde(default = "default_simulation_timeout_secs")]
    simulation_timeout_secs: u64,
    /// Simulation execution mode: `failover` or `consensus`.
    #[serde(default = "default_simulation_mode")]
    simulation_mode: String,
    /// Database URL for job queue (PostgreSQL or SQLite)
    #[serde(default = "default_database_url")]
    database_url: String,
    /// Job timeout in seconds (default 300).
    #[serde(default = "default_job_timeout_secs")]
    job_timeout_secs: u64,
    /// Max concurrent jobs (default 10).
    #[serde(default = "default_max_concurrent_jobs")]
    max_concurrent_jobs: usize,
    /// Fee data collection interval in seconds (default 5).
    #[serde(default = "default_fee_collection_interval")]
    fee_collection_interval_secs: u64,
    /// Fee data retention period in days (default 30).
    #[serde(default = "default_fee_retention_days")]
    fee_retention_days: u32,
    /// Enable fee market analysis (default true).
    #[serde(default = "default_fee_analysis_enabled")]
    fee_analysis_enabled: bool,
    /// Emergency pause for message verification (default false).
    /// When true, all verification endpoints return an error.
    #[serde(default = "default_emergency_verification_paused")]
    emergency_verification_paused: bool,
    /// Filesystem path that backs the disk-persistent L2 cache. When
    /// empty the L2 tier is disabled and the service runs L1-only (same
    /// behaviour as before #104).
    #[serde(default = "default_disk_cache_path")]
    disk_cache_path: String,
    /// Number of ledgers a cached entry may lag the current ledger before
    /// L2 treats it as stale. Default 100 ≈ 8 minutes at 5 s/ledger.
    #[serde(default = "default_max_ledger_age")]
    max_ledger_age: u32,
}

fn default_health_check_interval() -> u64 {
    30
}

fn default_simulation_timeout_secs() -> u64 {
    30
}

fn default_simulation_mode() -> String {
    "failover".to_string()
}

fn default_gossip_interval_secs() -> u64 {
    30
}

fn default_database_url() -> String {
    "sqlite://soroscope.db".to_string()
}

fn default_job_timeout_secs() -> u64 {
    300
}

fn default_max_concurrent_jobs() -> usize {
    10
}

fn default_fee_collection_interval() -> u64 {
    5
}

fn default_fee_retention_days() -> u32 {
    30
}

fn default_fee_analysis_enabled() -> bool {
    true
}

fn default_emergency_verification_paused() -> bool {
    false
fn default_disk_cache_path() -> String {
    // Empty == L2 disabled. Operators who want persistence set this in
    // env / config.toml explicitly; we don't create a hidden directory
    // in the CWD by default.
    String::new()
}

fn default_max_ledger_age() -> u32 {
    100
}

fn load_config() -> Result<AppConfig, ConfigError> {
    dotenvy::dotenv().ok();

    let settings = Config::builder()
        .add_source(config::Environment::default())
        .set_default("server_port", 8080)?
        .set_default("rust_log", "info")?
        .set_default("soroban_rpc_url", "https://soroban-testnet.stellar.org")?
        .set_default("network_passphrase", "Test SDF Network ; September 2015")?
        .set_default("redis_url", "redis://127.0.0.1:6379")?
        .set_default("rpc_providers", "")?
        .set_default("registry_instance_id", "")?
        .set_default("registry_public_url", "")?
        .set_default("registry_seed_peers", "")?
        .set_default("health_check_interval_secs", 30)?
        .set_default("gossip_interval_secs", 30)?
        .set_default("simulation_timeout_secs", 30)?
        .set_default("simulation_mode", "failover")?
        .set_default("database_url", "sqlite://soroscope.db")?
        .set_default("job_timeout_secs", 300)?
        .set_default("max_concurrent_jobs", 10)?
        .set_default("fee_collection_interval_secs", 5)?
        .set_default("fee_retention_days", 30)?
        .set_default("fee_analysis_enabled", true)?
        .set_default("emergency_verification_paused", false)?
        .build()?
        .set_default("disk_cache_path", "")?
        .set_default("max_ledger_age", 100)?
        .build()?;

    settings.try_deserialize()
}

/// Parse the `RPC_PROVIDERS` env var (JSON array) or fall back to wrapping the
/// single `SOROBAN_RPC_URL` into a one-element provider list.
fn build_providers(config: &AppConfig) -> Vec<RpcProvider> {
    if !config.rpc_providers.is_empty() {
        match serde_json::from_str::<Vec<RpcProvider>>(&config.rpc_providers) {
            Ok(providers) if !providers.is_empty() => {
                tracing::info!(
                    count = providers.len(),
                    "Loaded RPC providers from RPC_PROVIDERS"
                );
                return providers;
            }
            Ok(_) => {
                tracing::warn!("RPC_PROVIDERS is empty array, falling back to SOROBAN_RPC_URL");
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Failed to parse RPC_PROVIDERS, falling back to SOROBAN_RPC_URL"
                );
            }
        }
    }

    vec![RpcProvider {
        name: "default".to_string(),
        url: config.soroban_rpc_url.clone(),
        auth_header: None,
        auth_value: None,
        advertise: None,
    }]
}

fn parse_seed_peers(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    if trimmed.starts_with('[') {
        return serde_json::from_str::<Vec<String>>(trimmed).unwrap_or_default();
    }

    trimmed
        .split(',')
        .map(|peer| peer.trim().trim_end_matches('/').to_string())
        .filter(|peer| !peer.is_empty())
        .collect()
}

fn build_registry_config(config: &AppConfig) -> RegistryConfig {
    let instance_id = if config.registry_instance_id.trim().is_empty() {
        uuid::Uuid::new_v4().to_string()
    } else {
        config.registry_instance_id.trim().to_string()
    };

    let public_base_url = if config.registry_public_url.trim().is_empty() {
        Some(format!("http://127.0.0.1:{}", config.server_port))
    } else {
        Some(
            config
                .registry_public_url
                .trim()
                .trim_end_matches('/')
                .to_string(),
        )
    };

    RegistryConfig {
        instance_id,
        public_base_url,
        seed_peers: parse_seed_peers(&config.registry_seed_peers),
    }
}

/// Shared application state injected into every Axum handler via [`State`].
pub struct AppState {
    engine: SimulationEngine,
    provider_registry: Arc<ProviderRegistry>,
    cache: Arc<SimulationCache>,
    insights_engine: InsightsEngine,
    gas_golfing_analyzer: GasGolfingAnalyzer,
    /// Simulation timeout for RPC requests
    simulation_timeout: std::time::Duration,
    /// Job queue for background task processing
    #[allow(dead_code)]
    job_queue: JobQueue,
    /// Fee market analytics engine
    fee_analytics_engine: FeeAnalyticsEngine,
    /// Fee data store
    fee_store: Arc<FeeStore>,
    /// Prometheus metrics collectors.
    metrics: Arc<AppMetrics>,
    /// WebSocket event bus for simulation jobs.
    simulation_bus: Arc<SimulationBus>,
}

#[derive(Clone)]
struct AppMetrics {
    registry: Registry,
    simulation_latency_seconds: HistogramVec,
    rpc_error_count_total: IntCounterVec,
    simulation_requests_total: IntCounterVec,
    resource_utilization_percent: prometheus::GaugeVec,
}

impl AppMetrics {
    fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();

        let simulation_latency_seconds = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "simulation_latency_seconds",
                "Latency of simulation requests in seconds",
            ),
            &["endpoint"],
        )?;
        let rpc_error_count_total = IntCounterVec::new(
            Opts::new(
                "rpc_error_count_total",
                "Total number of RPC and simulation errors",
            ),
            &["endpoint", "error_type"],
        )?;
        let simulation_requests_total = IntCounterVec::new(
            Opts::new(
                "simulation_requests_total",
                "Total number of simulation requests by endpoint and cache status",
            ),
            &["endpoint", "cache_status"],
        )?;
        let resource_utilization_percent = prometheus::GaugeVec::new(
            Opts::new(
                "resource_utilization_percent",
                "Resource utilization percentage from latest simulation sample",
            ),
            &["resource"],
        )?;

        registry.register(Box::new(simulation_latency_seconds.clone()))?;
        registry.register(Box::new(rpc_error_count_total.clone()))?;
        registry.register(Box::new(simulation_requests_total.clone()))?;
        registry.register(Box::new(resource_utilization_percent.clone()))?;

        Ok(Self {
            registry,
            simulation_latency_seconds,
            rpc_error_count_total,
            simulation_requests_total,
            resource_utilization_percent,
        })
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AnalyzeRequest {
    #[schema(example = "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC")]
    pub contract_id: String,
    #[schema(example = "hello")]
    pub function_name: String,
    #[schema(example = "[]")]
    pub args: Option<Vec<String>>,
    /// Map of Key-Base64 to Value-Base64 ledger entry overrides
    pub ledger_overrides: Option<HashMap<String, String>>,
    /// Protocol version to simulate (e.g. 21)
    pub protocol_version: Option<u32>,
    /// Whether to enable experimental host functions
    pub enable_experimental: Option<bool>,
    /// Whether to generate and include Merkle tree root of the state snapshot
    #[serde(default)]
    #[schema(example = false, description = "Generate Merkle tree root from state snapshot")]
    pub include_merkle_tree: Option<bool>,
}

#[derive(Serialize, ToSchema)]
pub struct ResourceReport {
    /// CPU instructions consumed
    #[schema(example = 1500)]
    pub cpu_instructions: u64,
    /// RAM bytes consumed
    #[schema(example = 3000)]
    pub ram_bytes: u64,
    /// Ledger read bytes
    #[schema(example = 1024)]
    pub ledger_read_bytes: u64,
    /// Ledger write bytes
    #[schema(example = 512)]
    pub ledger_write_bytes: u64,
    /// Transaction size in bytes
    #[schema(example = 450)]
    pub transaction_size_bytes: u64,
    /// Estimated cost in stroops
    #[schema(example = 1000)]
    pub cost_stroops: u64,
    /// Report showing which data was injected vs live
    pub state_dependency: Option<Vec<StateDependencyReport>>,
    /// TTL status for touched ledger entries and extension suggestions.
    pub ttl_analysis: Option<TtlAnalysisApiReport>,
    /// Efficiency score (0–100) and optimisation insights.
    pub nutrition: NutritionReport,
    /// Cross-contract call graph
    pub call_graph: Option<crate::simulation::CallGraph>,
    /// Call graph in Mermaid format
    pub call_graph_mermaid: Option<String>,
    /// Snapshot of the ledger state used/touched during simulation
    pub state_snapshot: Option<crate::simulation::SimulationStateSnapshot>,
    /// Protocol version used for this simulation
    #[schema(example = 20)]
    pub protocol_version: u32,
    /// Testnet average resource usage for comparison
    pub testnet_averages: TestnetAverages,
}

#[derive(Serialize, ToSchema)]
pub struct TestnetAverages {
    /// Average CPU instructions for typical Soroban transactions
    pub cpu_instructions: u64,
    /// Average RAM bytes for typical Soroban transactions
    pub ram_bytes: u64,
    /// Average ledger read bytes for typical Soroban transactions
    pub ledger_read_bytes: u64,
    /// Average ledger write bytes for typical Soroban transactions
    pub ledger_write_bytes: u64,
    /// Average transaction size bytes for typical Soroban transactions
    pub transaction_size_bytes: u64,
    /// Merkle tree root hash (hex-encoded) of the state snapshot, if requested
    #[schema(description = "Merkle tree root hash of the state snapshot (hex-encoded)")]
    pub merkle_tree_root: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct TtlAnalysisApiReport {
    pub current_ledger: u64,
    pub touched_entries: Vec<TtlEntryApiReport>,
    pub extend_ttl_suggestions: Vec<ExtendTtlSuggestionApi>,
}

#[derive(Serialize, ToSchema)]
pub struct TtlEntryApiReport {
    pub key: String,
    pub live_until_ledger: u32,
    pub remaining_ledgers: i64,
}

#[derive(Serialize, ToSchema)]
pub struct ExtendTtlSuggestionApi {
    pub key: String,
    pub current_live_until_ledger: u32,
    pub remaining_ledgers: i64,
    pub extend_to_ledger: u32,
    pub ledgers_to_extend_by: u32,
    pub suggested_operation: String,
}

/// "Nutrition label" for the contract invocation.
#[derive(Serialize, ToSchema)]
pub struct NutritionReport {
    /// Weighted efficiency score (0 = poor, 100 = optimal).
    pub efficiency_score: u32,
    /// Actionable optimisation insights.
    pub insights: Vec<InsightEntry>,
}

/// A single optimisation insight.
#[derive(Serialize, ToSchema)]
pub struct InsightEntry {
    pub severity: String,
    pub rule: String,
    pub message: String,
    pub suggested_fix: String,
}

#[derive(Serialize, ToSchema, Debug)]
pub struct StateDependencyReport {
    pub key: String,
    pub source: String,
}

#[derive(Deserialize, ToSchema)]
pub struct OptimizeLimitsRequest {
    #[schema(example = "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC")]
    pub contract_id: String,
    #[schema(example = "hello")]
    pub function_name: String,
    #[schema(example = "[]")]
    #[serde(default)]
    pub args: Vec<String>,
    #[schema(example = 0.05)]
    #[serde(default = "default_safety_margin")]
    pub safety_margin: f64,
}

fn default_safety_margin() -> f64 {
    0.05
}

#[derive(Serialize, ToSchema)]
pub struct OptimizeLimitsResponse {
    pub cpu: crate::simulation::OptimizationBuffer,
    pub ram: crate::simulation::OptimizationBuffer,
    pub ledger_read: crate::simulation::OptimizationBuffer,
    pub ledger_write: crate::simulation::OptimizationBuffer,
    pub recommended: crate::simulation::SorobanResources,
}

// ── Fee Market Types ─────────────────────────────────────────────────────

/// Request body for fee recommendation endpoint
#[derive(Debug, Deserialize, ToSchema)]
pub struct FeeRecommendationRequest {
    /// Desired inclusion speed: "next_ledger", "next_3_ledgers", "economy", "standard", "priority"
    #[schema(example = "priority")]
    pub inclusion_speed: Option<String>,
    /// Custom safety margin (default 0.10 = 10%)
    #[schema(example = 0.10)]
    pub safety_margin: Option<f64>,
}

/// Response with fee recommendations
#[derive(Debug, Serialize, ToSchema)]
pub struct FeeRecommendationResponse {
    /// Recommended fee bid in stroops
    pub recommended_bid: u64,
    /// Estimated resource fee
    pub resource_fee_estimate: u64,
    /// Total estimated cost
    pub total_estimated_cost: u64,
    /// Confidence in inclusion (0.0-1.0)
    pub inclusion_confidence: f64,
    /// Expected number of ledgers for inclusion
    pub expected_inclusion_ledgers: u32,
    /// Current market conditions
    pub market_conditions: MarketConditions,
    /// Breakdown of prediction models
    pub model_breakdown: ModelBreakdown,
    /// Timestamp of prediction
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Request for historical fee data
#[derive(Debug, Deserialize, ToSchema)]
pub struct FeeHistoryRequest {
    /// Number of recent ledgers to retrieve (default 50)
    #[schema(example = 50)]
    pub limit: Option<i64>,
    /// Starting ledger sequence (optional)
    #[schema(example = 1000)]
    pub from_ledger: Option<i64>,
    /// Ending ledger sequence (optional)
    #[schema(example = 1100)]
    pub to_ledger: Option<i64>,
}

/// Historical fee data response
#[derive(Debug, Serialize, ToSchema)]
pub struct FeeHistoryResponse {
    /// List of fee samples
    pub samples: Vec<crate::fee_store::LedgerFeeSample>,
    /// Total count of samples
    pub total_count: i64,
}

/// Request body for the WASM-bytes analysis endpoint.
#[derive(Debug, Deserialize, ToSchema)]
pub struct AnalyzeWasmRequest {
    /// Base64-encoded WASM binary.
    #[schema(example = "<base64-encoded .wasm bytes>")]
    pub wasm_bytes: String,
    /// Name of the exported function to invoke.
    #[schema(example = "hello")]
    pub function_name: String,
    /// Optional function arguments (void | true | false | integers | symbols).
    #[schema(example = "[]")]
    pub args: Option<Vec<String>>,
    /// Protocol version to simulate (e.g. 21)
    pub protocol_version: Option<u32>,
    /// Whether to enable experimental host functions
    pub enable_experimental: Option<bool>,
}

/// Request body for the WASM profiling endpoint.
#[derive(Debug, Deserialize)]
pub struct ProfileWasmRequest {
    /// Base64-encoded WASM binary.
    pub wasm_bytes: String,
    /// Name of the exported function to invoke.
    pub function_name: String,
    /// Optional function arguments.
    #[serde(default)]
    pub args: Vec<String>,
}

/// Response body for the WASM profiling endpoint.
#[derive(Debug, Serialize)]
pub struct ProfileResponse {
    /// Flamegraph and per-function counts.
    pub profile: simulation::ProfileResult,
    /// Standard Soroban resource metrics (CPU, RAM, etc.).
    pub resources: simulation::SorobanResources,
}

/// Request body for the WASM execution-branch analysis endpoint (Issue #101).
#[derive(Debug, Deserialize, ToSchema)]
pub struct AnalyzeWasmBranchesRequest {
    /// Base64-encoded WASM binary to analyse.
    #[schema(example = "<base64-encoded .wasm bytes>")]
    pub wasm_bytes: String,
    /// Exported function whose execution branches should be enumerated.
    #[schema(example = "transfer")]
    pub function_name: String,
    /// Baseline argument vector used for the first (reference) simulation run.
    /// Additional permutations are generated automatically.
    #[schema(example = "[]")]
    pub args: Option<Vec<String>>,
}

/// API response for the WASM execution-branch analysis endpoint.
#[derive(Debug, Serialize, ToSchema)]
pub struct WasmBranchAnalysisResponse {
    /// Name of the analysed function.
    pub function_name: String,
    /// Total branch-generating instructions found via static analysis.
    pub total_branch_count: usize,
    /// Maximum control-flow nesting depth observed.
    pub max_nesting_depth: usize,
    /// Per-category branch counts.
    pub branch_type_breakdown: crate::wasm_branch_analysis::BranchTypeBreakdown,
    /// Conservative upper bound on distinct execution paths (capped at 64).
    pub estimated_paths: usize,
    /// Inventory of branch points from static analysis.
    pub branches: Vec<crate::wasm_branch_analysis::BranchInfo>,
    /// Per-path resource measurements from dynamic simulation.
    pub simulated_paths: Vec<crate::wasm_branch_analysis::PathResult>,
    /// Resource consumption for the provided baseline arguments.
    pub baseline_resources: crate::simulation::SorobanResources,
    /// Highest resource consumption across all simulated paths.
    pub worst_case_resources: crate::simulation::SorobanResources,
    /// Lowest resource consumption across all simulated paths.
    pub best_case_resources: crate::simulation::SorobanResources,
    /// Number of distinct resource profiles observed.
    pub distinct_profiles: usize,
    /// Human-readable note about path coverage.
    pub coverage_note: String,
}

/// Convert a `SimulationResult` (library type) into the API `ResourceReport`.
fn to_report(result: &SimulationResult, insights_engine: &InsightsEngine, merkle_tree_root: Option<String>) -> ResourceReport {
    let insights_report = insights_engine.analyze(&result.resources);

    ResourceReport {
        cpu_instructions: result.resources.cpu_instructions,
        ram_bytes: result.resources.ram_bytes,
        ledger_read_bytes: result.resources.ledger_read_bytes,
        ledger_write_bytes: result.resources.ledger_write_bytes,
        transaction_size_bytes: result.resources.transaction_size_bytes,
        cost_stroops: result.cost_stroops,
        state_dependency: result.state_dependency.as_ref().map(|deps| {
            deps.iter()
                .map(|d| StateDependencyReport {
                    key: d.key.clone(),
                    source: format!("{:?}", d.source),
                })
                .collect()
        }),
        ttl_analysis: result
            .ttl_analysis
            .as_ref()
            .map(|ttl| TtlAnalysisApiReport {
                current_ledger: ttl.current_ledger,
                touched_entries: ttl
                    .touched_entries
                    .iter()
                    .map(|e| TtlEntryApiReport {
                        key: e.key.clone(),
                        live_until_ledger: e.live_until_ledger,
                        remaining_ledgers: e.remaining_ledgers,
                    })
                    .collect(),
                extend_ttl_suggestions: ttl
                    .extend_ttl_suggestions
                    .iter()
                    .map(|s| ExtendTtlSuggestionApi {
                        key: s.key.clone(),
                        current_live_until_ledger: s.current_live_until_ledger,
                        remaining_ledgers: s.remaining_ledgers,
                        extend_to_ledger: s.extend_to_ledger,
                        ledgers_to_extend_by: s.ledgers_to_extend_by,
                        suggested_operation: s.suggested_operation.clone(),
                    })
                    .collect(),
            }),
        nutrition: NutritionReport {
            efficiency_score: insights_report.efficiency_score,
            insights: insights_report
                .insights
                .into_iter()
                .map(|i| InsightEntry {
                    severity: format!("{:?}", i.severity),
                    rule: i.rule,
                    message: i.message,
                    suggested_fix: i.suggested_fix,
                })
                .collect(),
        },
        call_graph: result.call_graph.clone(),
        call_graph_mermaid: result.call_graph.as_ref().map(|g| g.to_mermaid()),
        state_snapshot: result.state_snapshot.clone(),
        protocol_version: result.protocol_version,
        testnet_averages: TestnetAverages {
            cpu_instructions: 3_000_000,
            ram_bytes: 512_000,
            ledger_read_bytes: 2_048,
            ledger_write_bytes: 1_024,
            transaction_size_bytes: 600,
        },
        merkle_tree_root,
    }
}

#[utoipa::path(
    post,
    path = "/analyze",
    request_body = AnalyzeRequest,
    responses(
        (status = 200, description = "Resource analysis successful", body = ResourceReport),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Analysis failed")
    ),
    security(
        ("jwt" = [])
    ),
    tag = "Analysis"
)]
async fn analyze(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AnalyzeRequest>,
) -> Result<(HeaderMap, Json<ResourceReport>), AppError> {
    // Create a tracing span with structured fields for this request
    let span = tracing::info_span!(
        "analyze",
        contract_id = %payload.contract_id,
        function_name = %payload.function_name,
    );
    let _enter = span.enter();

    tracing::info!("Received analyze request");

    let args = payload.args.clone().unwrap_or_default();
    let cache_key =
        SimulationCache::generate_key(&payload.contract_id, &payload.function_name, &args);

    // Track simulation latency
    let start_time = std::time::Instant::now();

    let (result, cache_status): (SimulationResult, &'static str) =
        if let Some(cached) = state.cache.get(&cache_key).await {
            tracing::debug!("Cache HIT for key: {}", cache_key);
            (cached, "HIT")
        } else {
            tracing::debug!("Cache MISS for key: {}", cache_key);

            // Wrap the simulation call with a timeout to prevent hanging
            let sim_result = tokio::time::timeout(
                state.simulation_timeout,
                state.engine.simulate_from_contract_id(
                    &payload.contract_id,
                    &payload.function_name,
                    args,
                    payload.ledger_overrides.clone(),
                    payload.protocol_version,
                    payload.enable_experimental,
                ),
            )
            .await
            .map_err(|_| {
                state
                    .metrics
                    .rpc_error_count_total
                    .with_label_values(&["/analyze", "timeout"])
                    .inc();
                tracing::error!("Simulation timed out after {:?}", state.simulation_timeout);
                AppError::Internal(format!(
                    "Simulation timed out after {} seconds",
                    state.simulation_timeout.as_secs()
                ))
            })?;

            let sim: SimulationResult = match sim_result {
                Ok(sim) => sim,
                Err(err) => {
                    state
                        .metrics
                        .rpc_error_count_total
                        .with_label_values(&["/analyze", "simulation_error"])
                        .inc();
                    return Err(err.into());
                }
            };
            state.cache.set(cache_key, sim.clone()).await;
            (sim, "MISS")
        };

    let latency_ms = start_time.elapsed().as_millis() as u64;
    state
        .metrics
        .simulation_latency_seconds
        .with_label_values(&["/analyze"])
        .observe(start_time.elapsed().as_secs_f64());
    state
        .metrics
        .simulation_requests_total
        .with_label_values(&["/analyze", cache_status])
        .inc();

    // Log comprehensive simulation metrics
    tracing::info!(
        latency_ms = latency_ms,
        cache_status = cache_status,
        cpu_instructions = result.resources.cpu_instructions,
        ram_bytes = result.resources.ram_bytes,
        ledger_read_bytes = result.resources.ledger_read_bytes,
        ledger_write_bytes = result.resources.ledger_write_bytes,
        transaction_size_bytes = result.resources.transaction_size_bytes,
        cost_stroops = result.cost_stroops,
        latest_ledger = result.latest_ledger,
        "Simulation completed successfully"
    );

    state.cache.log_stats();
    let insights_report = state.insights_engine.analyze(&result.resources);
    state
        .metrics
        .resource_utilization_percent
        .with_label_values(&["efficiency_score"])
        .set(insights_report.efficiency_score as f64);

    // Generate Merkle tree root if requested
    let merkle_tree_root = if payload.include_merkle_tree.unwrap_or(false) {
        result
            .state_snapshot
            .as_ref()
            .and_then(|snapshot| {
                // Extract ledger entries as leaves for the Merkle tree
                let leaves: Vec<Vec<u8>> = snapshot
                    .ledger_entries
                    .values()
                    .filter_map(|entry_b64| hex::decode(entry_b64).ok())
                    .collect();

                if leaves.is_empty() {
                    tracing::warn!("No ledger entries available for Merkle tree generation");
                    None
                } else {
                    match MerkleTree::new(leaves) {
                        Ok(tree) => {
                            tracing::info!("Generated Merkle tree with {} leaves", tree.leaf_count);
                            Some(tree.get_root_hex())
                        }
                        Err(e) => {
                            tracing::error!("Failed to generate Merkle tree: {}", e);
                            None
                        }
                    }
                }
            })
    } else {
        None
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("x-soroscope-cache"),
        HeaderValue::from_static(cache_status),
    );
    headers.insert(
        HeaderName::from_static("x-soroscope-latency-ms"),
        HeaderValue::from_str(&latency_ms.to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("0")),
    );

    Ok((headers, Json(to_report(&result, &state.insights_engine, merkle_tree_root))))
}

#[utoipa::path(
    post,
    path = "/analyze/wasm",
    request_body = AnalyzeWasmRequest,
    responses(
        (status = 200, description = "Resource analysis successful", body = ResourceReport),
        (status = 400, description = "Invalid base64 or WASM data"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Analysis failed")
    ),
    security(
        ("jwt" = [])
    ),
    tag = "Analysis"
)]
async fn analyze_wasm(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AnalyzeWasmRequest>,
) -> Result<Json<ResourceReport>, AppError> {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

    tracing::info!(
        function_name = %payload.function_name,
        "Received WASM analyze request"
    );

    let wasm_bytes = BASE64
        .decode(&payload.wasm_bytes)
        .map_err(|e| AppError::BadRequest(format!("Invalid base64 WASM data: {}", e)))?;

    let function_name = payload.function_name.clone();
    let args = payload.args.clone().unwrap_or_default();

    let start_time = std::time::Instant::now();
    let resources = tokio::task::spawn_blocking(move || {
        simulation::profile_contract(
            wasm_bytes,
            function_name,
            args,
            payload.protocol_version,
            payload.enable_experimental,
        )
    })
    .await
    .map_err(|e| {
        state
            .metrics
            .rpc_error_count_total
            .with_label_values(&["/analyze/wasm", "panic"])
            .inc();
        AppError::Internal(format!("Contract profiling task panicked: {}", e))
    })?
    .map_err(|e| {
        state
            .metrics
            .rpc_error_count_total
            .with_label_values(&["/analyze/wasm", "wasm_profile_error"])
            .inc();
        AppError::Internal(format!("Contract profiling failed: {}", e))
    })?;
    state
        .metrics
        .simulation_latency_seconds
        .with_label_values(&["/analyze/wasm"])
        .observe(start_time.elapsed().as_secs_f64());
    state
        .metrics
        .simulation_requests_total
        .with_label_values(&["/analyze/wasm", "LOCAL"])
        .inc();

    let sim_result = simulation::SimulationResult {
        resources,
        transaction_hash: None,
        latest_ledger: 0,
        cost_stroops: 0,
        state_dependency: None,
        ttl_analysis: None,
        transaction_data: String::new(),
        call_graph: None,
        state_snapshot: None,
        protocol_version: payload.protocol_version.unwrap_or(20),
    };

    let report = to_report(&sim_result, &state.insights_engine);
    state
        .metrics
        .resource_utilization_percent
        .with_label_values(&["efficiency_score"])
        .set(report.nutrition.efficiency_score as f64);

    Ok(Json(report))
}

async fn metrics_handler(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let metric_families = state.metrics.registry.gather();
    let encoder = TextEncoder::new();
    let mut buffer = Vec::new();
    encoder
        .encode(&metric_families, &mut buffer)
        .map_err(|e| AppError::Internal(format!("Failed to encode Prometheus metrics: {}", e)))?;
    let output = String::from_utf8(buffer)
        .map_err(|e| AppError::Internal(format!("Metrics output encoding error: {}", e)))?;
    Ok((
        StatusCode::OK,
        [("Content-Type", encoder.format_type().to_string())],
        output,
    ))
}

async fn analyze_wasm_profile(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ProfileWasmRequest>,
) -> Result<Json<ProfileResponse>, AppError> {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

    tracing::info!(
        function_name = %payload.function_name,
        "Received WASM profile request"
    );

    let wasm_bytes = BASE64
        .decode(&payload.wasm_bytes)
        .map_err(|e| AppError::BadRequest(format!("Invalid base64 WASM data: {}", e)))?;

    let function_name = payload.function_name.clone();
    let args = payload.args.clone();

    let result = tokio::time::timeout(
        state.simulation_timeout,
        tokio::task::spawn_blocking(move || {
            simulation::profile_contract_with_flamegraph(wasm_bytes, function_name, args)
        }),
    )
    .await
    .map_err(|_| {
        AppError::Internal(format!(
            "Profiling request timed out after {} seconds",
            state.simulation_timeout.as_secs()
        ))
    })?
    .map_err(|e| AppError::Internal(format!("Profiling task panicked: {}", e)))?
    .map_err(|e| AppError::BadRequest(format!("Profiling failed: {}", e)))?;

    let (resources, profile) = result;

    Ok(Json(ProfileResponse { profile, resources }))
}

// ── WASM branch analysis handler (Issue #101) ─────────────────────────────────

#[utoipa::path(
    post,
    path = "/analyze/wasm/branches",
    request_body = AnalyzeWasmBranchesRequest,
    responses(
        (status = 200, description = "Branch analysis successful", body = WasmBranchAnalysisResponse),
        (status = 400, description = "Invalid base64 or WASM data"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Branch analysis failed")
    ),
    security(
        ("jwt" = [])
    ),
    tag = "Analysis"
)]
async fn analyze_wasm_branches(
    State(_state): State<Arc<AppState>>,
    Json(payload): Json<AnalyzeWasmBranchesRequest>,
) -> Result<Json<WasmBranchAnalysisResponse>, AppError> {
    use crate::wasm_branch_analysis::analyze_wasm_branches as run_analysis;
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

    tracing::info!(
        function_name = %payload.function_name,
        "Received WASM branch analysis request"
    );

    let wasm_bytes = BASE64
        .decode(&payload.wasm_bytes)
        .map_err(|e| AppError::BadRequest(format!("Invalid base64 WASM data: {}", e)))?;

    let function_name = payload.function_name.clone();
    let args = payload.args.clone().unwrap_or_default();

    let report = tokio::task::spawn_blocking(move || run_analysis(wasm_bytes, function_name, args))
        .await
        .map_err(|e| AppError::Internal(format!("Branch analysis task panicked: {}", e)))?
        .map_err(|e| AppError::Internal(format!("Branch analysis failed: {}", e)))?;

    tracing::info!(
        function_name = %payload.function_name,
        total_branch_count = report.total_branch_count,
        simulated_paths = report.simulated_paths.len(),
        distinct_profiles = report.distinct_profiles,
        worst_cpu = report.worst_case_resources.cpu_instructions,
        worst_ram = report.worst_case_resources.ram_bytes,
        "Branch analysis completed"
    );

    Ok(Json(WasmBranchAnalysisResponse {
        function_name: report.function_name,
        total_branch_count: report.total_branch_count,
        max_nesting_depth: report.max_nesting_depth,
        branch_type_breakdown: report.branch_type_breakdown,
        estimated_paths: report.estimated_paths,
        branches: report.branches,
        simulated_paths: report.simulated_paths,
        baseline_resources: report.baseline_resources,
        worst_case_resources: report.worst_case_resources,
        best_case_resources: report.best_case_resources,
        distinct_profiles: report.distinct_profiles,
        coverage_note: report.coverage_note,
    }))
}

#[utoipa::path(
    post,
    path = "/analyze/optimize-limits",
    request_body = OptimizeLimitsRequest,
    responses(
        (status = 200, description = "Resource optimization successful", body = OptimizeLimitsResponse),
        (status = 500, description = "Optimization failed")
    ),
    tag = "Analysis"
)]
async fn optimize_limits(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<OptimizeLimitsRequest>,
) -> Result<Json<OptimizeLimitsResponse>, AppError> {
    tracing::info!(
        "Optimizing limits for contract: {}, function: {}",
        payload.contract_id,
        payload.function_name
    );

    let report = state
        .engine
        .optimize_limits(
            &payload.contract_id,
            &payload.function_name,
            payload.args,
            payload.safety_margin,
        )
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(OptimizeLimitsResponse {
        cpu: report.cpu,
        ram: report.ram,
        ledger_read: report.ledger_read,
        ledger_write: report.ledger_write,
        recommended: report.recommended,
    }))
}

// ── Compare types ────────────────────────────────────────────────────────────

#[derive(Serialize, ToSchema)]
pub struct CompareApiResponse {
    pub report: RegressionReport,
}

// ── Compare handler ──────────────────────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/analyze/compare",
    request_body(content_type = "multipart/form-data", content = String,
        description = "Multipart form with fields: mode (local_vs_local|local_vs_deployed), current_wasm, base_wasm (files), contract_id, function_name, args (text)"
    ),
    responses(
        (status = 200, description = "Comparison report", body = CompareApiResponse),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Comparison failed")
    ),
    tag = "Analysis"
)]
async fn compare_handler(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<CompareApiResponse>, AppError> {
    let mut mode_str: Option<String> = None;
    let mut current_wasm_bytes: Option<Vec<u8>> = None;
    let mut base_wasm_bytes: Option<Vec<u8>> = None;
    let mut contract_id: Option<String> = None;
    let mut function_name: Option<String> = None;
    let mut args: Vec<String> = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("Failed to read multipart field: {}", e)))?
    {
        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "mode" => {
                mode_str = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| AppError::BadRequest(format!("Invalid mode field: {}", e)))?,
                );
            }
            "current_wasm" => {
                current_wasm_bytes = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| {
                            AppError::BadRequest(format!("Failed to read current_wasm: {}", e))
                        })?
                        .to_vec(),
                );
            }
            "base_wasm" => {
                base_wasm_bytes = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| {
                            AppError::BadRequest(format!("Failed to read base_wasm: {}", e))
                        })?
                        .to_vec(),
                );
            }
            "contract_id" => {
                contract_id =
                    Some(field.text().await.map_err(|e| {
                        AppError::BadRequest(format!("Invalid contract_id: {}", e))
                    })?);
            }
            "function_name" => {
                function_name =
                    Some(field.text().await.map_err(|e| {
                        AppError::BadRequest(format!("Invalid function_name: {}", e))
                    })?);
            }
            "args" => {
                let args_json = field
                    .text()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("Invalid args: {}", e)))?;
                args = serde_json::from_str(&args_json).unwrap_or_default();
            }
            _ => { /* ignore unknown fields */ }
        }
    }

    let mode = mode_str.unwrap_or_else(|| "local_vs_local".to_string());

    let compare_mode = match mode.as_str() {
        "local_vs_local" => {
            let current_bytes = current_wasm_bytes
                .ok_or_else(|| AppError::BadRequest("Missing current_wasm file".to_string()))?;
            let base_bytes = base_wasm_bytes
                .ok_or_else(|| AppError::BadRequest("Missing base_wasm file".to_string()))?;

            let current_tmp = write_temp_wasm(&current_bytes)?;
            let base_tmp = write_temp_wasm(&base_bytes)?;

            CompareMode::LocalVsLocal {
                current_wasm: current_tmp,
                base_wasm: base_tmp,
            }
        }
        "local_vs_deployed" => {
            let current_bytes = current_wasm_bytes
                .ok_or_else(|| AppError::BadRequest("Missing current_wasm file".to_string()))?;
            let cid = contract_id
                .ok_or_else(|| AppError::BadRequest("Missing contract_id".to_string()))?;
            let fname = function_name
                .ok_or_else(|| AppError::BadRequest("Missing function_name".to_string()))?;

            let current_tmp = write_temp_wasm(&current_bytes)?;

            CompareMode::LocalVsDeployed {
                current_wasm: current_tmp,
                contract_id: cid,
                function_name: fname,
                args,
            }
        }
        other => {
            return Err(AppError::BadRequest(format!(
                "Unknown mode '{}'. Use 'local_vs_local' or 'local_vs_deployed'",
                other
            )));
        }
    };

    let report = comparison::run_comparison(&state.engine, compare_mode)
        .await
        .map_err(|e| AppError::Internal(format!("Comparison failed: {}", e)))?;

    Ok(Json(CompareApiResponse { report }))
}

/// Write WASM bytes to a temporary file and return the path.
fn write_temp_wasm(bytes: &[u8]) -> Result<std::path::PathBuf, AppError> {
    use std::io::Write;
    let mut tmp = tempfile::Builder::new()
        .suffix(".wasm")
        .tempfile()
        .map_err(|e| AppError::Internal(format!("Failed to create temp file: {}", e)))?;
    tmp.write_all(bytes)
        .map_err(|e| AppError::Internal(format!("Failed to write temp file: {}", e)))?;
    let (_, path) = tmp
        .keep()
        .map_err(|e| AppError::Internal(format!("Failed to persist temp file: {}", e)))?;
    Ok(path)
}

// ── Gas Golfing Types ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize, ToSchema)]
pub struct GasGolfingRequest {
    /// Base64-encoded WASM bytecode
    #[schema(example = "AGFzbQEAAAABBgFgAX8BfwMCAQAFAwMADAEAAQgBAUcBAQABAQgBAUcBAQACAgcABAEGCw==")]
    pub wasm_bytes: String,
    /// Contract name for identification
    #[schema(example = "my_contract")]
    pub contract_name: String,
}

#[derive(Serialize, ToSchema)]
pub struct GasGolfingResponse {
    pub report: GasGolfingReport,
}

// ── Gas Golfing Handler ───────────────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/analyze/gas-golfing",
    request_body = GasGolfingRequest,
    responses(
        (status = 200, description = "Gas golfing analysis completed", body = GasGolfingResponse),
        (status = 400, description = "Invalid WASM data"),
        (status = 500, description = "Analysis failed")
    ),
    tag = "Analysis"
)]
async fn analyze_gas_golfing(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<GasGolfingRequest>,
) -> Result<Json<GasGolfingResponse>, AppError> {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

    tracing::info!(
        contract_name = %payload.contract_name,
        "Received gas golfing analysis request"
    );

    let wasm_bytes = BASE64
        .decode(&payload.wasm_bytes)
        .map_err(|e| AppError::BadRequest(format!("Invalid base64 WASM data: {}", e)))?;

    let contract_name = payload.contract_name.clone();

    let report = tokio::task::spawn_blocking(move || {
        state
            .gas_golfing_analyzer
            .analyze_wasm(&wasm_bytes, &contract_name)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Gas golfing analysis task panicked: {}", e)))?;

    Ok(Json(GasGolfingResponse { report }))
}

// ── Fee Market API Handlers ──────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/fees/recommend",
    params(
        ("inclusion_speed" = Option<String>, Query, description = "Desired inclusion speed: next_ledger, next_3_ledgers, economy, standard, priority"),
        ("safety_margin" = Option<f64>, Query, description = "Custom safety margin (default 0.10)")
    ),
    responses(
        (status = 200, description = "Fee recommendation successful", body = FeeRecommendationResponse),
        (status = 500, description = "Failed to generate recommendation")
    ),
    tag = "Fee Market"
)]
async fn fee_recommend(
    State(state): State<Arc<AppState>>,
) -> Result<Json<FeeRecommendationResponse>, AppError> {
    tracing::info!("Generating fee recommendation");

    // Get recent samples for analysis
    let samples = state
        .fee_store
        .get_recent_samples(100)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to fetch fee data: {}", e)))?;

    // Get current ledger from latest sample or use 0
    let current_ledger = samples
        .first()
        .map(|s| s.ledger_sequence as u64)
        .unwrap_or(0);

    // Generate prediction
    let prediction = state.fee_analytics_engine.predict(&samples, current_ledger);
    let market_conditions = state
        .fee_analytics_engine
        .get_market_conditions(&samples, current_ledger);
    let model_breakdown = state.fee_analytics_engine.get_model_breakdown(&samples);

    // Determine recommended bid based on prediction
    let (recommended_bid, expected_ledgers) = (prediction.priority_bid, 1);

    Ok(Json(FeeRecommendationResponse {
        recommended_bid,
        resource_fee_estimate: 0, // Will be calculated based on transaction resources
        total_estimated_cost: recommended_bid,
        inclusion_confidence: prediction.confidence_score,
        expected_inclusion_ledgers: expected_ledgers,
        market_conditions,
        model_breakdown,
        timestamp: chrono::Utc::now(),
    }))
}

#[utoipa::path(
    get,
    path = "/fees/history",
    params(
        ("limit" = Option<i64>, Query, description = "Number of recent ledgers to retrieve"),
        ("from_ledger" = Option<i64>, Query, description = "Starting ledger sequence"),
        ("to_ledger" = Option<i64>, Query, description = "Ending ledger sequence")
    ),
    responses(
        (status = 200, description = "Fee history retrieved successfully", body = FeeHistoryResponse),
        (status = 500, description = "Failed to fetch fee history")
    ),
    tag = "Fee Market"
)]
async fn fee_history(
    State(state): State<Arc<AppState>>,
) -> Result<Json<FeeHistoryResponse>, AppError> {
    tracing::info!("Fetching fee history");

    let limit = 50; // Default limit
    let samples = state
        .fee_store
        .get_recent_samples(limit)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to fetch fee history: {}", e)))?;

    let total_count = state
        .fee_store
        .get_sample_count()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to get sample count: {}", e)))?;

    Ok(Json(FeeHistoryResponse {
        samples,
        total_count,
    }))
}

#[utoipa::path(
    get,
    path = "/fees/analytics",
    responses(
        (status = 200, description = "Fee analytics retrieved successfully", body = serde_json::Value),
        (status = 500, description = "Failed to fetch analytics")
    ),
    tag = "Fee Market"
)]
async fn fee_analytics(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    tracing::info!("Fetching fee analytics");

    // Get recent samples for analysis
    let samples = state
        .fee_store
        .get_recent_samples(200)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to fetch fee data: {}", e)))?;

    let current_ledger = samples
        .first()
        .map(|s| s.ledger_sequence as u64)
        .unwrap_or(0);

    let prediction = state.fee_analytics_engine.predict(&samples, current_ledger);
    let market_conditions = state
        .fee_analytics_engine
        .get_market_conditions(&samples, current_ledger);
    let model_breakdown = state.fee_analytics_engine.get_model_breakdown(&samples);

    let response = serde_json::json!({
        "current_ledger": current_ledger,
        "prediction": prediction,
        "market_conditions": market_conditions,
        "model_breakdown": model_breakdown,
        "sample_count": samples.len(),
        "timestamp": chrono::Utc::now(),
    });

    Ok(Json(response))
}

#[derive(OpenApi)]
#[openapi(
    paths(
        analyze, analyze_wasm, optimize_limits, compare_handler,
        auth::challenge_handler, auth::verify_handler, auth::jwks_handler,
        fee_recommend, fee_history, fee_analytics
    ),
    components(schemas(
        AnalyzeRequest, AnalyzeWasmRequest, AnalyzeWasmBranchesRequest,
        WasmBranchAnalysisResponse, ResourceReport,
        OptimizeLimitsRequest, OptimizeLimitsResponse,
        CompareApiResponse, RegressionReport, ResourceDelta, RegressionFlag,
        crate::wasm_branch_analysis::BranchInfo,
        crate::wasm_branch_analysis::BranchType,
        crate::wasm_branch_analysis::BranchTypeBreakdown,
        crate::wasm_branch_analysis::PathResult,
        auth::ChallengeRequest, auth::ChallengeResponse,
        auth::VerifyRequest, auth::VerifyResponse,
        auth::JwkSetResponse, auth::JwkResponse,
        crate::simulation::OptimizationBuffer,
        crate::simulation::SorobanResources,
        FeeRecommendationRequest, FeeRecommendationResponse,
        FeeHistoryRequest, FeeHistoryResponse,
        crate::fee_store::LedgerFeeSample,
        crate::fee_analytics::MarketConditions,
        crate::fee_analytics::ModelBreakdown,
        crate::fee_analytics::TrendDirection
    )),
    tags(
        (name = "Analysis", description = "Soroban contract resource analysis endpoints"),
        (name = "Auth", description = "SEP-10 wallet authentication"),
        (name = "Fee Market", description = "Stellar/Soroban fee market analysis and prediction"),
        (name = "Streaming", description = "WebSocket real-time simulation progress streaming")
    ),
    info(
        title = "SoroScope API",
        version = "0.1.0",
        description = "API for analyzing Soroban smart contract resource consumption and fee market predictions"
    )
)]
struct ApiDoc;

async fn health_check() -> &'static str {
    "OK"
}

async fn registry_providers(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<crate::rpc_provider::ProviderHealthReport>> {
    Json(state.provider_registry.provider_reports().await)
}

async fn registry_peers(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<crate::rpc_provider::PeerHealthReport>> {
    Json(state.provider_registry.peer_reports().await)
}

async fn registry_gossip(
    State(state): State<Arc<AppState>>,
    Json(snapshot): Json<RegistrySnapshot>,
) -> Json<RegistrySnapshot> {
    state.provider_registry.merge_snapshot(snapshot).await;
    Json(state.provider_registry.registry_snapshot().await)
}

#[tokio::main]
async fn main() {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("SoroScope Starting...");

    let config = load_config().expect("Failed to load configuration");
    tracing::info!("SoroScope initialized with config: {:?}", config);
    tracing::info!(
        redis_url = %config.redis_url,
        "Cache config: using in-memory (moka) MVP; Redis URL reserved for future migration"
    );

    let args: Vec<String> = env::args().collect();

    if args.len() > 1 && args[1] == "benchmark" {
        tracing::info!("Starting SoroScope Benchmark...");

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
            if let Err(e) = benchmarks::run_token_benchmark(path, simulation_service.as_ref()).await {
                eprintln!("Benchmark failed: {}", e);
            let db_path = env::var("SOROSCOPE_DB_PATH")
                .unwrap_or_else(|_| "soroscope_metrics.db".to_string());
            let webhook_url = env::var("SOROSCOPE_ALERT_WEBHOOK_URL").ok();
            let simulation_service = SimulationService::new(db_path, webhook_url)
                .expect("initialize simulation service");
            if let Err(e) = benchmarks::run_token_benchmark(path, &simulation_service).await {
                tracing::error!("Benchmark failed: {}", e);
            }
        } else {
            tracing::error!(
                "Could not find soroban_token_contract.wasm. Build the contract first."
            );
        }

        return;
    }

    // ── CLI: merkle subcommand ──────────────────────────────────────────
    if args.len() > 1 && args[1] == "merkle" {
        if args.len() < 4 {
            eprintln!("Usage: soroscope-core merkle <build|build-file|proof> <args>");
            eprintln!("Commands:");
            eprintln!("  build <leaf1> <leaf2> ...              Build a Merkle tree and print the root hash");
            eprintln!("  build-file <file>                      Build a Merkle tree from a newline-delimited leaf file");
            eprintln!("  proof <leaf_index> <leaf1> <leaf2> ... Generate a Merkle proof for the given leaf index");
            std::process::exit(1);
        }

        let command = &args[2];
        match command.as_str() {
            "build" => {
                if args.len() < 4 {
                    eprintln!("Usage: soroscope-core merkle build <leaf1> <leaf2> ...");
                    std::process::exit(1);
                }
                let leaves: Vec<Vec<u8>> = args[3..].iter().map(|arg| arg.as_bytes().to_vec()).collect();
                let mut tree = merkle_tree::MerkleTree::new(32);
                match tree.build(leaves) {
                    Ok(()) => println!("{}", tree.get_root_hex()),
                    Err(err) => {
                        eprintln!("Error building Merkle tree: {}", err);
                        std::process::exit(1);
                    }
                }
            }
            "proof" => {
                if args.len() < 5 {
                    eprintln!("Usage: soroscope-core merkle proof <leaf_index> <leaf1> <leaf2> ...");
                    std::process::exit(1);
                }
                let leaf_index = match args[3].parse::<usize>() {
                    Ok(index) => index,
                    Err(_) => {
                        eprintln!("Leaf index must be a non-negative integer.");
                        std::process::exit(1);
                    }
                };
                let leaves: Vec<Vec<u8>> = args[4..].iter().map(|arg| arg.as_bytes().to_vec()).collect();
                let mut tree = merkle_tree::MerkleTree::new(32);
                if let Err(err) = tree.build(leaves) {
                    eprintln!("Error building Merkle tree: {}", err);
                    std::process::exit(1);
                }
                let proof = match tree.generate_proof(leaf_index) {
                    Ok(proof) => proof,
                    Err(err) => {
                        eprintln!("Error generating Merkle proof: {}", err);
                        std::process::exit(1);
                    }
                };
                let output = serde_json::json!({
                    "root": tree.get_root_hex(),
                    "leaf_index": leaf_index,
                    "leaf_count": tree.leaf_count(),
                    "proof": proof,
                });
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            }
            "build-file" => {
                if args.len() < 4 {
                    eprintln!("Usage: soroscope-core merkle build-file <file>");
                    eprintln!("  Each non-empty line in <file> is treated as a leaf value.");
                    std::process::exit(1);
                }
                let file_path = &args[3];
                let content = match std::fs::read_to_string(file_path) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Error reading {}: {}", file_path, e);
                        std::process::exit(1);
                    }
                };
                let leaves: Vec<Vec<u8>> = content
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(|l| l.as_bytes().to_vec())
                    .collect();
                if leaves.is_empty() {
                    eprintln!("Error: file contains no leaf values.");
                    std::process::exit(1);
                }
                let mut tree = merkle_tree::MerkleTree::new(32);
                match tree.build(leaves) {
                    Ok(()) => println!("{}", tree.get_root_hex()),
                    Err(err) => {
                        eprintln!("Error building Merkle tree: {}", err);
                        std::process::exit(1);
                    }
                }
            }
            unknown => {
                eprintln!("Unknown merkle command: {}", unknown);
                eprintln!("Available commands: build, build-file, proof");
                std::process::exit(1);
            }
        }

        return;
    }

    // Default Web Server
    println!("SoroScope CLI Initialized. Run with 'benchmark' argument to profile token contract.");

    // build our application with a single route
    let app = Router::new()
        .route(
            "/",
            get(|| async {
                "Hello from SoroScope! Use POST /simulations/analyze to persist + compare simulation metrics."
            }),
        )
        .route("/health", get(|| async { "ok" }))
        .route(
            "/error",
            get(|| async { Err::<&str, AppError>(AppError::BadRequest("Test error".to_string())) }),
        )
        .route("/simulations/analyze", post(analyze_simulation))
        .with_state(simulation_service);
    // ── CLI: compare subcommand ──────────────────────────────────────────
    if args.len() > 1 && args[1] == "compare" {
        if args.len() < 4 {
            eprintln!("Usage: soroscope-core compare <current.wasm> <base.wasm>");
            eprintln!("\nCompare two WASM contract versions and detect resource regressions.");
            eprintln!("\nArguments:");
            eprintln!("  <current.wasm>  Path to the new (current) version WASM file");
            eprintln!("  <base.wasm>     Path to the reference (base) version WASM file");
            std::process::exit(1);
        }

        let current_path = PathBuf::from(&args[2]);
        let base_path = PathBuf::from(&args[3]);

        if !current_path.exists() {
            eprintln!(
                "Error: Current WASM file not found: {}",
                current_path.display()
            );
            std::process::exit(1);
        }
        if !base_path.exists() {
            eprintln!("Error: Base WASM file not found: {}", base_path.display());
            std::process::exit(1);
        }

        let providers = build_providers(&config);
        let registry = rpc_provider::ProviderRegistry::new(providers);
        let engine = SimulationEngine::with_registry(std::sync::Arc::clone(&registry));

        let compare_mode = comparison::CompareMode::LocalVsLocal {
            current_wasm: current_path,
            base_wasm: base_path,
        };

        match comparison::run_comparison(&engine, compare_mode).await {
            Ok(report) => {
                comparison::print_report(&report);
            }
            Err(e) => {
                eprintln!("Error: Comparison failed: {}", e);
                std::process::exit(1);
            }
        }

        return;
    }

    // ── CLI: export subcommand ──────────────────────────────────────────
    if args.len() > 1 && args[1] == "export" {
        if args.len() < 6 {
            eprintln!(
                "Usage: soroscope-core export <contract_id> <function> <args_json> <output_file>"
            );
            eprintln!("\nSimulate a transaction and export the touched state to a JSON file.");
            std::process::exit(1);
        }

        let contract_id = &args[2];
        let function = &args[3];
        let args_json = &args[4];
        let output_file = &args[5];

        let parsed_args: Vec<String> = serde_json::from_str(args_json).unwrap_or_default();

        let providers = build_providers(&config);
        let registry = rpc_provider::ProviderRegistry::new(providers);
        let engine = SimulationEngine::with_registry(std::sync::Arc::clone(&registry));

        match engine
            .simulate_from_contract_id(contract_id, function, parsed_args, None, None, None)
            .await
        {
            Ok(result) => {
                if let Some(snapshot) = result.state_snapshot {
                    let json = serde_json::to_string_pretty(&snapshot).unwrap();
                    if let Err(e) = std::fs::write(output_file, json) {
                        eprintln!("Error: Failed to write snapshot to {}: {}", output_file, e);
                        std::process::exit(1);
                    }
                    println!("State snapshot exported to {}", output_file);
                } else {
                    eprintln!("Error: No state snapshot generated.");
                    std::process::exit(1);
                }
            }
            Err(e) => {
                eprintln!("Error: Simulation failed: {}", e);
                std::process::exit(1);
            }
        }

        return;
    }

    // ── CLI: restore subcommand ──────────────────────────────────────────
    if args.len() > 1 && args[1] == "restore" {
        if args.len() < 6 {
            eprintln!("Usage: soroscope-core restore <snapshot_file> <contract_id> <function> <args_json>");
            eprintln!("\nRestore state from a JSON file and run a simulation.");
            std::process::exit(1);
        }

        let snapshot_file = &args[2];
        let contract_id = &args[3];
        let function = &args[4];
        let args_json = &args[5];

        let snapshot_json =
            std::fs::read_to_string(snapshot_file).expect("Failed to read snapshot file");
        let snapshot: crate::simulation::SimulationStateSnapshot =
            serde_json::from_str(&snapshot_json).expect("Failed to parse snapshot JSON");

        let parsed_args: Vec<String> = serde_json::from_str(args_json).unwrap_or_default();

        let providers = build_providers(&config);
        let registry = rpc_provider::ProviderRegistry::new(providers);
        let engine = SimulationEngine::with_registry(std::sync::Arc::clone(&registry));

        match engine
            .simulate_from_contract_id(
                contract_id,
                function,
                parsed_args,
                Some(snapshot.ledger_entries),
                None,
                None,
            )
            .await
        {
            Ok(result) => {
                println!("Simulation successful with restored state.");
                println!("Resources: {:?}", result.resources);
                if let Some(deps) = result.state_dependency {
                    println!("State dependencies: {} entries", deps.len());
                }
            }
            Err(e) => {
                eprintln!("Error: Simulation failed: {}", e);
                std::process::exit(1);
            }
        }

        return;
    }

    tracing::info!("Starting SoroScope API Server...");

    let auth_state = Arc::new(auth::AuthState::new(
        config.jwt_private_key.clone(),
        None,
        config.network_passphrase.clone(),
        config.emergency_verification_paused,
    ));
    tracing::info!(
        "SEP-10 server account: {}",
        auth_state.server_stellar_address()
    );
    // ── Multi-node RPC setup ────────────────────────────────────────────
    let providers = build_providers(&config);
    let provider_names: Vec<&str> = providers.iter().map(|p| p.name.as_str()).collect();
    tracing::info!(providers = ?provider_names, "RPC provider pool");

    let registry = ProviderRegistry::new_with_config(providers, build_registry_config(&config));
    tracing::info!(
        instance_id = registry.instance_id(),
        public_url = ?registry.public_base_url(),
        "Provider registry initialized"
    );

    // Spawn background health checker.
    let health_interval = std::time::Duration::from_secs(config.health_check_interval_secs);
    let _health_handle = registry.spawn_health_checker(health_interval);
    tracing::info!(
        interval_secs = config.health_check_interval_secs,
        "Background RPC health checker started"
    );

    let gossip_interval = std::time::Duration::from_secs(config.gossip_interval_secs);
    let _gossip_handle = registry.spawn_gossip_task(gossip_interval);
    tracing::info!(
        interval_secs = config.gossip_interval_secs,
        "Provider gossip sync started"
    );

    let simulation_timeout = std::time::Duration::from_secs(config.simulation_timeout_secs);
    let simulation_mode = SimulationMode::from_config(&config.simulation_mode)
        .expect("Invalid simulation mode configuration");
    tracing::info!(
        timeout_secs = config.simulation_timeout_secs,
        "Simulation timeout configured"
    );
    tracing::info!(mode = ?simulation_mode, "Simulation mode configured");

    // ── Fee Market Setup ────────────────────────────────────────────────
    let database_url = &config.database_url;
    tracing::info!(database_url = %database_url, "Initializing database");

    let db_pool = sqlx::SqlitePool::connect(database_url)
        .await
        .expect("Failed to connect to database");

    // Run migrations
    sqlx::migrate!()
        .run(&db_pool)
        .await
        .expect("Failed to run database migrations");

    tracing::info!("Database migrations completed");

    let fee_store = Arc::new(FeeStore::new(db_pool.clone()));
    let fee_analytics_engine = FeeAnalyticsEngine::new();
    let job_queue_config = JobQueueConfig {
        job_timeout_secs: config.job_timeout_secs,
        max_concurrent_jobs: config.max_concurrent_jobs,
        ..JobQueueConfig::default()
    };
    let job_queue = JobQueue::new(database_url, &config.redis_url, job_queue_config.clone())
        .await
        .expect("Failed to initialize job queue");
    // ── WebSocket event bus ─────────────────────────────────────────────
    let simulation_bus = SimulationBus::new();

    let job_worker = JobWorker::new(
        job_queue.clone(),
        SimulationEngine::with_registry_and_timeout_and_mode(
            Arc::clone(&registry),
            simulation_timeout,
            simulation_mode,
        ),
        InsightsEngine::new(),
        job_queue_config,
    )
    .with_bus(Arc::clone(&simulation_bus));

    tokio::spawn(async move {
        job_worker.run().await;
    });

    // ── Distributed Job Queue Setup ─────────────────────────────────────
    let job_config = JobQueueConfig {
        job_timeout_secs: config.job_timeout_secs,
        max_concurrent_jobs: config.max_concurrent_jobs,
        ..Default::default()
    };

    let job_queue = JobQueue::new(&config.database_url, &config.redis_url, job_config.clone())
        .await
        .expect("Failed to initialize JobQueue");

    // Spawn background cleanup task
    job_queue.spawn_cleanup_task();

    // Spawn worker
    let worker = JobWorker::new(
        job_queue.clone(),
        SimulationEngine::with_registry_and_timeout(Arc::clone(&registry), simulation_timeout),
        InsightsEngine::new(),
        job_config,
    );

    tokio::spawn(async move {
        worker.run().await;
    });

    tracing::info!("Job queue and worker started (Redis backend)");

    // Start background fee collector if enabled
    if config.fee_analysis_enabled {
        let collector_config = FeeCollectorConfig {
            collection_interval_secs: config.fee_collection_interval_secs,
            batch_size: 10,
            request_timeout: std::time::Duration::from_secs(10),
        };

        let collector = Arc::new(FeeCollector::new(
            Arc::clone(&registry),
            Arc::clone(&fee_store),
            collector_config,
        ));

        tokio::spawn(async move {
            collector.run_collection_loop().await;
        });

        tracing::info!(
            interval_secs = config.fee_collection_interval_secs,
            "Fee market collector started"
        );

        // Schedule periodic cleanup of old fee data
        let cleanup_store = Arc::clone(&fee_store);
        let retention_days = config.fee_retention_days;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600)); // Every hour
            loop {
                interval.tick().await;
                if let Err(e) = cleanup_store
                    .cleanup_old_samples(retention_days as i32)
                    .await
                {
                    tracing::error!(error = %e, "Failed to cleanup old fee samples");
                }
            }
        });
    } else {
        tracing::info!("Fee market analysis is disabled");
    }

    // ── Persistent Cache Setup (L2) ─────────────────────────────────────
    let sled_db = sled::open("soroscope_cache").expect("Failed to open sled database");
    let simulation_cache = SimulationCache::new(&sled_db);
    let contract_cache = Arc::new(ContractCache::new(&sled_db));

    let app_state = Arc::new(AppState {
        engine: SimulationEngine::with_registry_and_cache(
            Arc::clone(&registry),
            Arc::clone(&contract_cache),
        ),
        provider_registry: Arc::clone(&registry),
        cache: simulation_cache,
        insights_engine: InsightsEngine::new(),
        gas_golfing_analyzer: GasGolfingAnalyzer::new(),
        simulation_timeout,
        job_queue,
        fee_analytics_engine,
        fee_store,
        metrics: Arc::new(AppMetrics::new().expect("Failed to initialize Prometheus metrics")),
        simulation_bus,
    });

    let cors = CorsLayer::new().allow_origin(Any);

    let protected = Router::new()
        .route("/analyze", post(analyze))
        .route("/analyze/wasm", post(analyze_wasm))
        .route("/analyze/wasm/branches", post(analyze_wasm_branches))
        .route("/analyze/optimize-limits", post(optimize_limits))
        .route("/analyze/compare", post(compare_handler))
        .route("/analyze/gas-golfing", post(analyze_gas_golfing))
        .route_layer(middleware::from_fn(auth::auth_middleware));

    let app = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route(
            "/",
            get(|| async {
                "Hello from SoroScope! Usage: cargo run -p soroscope-core -- benchmark"
            }),
        )
        .route("/health", get(health_check))
        .route("/metrics", get(metrics_handler))
        .route("/auth/challenge", post(auth::challenge_handler))
        .route("/auth/verify", post(auth::verify_handler))
        .route("/auth/emergency-pause", post(auth::emergency_pause_handler))
        .route("/auth/jwks", get(auth::jwks_handler))
        // Fee market routes (public access)
        .route("/fees/recommend", get(fee_recommend))
        .route("/fees/history", get(fee_history))
        .route("/fees/analytics", get(fee_analytics))
        // WebSocket streaming (Issue #105) — no auth required on the upgrade;
        // the client passes the job_id in the path.
        .route("/ws/jobs/:job_id", get(ws::ws_handler))
        .merge(protected)
        .layer(Extension(auth_state))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(app_state); // ← thread AppState through all handlers

    let bind_addr = format!("0.0.0.0:{}", config.server_port);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("Failed to bind to address");

    tracing::info!(
        "Server listening on http://{}",
        listener.local_addr().unwrap()
    );
    tracing::info!(
        "Swagger UI available at http://{}/swagger-ui",
        listener.local_addr().unwrap()
    );

    axum::serve(listener, app)
        .await
        .expect("Server failed to start");
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(any())]
mod tests {
    use super::*;
    use crate::simulation::{SimulationError, SorobanResources};

    #[test]
    fn test_error_mapping_node_error() {
        let sim_err = SimulationError::NodeError("Invalid contract ID".to_string());
        let app_err: AppError = sim_err.into();

        match app_err {
            AppError::BadRequest(msg) => {
                assert!(msg.contains("Invalid contract ID"));
            }
            _ => panic!("Expected BadRequest, got {:?}", app_err),
        }
    }

    #[test]
    fn test_error_mapping_invalid_contract() {
        let sim_err = SimulationError::InvalidContract("Contract not found".to_string());
        let app_err: AppError = sim_err.into();

        match app_err {
            AppError::BadRequest(msg) => {
                assert!(msg.contains("Contract not found"));
            }
            _ => panic!("Expected BadRequest, got {:?}", app_err),
        }
    }

    #[test]
    fn test_error_mapping_timeout() {
        let sim_err = SimulationError::NodeTimeout;
        let app_err: AppError = sim_err.into();

        match app_err {
            AppError::Internal(msg) => {
                assert!(msg.contains("timed out"));
            }
            _ => panic!("Expected Internal, got {:?}", app_err),
        }
    }

    #[test]
    fn test_error_mapping_rpc_request_failed() {
        let sim_err = SimulationError::RpcRequestFailed("Connection refused".to_string());
        let app_err: AppError = sim_err.into();

        match app_err {
            AppError::Internal(msg) => {
                assert!(msg.contains("Connection refused"));
            }
            _ => panic!("Expected Internal, got {:?}", app_err),
        }
    }

    #[test]
    fn test_error_mapping_network_error() {
        // Create a mock reqwest error (we can't easily create one, so test via RpcRequestFailed)
        let sim_err = SimulationError::RpcRequestFailed("Network unreachable".to_string());
        let app_err: AppError = sim_err.into();

        match app_err {
            AppError::Internal(msg) => {
                assert!(msg.contains("Network unreachable"));
            }
            _ => panic!("Expected Internal, got {:?}", app_err),
        }
    }

    #[test]
    fn test_resource_report_includes_cost_stroops() {
        let sim_result = SimulationResult {
            resources: SorobanResources {
                cpu_instructions: 1000000,
                ram_bytes: 2048,
                ledger_read_bytes: 512,
                ledger_write_bytes: 256,
                transaction_size_bytes: 1024,
            },
            transaction_hash: None,
            latest_ledger: 12345,
            cost_stroops: 5000,
            state_dependency: None,
            ttl_analysis: None,
            transaction_data: "AAA".to_string(),
            call_graph: None,
            state_snapshot: None,
            protocol_version: 0,
        };

        let insights_engine = InsightsEngine::new();
        let report = to_report(&sim_result, &insights_engine);

        assert_eq!(report.cost_stroops, 5000);
        assert_eq!(report.cpu_instructions, 1000000);
        assert_eq!(report.ram_bytes, 2048);
        assert_eq!(report.ledger_read_bytes, 512);
        assert_eq!(report.ledger_write_bytes, 256);
        assert_eq!(report.transaction_size_bytes, 1024);
    }

    #[test]
    fn test_app_config_default_simulation_timeout() {
        // Verify the default timeout function returns 30 seconds
        assert_eq!(default_simulation_timeout_secs(), 30);
    }

    #[test]
    fn test_app_config_default_simulation_mode() {
        assert_eq!(default_simulation_mode(), "failover");
    }

    #[test]
    fn test_simulation_engine_timeout_configurable() {
        use std::time::Duration;

        // Create a mock registry (we can't easily create one without mocking)
        // Instead, test that the SimulationEngine has timeout methods
        let engine = SimulationEngine::new("https://test.com".to_string());

        // Default should be 30 seconds
        assert_eq!(engine.timeout(), Duration::from_secs(30));
    }
    // ── API integration tests for /analyze/wasm/profile ──────────────────────

    /// Build a minimal valid WASM module with one exported function `add` that
    /// returns i32 (i32.const 42; end). Mirrors the helper in simulation.rs.
    fn minimal_wasm_bytes() -> Vec<u8> {
        use wasm_encoder::{
            CodeSection, ExportKind, ExportSection, Function, FunctionSection, Module, TypeSection,
            ValType,
        };
        let mut module = Module::new();
        let mut types = TypeSection::new();
        types.ty().function([], [ValType::I32]);
        module.section(&types);
        let mut functions = FunctionSection::new();
        functions.function(0);
        module.section(&functions);
        let mut exports = ExportSection::new();
        exports.export("add", ExportKind::Func, 0);
        module.section(&exports);
        let mut codes = CodeSection::new();
        let mut f = Function::new(vec![]);
        f.instruction(&wasm_encoder::Instruction::I32Const(42));
        f.instruction(&wasm_encoder::Instruction::End);
        codes.function(&f);
        module.section(&codes);
        module.finish()
    }

    fn build_test_app() -> Router {
        use std::sync::Arc;
        let app_state = Arc::new(AppState {
            engine: SimulationEngine::new("https://test.example.com".to_string()),
            cache: SimulationCache::new(),
            insights_engine: InsightsEngine::new(),
            simulation_timeout: std::time::Duration::from_secs(30),
        });
        let auth_state = Arc::new(auth::AuthState::new(
            "test-secret".to_string(),
            None,
            "Test SDF Network ; September 2015".to_string(),
        ));
        let protected = Router::new()
            .route("/analyze/wasm/profile", post(analyze_wasm_profile))
            .route_layer(middleware::from_fn(auth::auth_middleware));
        Router::new()
            .merge(protected)
            .layer(Extension(auth_state))
            .with_state(app_state)
    }

    fn make_jwt(secret: &str) -> String {
        use jsonwebtoken::{encode, EncodingKey, Header};
        use serde_json::json;
        let claims = json!({
            "sub": "test-user",
            "exp": 9999999999u64,
        });
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_profile_endpoint_valid_request_returns_200() {
        use axum::body::Body;
        use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
        use http::{Request, StatusCode};
        use tower::ServiceExt;

        let app = build_test_app();
        let wasm_b64 = BASE64.encode(minimal_wasm_bytes());
        let body = serde_json::json!({
            "wasm_bytes": wasm_b64,
            "function_name": "add",
            "args": []
        });
        let token = make_jwt("test-secret");
        let req = Request::builder()
            .method("POST")
            .uri("/analyze/wasm/profile")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", token))
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_profile_endpoint_invalid_base64_returns_400() {
        use axum::body::Body;
        use http::{Request, StatusCode};
        use tower::ServiceExt;

        let app = build_test_app();
        let body = serde_json::json!({
            "wasm_bytes": "!!!not-valid-base64!!!",
            "function_name": "add",
            "args": []
        });
        let token = make_jwt("test-secret");
        let req = Request::builder()
            .method("POST")
            .uri("/analyze/wasm/profile")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", token))
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_profile_endpoint_invalid_wasm_returns_400() {
        use axum::body::Body;
        use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
        use http::{Request, StatusCode};
        use tower::ServiceExt;

        let app = build_test_app();
        let bad_wasm = BASE64.encode(b"this is not wasm");
        let body = serde_json::json!({
            "wasm_bytes": bad_wasm,
            "function_name": "add",
            "args": []
        });
        let token = make_jwt("test-secret");
        let req = Request::builder()
            .method("POST")
            .uri("/analyze/wasm/profile")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", token))
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_profile_endpoint_unknown_function_returns_400() {
        use axum::body::Body;
        use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
        use http::{Request, StatusCode};
        use tower::ServiceExt;

        let app = build_test_app();
        let wasm_b64 = BASE64.encode(minimal_wasm_bytes());
        let body = serde_json::json!({
            "wasm_bytes": wasm_b64,
            "function_name": "nonexistent_function",
            "args": []
        });
        let token = make_jwt("test-secret");
        let req = Request::builder()
            .method("POST")
            .uri("/analyze/wasm/profile")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", token))
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_profile_endpoint_no_jwt_returns_401() {
        use axum::body::Body;
        use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
        use http::{Request, StatusCode};
        use tower::ServiceExt;

        let app = build_test_app();
        let wasm_b64 = BASE64.encode(minimal_wasm_bytes());
        let body = serde_json::json!({
            "wasm_bytes": wasm_b64,
            "function_name": "add",
            "args": []
        });
        let req = Request::builder()
            .method("POST")
            .uri("/analyze/wasm/profile")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}

async fn analyze_simulation(
    State(simulation_service): State<Arc<SimulationService>>,
    Json(metric): Json<SimulationMetric>,
) -> Result<Json<AnalysisResult>, AppError> {
    let result = simulation_service.record_and_analyze(metric).await?;
    Ok(Json(result))
}
