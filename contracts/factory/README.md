# Liquidity Pool Factory

The Liquidity Pool Factory is responsible for deploying new liquidity pool contracts for unique token pairs on Soroban.

## Emergency Guard Integration

To enhance protocol security, the factory integrates with the `EmergencyGuard` contract to provide standardized emergency pause and admin management capabilities.

### Guard Parameters

The factory uses the following guard parameters for emergency management:

#### Initialization Parameters

- **admins** (`Vec<Address>`): List of authorized administrator addresses
- **threshold** (`u32`): Number of required signatures for multi-sig operations (must be > 0 and ≤ admins.len())

#### Pause Type Flags

The factory supports granular pause operations using a bitmask (`u32`):

**Standard EmergencyGuard Flags:**
- `SWAP` (Bit 0, `1 << 0`): Pauses trading operations
- `DEPOSIT` (Bit 1, `1 << 1`): Pauses adding liquidity
- `WITHDRAW` (Bit 2, `1 << 2`): Pauses removing liquidity
- `TRANSFER` (Bit 3, `1 << 3`): Pauses LP token transfers
- `MINT` (Bit 4, `1 << 4`): Pauses minting of new LP tokens
- `BURN` (Bit 5, `1 << 5`): Pauses burning of LP tokens

**Factory-Specific Flags:**
- `CREATE_PAIR` (Bit 6, `1 << 6`): Pauses new liquidity pool pair creation in the factory

#### Storage Keys

The factory uses these storage keys for guard state:

```rust
DataKey::GuardPauseState      -> PauseType(u32)      // Bitmask of paused operations
GuardDataKey::Admins          -> Vec<Address>        // List of authorized admins
GuardDataKey::SignatureThreshold -> u32              // Required multi-sig threshold
```

### Guard Events

The factory emits standardized events for all guard-related operations:

#### Initialization Events

**Event:** `emergency_guard_initialized`
- **Payload:** `GuardInitializedEvent { admins: Vec<Address>, threshold: u32 }`
- **Emitted when:** Factory guard is initialized with admin committee

#### Pause State Events

**Event:** `emergency_guard_pause_state_changed`
- **Payload:** `PauseStateChangedEvent { admin: Address, operation: u32, paused: bool }`
- **Emitted when:** An admin changes pause state for a specific operation

**Event:** `emergency_guard.set_pause`
- **Payload:** `(operation: u32, paused: bool)`
- **Emitted when:** Pause state is updated

#### Emergency Events

**Event:** `emergency_guard_emergency_paused_all`
- **Payload:** `EmergencyPausedEvent { approvers: Vec<Address> }`
- **Emitted when:** Multi-sig emergency pause all operations is executed

**Event:** `emergency_guard.emergency_pause_all`
- **Payload:** `(approvers: Vec<Address>)`
- **Emitted when:** Emergency pause is activated

#### Resume Events

**Event:** `emergency_guard_resumed_all`
- **Payload:** `ResumedEvent { approvers: Vec<Address> }`
- **Emitted when:** Multi-sig resume all operations is executed

**Event:** `emergency_guard.resume_all`
- **Payload:** `(approvers: Vec<Address>)`
- **Emitted when:** All operations are resumed

#### Admin Management Events

**Event:** `emergency_guard_admin_added`
- **Payload:** `AdminAddedEvent { approvers: Vec<Address>, new_admin: Address }`
- **Emitted when:** Multi-sig adds a new admin

**Event:** `emergency_guard.admin_added`
- **Payload:** `()`
- **Emitted when:** Admin is added to the committee

**Event:** `emergency_guard_admin_removed`
- **Payload:** `AdminRemovedEvent { approvers: Vec<Address>, admin: Address }`
- **Emitted when:** Multi-sig removes an admin

**Event:** `emergency_guard.admin_removed`
- **Payload:** `()`
- **Emitted when:** Admin is removed from the committee

### Multi-Sig Security

The `EmergencyGuard` uses a multi-signature architecture. Administrative actions cannot be executed by a single user; instead, they require a quorum of authorized administrators to approve the action.

**Key Multi-Sig Features:**
- **Thresholds**: During initialization, a signature `threshold` is set (e.g., 2 out of 3 admins)
- **Required Operations**: Multi-sig approval is required for critical actions:
  - `emergency_guard_pause(env, approvers)` - Emergency pause all operations
  - `resume_guard(env, approvers)` - Resume all operations
  - `add_admin(env, approvers, new_admin)` - Add new admin
  - `remove_admin(env, approvers, admin)` - Remove admin
- **Validation**: When an action is invoked, you must pass an array of `approvers`. The contract verifies that:
  - The number of unique approvers meets or exceeds the threshold
  - Each approver is an authorized admin
  - Each approver has validly signed the invocation (using Soroban's `addr.require_auth()`)

### Granular Pause Features

Instead of an "all-or-nothing" pause, the `EmergencyGuard` uses a bitmask (`u32`) to pause specific operations granularly. This allows the protocol to halt risky operations while keeping safe operations functional.

**Usage Example:**
To pause `SWAP` and `DEPOSIT` simultaneously, the admins would calculate the bitmask:
`1 (SWAP) | 2 (DEPOSIT) = 3`
And then invoke `set_pause(env, admin, 3, true)`.

To pause only factory pair creation:
`set_pause(env, admin, CREATE_PAIR, true)` where `CREATE_PAIR = 1 << 6`

## Emergency Management

### Initialization

Initialize the factory guard with an admin committee:

```rust
// Initialize with multi-sig (3 admins, threshold = 2)
let admins = vec![&env, admin1, admin2, admin3];
factory.initialize(&env, admins, 2);

// Alternative: initialize_guard (same function)
factory.initialize_guard(&env, admins, 2);
```

### Emergency Pause Operations

#### Single-Admin Pause (Specific Operations)

Any single admin can pause specific operations without multi-sig:

```rust
// Pause pair creation
factory.set_operation_paused(&env, &admin, CREATE_PAIR, true);

// Pause multiple operations
factory.set_operation_paused(&env, &admin, SWAP | DEPOSIT, true);

// Resume specific operation
factory.set_operation_paused(&env, &admin, CREATE_PAIR, false);
```

#### Multi-Sig Emergency Pause (All Operations)

Requires multiple admin approvals based on threshold:

```rust
// Emergency pause all operations (requires threshold approvals)
let approvers = vec![&env, admin1, admin2]; // Must meet threshold
factory.emergency_guard_pause(&env, approvers);

// Resume all operations (requires threshold approvals)
factory.resume_guard(&env, approvers);
```

### Admin Management

#### Add Admin (Multi-Sig Required)

```rust
let approvers = vec![&env, admin1, admin2]; // Must meet threshold
factory.add_admin(&env, approvers, &new_admin_address);

// Alternative: add_guard_admin (same function)
factory.add_guard_admin(&env, approvers, &new_admin_address);
```

#### Remove Admin (Multi-Sig Required)

```rust
let approvers = vec![&env, admin1, admin2]; // Must meet threshold
factory.remove_admin(&env, approvers, &admin_to_remove);

// Alternative: remove_guard_admin (same function)
factory.remove_guard_admin(&env, approvers, &admin_to_remove);
```

**Safety Constraint:** Cannot remove admins if it would bring the count below the threshold.

### Query Functions

```rust
// Check if a specific operation is paused
let is_paused = factory.is_guard_paused(&env, CREATE_PAIR);
// Alternative: is_paused (same function)
let is_paused = factory.is_paused(&env, CREATE_PAIR);

// Get current pause state as bitmask
let pause_state = factory.get_pause_state(&env);

// Get list of admins
let admins = factory.get_admins(&env);

// Get required threshold
let threshold = factory.get_threshold(&env);

// Check if address is an admin
let is_admin = factory.is_admin(&env, &address);
```

### Integration with Deployed Pools

When deploying a new pool via the Factory, the resulting pool contract should query the factory's guard pause state before executing sensitive operations:

```rust
// Check if pair creation is paused before deploying
if factory.is_guard_paused(&env, CREATE_PAIR) {
    panic!("Pair creation is currently paused");
}

// Deploy pool
let pool_address = factory.create_pair(&env, &token_a, &token_b, &wasm_hash);
```

This guarantees that all pools spawned by the factory can be halted by the multi-sig administrative committee in the event of an emergency, protecting user funds and protocol integrity at all times.

### Error Handling

The guard returns standardized errors:

- `NotInitialized`: Guard has not been initialized
- `Unauthorized`: Caller is not an authorized admin
- `Paused`: Operation is currently paused
- `InsufficientSignatures`: Multi-sig threshold not met
- `InvalidThreshold`: Threshold configuration is invalid
- `AdminNotFound`: Admin address not found in committee
- `AlreadyInitialized`: Guard already initialized

### Security Best Practices

1. **Multi-Sig Configuration**: Use at least 2-of-3 or 3-of-5 multi-sig for production
2. **Threshold Validation**: Never set threshold to 0 or greater than admin count
3. **Admin Key Security**: Store admin keys in secure hardware wallets or key management systems
4. **Emergency Testing**: Regularly test emergency pause procedures on testnet
5. **Event Monitoring**: Monitor guard events for audit trails
6. **Gradual Unpause**: After emergency, unpause operations incrementally to verify safety
