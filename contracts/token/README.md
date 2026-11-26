# Soroban Token Contract with EmergencyGuard

## Overview

The Soroban Token Contract is a standardized token implementation for the Stellar blockchain that combines basic token functionality with advanced emergency controls. It features:

- **Standard Token Operations**: Mint, burn, transfer with allowances
- **Granular Pause Controls**: Independently pause specific operations (mint, transfer, burn)
- **Multi-Signature Support**: Require multiple guard admin approvals for critical actions
- **Admin Rotation**: Securely manage admin authority via threshold validation
- **Event Logging**: Comprehensive audit trails for all guard administrative actions

## Architecture

### Core Components

The token contract embeds the `emergency_guard` library. Guard state is stored in the **same contract instance storage** under `GuardDataKey` entries (not a separate deployed guard contract):

```
┌─────────────────────────────────────────┐
│     Token Contract                      │
│  ├─ Basic Operations (mint, transfer)   │
│  ├─ Balance Management                  │
│  ├─ Allowances                          │
│  └─ DataKey::Admin (mint authority)     │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│     EmergencyGuard (library)            │
│  ├─ GuardDataKey::PauseState (bitmask)  │
│  ├─ GuardDataKey::Admins                │
│  ├─ GuardDataKey::SignatureThreshold    │
│  └─ Multi-sig via check_multi_sig       │
└─────────────────────────────────────────┘
```

### Pause Operations (Bitmask-based)

Operations are represented as bit flags for efficient storage and independent control:

```rust
pub const TRANSFER: u32 = 1 << 3;    // 0x00000008 - Pause transfers & approvals
pub const MINT: u32 = 1 << 4;        // 0x00000010 - Pause minting
pub const BURN: u32 = 1 << 5;        // 0x00000020 - Pause burning
```

## Guard Parameters

On `initialize`, the token calls `EmergencyGuard::initialize(admins, threshold)`:

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `admins` | `Vec<Address>` | `[admin]` | Guard administrators authorized for pause and multi-sig actions |
| `threshold` | `u32` | `1` | Minimum distinct guard admin signatures required for multi-sig operations |

### Storage Keys (`GuardDataKey`)

| Key | Type | Description |
|-----|------|-------------|
| `PauseState` | `PauseType(u32)` | Bitmask of paused operations |
| `Admins` | `Vec<Address>` | Current guard administrator list |
| `SignatureThreshold` | `u32` | Required signature count for multi-sig |

Token-specific storage (`DataKey::Admin`) holds the mint authority. After `set_admin`, both `DataKey::Admin` and `GuardDataKey::Admins` are updated to the new address (single-admin setups).

## Initialization

When the token is initialized, the EmergencyGuard is automatically set up with:
- **Admin**: The contract admin (set during initialization)
- **Signature Threshold**: 1 (single admin can pause specific operations)
- **Initial Pause State**: No operations paused (`0`)

Example initialization:
```bash
stellar contract invoke \
  --source-account <account> \
  --network testnet \
  -- initialize \
    --admin <admin-address> \
    --decimal 18 \
    --name "My Token" \
    --symbol "MTK"
```

## Guard Events

All guard events are emitted by the `emergency_guard` library into the token contract's event log:

| Event topic | Payload | When emitted |
|-------------|---------|--------------|
| `emergency_guard_initialized` | `{ admins, threshold }` | `initialize` |
| `emergency_guard_pause_state_changed` | `{ admin, operation, paused }` | `set_pause` / `guard_pause` |
| `emergency_guard_emergency_paused_all` | `{ approvers }` | `emergency_pause_all` |
| `emergency_guard_resumed_all` | `{ approvers }` | `resume_all` / `guard_unpause` |
| `emergency_guard_admin_added` | `{ approvers, new_admin }` | `guard_add_admin` |
| `emergency_guard_admin_removed` | `{ approvers, admin }` | `guard_remove_admin` |

## Features

### 1. Standard Token Operations

The token supports all standard Soroban token operations:
- `mint(admin, to, amount)` - Create new tokens (respects `MINT` pause; requires token admin auth)
- `transfer(from, to, amount)` - Transfer tokens (respects `TRANSFER` pause)
- `burn(from, amount)` - Destroy tokens (respects `BURN` pause)
- `balance(account)` - Query token balance
- `approve(from, spender, amount, expiration)` - Set spending allowance (respects `TRANSFER` pause)

### 2. Pause Controls

Single guard admin (any member of `GuardDataKey::Admins`):

```rust
pause_minting(admin) / resume_minting(admin)
pause_transfers(admin) / resume_transfers(admin)
pause_burning(admin) / resume_burning(admin)
guard_pause(admin, operation, paused) -> Result<(), GuardError>
```

Multi-sig (requires `threshold` valid guard admin signatures via `check_multi_sig`):

```rust
emergency_pause_all(approvers)
resume_all(approvers) / guard_unpause(approvers)
```

Queries:

```rust
get_pause_state() -> u32          // raw bitmask
is_operation_paused(operation) -> bool
guard_is_paused(operation) -> bool
```

### 3. Admin Controls

Admin rotation uses `EmergencyGuard::check_multi_sig` for threshold validation before updating guard and token admins:

```rust
// Rotate admin — approvers must meet signature threshold
set_admin(approvers: Vec<Address>, new_admin: Address)

// Multi-sig admin list management
guard_add_admin(approvers, new_admin) -> Result<(), GuardError>
guard_remove_admin(approvers, admin) -> Result<(), GuardError>
get_guard_admins() -> Vec<Address>
guard_admins() -> Vec<Address>
get_guard_threshold() -> u32
```

Unauthorized callers (non-guard admins or insufficient signatures) receive `GuardError::Unauthorized` or `GuardError::InsufficientSignatures`.

### 4. Error Handling

| `GuardError` | Code | When |
|--------------|------|------|
| `NotInitialized` | 0 | Guard not yet initialized |
| `Unauthorized` | 1 | Caller not a guard admin |
| `Paused` | 2 | Operation is paused |
| `InsufficientSignatures` | 3 | Multi-sig threshold not met |
| `InvalidThreshold` | 4 | Invalid threshold configuration |
| `AdminNotFound` | 5 | Admin not in guard list |
| `AlreadyInitialized` | 6 | Double initialization |

## Storage

The token contract stores:
- **Token Metadata**: Name, symbol, decimals (`DataKey` metadata keys)
- **Balances**: Map of address to balance
- **Allowances**: Map of (holder, spender) to allowance
- **Admin**: Current mint administrator (`DataKey::Admin`)
- **Guard State**: `GuardDataKey::{PauseState, Admins, SignatureThreshold}` in instance storage

## Security Considerations

1. **Authorization**: Sensitive operations require guard admin authentication via `require_auth` (single-admin pause) or `check_multi_sig` (multi-sig actions)
2. **Threshold Validation**: `set_admin`, `guard_add_admin`, `guard_remove_admin`, `emergency_pause_all`, and `resume_all` all enforce the configured signature threshold
3. **Granular Control**: Each operation type can be paused independently via bitmask
4. **Audit Trail**: All pause, resume, and admin changes emit structured events

## API Reference

### Token Operations

```rust
fn initialize(e: Env, admin: Address, decimal: u32, name: String, symbol: String)
fn mint(e: Env, to: Address, amount: i128)
fn transfer(e: Env, from: Address, to: Address, amount: i128)
fn burn(e: Env, from: Address, amount: i128)
fn set_admin(e: Env, approvers: Vec<Address>, new_admin: Address)
```

### Emergency Guard Operations

```rust
fn guard_pause(e: Env, admin: Address, operation: u32, paused: bool) -> Result<(), GuardError>
fn guard_unpause(e: Env, approvers: Vec<Address>) -> Result<(), GuardError>
fn guard_add_admin(e: Env, approvers: Vec<Address>, new_admin: Address) -> Result<(), GuardError>
fn guard_remove_admin(e: Env, approvers: Vec<Address>, admin: Address) -> Result<(), GuardError>
fn get_pause_state(e: Env) -> u32
fn get_guard_admins(e: Env) -> Vec<Address>
fn get_guard_threshold(e: Env) -> u32
```

## Related Documentation

- [EmergencyGuard README](../emergency_guard/README.md)
- [Token Guard Storage Analysis](../../docs/token_guard_storage_analysis.md)

## Development

The token contract is built with:
- **Language**: Rust
- **Framework**: Soroban SDK 22.0.0
- **Dependencies**: `emergency_guard` crate (`default-features = false`)

To build the contract:
```bash
cd contracts/token
cargo build --target wasm32-unknown-unknown --release
```

To run tests:
```bash
cargo test -p soroban-token-contract --lib
```

## License

This contract is part of the SoroScope project.
