# EmergencyGuard - Visual Guide

## Quick Overview

```
Your Contracts:
┌─────────────────────────────────────────┐
│ liquidity_pool                          │
│ ├── swap()              ← check pause?  │
│ ├── deposit()           ← check pause?  │
│ ├── withdraw()          ← check pause?  │
│ ├── pause_swaps()       ← admin only    │
│ ├── rotate_admin()      ← admin only    │
│ └── ...                                 │
└─────────────────────────────────────────┘
         │ uses
         ▼
EmergencyGuard:
┌─────────────────────────────────────────┐
│ DefaultEmergencyGuard::                  │
│ ├── init_guard()        ← initialize    │
│ ├── check_not_paused()  ← before ops    │
│ ├── set_pause_state()   ← toggle pause  │
│ ├── rotate_admin()      ← transfer auth │
│ ├── emergency_pause_all() ← emergency   │
│ └── ...                                 │
└─────────────────────────────────────────┘
         │ stores in
         ▼
Soroban Storage:
┌─────────────────────────────────────────┐
│ PauseState: u32 (bitmask)               │
│ Admins: Vec<Address>                    │
│ Threshold: u32                          │
└─────────────────────────────────────────┘
```

## Pause State Visualization

### Nothing Paused

```
Bit:  7 6 5 4 3 2 1 0
      0 0 0 0 0 0 0 0
      │ │ │ │ │ │ │ └─ SWAP
      │ │ │ │ │ │ └─── DEPOSIT
      │ │ │ │ │ └───── WITHDRAW
      │ │ │ │ └─────── TRANSFER
      │ │ │ └───────── MINT
      │ │ └─────────── BURN
      │ └───────────── (unused)
      └─────────────── (unused)

State: 0x00 (hex)

Can do: swap ✓ deposit ✓ withdraw ✓ transfer ✓ mint ✓ burn ✓
```

### Swaps Paused

```
Bit:  7 6 5 4 3 2 1 0
      0 0 0 0 0 0 0 1  ← Bit 0 set
      │ │ │ │ │ │ │ └─ SWAP
      │ │ │ │ │ │ └─── DEPOSIT
      │ │ │ │ │ └───── WITHDRAW
      │ │ │ │ └─────── TRANSFER
      │ │ │ └───────── MINT
      │ │ └─────────── BURN
      │ └───────────── (unused)
      └─────────────── (unused)

State: 0x01 (hex)

Can do: swap ✗ deposit ✓ withdraw ✓ transfer ✓ mint ✓ burn ✓
```

### Swaps + Deposits Paused

```
Bit:  7 6 5 4 3 2 1 0
      0 0 0 0 0 0 1 1  ← Bits 0 and 1 set
      │ │ │ │ │ │ │ └─ SWAP
      │ │ │ │ │ │ └─── DEPOSIT
      │ │ │ │ │ └───── WITHDRAW
      │ │ │ │ └─────── TRANSFER
      │ │ │ └───────── MINT
      │ │ └─────────── BURN
      │ └───────────── (unused)
      └─────────────── (unused)

State: 0x03 (hex)

Can do: swap ✗ deposit ✗ withdraw ✓ transfer ✓ mint ✓ burn ✓
```

### Emergency Pause All

```
Bit:  7 6 5 4 3 2 1 0
      1 1 1 1 1 1 1 1  ← All bits set
      │ │ │ │ │ │ │ └─ SWAP
      │ │ │ │ │ │ └─── DEPOSIT
      │ │ │ │ │ └───── WITHDRAW
      │ │ │ │ └─────── TRANSFER
      │ │ │ └───────── MINT
      │ │ └─────────── BURN
      │ └───────────── (unused)
      └─────────────── (unused)

State: 0xFF (hex)

Can do: swap ✗ deposit ✗ withdraw ✗ transfer ✗ mint ✗ burn ✗
```

## Operation Flow

### Normal Operation (Nothing Paused)

```
User calls swap(amount)
       ▼
┌──────────────────────────────┐
│ DefaultEmergencyGuard::       │
│ check_not_paused(SWAP)       │
└──────────────────────────────┘
       ▼
   Is SWAP bit set?
   ┌─────┬─────┐
   No    Yes
   │      │
   ▼      ▼
  OK    Error: Paused
       │
       ▼
  Continue with swap
  Execute business logic
  Return result
```

### When Paused

```
Admin calls pause_swaps()
       ▼
┌──────────────────────────────┐
│ DefaultEmergencyGuard::       │
│ set_pause_state(SWAP, true)  │
└──────────────────────────────┘
       ▼
  Update storage:
  Old: 0x00 (00000000)
  New: 0x01 (00000001)  ← SWAP bit set
       │
       ▼
  Emit log event

User calls swap(amount)
       ▼
┌──────────────────────────────┐
│ DefaultEmergencyGuard::       │
│ check_not_paused(SWAP)       │
└──────────────────────────────┘
       ▼
   Is SWAP bit set?
   ┌─────┬─────┐
   No    Yes
   │      │
   ▼      ▼
  OK    Error: Paused ← Swaps blocked!

Deposits still work:
┌──────────────────────────────┐
│ DefaultEmergencyGuard::       │
│ check_not_paused(DEPOSIT)    │
└──────────────────────────────┘
       ▼
   Is DEPOSIT bit set?
   ┌─────┬─────┐
   No    Yes
   │      │
   ▼      ▼
  OK    Error: Paused
  ✓ Deposits continue!
```

## Admin Rotation Flow

### Before Rotation

```
Storage:
  Admins: [alice, bob, charlie]

Who can pause?
  ✓ alice
  ✓ bob
  ✓ charlie
```

### Rotation Request

```
alice.require_auth() ✓
DefaultEmergencyGuard::rotate_admin(david)
```

### After Rotation

```
Storage:
  Admins: [david, bob, charlie]

Who can pause now?
  ✗ alice (REMOVED)
  ✓ bob
  ✓ charlie
  ✓ david (NEW)
```

## Multi-Admin Example

### Setup

```
Admin List:  [alice, bob, charlie]
Threshold:   2 (need 2 of 3)

Operations:
  set_pause_state()      → any admin (Alice can pause)
  emergency_pause_all()  → any admin (Bob can emergency pause)
  rotate_admin()         → any admin (Charlie can rotate)
  add_admin()            → any admin
  remove_admin()         → any admin
```

### Future: Multi-Sig Voting

```
Admin List:    [alice, bob, charlie]
Threshold:     2

To pause all:
  alice votes:  pause_all
               Signatures: 1/2

  bob votes:   pause_all
               Signatures: 2/2 ← Threshold reached!

  Operation executes!
```

## Contract Integration Example

### Before EmergencyGuard

```rust
pub fn swap(env: Env, amount: i128) -> i128 {
    // Manual pause check
    let paused: bool = env.storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false);

    if paused {
        panic!("Paused");
    }

    // ... swap logic
}

pub fn deposit(env: Env, amount: i128) {
    // Duplicate pause check
    let paused: bool = env.storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false);

    if paused {
        panic!("Paused");
    }

    // ... deposit logic
}

pub fn set_paused(env: Env, paused: bool) {
    // Manual admin check
    let admin: Address = env.storage()
        .instance()
        .get(&DataKey::Admin)
        .unwrap();
    admin.require_auth();

    // Manual pause update
    env.storage().instance().set(&DataKey::Paused, &paused);
}

// No granular control
// No admin rotation
// No event logging
```

### After EmergencyGuard

```rust
pub fn swap(env: Env, amount: i128) -> i128 {
    // One-liner pause check
    DefaultEmergencyGuard::check_not_paused(&env, PauseType::SWAP)?;

    // ... swap logic
}

pub fn deposit(env: Env, amount: i128) {
    // One-liner pause check
    DefaultEmergencyGuard::check_not_paused(&env, PauseType::DEPOSIT)?;

    // ... deposit logic
}

pub fn pause_swaps(env: Env) {
    // Standardized pause control
    DefaultEmergencyGuard::set_pause_state(&env, PauseType::SWAP, true)?;
    // Automatic: auth check, storage update, event logging
}

pub fn pause_deposits(env: Env) {
    // New granular control
    DefaultEmergencyGuard::set_pause_state(&env, PauseType::DEPOSIT, true)?;
}

pub fn rotate_admin(env: Env, new_admin: Address) {
    // New admin rotation
    DefaultEmergencyGuard::rotate_admin(&env, new_admin)?;
}

// ✓ Granular control (pause swaps, keep deposits)
// ✓ Admin rotation (secure authority transfer)
// ✓ Event logging (audit trail)
// ✓ No code duplication
// ✓ Multi-sig ready
```

## Testing Workflow

```
Test Initialize
  ▼
DefaultEmergencyGuard::init_guard()
  ├── Stores admins
  ├── Stores threshold
  └── Initializes pause state to 0
  ▼
Test Operations
  ├── check_not_paused() returns Ok when unpaused
  ├── check_not_paused() returns Err when paused
  └── get_pause_state() returns correct bitmask
  ▼
Test Granular Pause
  ├── Pause SWAP (0x01)
  ├── Deposit still works (DEPOSIT not paused)
  ├── Withdraw still works (WITHDRAW not paused)
  ├── Unpause SWAP
  └── All work again
  ▼
Test Admin Rotation
  ├── alice is in admins
  ├── alice.rotate_admin(david)
  ├── alice no longer in admins
  ├── david is in admins
  └── david can pause operations
  ▼
Test Emergency
  ├── emergency_pause_all() sets all bits
  ├── All operations fail with Paused
  ├── resume_all() clears all bits
  └── All operations work again
  ▼
All Tests Pass ✓
```

## Storage Comparison

### Single Operation (Current Way)

```
liquidity_pool:
  storage.instance().set(DataKey::Paused, false)
  Size: 1 byte

token:
  storage.instance().set(DataKey::Paused, false)
  Size: 1 byte

factory:
  storage.instance().set(DataKey::Paused, false)
  Size: 1 byte

... repeated for 100+ contracts ...

Total: 100+ bytes for single operation across contracts
```

### Multiple Operations (New Way)

```
All contracts:
  storage.instance().set(DataKey::PauseState, PauseType::new(0))
  Size: 4 bytes

  Can pause 32 different operations!

  Admins: vec![alice, bob]
  Size: 64 bytes (2 addresses × 32 bytes)

  Threshold: 2
  Size: 4 bytes

Total: 72 bytes for all contracts, supports 32 operations each!

Savings: 28+ bytes per contract (87.5% reduction)
```

---

Created with detailed ASCII diagrams to help visualize the system behavior.
