# Soroban Token Contract with EmergencyGuard

## Overview

The Soroban Token Contract is a standardized token implementation for the Stellar blockchain that combines basic token functionality with advanced emergency controls. It features:

- **Standard Token Operations**: Mint, burn, transfer with allowances
- **Granular Pause Controls**: Independently pause specific operations (mint, transfer, burn)
- **Multi-Signature Support**: Require multiple admin approvals for critical actions
- **Admin Rotation**: Securely manage admin authority
- **Event Logging**: Comprehensive audit trails for all administrative actions

## Architecture

### Core Components

The token contract integrates the `EmergencyGuard` trait for emergency controls:

```
┌─────────────────────────────────────────┐
│     Token Contract                      │
│  ├─ Basic Operations (mint, transfer)   │
│  ├─ Balance Management                  │
│  └─ Allowances                          │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│     EmergencyGuard Trait                │
│  ├─ Granular Pause Control              │
│  ├─ Admin Management                    │
│  ├─ Multi-Signature Support             │
│  └─ Pause State Storage                 │
└─────────────────────────────────────────┘
```

### Pause Operations (Bitmask-based)

Operations are represented as bit flags for efficient storage and independent control:

```rust
pub const MINT: u32 = 1 << 4;        // 0x00000010 - Pause minting
pub const TRANSFER: u32 = 1 << 3;    // 0x00000008 - Pause transfers
pub const BURN: u32 = 1 << 5;        // 0x00000020 - Pause burning
```

## Initialization

When the token is initialized, the EmergencyGuard is automatically set up with:
- **Admin**: The contract admin (set during initialization)
- **Signature Threshold**: 1 (single admin can pause specific operations)
- **Initial Pause State**: No operations paused

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

## Features

### 1. Standard Token Operations

The token supports all standard Soroban token operations:
- `mint(admin, to, amount)` - Create new tokens (can be paused)
- `transfer(from, to, amount)` - Transfer tokens (can be paused)
- `burn(from, amount)` - Destroy tokens (can be paused)
- `balance(account)` - Query token balance
- `approve(from, spender, amount, expiration)` - Set spending allowance

### 2. Pause Controls

The token contract provides granular pause controls for emergency situations:

```rust
// Check if an operation is paused
is_paused(operation: u32) -> bool

// Pause a specific operation (single admin)
guard_pause(admin: Address, operation: u32, paused: bool) -> Result<(), GuardError>

// Unpause all operations (multi-sig required)
guard_unpause(approvers: Vec<Address>) -> Result<(), GuardError>
```

### 3. Error Handling

The contract returns specific error codes:
- `NotInitialized` - Guard not yet initialized
- `Unauthorized` - Caller not authorized
- `Paused` - Operation is paused
- `InsufficientSignatures` - Not enough approvals
- `InvalidThreshold` - Invalid multi-sig threshold
- `AdminNotFound` - Admin not found
- `AlreadyInitialized` - Guard already initialized

## Storage

The token contract stores:
- **Token Metadata**: Name, symbol, decimals
- **Balances**: Map of address to balance
- **Allowances**: Map of (holder, spender) to allowance
- **Admin**: Current contract administrator
- **Guard State**: Pause state and admin configuration

## Security Considerations

1. **Authorization**: All sensitive operations require caller authentication
2. **Multi-Signature**: Resume operations require multiple admin approvals
3. **Granular Control**: Each operation can be controlled independently
4. **Audit Trail**: All pause/unpause actions are logged

## API Reference

### Token Operations

```rust
// Initialize token with emergency guard
fn initialize(
    e: Env,
    admin: Address,
    decimal: u32,
    name: String,
    symbol: String
) -> Result<(), GuardError>

// Mint tokens (respects MINT pause flag)
fn mint(e: Env, to: Address, amount: i128)

// Transfer tokens (respects TRANSFER pause flag)
fn transfer(e: Env, from: Address, to: Address, amount: i128)

// Burn tokens (respects BURN pause flag)
fn burn(e: Env, from: Address, amount: i128)
```

### Emergency Guard Operations

```rust
// Pause a specific operation
fn guard_pause(
    e: Env,
    admin: Address,
    operation: u32,
    paused: bool
) -> Result<(), GuardError>

// Resume all operations (multi-sig required)
fn guard_unpause(
    e: Env,
    approvers: Vec<Address>
) -> Result<(), GuardError>
```

## Related Documentation

For more information on the EmergencyGuard trait, see:
- [EmergencyGuard README](../emergency_guard/README.md)

## Development

The token contract is built with:
- **Language**: Rust
- **Framework**: Soroban SDK 22.0.0
- **Dependencies**: `emergency_guard` crate

To build the contract:
```bash
cd contracts/token
cargo build --target wasm32-unknown-unknown --release
```

## License

This contract is part of the SoroScope project.
