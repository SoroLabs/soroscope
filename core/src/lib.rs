use std::sync::Arc;

pub mod auth;
pub mod benchmarks;
pub mod cache;
pub mod call_trace_parser;
pub mod comparison;
pub mod contract_registry;
pub mod cors;
pub mod errors;
pub mod gas_golfing;
pub mod insights;
pub mod jobs;
pub mod merkle_tree;
pub mod parser;
pub mod routing;
pub mod rpc_provider;
pub mod rpc_throttle;
pub mod runner;
pub mod simulation;
pub mod simulation_service;
pub mod task_queue;
pub mod trace_propagation;
pub mod wasm_branch_analysis;
pub mod webhook_validation;
pub mod webhooks;
pub mod worker_pool;
pub mod ws;
pub mod xdr_decoder;

pub use errors::AppError;

#[derive(Clone)]
pub struct AppState {
    pub job_queue: Arc<jobs::JobQueue>,
    pub simulation_bus: Arc<ws::SimulationBus>,
}

#[cfg(test)]
pub mod fuzz_simulation;
#[cfg(test)]
pub mod fuzz_tests;
