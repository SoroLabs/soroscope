# Storage Refactor: Gas Analysis

This document compares ledger-entry reads/writes **before** and **after** the
DataKey refactoring across the four affected contracts.  Numbers are
*ledger-entry operations* (each entry in the transaction footprint costs
`readBytes` or `writeBytes` fees on top of the base transaction fee).

---

## Background: Soroban Storage Cost Model

| Storage tier | Rent model | Footprint cost |
|---|---|---|
| **Instance** | Shared with contract instance; one TTL for all keys | 1 footprint entry for the whole instance map |
| **Persistent** | Per-entry TTL; each key is a separate ledger entry | 1 footprint entry **per key** |
| **Temporary** | Per-entry TTL; auto-expires | 1 footprint entry **per key** |

Key insight: grouping N fields into one struct stored under a single key
reduces the footprint from N entries to 1 entry, cutting both `readBytes` and
`writeBytes` fees proportionally.

---

## 1. `liquidity_pool` contract

### Before

| Function | Instance reads | Instance writes | Persistent reads | Persistent writes | Total ops |
|---|---|---|---|---|---|
| `initialize` | 1 (has check) | **9** | 0 | 0 | 10 |
| `deposit` | **5** | **3** | 1 | 1 | 10 |
| `swap` | **5** | **2** | 0 | 0 | 7 |
| `withdraw` | **4** | **3** | 1 | 1 | 9 |
| `burn` | 1 | 1 | 1 | 1 | 4 |
| `set_fee` | 3 | 2 | 0 | 0 | 5 |
| `configure_fee_oracle` | 1 | 3 | 0 | 0 | 4 |
| `sync_fee_from_oracle` | **5** | **2** | 0 | 0 | 7 |
| `execute_fee_update` | 2 | 1 | 0 | 0 | 3 |

### After (PoolState + OracleConfig structs)

| Function | Instance reads | Instance writes | Persistent reads | Persistent writes | Total ops | Δ ops |
|---|---|---|---|---|---|---|
| `initialize` | 1 (has check) | **1** | 0 | 0 | 2 | **−8** |
| `deposit` | **1** | **1** | 1 | 1 | 4 | **−6** |
| `swap` | **1** | **1** | 0 | 0 | 2 | **−5** |
| `withdraw` | **1** | **1** | 1 | 1 | 4 | **−5** |
| `burn` | 1 | 1 | 1 | 1 | 4 | 0 |
| `set_fee` | 1 | 1 | 0 | 0 | 2 | **−3** |
| `configure_fee_oracle` | 1 | 2 | 0 | 0 | 3 | **−1** |
| `sync_fee_from_oracle` | **2** | **2** | 0 | 0 | 4 | **−3** |
| `execute_fee_update` | 2 | 1 | 0 | 0 | 3 | 0 |

**Summary:** The hot paths (`deposit`, `swap`, `withdraw`) each save 5–6
ledger-entry operations per call.  `initialize` saves 8 writes.

### What changed

- **Before:** 17 `DataKey` variants; each instance field was a separate entry.
- **After:** 5 `DataKey` variants.
  - `DataKey::Pool` → `PoolState` struct (9 fields: token_a, token_b,
    reserve_a, reserve_b, total_shares, fee_bps, base_fee_bps, admin, paused).
  - `DataKey::Oracle` → `OracleConfig` struct (4 fields: oracle, last_price,
    last_volatility_bps, timelock_ledgers).
  - `DataKey::PendingFeeUpdate`, `DataKey::Balance(Address)`,
    `DataKey::Allowance(AllowanceDataKey)` unchanged.

---

## 2. `token` contract

### Before

| Function | Instance reads | Instance writes | Total ops |
|---|---|---|---|
| `initialize` | 1 (has check) | **4** (admin + name + symbol + decimals) | 5 |
| `name` | **1** | 0 | 1 |
| `symbol` | **1** | 0 | 1 |
| `decimals` | **1** | 0 | 1 |

### After (TokenMetadata struct)

| Function | Instance reads | Instance writes | Total ops | Δ ops |
|---|---|---|---|---|
| `initialize` | 1 (has check) | **2** (admin + metadata) | 3 | **−2** |
| `name` | 1 | 0 | 1 | 0 |
| `symbol` | 1 | 0 | 1 | 0 |
| `decimals` | 1 | 0 | 1 | 0 |

**Summary:** `initialize` saves 2 writes.  Read functions are unchanged in
count but now share the same footprint entry (`Metadata`), so if a single
invocation calls `name`, `symbol`, and `decimals` the entry is loaded once
from the ledger and cached in the host's working set.

### What changed

- **Before:** `DataKey::Name`, `DataKey::Symbol`, `DataKey::Decimals` — 3
  separate instance entries.
- **After:** `DataKey::Metadata` → `TokenMetadata { name, symbol, decimals }`
  — 1 instance entry.
- `write_metadata(&e, &name, &symbol, decimal)` replaces three separate
  `write_name` / `write_symbol` / `write_decimal` calls in `initialize`.
- Legacy single-field writers (`write_name`, `write_symbol`, `write_decimal`)
  are preserved for admin update paths; they do a read-modify-write on the
  single `Metadata` entry.

---

## 3. `factory` contract

### Before

| Function | Persistent reads | Persistent writes | Total ops |
|---|---|---|---|
| `create_pair` | 1 (has check) | 1 | 2 |
| `get_pair` | 1 | 0 | 1 |

### After (instance storage)

| Function | Instance reads | Instance writes | Total ops | Δ ops |
|---|---|---|---|---|
| `create_pair` | 1 (has check) | 1 | 2 | 0 |
| `get_pair` | 1 | 0 | 1 | 0 |

**Summary:** Operation count is identical, but the storage tier changes from
**persistent** to **instance**.

### Why this matters

- **Persistent storage** charges per-entry rent proportional to the entry's
  size and the number of ledgers until expiry.  Each `Pair` entry would need
  its own TTL bump on every access or it risks expiring.
- **Instance storage** shares the contract instance's TTL.  A single
  `extend_ttl` on the instance keeps *all* pair mappings alive.  For a factory
  with many pairs this is a significant ongoing rent saving.
- The factory is a singleton (one deployed address), so instance storage is
  semantically correct: pair mappings are global state, not per-user state.

---

## 4. `storage_heavy` contract

### Before

| Function | Reads | Writes | Total ops |
|---|---|---|---|
| `read_persistent` | 1 | 0 | 1 |
| `read_temporary` | 1 | 0 | 1 |
| `batch_write_persistent(N)` | 0 | N | N |
| `batch_write_temporary(N)` | 0 | N | N |

### After (batch_read added)

| Function | Reads | Writes | Total ops | vs N single calls |
|---|---|---|---|---|
| `batch_read_persistent(N)` | N | 0 | N | **−(N−1) base tx fees** |
| `batch_read_temporary(N)` | N | 0 | N | **−(N−1) base tx fees** |

**Summary:** The ledger-entry count per key is the same, but batching N reads
into one transaction eliminates N−1 base transaction fees and N−1 round-trips.
For N=10 keys this is roughly a 10× reduction in transaction overhead.

---

## Overall Savings Summary

| Contract | Function | Ops before | Ops after | Saved |
|---|---|---|---|---|
| liquidity_pool | `initialize` | 10 | 2 | **−8** |
| liquidity_pool | `deposit` | 10 | 4 | **−6** |
| liquidity_pool | `swap` | 7 | 2 | **−5** |
| liquidity_pool | `withdraw` | 9 | 4 | **−5** |
| liquidity_pool | `set_fee` | 5 | 2 | **−3** |
| liquidity_pool | `sync_fee_from_oracle` | 7 | 4 | **−3** |
| token | `initialize` | 5 | 3 | **−2** |
| factory | `create_pair` | 2 persistent | 2 instance | rent model |
| storage_heavy | `batch_read(N)` | N×(base fee) | 1×(base fee) | **−(N−1) base fees** |

The most impactful changes are in `liquidity_pool` where the hot paths
(`deposit`, `swap`, `withdraw`) each save 5–6 ledger-entry operations, directly
reducing `readBytes` and `writeBytes` resource fees on every user interaction.
