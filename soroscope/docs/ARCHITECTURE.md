# EmergencyGuard - Architecture & Design Summary

## System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    All Soroban Contracts                        │
│  (liquidity_pool, token, factory, cross_call, etc.)            │
└──────────────────────────┬──────────────────────────────────────┘
                           │ depends on
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│                 EmergencyGuard Crate                            │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ DefaultEmergencyGuard (Main Implementation)              │  │
│  │  • init_guard()                                          │  │
│  │  • check_not_paused()                                    │  │
│  │  • set_pause_state() - granular pause control            │  │
│  │  • emergency_pause_all() / resume_all()                  │  │
│  │  • rotate_admin() - secure admin transitions             │  │
│  │  • add_admin() / remove_admin()                          │  │
│  │  • get_admins() / get_threshold()                        │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ PauseType (Bitmask - 32 operations in u32)               │  │
│  │  • SWAP        = 1 << 0                                  │  │
│  │  • DEPOSIT     = 1 << 1                                  │  │
│  │  • WITHDRAW    = 1 << 2                                  │  │
│  │  • TRANSFER    = 1 << 3                                  │  │
│  │  • MINT        = 1 << 4                                  │  │
│  │  • BURN        = 1 << 5                                  │  │
│  │  (plus 26 more available)                                │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ Storage (Instance Storage via DataKey)                   │  │
│  │  • PauseState       -> u32 (bitmask)                    │  │
│  │  • Admins           -> Vec<Address>                      │  │
│  │  • SignatureThreshold -> u32                             │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ Error Types                                              │  │
│  │  • Unauthorized                                          │  │
│  │  • Paused                                                │  │
│  │  • InsufficientSignatures                                │  │
│  │  • InvalidThreshold                                      │  │
│  │  • AdminNotFound                                         │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

## Usage Pattern

```
Contract Initialization
       ▼
┌──────────────────────────┐
│ pub fn initialize(...) { │
│   ...                    │
│   init_guard(admins, 1)  │────► Initialize guard with admin list
│ }                        │
└──────────────────────────┘
       ▲
       │ (once)

Operation Execution
       ▼
┌────────────────────────────────────┐
│ pub fn swap(amount) {              │
│   check_not_paused(SWAP)?  ◄────── Check if operation paused
│   // Execute swap                  │
│   transfer_tokens(...)             │
│ }                                  │
└────────────────────────────────────┘
       ▲
       │ (every operation)

Admin Control
       ▼
┌──────────────────────────────────┐
│ pub fn pause_swaps() {            │
│   set_pause_state(SWAP, true)     │ Pause swaps, keep others active
│ }                                 │
└──────────────────────────────────┘
       ▲
       │ (admin only)

Emergency Control
       ▼
┌──────────────────────────────────┐
│ pub fn emergency_pause_all() {    │
│   emergency_pause_all()  ◄─ Pause all (bitmask = U32::MAX)
│ }                                 │
└──────────────────────────────────┘
```

## Data Structure

### PauseState Bitmask Example

```
Initial State (Nothing Paused):
  0b00000000000000000000000000000000 = 0x00000000

Pause Swaps:
  0b00000000000000000000000000000001 = 0x00000001 (SWAP bit set)

Pause Swaps + Deposits:
  0b00000000000000000000000000000011 = 0x00000003 (SWAP + DEPOSIT bits)

Pause Swaps + Deposits + Withdrawals:
  0b00000000000000000000000000000111 = 0x00000007 (SWAP + DEPOSIT + WITHDRAW)

Emergency Pause All:
  0b11111111111111111111111111111111 = 0xFFFFFFFF (All bits set)
```

## Admin Rotation Flow

```
Current State:
  Admins: [admin1, admin2, admin3]

admin1 initiates rotation to admin4:
  admin1.require_auth() ✓

After rotation:
  Admins: [admin4, admin2, admin3]

Result:
  • admin1 is no longer an admin
  • admin4 is now an admin
  • No funds transferred
  • Change takes effect immediately
```

## Multi-Signature (Current vs Future)

### Current Implementation

```
Admin List: [admin1, admin2, admin3]
Threshold: 1

Any single admin can:
  ✓ set_pause_state()
  ✓ emergency_pause_all()
  ✓ rotate_admin()
```

### Future Enhancement

```
Admin List: [admin1, admin2, admin3]
Threshold: 2

For critical operations, require 2 of 3 signatures:
  require verify_signature(admin1, sig1) ✓
  require verify_signature(admin2, sig2) ✓

  Then: pause_all() executes
```

## Storage Efficiency

### Before (Boolean per contract)

```
liquidity_pool:  bool paused (1 byte per LP contract) + logic + tests
token:           bool paused (1 byte per token contract) + logic + tests
factory:         bool paused (1 byte per factory) + logic + tests
...multiply across 100+ contracts...
```

### After (Unified Bitmask)

```
ALL contracts: u32 pause_state (4 bytes total, 32 operations)
              Vec<Address> admins (shared)
              u32 threshold (shared)

Benefits:
  • 87.5% smaller pause state (1 byte vs 8+ bytes per operation)
  • Single implementation (no duplication)
  • 26 unused bits available for future operations
```

## Integration Checklist by Contract

| Contract       | Current Status        | Todo                     |
| -------------- | --------------------- | ------------------------ |
| liquidity_pool | EmergencyGuard bitmask (`PauseState` u32) | Done |
| token          | No pause              | Add pause support        |
| factory        | No pause              | Add pause support        |
| cross_call     | No pause              | Add pause support        |
| hello_soroban  | N/A                   | N/A                      |
| cpu_heavy      | N/A                   | N/A                      |
| storage_heavy  | N/A                   | N/A                      |

## Error Handling Examples

```rust
// Check if operation is paused
match DefaultEmergencyGuard::check_not_paused(&env, PauseType::SWAP) {
    Ok(()) => {
        // Continue with operation
    },
    Err(GuardError::Paused) => {
        // Operation blocked - already logged by guard
        return Err(ContractError::Paused);
    },
    Err(_) => {
        // Other errors (initialization issues, etc.)
        return Err(ContractError::InternalError);
    }
}
```

## Event Audit Trail

All administrative actions are logged:

```
[TIMESTAMP] Init Guard: admins=[addr1, addr2], threshold=2
[TIMESTAMP] Pause state updated: operation=1, paused=true
[TIMESTAMP] Emergency pause all activated by addr1
[TIMESTAMP] Resume all activated by addr2
[TIMESTAMP] Admin added: addr3
[TIMESTAMP] Admin removed: addr4
[TIMESTAMP] Admin rotated: addr1 -> addr5
```

## File Organization

```
contracts/
├── emergency_guard/
│   ├── Cargo.toml              ◄── Define package
│   ├── src/
│   │   ├── lib.rs              ◄── Core implementation
│   │   └── test.rs             ◄── Unit tests
│   ├── examples/
│   │   └── simple_token.rs      ◄── Example contract
│   └── README.md                ◄── API documentation
│
├── [other contracts]
│   └── Cargo.toml              ◄── Add emergency_guard dependency
│
├── IMPLEMENTATION_GUIDE.md      ◄── This file
├── EMERGENCY_GUARD_INTEGRATION.md ◄── How to integrate
└── EMERGENCY_GUARD_SETUP.md    ◄── Quick setup
```

## Key Features Summary

| Feature               | Description                   | Benefit                               |
| --------------------- | ----------------------------- | ------------------------------------- |
| **Granular Pausing**  | 32 individual operation types | Pause swaps, keep withdrawals working |
| **Multi-Sig Ready**   | Threshold + admin list        | Scale to N-of-M governance            |
| **Admin Rotation**    | Direct replacement            | No fund movement needed               |
| **Efficient Storage** | Bitmask (4 bytes)             | vs 8+ bytes traditional boolean       |
| **Event Logging**     | All actions logged            | Complete audit trail                  |
| **Error Types**       | Specific GuardError codes     | Clear error handling                  |
| **Test Coverage**     | Comprehensive unit tests      | Verify all operations work            |
| **Documentation**     | 3 detailed guides             | Easy onboarding                       |

## Deployment Timeline

```
Week 1: Complete ✅
  • Design & implement EmergencyGuard
  • Write tests and documentation
  • Create examples

Week 2: Integration (In Progress)
  • Integrate into liquidity_pool
  • Integrate into token
  • Integrate into factory

Week 3: Testing & Review
  • Test on testnet
  • Gather feedback
  • Optimize if needed

Week 4: Production
  • Merge to main
  • Deploy to mainnet
  • Monitor and support
```

## Security Guarantees

1. **Admin Authorization** - Only authorized admins can pause/unpause
2. **Atomic Operations** - Pause state changes are atomic
3. **No Fund Movement** - Admin rotation doesn't move funds
4. **Threshold Protection** - Can't remove below minimum admins
5. **Event Logging** - All operations logged for audit
6. **Graceful Degradation** - Pause doesn't lose user funds

## Future Enhancements

- [ ] Timelock for emergency_pause_all (e.g., 1 hour delay)
- [ ] Voting system for admin decisions
- [ ] Pause duration limits (auto-unpause)
- [ ] Cross-contract guard coordination
- [ ] Dashboard for monitoring pause states
- [ ] Integration with Stellar's multi-sig accounts
