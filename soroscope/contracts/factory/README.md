# Liquidity Pool Factory

The Liquidity Pool Factory is responsible for deploying new liquidity pool contracts for unique token pairs on Soroban.

## Emergency Guard Integration

To enhance protocol security, the pools deployed by the factory (and any associated vaults) can be integrated with the `EmergencyGuard` contract. The Emergency Guard provides a standardized mechanism for pausing operations securely across the protocol.

### Multi-Sig Security

The `EmergencyGuard` uses a multi-signature architecture. Administrative actions cannot be executed by a single user; instead, they require a quorum of authorized administrators to approve the action.

**Key Multi-Sig Features:**
- **Thresholds**: During initialization, a signature `threshold` is set (e.g., 2 out of 3 admins).
- **Required Operations**: Multi-sig approval is required for critical actions:
  - `pause(env, approvers, pause_state)`
  - `unpause(env, approvers, pause_state)`
  - `add_admin(env, approvers, new_admin)`
  - `remove_admin(env, approvers, admin)`
  - `update_threshold(env, approvers, new_threshold)`
- **Validation**: When an action is invoked, you must pass an array of `approvers`. The contract verifies that the number of unique approvers meets or exceeds the threshold, and that each approver has validly signed the invocation (using Soroban's `addr.require_auth()`).

### Granular Pause Features

Instead of an "all-or-nothing" pause, the `EmergencyGuard` uses a bitmask (`u32`) to pause specific operations granularly. This allows the protocol to halt risky operations while keeping safe operations functional.

**Available Pause Flags:**
- `SWAP` (Bit 0, `1 << 0`): Pauses trading operations in the pool.
- `DEPOSIT` (Bit 1, `1 << 1`): Pauses adding liquidity.
- `WITHDRAW` (Bit 2, `1 << 2`): Pauses removing liquidity.
- `TRANSFER` (Bit 3, `1 << 3`): Pauses LP token transfers.
- `MINT` (Bit 4, `1 << 4`): Pauses minting of new LP tokens.
- `FLASH_LOAN` (Bit 6, `1 << 6`): Pauses flash loan operations.

**Usage:**
To pause `SWAP` and `DEPOSIT` simultaneously, the admins would calculate the bitmask:
`1 (SWAP) | 2 (DEPOSIT) = 3`
And then invoke `pause(env, approvers, 3)`.

### How to integrate with deployed pools

When deploying a new pool via the Factory, the resulting pool contract should query the global `EmergencyGuard` contract's pause state before executing sensitive operations:

```rust
let pause_state = guard_client.is_paused(&pause_type);
if pause_state {
    panic!("Operation is currently paused");
}
```

This guarantees that all pools spawned by the factory can be halted by the multi-sig administrative committee in the event of an emergency, protecting user funds and protocol integrity at all times.
