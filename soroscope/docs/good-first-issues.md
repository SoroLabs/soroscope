# 🌟 Structured Issues for Soroscope

We have refined our issue list to ensure every contribution is meaningful and impactful. Below are the open issues for you to tackle, structured with clear context, expectations, and timeframes.

---

## 🟢 Trivial Issues (100 Points)

### #41 [Contract] docs: Add Rustdoc to Liquidity Pool

**Description**
The Liquidity Pool contract is the core of our DEX, but currently lacks inline documentation.

**Goal**
Add Rustdoc comments to public functions and structs to improve developer onboarding and code maintainability.

**Context**
The \`LiquidityPool\` contract uses the Soroban SDK. We need standard Rustdoc comments (\`///\`) for all public functions and structs so that `cargo doc` produces useful output.

**Requirements**
- Check \`contracts/liquidity_pool/src/lib.rs\`.
- Ensure \`initialize\`, \`deposit\`, \`swap\`, and \`withdraw\` have clear descriptions of parameters and return values.

**Implementation Guidelines**
- Use standard Rust documentation style.
- Verification: Run `cargo doc` to ensure it compiles without warnings.

**Complexity**
Trivial (100 points): Documentation only.

**Commitment**
Please provide an ETA (Estimated Time of Arrival) when requesting to be assigned to this issue. Failure to provide an ETA may result in the assignment being delayed or rejected.

---

### #42 [Contract] test: Add Zero-Amount Deposit Test

**Description**
Edge cases in DeFi contracts can lead to unexpected behavior. We need to verify robustness against zero-value inputs.

**Goal**
Verify that the `deposit` function handles zero amounts correctly (either by rejecting them or handling them gracefully).

**Context**
Current tests cover standard flows. We need a specific test case for \`amount_a = 0\` and \`amount_b = 0\`.

**Requirements**
- Add a test case in \`contracts/liquidity_pool/src/test.rs\`.
- Assert that the transaction either fails with a specific error or results in 0 shares minted without panic.

**Implementation Guidelines**
- Look at existing tests in `test.rs` for pattern.
- Create a new test function `test_deposit_zero_amount`.

**Complexity**
Trivial (100 points): Single test case.

**Commitment**
Please provide an ETA (Estimated Time of Arrival) when requesting to be assigned to this issue. Failure to provide an ETA may result in the assignment being delayed or rejected.

---

### #43 [Contract] refactor: Use 'const' for Error Codes

**Description**
Magic numbers in error reporting make debugging and maintaining validity checks difficult.

**Goal**
Refactor error codes to use explicit constants or enum variants coverage across the module.

**Context**
The \`Error\` enum in \`contracts/liquidity_pool/src/lib.rs\` is already an enum, but some logic might still be using raw numbers or implicit casts.

**Requirements**
- Scan `lib.rs` for any hardcoded integer returns in `Err(...)`.
- Replace them with `Error::Variant`.

**Implementation Guidelines**
- Ensure no logic change, only refactoring.
- Run `cargo test` to ensure no regressions.

**Complexity**
Trivial (100 points): Code cleanup.

**Commitment**
Please provide an ETA (Estimated Time of Arrival) when requesting to be assigned to this issue. Failure to provide an ETA may result in the assignment being delayed or rejected.

---

### #44 [Contract] test: Verify Event Emission in Token Contract

**Description**
Events are crucial for indexers and UIs to track contract activity.

**Goal**
Ensure that the token contract (or our usage of it) emits the expected events during operations.

**Context**
The \`LiquidityPool\` emits \`Deposit\`, \`Swap\`, and \`Withdraw\` events. We need to verify these are actually emitted with the correct data in \`test.rs\`.

**Requirements**
- Extend \`test_events\` in \`contracts/liquidity_pool/src/test.rs\`.
- Assert the *content* of the events match the transaction details.

**Implementation Guidelines**
- Use `e.events().all()` to inspect the event log.
- Match against expected topics and data.

**Complexity**
Trivial (100 points): Test assertion updates.

**Commitment**
Please provide an ETA (Estimated Time of Arrival) when requesting to be assigned to this issue. Failure to provide an ETA may result in the assignment being delayed or rejected.

---

## 🟡 Medium Issues (150 Points)

### #45 [Contract] feat: Implement 'approve' in Liquidity Pool

**Description**
Our LP Share token needs to support the standard `approve` pattern (allowance) for third-party compatibility.

**Goal**
Implement the `approve` function to allow third-party contracts to spend user shares.

**Context**
To be fully compatible with the Soroban Token Interface (SEP-41), we must support allowances.

**Requirements**
- Implement \`approve(e: Env, from: Address, spender: Address, amount: i128, expiration_ledger: u32)\`.
- Store allowances in \`Persistent\` storage.
- Add \`allowance\` read function.

**Implementation Guidelines**
- Use `DataKey::Allowance(AllowanceDataKey)` pattern for storage.
- Ensure proper authorization checks (`from.require_auth()`).

**Complexity**
Medium (150 points): State change and authorization logic.

**Commitment**
Please provide an ETA (Estimated Time of Arrival) when requesting to be assigned to this issue. Failure to provide an ETA may result in the assignment being delayed or rejected.

---

### #46 [Contract] feat: Implement 'transfer_from' in Liquidity Pool

**Description**
Completes the standard token interface by adding `transfer_from`.

**Goal**
Allow a spender to transfer tokens on behalf of an owner, up to the approved allowance.

**Context**
Depends on #45 (Approve). This is the second half of the allowance mechanism.

**Requirements**
- Implement \`transfer_from(e: Env, spender: Address, from: Address, to: Address, amount: i128)\`.
- Check that `allowance >= amount`.
- Decrement allowance and perform transfer.

**Implementation Guidelines**
- Reuse `transfer` logic if possible or extract shared logic.
- Ensure atomicity.

**Complexity**
Medium (150 points): Interdependent state logic.

**Commitment**
Please provide an ETA (Estimated Time of Arrival) when requesting to be assigned to this issue. Failure to provide an ETA may result in the assignment being delayed or rejected.

---

### #47 [Contract] feat: Add 'burn' to Liquidity Pool

**Description**
A raw `burn` function allows users to voluntarily destroy shares without withdrawing assets.

**Goal**
Implement a `burn` function for deflationary mechanisms or proof-of-burn.

**Context**
Currently, shares are only burned during `withdraw` (which returns assets). This new function should burn shares *without* returning assets (increasing the value of remaining shares).

**Requirements**
- Implement \`burn(e: Env, from: Address, amount: i128)\`.
- Reduce \`TotalShares\` and user balance.
- Do *not* send any tokens back to the user.
- Emit \`Burn\` event.

**Implementation Guidelines**
- Ensure `from` has enough balance.
- Update `TotalShares` storage.

**Complexity**
Medium (150 points): State manipulation and event emission.

**Commitment**
Please provide an ETA (Estimated Time of Arrival) when requesting to be assigned to this issue. Failure to provide an ETA may result in the assignment being delayed or rejected.

---

### #48 [Contract] feat: Add Pausable Functionality

**Description**
Security mechanism to stop contract operations in emergencies.

**Goal**
Allow an admin to pause deposits and swaps to prevent loss of funds during an incident.

**Context**
We need a `paused` state that acts as a circuit breaker for all state-changing functions.

**Requirements**
- Add \`Admin\` state to the contract (initialized in \`initialize\`).
- Add \`set_paused(e: Env, paused: bool)\`.
- Add checks in \`deposit\`, \`swap\`, \`withdraw\` to revert if \`paused\` is true.

**Implementation Guidelines**
- Define `DataKey::Admin` and `DataKey::Paused`.
- Create a helper `check_paused(&e)` to reduce code duplication.

**Complexity**
Medium (150 points): Access control and state management.

**Commitment**
Please provide an ETA (Estimated Time of Arrival) when requesting to be assigned to this issue. Failure to provide an ETA may result in the assignment being delayed or rejected.

---

## 🔴 High Issues (200 Points)

### #49 [Contract] feat: Admin Fee Control

**Description**
The protocol fee is currently hardcoded and immutable.

**Goal**
Make the swap fee adjustable by the admin, within safe limits.

**Context**
We need flexibility to adjust fees (e.g., 0.1% to 1%) without upgrading the entire contract.

**Requirements**
- Store \`FeeBasisPoints\` in storage (default 30).
- Valid range check: 0 to 100 bps.
- Update \`swap\` formula to use the stored fee.
- Add admin function \`set_fee\`.

**Implementation Guidelines**
- Reuse `Admin` key from #48 if available, or create new.
- Ensure math handles the dynamic variable correctly (overflow checks).

**Complexity**
High (200 points): Mathematical implications and access control.

**Commitment**
Please provide an ETA (Estimated Time of Arrival) when requesting to be assigned to this issue. Failure to provide an ETA may result in the assignment being delayed or rejected.

---

### #50 [Contract] test: Fuzz Testing for Swap Formula

**Description**
Constant product formula verification across the entire input space.

**Goal**
Ensure the swap formula `x * y = k` never violates invariants (never decreases k) through fuzz testing.

**Context**
Edge cases with large numbers or small numbers can cause rounding issues. Fuzzing explores these edge cases automatically.

**Requirements**
- Use \`soroban-sdk\` test fuzzing capabilities or \`proptest\`.
- Define invariants: `k_after >= k_before`.
- Test with max \`i128\` values.

**Implementation Guidelines**
- Write a property-based test.
- Run it with a large number of iterations.

**Complexity**
High (200 points): Advanced testing techniques.

**Commitment**
Please provide an ETA (Estimated Time of Arrival) when requesting to be assigned to this issue. Failure to provide an ETA may result in the assignment being delayed or rejected.

---

### #51 [Contract] feat: Implement Liquidity Pool Factory

**Status:** Completed via #51.
**Summary:** Implemented a factory contract that deploys unique `LiquidityPool` contracts using deterministic addresses (salt).
