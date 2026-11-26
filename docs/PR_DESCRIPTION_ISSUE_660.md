# Pull Request: [Contract] Add Token Burn From Authorization & Event Emission

## 🎯 Overview
This PR addresses Issue #660 by implementing dedicated contract event emission for the `burn` and `burn_from` functions in the Soroban Token smart contract. Currently, the contract does not publish any events on token burn operations, which blocks search indexers and downstream accounting tools from tracking burned tokens. 

By adding structured `BurnEvent` emissions, we ensure that indexers can track burned supplies accurately and dynamically.

## ✨ Features Implemented
- **Structured Event Payload (`BurnEvent`)**: Defined a new `#[contracttype]` struct containing:
  - `burner`: The `Address` that initiated/authorized the burn operation (e.g., the spender in delegated burns or the owner in direct burns).
  - `target_account`: The `Address` whose tokens were actually burned.
  - `amount`: The `i128` quantity of tokens burned.
- **Direct Burn Event Emission**: Updated `burn` to emit the burn event.
- **Delegated Burn Event Emission**: Updated `burn_from` to emit the burn event containing the authorized spender as the `burner`.
- **Dual-Directory Alignment**: Updated both the active root-level contract (`contracts/token`) and the stale nested contract (`soroscope/contracts/token`) to keep both implementations consistent.

## 🔧 Technical Implementation
- Added `contracttype` macro usage to imports in `contract.rs`.
- Defined:
  ```rust
  #[derive(Clone, Debug, Eq, PartialEq)]
  #[contracttype]
  pub struct BurnEvent {
      pub burner: Address,
      pub target_account: Address,
      pub amount: i128,
  }
  ```
- In `burn(e: Env, from: Address, amount: i128)`:
  - Published topic tuple `(String::from_str(&e, "burn"), from.clone())`
  - Published value `BurnEvent { burner: from.clone(), target_account: from, amount }`
- In `burn_from(e: Env, spender: Address, from: Address, amount: i128)`:
  - Published topic tuple `(String::from_str(&e, "burn"), from.clone())`
  - Published value `BurnEvent { burner: spender, target_account: from, amount }`

## 📋 Event Topics & Value Specification
- **Topics**:
  - Topic 1: `String("burn")` (the event classification topic)
  - Topic 2: `Address` (the target account whose balance was spent/burned)
- **Value**:
  - `BurnEvent` struct (contains `burner`, `target_account`, and `amount`)

## 🧪 Testing & Verification
Added comprehensive test suites to both the root and nested `test.rs` files:
- **`test_burn_emits_event`**:
  - Mints tokens to user.
  - Performs `burn`.
  - Asserts that a contract event with the `burn` topic and matching `user` address is emitted.
  - Deserializes and validates the `BurnEvent` payload: `burner == user`, `target_account == user`, `amount == 400`.
- **`test_burn_from_emits_event`**:
  - Mints tokens to user and approves spender.
  - Performs `burn_from`.
  - Asserts that a contract event with the `burn` topic and matching `user` address is emitted.
  - Deserializes and validates the `BurnEvent` payload: `burner == spender`, `target_account == user`, `amount == 300`.

## 📁 Files Changed
- `contracts/token/src/contract.rs` - Defined event and published events in `burn`/`burn_from`
- `contracts/token/src/test.rs` - Added unit tests for events and cleaned up duplicate imports
- `soroscope/contracts/token/src/contract.rs` - Mirrored nested implementation changes
- `soroscope/contracts/token/src/test.rs` - Mirrored nested test suite changes

## ✅ Checklist
- [x] Implementation completed according to issue specifications.
- [x] Code passes all linting and formatting checks (`cargo fmt`).
- [x] Unit tests added and passing successfully on both root and nested contract structures.
- [x] Followed conventional commits formatting.

## 🔗 Related Issues
Closes #660
