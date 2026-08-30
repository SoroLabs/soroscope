# Liquidity Pool EmergencyGuard Operations

This liquidity pool contract includes EmergencyGuard-style controls for pausing
pool operations and managing the guard administrator set. The guard is designed
for emergency response without shutting down unrelated workflows.

## What The Guard Protects

The pool uses a compact `u32` pause bitmask. Each protected operation maps to a
single bit, so checking whether an operation is paused is constant time and only
requires reading one pause-state value.

| Operation | Flag | Bit | Protected entry point |
| --- | --- | --- | --- |
| Swap | `pause_op::SWAP` | `1 << 0` | `swap` |
| Deposit | `pause_op::DEPOSIT` | `1 << 1` | `deposit` |
| Withdraw | `pause_op::WITHDRAW` | `1 << 2` | `withdraw` |
| Burn | `pause_op::BURN` | `1 << 5` | `burn` |
| All guard bits | `pause_op::ALL` | `u32::MAX` | Emergency pause and resume |

Granular pausing lets an admin stop the risky path while keeping safer paths
available. For example, swaps can be paused during a market incident while
withdrawals stay open.

## Initialization

Calling `initialize(admin, token_a, token_b)` bootstraps the guard with:

- `admin` as the first guard administrator.
- A multi-signature threshold of `1`.
- A pause state of `0`, meaning no operations are paused.

## Guard Parameters

The guard state is stored in instance storage using the following parameters:

- `GuardAdmins`: `Vec<Address>` - The list of accounts authorized to perform guard actions.
- `GuardThreshold`: `u32` - The number of unique admin signatures required for multi-sig operations.
- `GuardPauseState`: `u32` - The current bitmask representing which operations are active vs paused.

## Events

The guard implementation emits standardized events to allow external systems to index emergency actions:

- `emergency_guard_initialized`: Emitted when the guard is bootstrapped with `admins` and `threshold`.
- `emergency_guard_pause_state_changed`: Emitted when a single operation is paused or unpaused. Returns `admin`, `operation`, and `paused`.
- `emergency_guard_emergency_paused_all`: Emitted when all guarded operations are paused via multi-sig. Returns `approvers`.
- `emergency_guard_resumed_all`: Emitted when all operations are restored. Returns `approvers`.
- `emergency_guard_admin_added`: Emitted when a new administrator is authorized. Returns `approvers` and `new_admin`.
- `emergency_guard_admin_removed`: Emitted when an administrator is revoked. Returns `approvers` and `admin`.

## Granular Pause API

Use the dedicated helper functions for the common pool controls:

```rust
// Pause or resume only swaps.
pool.pause_swaps();
pool.resume_swaps();

// Pause or resume only deposits.
pool.pause_deposits();
pool.resume_deposits();

// Pause or resume only withdrawals.
pool.pause_withdrawals();
pool.resume_withdrawals();

// Pause or resume the core guarded operations at once.
pool.set_paused(true);
pool.set_paused(false);
```

For read-only inspection:

```rust
let state: u32 = pool.get_pause_state();
let swaps_paused: bool = pool.is_paused_op(pause_op::SWAP);
let deposits_paused: bool = pool.is_paused_op(pause_op::DEPOSIT);
```

## Multi-Sig Admin Operations

Critical guard changes receive an `approvers: Vec<Address>` argument. The
contract checks unique approvers, confirms they are current guard admins, and
calls `require_auth()` on each counted approver. The operation succeeds when the
number of valid approvers is at least `GuardThreshold`.

With the default threshold of `1`, one current guard admin can approve:

```rust
let approvers = vec![&env, admin.clone()];

pool.emergency_pause_all(approvers.clone());
pool.resume_all(approvers.clone());
pool.add_guard_admin(approvers.clone(), new_admin.clone());
pool.remove_guard_admin(approvers, old_admin.clone());
```

Duplicate approver addresses do not increase the approval count. Removing an
admin is rejected when it would reduce the admin count below the current
threshold.

## Emergency Workflows

### Pause swaps only

Use this when price discovery, oracle input, or routing behavior is suspect but
liquidity exits should remain available.

```rust
pool.pause_swaps();

assert!(pool.is_paused_op(pause_op::SWAP));
assert!(!pool.is_paused_op(pause_op::WITHDRAW));
```

### Pause deposits only

Use this when new liquidity should be halted while the pool is being reviewed.

```rust
pool.pause_deposits();

assert!(pool.is_paused_op(pause_op::DEPOSIT));
assert!(!pool.is_paused_op(pause_op::SWAP));
```

### Full emergency pause

Use this when the safest response is to block every guarded operation.

```rust
let approvers = vec![&env, admin.clone()];

pool.emergency_pause_all(approvers);

assert!(pool.is_paused_op(pause_op::SWAP));
assert!(pool.is_paused_op(pause_op::DEPOSIT));
assert!(pool.is_paused_op(pause_op::WITHDRAW));
assert!(pool.is_paused_op(pause_op::BURN));
```

### Resume after remediation

Only resume after the incident has been investigated and the same approval model
has authorized the recovery.

```rust
let approvers = vec![&env, admin.clone()];

pool.resume_all(approvers);
assert_eq!(pool.get_pause_state(), 0);
```

## Admin Management

The guard administrator set can be inspected and changed through the pool API.

```rust
let admins = pool.get_guard_admins();
let threshold = pool.get_guard_threshold();

let approvers = vec![&env, admin.clone()];
pool.add_guard_admin(approvers.clone(), new_admin.clone());
pool.remove_guard_admin(approvers, retired_admin.clone());
```

Recommended operating model:

- Keep at least two guard admins for production deployments.
- Use a threshold that matches the incident response policy.
- Rotate or remove compromised admins before resuming paused operations.
- Keep an off-chain runbook that maps each admin address to an operator.

## Error Handling

Guarded calls return the pool's `Error` enum:

- `Error::Paused`: the requested operation is currently paused.
- `Error::Unauthorized`: the caller or approver set is not authorized.
- `Error::NotInitialized`: guard state has not been initialized.

Callers should treat `Error::Paused` as an expected operational state, not as a
contract failure. The pause can be cleared by an authorized guard admin workflow.

## Design Notes

The guard path is intentionally lean:

- Pause checks are `O(1)` bit tests against a single `u32`.
- Pause state uses one compact instance-storage value.
- Admin and approver validation is linear in the number of supplied addresses,
  which is appropriate because admin lists are expected to be small.
- Public helper functions avoid forcing callers to construct bitmasks for common
  actions such as pausing swaps or deposits.

For the reusable EmergencyGuard contract details, see
[`../emergency_guard/README.md`](../emergency_guard/README.md).
# Liquidity Pool Contract

Constant-product AMM with LP shares, dynamic oracle fees, and **EmergencyGuard** granular pause controls.

## Emergency pause (bitmask)

Pause state is a single **`u32` bitmask** (4 bytes) shared with the `emergency_guard` crate, stored under `PauseState` in instance storage. Each operation is one bit (`PauseType::SWAP`, `DEPOSIT`, `WITHDRAW`, `BURN`, `TRANSFER`, etc.).

| Function | Description |
|----------|-------------|
| `guard_pause(admin, operation, paused)` | Pause/unpause one operation |
| `guard_is_paused(operation)` | Query one bit |
| `get_pause_state()` | Raw bitmask |
| `emergency_pause(approvers)` | Multi-sig pause all |
| `resume(approvers)` | Multi-sig clear all |
| `rotate_admin(approvers, old, new)` | Replace pool + guard admin |

Core AMM paths (`deposit`, `swap`, `withdraw`, `burn`, `transfer`) call `require_not_paused` before executing.

## Initialization

```rust
pool.initialize(admin, token_a, token_b)?;
// Bootstraps EmergencyGuard with [admin], threshold 1
```

## Fee admin

`DataKey::Admin` is the pool fee admin (may differ from guard admins after rotation). Use `set_fee`, `configure_fee_oracle`, `sync_fee_from_oracle`, and `execute_fee_update`.
