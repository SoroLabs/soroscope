# ADR-002: Database Choice

- Status: accepted
- Date: 2026-10-19

## Context

Choose between PostgreSQL, SQLite, and in-memory storage.

## Decision

- PostgreSQL for production (concurrency, durability, tooling).
- SQLite for local dev/tests (zero-ops file DB).
- In-memory for hermetic unit tests (no I/O).

Abstract behind the `StateCache` trait so backends are swappable.

## Consequences

- Production gets durability; tests stay fast and hermetic.
