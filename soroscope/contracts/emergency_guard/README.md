# EmergencyGuard - Standardized Emergency Controls for Soroban Contracts

## Overview

The EmergencyGuard trait provides a standardized, reusable mechanism for emergency pause and admin management across all contracts in the workspace. It features:

- **Granular Pausing**: Pause specific operations (swaps, deposits, withdrawals, transfers, minting, burning) independently
- **Multi-Signature Support**: Built-in support for multi-sig authorization patterns
- **Admin Rotation**: Securely rotate admin authority without transferring funds
- **Efficient Storage**: Uses bitmask for compact pause state storage
- **Event Logging**: Logs all administrative actions for audit trails

## Architecture

### PauseType (Bitmask-based)

Operations are represented as bit flags for efficient storage:

```rust
pub const SWAP: u32 = 1 << 0;      // 0x00000001
pub const DEPOSIT: u32 = 1 << 1;   // 0x00000002
pub const WITHDRAW: u32 = 1 << 2;  // 0x00000004
pub const TRANSFER: u32 = 1 << 3;  // 0x00000008
pub const MINT: u32 = 1 << 4;      // 0x00000010
pub const BURN: u32 = 1 << 5;      // 0x00000020
```

### Storage Structure

```
DataKey::PauseState      -> PauseType(u32)      // Bitmask of paused operations
DataKey::Admins          -> Vec<Address>        // List of authorized admins
DataKey::SignatureThreshold -> u32              // Required multi-sig threshold
DataKey::AdminQueue      -> Vec<Address>        // Reserved for admin rotation queue
DataKey::PendingAdmin    -> Address             // Reserved for pending admin rotations
```

### Multi-Signature Support

The default implementation supports:

1. **Single Admin Mode** (threshold = 1): Any single admin can pause/unpause
2. **Multi-Admin Mode** (threshold > 1): Requires N of M admins to approve

For actual multi-sig signing in production:

- Extend with Soroban's `InvokeContractArgs` for signature verification
- Use wallet-based multi-sig solutions
- Implement timelock + voting patterns for critical operations

## Usage Guide

### 1. Initialize Emergency Guard

```rust
use emergency_guard::DefaultEmergencyGuard;
use soroban_sdk::{contractimpl, vec, Address, Env};

#[contractimpl]
impl MyContract {
    pub fn initialize(env: Env, admin1: Address, admin2: Address) {
        let admins = vec![&env, admin1, admin2];
        let threshold = 2; // Require 2 of 2 admins

        DefaultEmergencyGuard::init_guard(env, admins, threshold)
            .expect("Failed to initialize guard");
    }
}
```

### 2. Check Before Operations

```rust
#[contractimpl]
impl MyContract {
    pub fn swap(env: Env, amount_in: i128, amount_out_min: i128) -> i128 {
        // Check if swaps are paused
        DefaultEmergencyGuard::check_not_paused(&env, PauseType::SWAP)
            .expect("Swaps are paused");

        // Perform swap logic
        amount_in * 2 // dummy
    }

    pub fn deposit(env: Env, amount: i128) {
        // Check if deposits are paused
        DefaultEmergencyGuard::check_not_paused(&env, PauseType::DEPOSIT)
            .expect("Deposits are paused");

        // Perform deposit logic
    }
}
```

### 3. Admin Controls

```rust
#[contractimpl]
impl MyContract {
    // Pause specific operations
    pub fn pause_swaps(env: Env) {
        DefaultEmergencyGuard::set_pause_state(&env, PauseType::SWAP, true)
            .expect("Unauthorized or invalid");
    }

    // Emergency pause everything
    pub fn emergency_pause(env: Env) {
        DefaultEmergencyGuard::emergency_pause_all(&env)
            .expect("Unauthorized");
    }

    // Resume operations
    pub fn resume(env: Env) {
        DefaultEmergencyGuard::unpause_all(&env)
            .expect("Unauthorized");
    }

    // Manage admins
    pub fn add_admin(env: Env, new_admin: Address) {
        DefaultEmergencyGuard::add_admin(&env, new_admin)
            .expect("Unauthorized or threshold violated");
    }

    pub fn rotate_admin(env: Env, new_admin: Address) {
        DefaultEmergencyGuard::rotate_admin(&env, new_admin)
            .expect("Unauthorized");
    }
}
```

### 4. Query State

```rust
#[contractimpl]
impl MyContract {
    pub fn is_paused(env: Env, operation: u32) -> bool {
        let state = DefaultEmergencyGuard::get_pause_state(&env);
        let pause_type = PauseType::new(state);
        pause_type.is_paused(operation)
    }

    pub fn get_admins(env: Env) -> Vec<Address> {
        DefaultEmergencyGuard::get_admins(&env)
    }

    pub fn get_threshold(env: Env) -> u32 {
        DefaultEmergencyGuard::get_threshold(&env)
    }
}
```

## Integration Example: Liquidity Pool

Here's how to integrate EmergencyGuard into the existing liquidity_pool contract:

### Step 1: Add dependency to Cargo.toml

```toml
[dependencies]
emergency_guard = { path = "../emergency_guard" }
```

### Step 2: Update contract initialization

```rust
#[contractimpl]
impl LiquidityPool {
    pub fn initialize(
        env: Env,
        admin: Address,
        token_a: Address,
        token_b: Address,
        fee_bps: u32,
    ) {
        // Existing initialization code...

        // Initialize emergency guard with single admin
        let admins = vec![&env, admin.clone()];
        DefaultEmergencyGuard::init_guard(&env, admins, 1)
            .expect("Failed to initialize emergency guard");
    }
}
```

### Step 3: Add pause checks to operations

**Before:**

```rust
pub fn swap(env: Env, in_amount: i128) -> i128 {
    // swap logic
}
```

**After:**

```rust
pub fn swap(env: Env, in_amount: i128) -> i128 {
    DefaultEmergencyGuard::check_not_paused(&env, PauseType::SWAP)
        .expect("Swaps are paused");
    // swap logic
}
```

### Step 4: Update admin functions

**Before:**

```rust
pub fn set_paused(env: Env, paused: bool) {
    admin.require_auth();
    env.storage().instance().set(&DataKey::Paused, &paused);
}
```

**After:**

```rust
pub fn set_paused(env: Env, paused: bool) {
    DefaultEmergencyGuard::set_pause_state(&env, PauseType::SWAP, paused)
        .expect("Unauthorized");
}

pub fn pause_deposits(env: Env) {
    DefaultEmergencyGuard::set_pause_state(&env, PauseType::DEPOSIT, true)
        .expect("Unauthorized");
}

pub fn emergency_pause_all(env: Env) {
    DefaultEmergencyGuard::emergency_pause_all(&env)
        .expect("Unauthorized");
}
```

## Advanced Features

### Custom Pause Types

Extend PauseType for custom operations:

```rust
pub const CUSTOM_OPERATION_1: u32 = 1 << 6;  // 0x00000040
pub const CUSTOM_OPERATION_2: u32 = 1 << 7;  // 0x00000080
```

### Multi-Signature Implementation

For production multi-sig, extend with Soroban's contract invocation:

```rust
pub fn emergency_pause_all_multisig(
    env: Env,
    signatures: Vec<(Address, Bytes)>, // Signed approvals
) -> GuardResult<()> {
    let caller = env.invoker();
    let required_sigs = DefaultEmergencyGuard::get_threshold(&env);

    // Verify signatures from different admins
    let mut verified_count = 0;
    for (admin, sig) in signatures {
        if verify_admin_signature(&env, &admin, sig) {
            verified_count += 1;
        }
    }

    if verified_count >= required_sigs {
        DefaultEmergencyGuard::emergency_pause_all(&env)
    } else {
        Err(GuardError::InsufficientSignatures)
    }
}
```

### Admin Rotation Workflow

```rust
// Step 1: Current admin initiates rotation (must sign)
pub fn rotate_admin(env: Env, new_admin: Address) {
    DefaultEmergencyGuard::rotate_admin(&env, new_admin)
        .expect("Only current admin can rotate");
    // old_admin is replaced with new_admin in admins list
}

// Step 2: New admin takes control immediately
// (No multi-sig needed if rotation is signed by old admin)
```

## Error Handling

```rust
use emergency_guard::GuardError;

match DefaultEmergencyGuard::check_not_paused(&env, PauseType::SWAP) {
    Ok(()) => { /* Continue */ },
    Err(GuardError::Paused) => panic!("Operation is paused"),
    Err(GuardError::Unauthorized) => panic!("Not authorized"),
    Err(_) => panic!("Other error"),
}
```

## Testing

### Unit Tests

```rust
#[test]
fn test_granular_pause() {
    let env = Env::default();
    let admin = Address::random(&env);

    // Initialize
    DefaultEmergencyGuard::init_guard(&env, vec![&env, admin.clone()], 1)
        .unwrap();

    // Pause swaps
    DefaultEmergencyGuard::set_pause_state(&env, PauseType::SWAP, true)
        .unwrap();

    // Verify swap is paused
    assert!(
        DefaultEmergencyGuard::check_not_paused(&env, PauseType::SWAP).is_err()
    );

    // Verify deposits are not paused
    assert!(
        DefaultEmergencyGuard::check_not_paused(&env, PauseType::DEPOSIT).is_ok()
    );
}
```

### Contract Tests

```rust
#[test]
fn test_swap_respects_pause() {
    let env = Env::default();
    let contract_id = env.register_contract(None, MyContract);
    let client = MyContractClient::new(&env, &contract_id);

    // Initialize with pause disabled
    client.initialize(&admin);

    // Verify swap works
    let result = client.swap(&100);
    assert!(result.is_ok());

    // Pause swaps
    client.pause_swaps(&admin);

    // Verify swap fails
    let result = client.swap(&100);
    assert!(result.is_err());
}
```

## Migration Guide

### For Existing Liquidity Pool Contract

1. Add `emergency_guard` dependency
2. In `initialize()`: Call `DefaultEmergencyGuard::init_guard()`
3. In operation functions: Add `DefaultEmergencyGuard::check_not_paused()` calls
4. Remove old `DataKey::Paused` and `set_paused()` function
5. Add new granular pause functions
6. Update tests to use new guard

### For New Contracts

1. Initialize guard in `initialize()` or `new()`
2. Call `check_not_paused()` before pausable operations
3. Expose admin functions for pause/unpause as needed

## Security Considerations

1. **Admin Keys**: Store admin keys in secure key management systems
2. **Multi-Sig Verification**: In production, implement proper signature verification
3. **Timelock**: Consider adding timelock for critical operations (e.g., `emergency_pause_all`)
4. **Audit Logs**: All operations are logged via `env.events()`
5. **Admin Rotation**: Direct rotation is secure; old admin is immediately replaced
6. **Threshold Validation**: Cannot remove admins below threshold

## Future Enhancements

- [ ] Timelock for critical operations
- [ ] Voting-based admin decisions
- [ ] Admin governance token
- [ ] Granular operation groups (e.g., all swap-related operations)
- [ ] Pause duration limits (auto-unpause after X blocks)
- [ ] Cross-contract guard coordination
