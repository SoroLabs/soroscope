#![deny(warnings)]

use std::sync::Arc;

pub mod auth;
pub mod benchmarks;
pub mod branch_coverage;
pub mod cache;
pub mod call_trace_parser;
pub mod comparison;
pub mod contract_registry;
pub mod cors;
pub mod differential;
pub mod engine;
pub mod errors;
pub mod failure;
pub mod fee_analytics;
pub mod fee_collector;
pub mod fee_quote;
pub mod fee_store;
pub mod gas_golfing;
pub mod graphql;
pub mod grpc;
pub mod host_import_heat;
pub mod insights;
pub mod jobs;
pub mod leader_lock;
pub mod logging;
pub mod merkle_tree;
pub mod metrics;
pub mod parser;
pub mod parsed_module;
pub mod routing;
pub mod rpc_provider;
pub mod rpc_throttle;
pub mod runner;
pub mod resource_fixtures;
pub mod sac_transfer;
pub mod simulation;
pub mod xdr_decoder;
pub mod wasm_branch_analysis;
pub mod webhook_validation;
pub mod webhooks;
pub mod worker_pool;
pub mod ws;

pub use errors::AppError;
pub use logging::{build_env_filter, init_logging, structured_logging_middleware, LogFormat};
pub use metrics::AppMetrics;
pub use task_queue::{TelemetryEvent, TelemetryEventQueue, TelemetrySubscriber};

#[derive(Clone)]
pub struct AppState {
    pub job_queue: Arc<jobs::JobQueue>,
    pub simulation_bus: Arc<ws::SimulationBus>,
}

#[cfg(test)]
pub mod fuzz_simulation;
#[cfg(test)]
pub mod fuzz_tests;
