# LP Deposit/Withdrawal Fee Implementation Status

## Branch
`feature/issue-104-amm-deposit-fee`

## Completed Work

### ✅ Documentation
- **README.md**: Comprehensive documentation of LP fee feature, economics, and usage
- **LP_FEE_IMPLEMENTATION.md**: Detailed implementation guide with code examples
- Both files committed to the feature branch

### ✅ Test Suite
- **lp_fee_test.rs**: Complete test suite with 17 comprehensive tests covering:
  - Default LP fee verification (5 bps)
  - Admin control and authorization
  - Fee boundary testing (0 bps to MAX 100 bps)
  - Deposit fee deduction accuracy
  - Withdrawal fee deduction accuracy
  - JIT liquidity penalty verification
  - Share value compounding for long-term LPs
  - Event emission verification
  - Multiple deposit scenarios
  - Zero fee and maximum fee edge cases

### ✅ Design Decisions
1. **Fee Structure**: Basis points (bps) where 10,000 bps = 100%
2. **Default Fee**: 5 bps (0.05%) on both deposit and withdrawal
3. **Maximum Fee**: 100 bps (1%) to prevent excessive extraction
4. **Fee Retention**: Fees stay in pool reserves (not extracted), compounding value for remaining LPs
5. **Admin Control**: Only authorized admin can configure LP fee rate
6. **Storage**: Single `LpFeeBps` key stores the fee rate
7. **Events**: Separate events for deposit fees and withdrawal fees with full transparency

## Remaining Work

### ⚠️ Code Cleanup Required

The `contracts/liquidity_pool/src/lib.rs` file has significant code duplication issues that need to be resolved before adding the LP fee logic. The file contains:
- Duplicate struct definitions (PoolState, FeeUpdateScheduledEvent, etc.)
- Duplicate constant declarations
- Duplicate import statements
- Inconsistent DataKey enum definitions

**Recommended Approach**:
1. Create a clean backup of the working logic
2. Remove all duplicate code sections
3. Consolidate into a single clean implementation
4. Apply the LP fee changes from `LP_FEE_IMPLEMENTATION.md`

### 📝 Implementation Tasks

Once the file is cleaned up, apply these changes (from LP_FEE_IMPLEMENTATION.md):

#### 1. Add Event Structures
Add `LpDepositFeeEvent` and `LpWithdrawFeeEvent` after the existing event definitions.

#### 2. Add Constants
```rust
pub const DEFAULT_LP_FEE_BPS: i128 = 5;   // 0.05%
pub const MAX_LP_FEE_BPS: i128 = 100;     // 1%
```

#### 3. Update DataKey Enum
Add `LpFeeBps` to store the LP fee rate.

#### 4. Update Initialize Function
Set default LP fee during initialization:
```rust
e.storage().instance().set(&DataKey::LpFeeBps, &DEFAULT_LP_FEE_BPS);
```

#### 5. Add Admin Functions
```rust
pub fn set_lp_fee_bps(e: Env, fee_bps: i128) -> Result<(), Error>
pub fn get_lp_fee_bps(e: Env) -> i128
```

#### 6. Modify Deposit Function
Apply fee deduction logic:
- Calculate gross_shares
- Deduct fee_shares = gross_shares * lp_fee_bps / 10000
- Mint only net_shares to user
- Emit LpDepositFeeEvent

#### 7. Modify Withdraw Function
Apply fee deduction logic:
- Calculate gross_amount_a and gross_amount_b
- Deduct fees from each token
- Transfer only net amounts to user
- Retain fees in pool reserves
- Emit LpWithdrawFeeEvent

#### 8. Integrate Test Suite
Add `mod lp_fee_test;` to the test module declarations.

## Testing & Verification

### Unit Tests
```bash
cd contracts/liquidity_pool
cargo test
```

Expected: All 17 LP fee tests should pass.

### Linting
```bash
cargo clippy --all-targets
cargo fmt --check
```

### Integration Verification
1. Deploy contract to testnet
2. Initialize pool with default LP fee
3. Test deposit with fee deduction
4. Test withdrawal with fee deduction
5. Verify JIT liquidity is unprofitable
6. Verify long-term LP share value increases

## Economics Summary

### JIT Liquidity Attack Mitigation

**Without LP Fee**:
- Attacker deposits right before large trade
- Captures proportional trading fees
- Withdraws immediately after
- Net profit with minimal risk

**With LP Fee (5 bps)**:
- Deposit: 1,000 tokens → 999.5 shares (0.5 fee)
- Immediate withdrawal: 999.5 shares → 999 tokens (0.5 fee)
- Round-trip cost: ~1 token (0.1%)
- For this to be profitable, trading fees captured must exceed 0.1%

**Threshold Analysis**:
- 5 bps LP fee = 0.05% each way = 0.10% round-trip
- Trading fee = 30 bps = 0.30%
- For JIT to break even, attacker needs to capture > 33% of the trading fees
- This requires providing > 33% of liquidity at time of trade
- Much harder to profit on small/medium trades

### Long-term LP Benefit

Each JIT cycle that fails to profit leaves fees in the pool:
- Deposit fee → Fewer shares minted, reserves unchanged → Higher value per share
- Withdrawal fee → Reserves partially retained → Higher value per remaining share

Over time, these retained fees compound, providing passive yield to patient LPs.

## Pull Request Checklist

When ready to create PR:
- [ ] lib.rs cleaned up and LP fee code integrated
- [ ] All tests passing (cargo test)
- [ ] Linting passing (cargo clippy, cargo fmt)
- [ ] README.md included
- [ ] LP_FEE_IMPLEMENTATION.md included as reference
- [ ] Test suite (lp_fee_test.rs) included
- [ ] Conventional commit messages used
- [ ] PR title: `[Contract] Implement LP mint/burn liquidity fee`
- [ ] PR description includes:
  - Problem statement (JIT liquidity)
  - Solution overview (5 bps fee)
  - Testing summary
  - Economic impact analysis

## Notes

### File Structure Issue
The main blocker is the duplicate code in `lib.rs`. This appears to be from merge conflicts or file corruption. The file structure needs to be cleaned before the LP fee changes can be safely applied.

### Alternative Approach
If cleaning up `lib.rs` proves too complex:
1. Identify the most recent working version from git history
2. Checkout that version
3. Apply LP fee changes cleanly
4. Re-merge any other necessary features carefully

### Contact
For questions or assistance with the cleanup/integration, refer to:
- LP_FEE_IMPLEMENTATION.md for implementation details
- README.md for feature documentation
- lp_fee_test.rs for expected behavior
