# ADR-003: RPC Provider Architecture

- Status: accepted
- Date: 2026-10-19

## Context

Simulation needs Soroban RPC access with failover and testability.

## Decision

Use a pluggable provider behind `SimulationProvider`: a production
`rpc_provider` with health scoring/circuit breaker, plus mock providers
in tests. Callers depend on the trait, never on a concrete endpoint.

## Consequences

- Providers can be swapped/added without touching the engine.
- Tests inject fakes; no network required.
