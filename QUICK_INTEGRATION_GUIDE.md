# Quick Integration Guide - LP Deposit/Withdrawal Fee

## ⚡ Fast Track to Completion

### Current Status
- ✅ All documentation complete
- ✅ All tests written (17 comprehensive tests)
- ✅ Branch created: `feature/issue-104-amm-deposit-fee`
- ⏳ Code needs to be integrated into `contracts/liquidity_pool/src/lib.rs`

### Problem
The `lib.rs` file has duplicate code sections that need cleanup before integration.

### Solution Path

## Option 1: Clean Integration (Recommended - 2-4 hours)

### Step 1: Backup and Clean
```bash
cd contracts/liquidity_pool
git checkout feature/issue-104-amm-deposit-fee
cp src/lib.rs src/lib.rs.backup
```

Review `src/lib.rs` and remove ALL duplicate sections:
- Duplicate imports (lines 1-20)
- Duplicate Error enum entries (lines 30-70)
- Duplicate PoolState definitions (appears 3x)
- Duplicate FeeUpdateScheduledEvent (appears 2x)
- Duplicate DataKey entries
- Duplicate constants (appears 2x around lines 184, 366)

### Step 2: Add LP Fee Code

Open `LP_FEE_IMPLEMENTATION.md` and apply changes in order:

#### A. Add Events (after existing event structs ~line 155)
```rust
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LpDepositFeeEvent {
    pub depositor: Address,
    pub gross_shares: i128,
    pub fee_shares: i128,
    pub net_shares: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LpWithdrawFeeEvent {
    pub withdrawer: Address,
    pub gross_amount_a: i128,
    pub gross_amount_b: i128,
    pub fee_amount_a: i128,
    pub fee_amount_b: i128,
    pub net_amount_a: i128,
    pub net_amount_b: i128,
}
```

#### B. Add Constants (with other constants ~line 366)
```rust
pub const DEFAULT_LP_FEE_BPS: i128 = 5;
pub const MAX_LP_FEE_BPS: i128 = 100;
```

#### C. Update DataKey Enum
Add after the existing keys:
```rust
LpFeeBps,
```

#### D. Update Initialize Function
Add this line in the `initialize` function:
```rust
e.storage().instance().set(&DataKey::LpFeeBps, &DEFAULT_LP_FEE_BPS);
```

#### E. Add Admin Functions
Add these two functions to the `impl LiquidityPool` block:
```rust
pub fn set_lp_fee_bps(e: Env, fee_bps: i128) -> Result<(), Error> {
    let admin = load_admin(&e)?;
    admin.require_auth();
    
    if fee_bps < 0 || fee_bps > MAX_LP_FEE_BPS {
        return Err(Error::InvalidFee);
    }
    
    e.storage().instance().set(&DataKey::LpFeeBps, &fee_bps);
    Ok(())
}

pub fn get_lp_fee_bps(e: Env) -> i128 {
    e.storage()
        .instance()
        .get::<_, i128>(&DataKey::LpFeeBps)
        .unwrap_or(DEFAULT_LP_FEE_BPS)
}
```

#### F. Replace Deposit Function
Find the `deposit` function and replace with the version from `LP_FEE_IMPLEMENTATION.md` (section 6).

Key changes:
- Calculate `gross_shares` first
- Apply fee: `fee_shares = gross_shares * lp_fee_bps / 10000`
- Mint only `net_shares`
- Emit `LpDepositFeeEvent`

#### G. Replace Withdraw Function
Find the `withdraw` function and replace with the version from `LP_FEE_IMPLEMENTATION.md` (section 7).

Key changes:
- Calculate `gross_amount_a` and `gross_amount_b` first
- Apply fees to each token
- Transfer only net amounts
- Emit `LpWithdrawFeeEvent`

### Step 3: Integrate Tests
Add to the test module declarations (around line 24):
```rust
#[cfg(test)]
mod lp_fee_test;
```

### Step 4: Verify
```bash
cargo test
cargo clippy --all-targets
cargo fmt --check
```

Fix any compilation errors. Common issues:
- Missing imports
- Incorrect function signatures
- Event publishing syntax

### Step 5: Commit
```bash
git add src/lib.rs
git commit -m "feature(amm-deposit-fee): Integrate LP fee logic into deposit/withdraw

- Add LpDepositFeeEvent and LpWithdrawFeeEvent
- Add DEFAULT_LP_FEE_BPS (5 bps) and MAX_LP_FEE_BPS (100 bps) constants
- Add LpFeeBps storage key
- Update initialize() to set default LP fee
- Add set_lp_fee_bps() and get_lp_fee_bps() admin functions
- Modify deposit() to deduct fee from minted shares
- Modify withdraw() to deduct fee from withdrawn amounts
- Integrate lp_fee_test module (17 tests)

All tests passing. LP fee mitigates JIT liquidity by imposing 0.10% round-trip cost."
```

## Option 2: Fresh Start (Alternative - 4-6 hours)

If `lib.rs` is too corrupted:

1. Find the last clean version from git history:
```bash
git log --oneline --all -- contracts/liquidity_pool/src/lib.rs
git show <commit-hash>:contracts/liquidity_pool/src/lib.rs > src/lib_clean.rs
```

2. Review `lib_clean.rs` to ensure it's not duplicated

3. Replace `lib.rs` with clean version:
```bash
mv src/lib.rs src/lib_corrupted.rs
mv src/lib_clean.rs src/lib.rs
```

4. Follow Option 1 steps 2-5

## ⚠️ Common Pitfalls

1. **Don't add LP fee event structs twice** - Check if they already exist
2. **Don't duplicate constants** - Only one `DEFAULT_LP_FEE_BPS` declaration
3. **Update both deposit AND withdraw** - Feature requires both sides
4. **Remember to set fee in initialize()** - Default must be configured
5. **Import String correctly** - Use `String::from_str(&e, "lp_fee")`
6. **Test module path** - Ensure `lp_fee_test.rs` is in `src/` directory

## ✅ Validation Checklist

Before creating PR, verify:

- [ ] `cargo test` passes (including 17 new LP fee tests)
- [ ] `cargo clippy` passes with no warnings
- [ ] `cargo fmt --check` passes
- [ ] No duplicate code in lib.rs
- [ ] README.md is in `contracts/liquidity_pool/`
- [ ] Commit messages follow conventional commits
- [ ] Branch is `feature/issue-104-amm-deposit-fee`

## 📋 Quick Test Verification

After integration, run this quick smoke test:

```bash
cd contracts/liquidity_pool

# Run just LP fee tests
cargo test lp_fee

# Should see output like:
# test lp_fee_test::test_default_lp_fee_is_5_bps ... ok
# test lp_fee_test::test_set_lp_fee_bps_by_admin ... ok
# test lp_fee_test::test_deposit_with_lp_fee_deducts_shares ... ok
# ... (17 tests total)

# Run all tests
cargo test

# Lint
cargo clippy --all-targets

# Format check
cargo fmt --check
```

## 🚀 Create Pull Request

Once all validation passes:

1. Push branch:
```bash
git push origin feature/issue-104-amm-deposit-fee
```

2. Create PR with:
   - **Title**: `[Contract] Implement LP mint/burn liquidity fee`
   - **Description**: Reference `LP_FEE_FEATURE_SUMMARY.md` for full details
   - **Link**: Close #104 (or relevant issue number)
   - **Test Coverage**: 17 new tests, all passing
   - **Economic Analysis**: 0.10% round-trip cost mitigates JIT attacks

3. Request reviews from:
   - Smart contract security reviewer
   - Economics/MEV specialist (for JIT attack analysis)
   - Senior Rust/Soroban developer

## 📞 Need Help?

**Reference Documents** (in order of detail):
1. `LP_FEE_IMPLEMENTATION.md` - Complete code implementations
2. `README.md` - Feature documentation and usage
3. `LP_FEE_FEATURE_SUMMARY.md` - High-level overview
4. `IMPLEMENTATION_STATUS.md` - Current status and blockers
5. `lp_fee_test.rs` - Expected behavior through tests

**Stuck on a specific issue?**
- Compilation errors: Check `LP_FEE_IMPLEMENTATION.md` code samples
- Test failures: Review `lp_fee_test.rs` for expected behavior
- Integration questions: See `IMPLEMENTATION_STATUS.md`
- Economic questions: See `README.md` economics section

---

**Estimated Time**: 2-4 hours for experienced Rust/Soroban developer  
**Branch**: `feature/issue-104-amm-deposit-fee`  
**Status**: 80% complete (documentation + tests done, integration remaining)
