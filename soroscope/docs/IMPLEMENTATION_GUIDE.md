# Complete EmergencyGuard Implementation Guide

## Quick Start

### 1. Review the Module

The `emergency_guard` crate is located at `contracts/emergency_guard/` and contains:

- **[lib.rs](contracts/emergency_guard/src/lib.rs)** - Core trait, types, and implementation
- **[README.md](contracts/emergency_guard/README.md)** - Detailed API documentation
- **[examples/simple_token.rs](contracts/emergency_guard/examples/simple_token.rs)** - Example usage

### 2. Integrate into Your Contract

See [EMERGENCY_GUARD_INTEGRATION.md](contracts/EMERGENCY_GUARD_INTEGRATION.md) for step-by-step integration instructions.

### 3. Key Types

#### PauseType (Bitmask Enum)

```rust
PauseType::SWAP        // Pause swaps (1 << 0)
PauseType::DEPOSIT     // Pause deposits (1 << 1)
PauseType::WITHDRAW    // Pause withdrawals (1 << 2)
PauseType::TRANSFER    // Pause transfers (1 << 3)
PauseType::MINT        // Pause minting (1 << 4)
PauseType::BURN        // Pause burning (1 << 5)
```

#### GuardError (Error Codes)

```rust
GuardError::Unauthorized           // Caller not authorized
GuardError::Paused                 // Operation is paused
GuardError::InsufficientSignatures // Not enough multi-sig approvals
GuardError::InvalidThreshold       // Invalid admin count or threshold
GuardError::AdminNotFound          // Admin not in list
GuardError::QueueFull              // Admin rotation queue full
```

## Core Trait Methods

### Initialization

```rust
// Initialize guard with list of admins and signature threshold
DefaultEmergencyGuard::init_guard(env: &Env, admins: Vec<Address>, threshold: u32)
    -> GuardResult<()>
```

### Operation Checks

```rust
// Check if operation is paused (use before any pausable operation)
DefaultEmergencyGuard::check_not_paused(env: &Env, operation: u32)
    -> GuardResult<()>

// Get bitmask of all paused operations
DefaultEmergencyGuard::get_pause_state(env: &Env) -> u32

// Check if specific operation is paused
let pause = PauseType::new(state);
pause.is_paused(PauseType::SWAP)  // true/false
```

### Admin Controls

```rust
// Pause/unpause specific operation (single admin)
DefaultEmergencyGuard::set_pause_state(env: &Env, operation: u32, paused: bool)
    -> GuardResult<()>

// Pause all operations (emergency)
DefaultEmergencyGuard::emergency_pause_all(env: &Env) -> GuardResult<()>

// Resume all operations
DefaultEmergencyGuard::resume_all(env: &Env) -> GuardResult<()>

// Get admin list
DefaultEmergencyGuard::get_admins(env: &Env) -> Vec<Address>

// Get required signature threshold
DefaultEmergencyGuard::get_threshold(env: &Env) -> u32

// Check if address is admin
DefaultEmergencyGuard::is_admin(env: &Env, addr: Address) -> bool

// Add new admin
DefaultEmergencyGuard::add_admin(env: &Env, new_admin: Address) -> GuardResult<()>

// Remove admin
DefaultEmergencyGuard::remove_admin(env: &Env, admin: Address) -> GuardResult<()>

// Rotate admin (current admin -> new admin)
DefaultEmergencyGuard::rotate_admin(env: &Env, new_admin: Address) -> GuardResult<()>
```

## Implementation Pattern

Every contract that uses EmergencyGuard follows this pattern:

### 1. Initialization

```rust
pub fn initialize(env: Env, admin: Address, ...) {
    // ... contract-specific init ...

    // Initialize guard
    let admins = vec![&env, admin];
    DefaultEmergencyGuard::init_guard(&env, admins, 1)?;
}
```

### 2. Protection

```rust
pub fn pausable_operation(env: Env, ...) {
    // Check pause state FIRST
    DefaultEmergencyGuard::check_not_paused(&env, PauseType::SWAP)?;

    // Then do operation
    // ...
}
```

### 3. Admin Functions

```rust
pub fn pause_swaps(env: Env) {
    DefaultEmergencyGuard::set_pause_state(&env, PauseType::SWAP, true)?;
}

pub fn rotate_admin(env: Env, new_admin: Address) {
    DefaultEmergencyGuard::rotate_admin(&env, new_admin)?;
}
```

## Integration Checklist

- [ ] Review [contracts/emergency_guard/README.md](contracts/emergency_guard/README.md)
- [ ] Review [EMERGENCY_GUARD_INTEGRATION.md](contracts/EMERGENCY_GUARD_INTEGRATION.md)
- [ ] Add `emergency_guard` to contract's `Cargo.toml`
- [ ] Add imports in contract's `lib.rs`
- [ ] Call `init_guard()` in `initialize()` function
- [ ] Add `check_not_paused()` calls before pausable operations
- [ ] Replace old pause logic with new functions
- [ ] Update tests to verify granular pausing
- [ ] Deploy and test on testnet
- [ ] Monitor events for all pause/admin changes

## File Organization

```
contracts/
├── emergency_guard/              # NEW: Shared guard implementation
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs               # Trait, types, impl
│   │   └── test.rs              # Unit tests
│   ├── examples/
│   │   └── simple_token.rs       # Example contract using guard
│   └── README.md                 # Detailed API docs
│
├── liquidity_pool/               # Example: Can be updated to use guard
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   └── test.rs
│   └── ...
│
├── token/                        # Example: Can be updated to use guard
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   └── ...
│   └── ...
│
└── ... other contracts ...
```

## Contract-Specific Integration Examples

### Token Contract

```rust
// Cargo.toml
[dependencies]
emergency_guard = { path = "../emergency_guard" }

// src/lib.rs
use emergency_guard::{DefaultEmergencyGuard, PauseType};

#[contractimpl]
impl TokenContract {
    pub fn initialize(env: Env, admin: Address, name: String) {
        // ... token init ...
        let admins = vec![&env, admin];
        DefaultEmergencyGuard::init_guard(&env, admins, 1)?;
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        DefaultEmergencyGuard::check_not_paused(&env, PauseType::TRANSFER)?;
        // ... transfer logic ...
    }

    pub fn mint(env: Env, to: Address, amount: i128) {
        DefaultEmergencyGuard::check_not_paused(&env, PauseType::MINT)?;
        // ... mint logic ...
    }

    pub fn pause_transfers(env: Env) {
        DefaultEmergencyGuard::set_pause_state(&env, PauseType::TRANSFER, true)?;
    }
}
```

### Factory Contract

```rust
#[contractimpl]
impl FactoryContract {
    pub fn initialize(env: Env, admin: Address) {
        let admins = vec![&env, admin];
        DefaultEmergencyGuard::init_guard(&env, admins, 1)?;
    }

    pub fn create_pool(env: Env, ...) {
        DefaultEmergencyGuard::check_not_paused(&env, PauseType::SWAP)?;
        // ... factory logic ...
    }
}
```

### Liquidity Pool Contract

See [EMERGENCY_GUARD_INTEGRATION.md](contracts/EMERGENCY_GUARD_INTEGRATION.md) for complete liquidity pool integration example.

## Multi-Signature Extension (Advanced)

The current implementation supports a simple admin list with threshold. For production multi-sig:

```rust
// Future: Extend to verify actual signatures
pub fn emergency_pause_all_multisig(
    env: Env,
    approvals: Vec<(Address, Bytes)>,  // (admin, signature)
) -> GuardResult<()> {
    let threshold = DefaultEmergencyGuard::get_threshold(&env);
    let mut verified_count = 0;

    for (admin, sig) in approvals {
        if verify_signature(&env, &admin, sig) {
            verified_count += 1;
        }
    }

    if verified_count >= threshold {
        DefaultEmergencyGuard::emergency_pause_all(&env)
    } else {
        Err(GuardError::InsufficientSignatures)
    }
}
```

## Testing

### Unit Tests

All tests in [emergency_guard/src/test.rs](contracts/emergency_guard/src/test.rs):

```bash
cd contracts/emergency_guard
cargo test
```

### Contract Tests

Run individual contract tests:

```bash
cd contracts/liquidity_pool
cargo test

cd contracts/token
cargo test
```

### Test Coverage

- [x] PauseType bitmask operations
- [x] Granular pause/unpause
- [x] Emergency pause all / resume all
- [x] Admin rotation
- [x] Admin add/remove with threshold validation
- [x] Unauthorized access prevention

## Security Considerations

### 1. Admin Keys

- Store admin private keys in secure HSMs
- Use different admins for different roles
- Regular key rotation

### 2. Multi-Signature

- For critical operations, require multiple signatures
- Use timelock for sensitive changes
- Consider N-of-M voting

### 3. Access Control

- Only admins can call `set_pause_state`, `rotate_admin`, etc.
- `require_auth()` is enforced via Soroban SDK
- All operations logged for audit

### 4. Pause State

- Granular pausing prevents unnecessary disruption
- Emergency pause available for critical situations
- Can resume operations as soon as issues resolved

### 5. Admin Rotation

- Direct replacement - no intermediate state
- Only old admin can initiate rotation
- New admin takes control immediately

## Troubleshooting

### "Emergency guard not initialized"

Check that `init_guard()` is called in your contract's `initialize()` function.

### "Unauthorized" error

Verify the caller address is in the admins list. Check with:

```rust
DefaultEmergencyGuard::is_admin(&env, &caller_address)
```

### "Operation is paused"

Expected behavior! Check `get_pause_state()` to see what's paused:

```rust
let state = DefaultEmergencyGuard::get_pause_state(&env);
let pause_type = PauseType::new(state);
assert!(pause_type.is_paused(PauseType::SWAP));  // true
```

### Admin rotation failed

Ensure the caller (`env.invoker()`) is in the admin list:

```rust
DefaultEmergencyGuard::rotate_admin(&env, &new_admin)
```

## Next Steps

1. ✅ Review EmergencyGuard implementation
2. ✅ Read detailed documentation and examples
3. **TODO**: Integrate into liquidity_pool contract
4. **TODO**: Integrate into token contract
5. **TODO**: Update factory and other contracts
6. **TODO**: Add multi-sig verification for production
7. **TODO**: Deploy to testnet and test
8. **TODO**: Gather community feedback
9. **TODO**: Merge to main branch

## References

- [EmergencyGuard API Documentation](contracts/emergency_guard/README.md)
- [Integration Guide](contracts/EMERGENCY_GUARD_INTEGRATION.md)
- [Example Implementation](contracts/emergency_guard/examples/simple_token.rs)
- [Soroban SDK Docs](https://docs.soroban.stellar.org/)

## Support

For questions or issues:

1. Check the documentation in `contracts/emergency_guard/README.md`
2. Review the integration examples in `EMERGENCY_GUARD_INTEGRATION.md`
3. Look at working examples in `simple_token.rs`
4. Check test cases in `src/test.rs`

---

**Status**: ✅ Complete - Ready for integration into existing contracts

**Branch**: `feature/emergency-guard-trait`

**Last Updated**: 2026-04-24
