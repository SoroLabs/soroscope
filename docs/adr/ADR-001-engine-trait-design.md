# ADR-001: Engine Trait Design

- Status: accepted
- Date: 2026-10-19

## Context

New contributors ask when to add a new trait in `core/src/engine/traits.rs`
vs extending an existing one.

## Decision

Keep small, role-based traits: `SimulationProvider` (RPC), `StateCache`
(storage), `Parser` (parsing). Add a new trait only for a new
capability/role with distinct implementations; extend an existing trait
only when the method belongs to that same role.

## Consequences

- Mocks stay small; each role can be faked independently.
- Trait growth is bounded per role.
