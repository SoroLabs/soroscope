# Contract Changes Summary — DID Registry, Governance, Batch Transfer

Closes #1476, #1477, #1478

Branch: `feat/contracts-did-governance-batch-upgrades`

## PR Description

### Summary

This batch implements three contract upgrades from the issue tracker. All changes are scoped to the
three target contracts and their unit tests; no CI configuration, secrets, environment files, or
dependencies were changed.

**#1476 — DID registry (`contracts/did_registry`)**

The original registry stored a single owner-controlled document with basic keys and legacy
`Service` records. It could not express W3C DID Core multi-controller authorization, standard
service endpoints (LinkedDomains, DIDCommMessaging), or retained deactivation history.

- `DIDDocument` now carries `controllers: Vec<Address>` and `service_endpoints: Vec<ServiceEndpoint>`
  (`id`, `service_type`, `uri`), while keeping the legacy `service` field for compatibility.
- `DIDMetadata` gains permanent `deactivated` / `deactivated_at` fields; `deactivate_did(controller, did)`
  marks a document inactive and every subsequent mutation returns `Error::DIDDeactivated`.
- Authorization is controller-based: `require_controller_auth` checks the caller is an active
  controller of a non-deactivated DID. `add_controller` / `remove_controller` support delegation,
  and `add_verification_method` / service endpoint functions accept a controller.
- Key history is preserved: `rotate_verification_method` and `deactivate_verification_method` mark
  old methods `deactivated = true` instead of silently dropping them; removing a method id removes
  its full history.
- Emits `DIDCreated`, `VerificationMethodAdded`, `ServiceEndpointUpdated`, and `DIDDeactivated`
  alongside the existing `DIDUpdated` events.
- Contract methods now return `Result<(), Error>` with explicit error codes instead of panicking on
  authorization/state failures. `DIDRegistryTrait` signatures were updated to match.

**#1477 — Governance (`contracts/governance`)**

The original contract had boolean voting, a single `for/against` tally, and immediate execution. It
lacked vote delegation checkpoints, abstention-based quorum, queued execution timing, and a veto path.

- Added `VoteSupport` (`Against = 0`, `For = 1`, `Abstain = 2`) and `ProposalState`
  (`Active`, `Queued`, `Executed`, `Expired`, `Vetoed`, `Cancelled`), plus `abstain_votes`,
  `start_ledger`, and `eta` fields on `Proposal`.
- Passing requires `for_votes + abstain_votes >= min_quorum` **and** `for_votes > against_votes`;
  abstentions never count toward the `for` majority. `cast_vote(bool)` remains as a compatibility
  wrapper over `cast_vote_with_support(VoteSupport)`.
- `delegate(from, to)` records a per-ledger checkpoint. Proposal snapshots resolve the delegation
  active at `proposal.start_ledger`, so later delegation changes cannot rewrite earlier proposals.
  Loop detection remains enforced.
- `queue_proposal(id, eta)` moves a passing proposal to `Queued`; `execute_proposal` accepts queued
  proposals only in `[eta, eta + grace_period]`, returning `QueueNotReady` before `eta` and
  `QueueExpired` after the window. `expire_proposal` finalizes expired proposals.
- `set_grace_period` and `set_veto_guardian` are admin-only; `veto_proposal(guardian, id)` lets the
  guardian veto a queued proposal.

**#1478 — Batch transfer (`contracts/batch_transfer`)**

The contract validated per transfer inside the loop and repeatedly read the sender balance. It had
no total overflow guard, no duplicate-recipient aggregation, and no explicit strict/best-effort APIs.

- Added `batch_transfer_strict` and `batch_transfer_best_effort` for a single sender. Both validate
  length equality, non-empty batches, `MAX_BATCH_SIZE` (100), and amount signs before any mutation.
- The total is computed with `checked_add` (`Error::Overflow`) and compared against the sender's
  balance before any transfer in strict mode; recipients are deduplicated via `Map<Address, (i128, Vec<u32>)>`
  so each unique address gets one aggregate transfer and one balance debit.
- `BatchSummary { total_amount, succeeded, skipped }` reports affected input indices in best-effort
  mode; invalid amounts and groups above the remaining balance are skipped.
- The internal size constant was renamed from `MAX_RECIPIENTS` to the public `MAX_BATCH_SIZE`.
  Existing `execute`, `quote`, and `execute_single_sender` entrypoints are unchanged.

### Files Changed

| File | Reason |
| --- | --- |
| `contracts/did_registry/src/lib.rs` | Export new DID types/events and define `Error` codes. |
| `contracts/did_registry/src/storage_types.rs` | Add controllers, service endpoints, key-history and event types. |
| `contracts/did_registry/src/contract.rs` | Implement controller authorization, deactivation, endpoints, events and trait updates. |
| `contracts/did_registry/src/test.rs` | Test delegation, service updates, deactivation, key history and events. |
| `contracts/governance/src/lib.rs` | Add `VoteSupport`, abstain tallies, delegation checkpoints, veto guardian and queue/grace state machine. |
| `contracts/governance/src/test.rs` | Test checkpoints, abstain quorum, eta/grace window, expiry and veto. |
| `contracts/batch_transfer/src/lib.rs` | Add up-front validation, checked totals, recipient dedup and strict/best-effort modes. |
| `contracts/batch_transfer/src/test.rs` | Test empty/mismatched/overflow batches, dedup, 50 recipients and skip semantics. |

### Test Output

```
cargo test -p soroban-did-registry-contract
  running 18 tests — 18 passed; 0 failed

cargo test -p soroscope-governance-contract
  unit tests:   17 passed; 0 failed
  integration:   1 passed; 0 failed  (tests/delegation.rs)

cargo test -p soroscope-batch-transfer-contract
  running 18 tests — 18 passed; 0 failed

cargo fmt -p soroban-did-registry-contract -p soroscope-governance-contract \
  -p soroscope-batch-transfer-contract -- --check   PASS

cargo clippy -p soroban-did-registry-contract -p soroscope-governance-contract \
  -p soroscope-batch-transfer-contract --all-targets   PASS (0 warnings in changed crates)
```

The acceptance commands in the issues use short package names (`did_registry`, `governance`,
`batch_transfer`) that do not match the manifest names; the real package names were used.

### Compatibility and Notes

- Extended public structs (`Proposal`, `VoteReceipt`, `DIDDocument`, `DIDMetadata`) and changed
  `DIDRegistryTrait` return types to `Result<(), Error>`. No public API, file, or DB column was
  deleted or renamed.
- Workspace-wide `cargo fmt --all -- --check` and `clippy -D warnings` fail on pre-existing issues
  in unrelated contracts (for example `contracts/emergency_guard`). Only the three touched packages
  were formatted and linted.
- The branch already contained uncommitted partial #1476 work when implementation resumed; it was
  preserved, fixed, and committed rather than discarded.
- Three atomic Conventional Commits were used: one per issue.
