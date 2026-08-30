//! Simulation runners.
//!
//! A runner is anything that can turn a [`ContractInvocation`] into a
//! [`SimulationResult`]. Two flavours ship today:
//!
//! * [`LocalRunner`] — in-process WASM execution via `soroban-env-host`,
//!   no network hop, deterministic against an injected ledger state.
//! * The existing RPC path on [`crate::simulation::SimulationEngine`] —
//!   calls the Stellar `simulateTransaction` endpoint.
//!
//! The engine tries local first and falls back to RPC when no WASM is
//! loaded for the target contract or a retriable error occurs.

pub mod local;

pub use local::{ContractInvocation, LocalRunner};
