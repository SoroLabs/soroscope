# EmergencyGuard Integration Example: Liquidity Pool Contract

This file demonstrates how to integrate the standardized EmergencyGuard trait into the existing liquidity_pool contract.

## Step-by-Step Integration

### 1. Update Cargo.toml Dependencies

Add the emergency_guard dependency to `contracts/liquidity_pool/Cargo.toml`:

```toml
[dependencies]
soroban-sdk = { version = "20.0.0", features = ["contract"] }
emergency_guard = { path = "../emergency_guard" }
```

### 2. Update lib.rs Imports

Add these imports at the top of `contracts/liquidity_pool/src/lib.rs`:

```rust
use emergency_guard::{DefaultEmergencyGuard, PauseType, GuardError};
```

### 3. Remove Old Pause Implementation

**REMOVE** the old pause code from `lib.rs`:

```rust
// DELETE: This old code
pub fn set_paused(env: Env, paused: bool) {
    let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
    admin.require_auth();
    env.storage().instance().set(&DataKey::Paused, &paused);
}

fn check_paused(env: &Env) {
    let paused: bool = env
        .storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false);
    if paused {
        panic_with_error!(env, Error::Paused);
    }
}

// DELETE: DataKey::Paused from enum
#[contracttype]
pub enum DataKey {
    // ...remove this:
    // Paused,
}
```

### 4. Update Initialize Function

**BEFORE:**

```rust
pub fn initialize(
    env: Env,
    admin: Address,
    token_a: Address,
    token_b: Address,
    fee_bps: u32,
) {
    let mut admin_address = env.storage().instance().get::<_, Address>(&DataKey::Admin);
    if admin_address.is_some() {
        panic_with_error!(env, Error::AlreadyInitialized);
    }

    admin.require_auth();

    if fee_bps >= 10000 {
        panic_with_error!(env, Error::InvalidFee);
    }

    env.storage().instance().set(&DataKey::Admin, &admin);
    // ... rest of initialization
}
```

**AFTER:**

```rust
pub fn initialize(
    env: Env,
    admin: Address,
    token_a: Address,
    token_b: Address,
    fee_bps: u32,
) {
    let mut admin_address = env.storage().instance().get::<_, Address>(&DataKey::Admin);
    if admin_address.is_some() {
        panic_with_error!(env, Error::AlreadyInitialized);
    }

    admin.require_auth();

    if fee_bps >= 10000 {
        panic_with_error!(env, Error::InvalidFee);
    }

    env.storage().instance().set(&DataKey::Admin, &admin);

    // Initialize emergency guard with single admin and threshold of 1
    let admins = vec![&env, admin];
    DefaultEmergencyGuard::init_guard(&env, admins, 1)
        .expect("Failed to initialize emergency guard");

    // ... rest of initialization
}
```

### 5. Update Deposit Function

**BEFORE:**

```rust
pub fn deposit(env: Env, user: Address, amount_a: i128, amount_b: i128) -> i128 {
    check_paused(&env);
    user.require_auth();
    // ... deposit logic
}
```

**AFTER:**

```rust
pub fn deposit(env: Env, user: Address, amount_a: i128, amount_b: i128) -> i128 {
    // Check if deposits are paused
    DefaultEmergencyGuard::check_not_paused(&env, PauseType::DEPOSIT)
        .expect("Deposits are paused");

    user.require_auth();
    // ... deposit logic
}
```

### 6. Update Swap Function

**BEFORE:**

```rust
pub fn swap(env: Env, user: Address, in_amount: i128) -> i128 {
    check_paused(&env);
    user.require_auth();
    // ... swap logic
}
```

**AFTER:**

```rust
pub fn swap(env: Env, user: Address, in_amount: i128) -> i128 {
    // Check if swaps are paused (can swap while deposits/withdrawals are paused)
    DefaultEmergencyGuard::check_not_paused(&env, PauseType::SWAP)
        .expect("Swaps are paused");

    user.require_auth();
    // ... swap logic
}
```

### 7. Update Withdraw Function

**BEFORE:**

```rust
pub fn withdraw(env: Env, user: Address, shares_amount: i128) -> (i128, i128) {
    check_paused(&env);
    user.require_auth();
    // ... withdraw logic
}
```

**AFTER:**

```rust
pub fn withdraw(env: Env, user: Address, shares_amount: i128) -> (i128, i128) {
    // Check if withdrawals are paused
    DefaultEmergencyGuard::check_not_paused(&env, PauseType::WITHDRAW)
        .expect("Withdrawals are paused");

    user.require_auth();
    // ... withdraw logic
}
```

### 8. Add New Admin Functions

**ADD** these new functions to `lib.rs` for granular pause control:

```rust
// Pause only swaps (other operations continue)
pub fn pause_swaps(env: Env) {
    DefaultEmergencyGuard::set_pause_state(&env, PauseType::SWAP, true)
        .expect("Unauthorized or invalid pause state");
}

// Resume only swaps
pub fn resume_swaps(env: Env) {
    DefaultEmergencyGuard::set_pause_state(&env, PauseType::SWAP, false)
        .expect("Unauthorized");
}

// Pause only deposits
pub fn pause_deposits(env: Env) {
    DefaultEmergencyGuard::set_pause_state(&env, PauseType::DEPOSIT, true)
        .expect("Unauthorized");
}

// Resume only deposits
pub fn resume_deposits(env: Env) {
    DefaultEmergencyGuard::set_pause_state(&env, PauseType::DEPOSIT, false)
        .expect("Unauthorized");
}

// Pause only withdrawals
pub fn pause_withdrawals(env: Env) {
    DefaultEmergencyGuard::set_pause_state(&env, PauseType::WITHDRAW, true)
        .expect("Unauthorized");
}

// Resume only withdrawals
pub fn resume_withdrawals(env: Env) {
    DefaultEmergencyGuard::set_pause_state(&env, PauseType::WITHDRAW, false)
        .expect("Unauthorized");
}

// Emergency: pause everything
pub fn emergency_pause_all(env: Env) {
    DefaultEmergencyGuard::emergency_pause_all(&env)
        .expect("Unauthorized");
}

// Resume all paused operations
pub fn resume_all(env: Env) {
    DefaultEmergencyGuard::resume_all(&env)
        .expect("Unauthorized");
}

// Get current pause state
pub fn get_pause_state(env: Env) -> u32 {
    DefaultEmergencyGuard::get_pause_state(&env)
}

// Check if specific operation is paused
pub fn is_paused(env: Env, operation: u32) -> bool {
    let state = DefaultEmergencyGuard::get_pause_state(&env);
    let pause_type = PauseType::new(state);
    pause_type.is_paused(operation)
}

// Get list of admins
pub fn get_admins(env: Env) -> Vec<Address> {
    DefaultEmergencyGuard::get_admins(&env)
}

// Get multi-sig threshold
pub fn get_threshold(env: Env) -> u32 {
    DefaultEmergencyGuard::get_threshold(&env)
}

// Add new admin
pub fn add_admin(env: Env, new_admin: Address) {
    DefaultEmergencyGuard::add_admin(&env, new_admin)
        .expect("Unauthorized or threshold would be violated");
}

// Remove admin
pub fn remove_admin(env: Env, admin: Address) {
    DefaultEmergencyGuard::remove_admin(&env, admin)
        .expect("Unauthorized or threshold would be violated");
}

// Rotate admin (current admin transfers to new admin)
pub fn rotate_admin(env: Env, new_admin: Address) {
    DefaultEmergencyGuard::rotate_admin(&env, new_admin)
        .expect("Unauthorized");
}
```

### 9. Update Tests

**BEFORE:**

```rust
#[test]
fn test_paused_functions() {
    let env = Env::default();
    let token_a = Address::random(&env);
    let token_b = Address::random(&env);
    let admin = Address::random(&env);
    let user = Address::random(&env);

    env.mock_all_auths();

    let contract_id = env.register_contract(None, LiquidityPool);
    let client = LiquidityPoolClient::new(&env, &contract_id);

    client.initialize(&admin, &token_a, &token_b, &500);

    // Pause the contract
    client.set_paused(&true);

    // Try to swap - should panic
    let err = env.auths();
    // Old test logic...
}
```

**AFTER:**

```rust
#[test]
fn test_granular_pause_swaps() {
    let env = Env::default();
    let token_a = Address::random(&env);
    let token_b = Address::random(&env);
    let admin = Address::random(&env);
    let user = Address::random(&env);

    env.mock_all_auths();

    let contract_id = env.register_contract(None, LiquidityPool);
    let client = LiquidityPoolClient::new(&env, &contract_id);

    client.initialize(&admin, &token_a, &token_b, &500);

    // Pause only swaps
    client.pause_swaps();

    // Verify swaps are paused
    assert!(client.is_paused(&PauseType::SWAP));

    // Verify deposits are NOT paused
    assert!(!client.is_paused(&PauseType::DEPOSIT));

    // Try to swap - should fail
    let result = client.try_swap(&user, &100);
    assert!(result.is_err());

    // Deposits should still work
    let result = client.try_deposit(&user, &100, &100);
    assert!(result.is_ok());
}

#[test]
fn test_emergency_pause_all() {
    let env = Env::default();
    let token_a = Address::random(&env);
    let token_b = Address::random(&env);
    let admin = Address::random(&env);

    env.mock_all_auths();

    let contract_id = env.register_contract(None, LiquidityPool);
    let client = LiquidityPoolClient::new(&env, &contract_id);

    client.initialize(&admin, &token_a, &token_b, &500);

    // Emergency pause everything
    client.emergency_pause_all();

    // Verify everything is paused
    assert!(client.is_paused(&PauseType::SWAP));
    assert!(client.is_paused(&PauseType::DEPOSIT));
    assert!(client.is_paused(&PauseType::WITHDRAW));
}

#[test]
fn test_admin_rotation() {
    let env = Env::default();
    let admin1 = Address::random(&env);
    let admin2 = Address::random(&env);
    let token_a = Address::random(&env);
    let token_b = Address::random(&env);

    env.mock_all_auths();

    let contract_id = env.register_contract(None, LiquidityPool);
    let client = LiquidityPoolClient::new(&env, &contract_id);

    client.initialize(&admin1, &token_a, &token_b, &500);

    // Verify admin1 is in admins list
    let admins = client.get_admins();
    assert!(admins.contains(&admin1));

    // Rotate admin from admin1 to admin2
    client.rotate_admin(&admin2);

    // Verify admin2 is now in admins list
    let admins = client.get_admins();
    assert!(admins.contains(&admin2));
    assert!(!admins.contains(&admin1));

    // admin2 can now pause operations
    client.pause_swaps();
    assert!(client.is_paused(&PauseType::SWAP));
}
```

## Complete Example: lib.rs Fragment

Here's a complete example of the key sections of an updated liquidity_pool contract:

```rust
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Vec, vec};
use emergency_guard::{DefaultEmergencyGuard, PauseType, GuardError};

#[contracttype]
pub enum DataKey {
    Admin,
    TokenA,
    TokenB,
    Fee,
    TotalSharesOutstanding,
    Balance(Address),
    Allowance(AllowanceDataKey),
}

#[contract]
pub struct LiquidityPool;

#[contractimpl]
impl LiquidityPool {
    pub fn initialize(
        env: Env,
        admin: Address,
        token_a: Address,
        token_b: Address,
        fee_bps: u32,
    ) {
        admin.require_auth();

        if fee_bps >= 10000 {
            panic!("Invalid fee");
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::TokenA, &token_a);
        env.storage().instance().set(&DataKey::TokenB, &token_b);
        env.storage().instance().set(&DataKey::Fee, &fee_bps);
        env.storage().instance().set(&DataKey::TotalSharesOutstanding, &0i128);

        // Initialize emergency guard
        let admins = vec![&env, admin];
        DefaultEmergencyGuard::init_guard(&env, admins, 1)
            .expect("Failed to initialize emergency guard");
    }

    pub fn deposit(env: Env, user: Address, amount_a: i128, amount_b: i128) -> i128 {
        DefaultEmergencyGuard::check_not_paused(&env, PauseType::DEPOSIT)
            .expect("Deposits are paused");

        user.require_auth();
        // ... deposit logic
        0
    }

    pub fn swap(env: Env, user: Address, in_amount: i128) -> i128 {
        DefaultEmergencyGuard::check_not_paused(&env, PauseType::SWAP)
            .expect("Swaps are paused");

        user.require_auth();
        // ... swap logic
        in_amount * 2
    }

    pub fn withdraw(env: Env, user: Address, shares_amount: i128) -> (i128, i128) {
        DefaultEmergencyGuard::check_not_paused(&env, PauseType::WITHDRAW)
            .expect("Withdrawals are paused");

        user.require_auth();
        // ... withdraw logic
        (0, 0)
    }

    pub fn pause_swaps(env: Env) {
        DefaultEmergencyGuard::set_pause_state(&env, PauseType::SWAP, true)
            .expect("Unauthorized");
    }

    pub fn emergency_pause_all(env: Env) {
        DefaultEmergencyGuard::emergency_pause_all(&env)
            .expect("Unauthorized");
    }

    pub fn resume_all(env: Env) {
        DefaultEmergencyGuard::resume_all(&env)
            .expect("Unauthorized");
    }

    pub fn rotate_admin(env: Env, new_admin: Address) {
        DefaultEmergencyGuard::rotate_admin(&env, new_admin)
            .expect("Unauthorized");
    }

    pub fn get_admins(env: Env) -> Vec<Address> {
        DefaultEmergencyGuard::get_admins(&env)
    }

    pub fn is_paused(env: Env, operation: u32) -> bool {
        let state = DefaultEmergencyGuard::get_pause_state(&env);
        let pause_type = PauseType::new(state);
        pause_type.is_paused(operation)
    }
}
```

## Benefits of This Integration

✅ **Consistency**: All contracts use the same pause mechanism  
✅ **Flexibility**: Granular control over different operations  
✅ **Safety**: Standardized admin management with rotation  
✅ **Auditability**: Events logged for all operations  
✅ **Maintainability**: Centralized logic, easier to upgrade  
✅ **Code Reuse**: No duplication across contracts

## Migration Checklist

- [ ] Update `Cargo.toml` with emergency_guard dependency
- [ ] Add imports for `DefaultEmergencyGuard`, `PauseType`
- [ ] Remove old `DataKey::Paused` and related code
- [ ] Update `initialize()` to call `init_guard()`
- [ ] Update all pausable functions with `check_not_paused()` calls
- [ ] Remove old `set_paused()` function
- [ ] Add new granular pause functions
- [ ] Update all tests
- [ ] Test granular pause scenarios
- [ ] Test admin rotation
- [ ] Deploy and verify
