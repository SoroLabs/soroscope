# Liquidity Pool Contract

## Overview

A constant product (x*y=k) AMM with LP share tokens, emergency pause controls, dynamic fee adjustment based on volatility, and **LP deposit/withdrawal fees** to mitigate just-in-time (JIT) liquidity attacks.

## Key Features

### 1. Constant Product AMM
- Automated market maker using the x*y=k formula
- LP shares represent proportional ownership of the pool
- Supports token swaps with configurable trading fees

### 2. LP Deposit/Withdrawal Fees (NEW)

#### Problem Statement
Rapid liquidity deposit/withdrawal cycles allow actors to capture trading fees without incurring inventory risk or protocol friction, diluting returns for long-term liquidity providers.

#### Solution
Configurable LP fees (default 5 bps / 0.05%) charged during:
- **Deposit**: Fee deducted from minted LP shares
- **Withdrawal**: Fee deducted from withdrawn token amounts

The fees stay within the pool reserves, inherently boosting the underlying value per LP token and rewarding long-term liquidity providers.

#### Configuration

**Default Fee**: 5 bps (0.05%) on both deposit and withdrawal

**Storage Key**: `LpFeeBps` stores the fee rate in basis points

**Admin Functions**:
```rust
// Set LP fee rate (0-100 bps, where 10,000 bps = 100%)
pub fn set_lp_fee_bps(e: Env, fee_bps: i128) -> Result<(), Error>

// Get current LP fee rate
pub fn get_lp_fee_bps(e: Env) -> i128
```

**Bounds**:
- Minimum: 0 bps (no fee)
- Maximum: 100 bps (1%)
- Default: 5 bps (0.05%)

`DataKey::Admin` is the pool fee admin (may differ from guard admins after rotation). Use `set_fee`, `configure_fee_oracle`, `sync_fee_from_oracle`, and `execute_fee_update`.

## Swapping and slippage protection

The pool exposes both swap directions. Which one you use decides which side you
can bound, and every swap needs one side bounded — an unbounded swap can be
sandwiched, since an attacker who front-runs the transaction moves the price so
the same trade fills far worse.

| Function | You fix | You bound | Returns |
|----------|---------|-----------|---------|
| `swap(to, buy_a, out, in_max)` | the output `out` | the input, via `in_max` | input actually paid |
| `swap_exact_in(to, buy_a, amount_in, min_amount_out)` | the input `amount_in` | the output, via `min_amount_out` | output actually delivered |

`buy_a` selects direction: `true` sends token B in and takes token A out, `false`
is the reverse.

### Exact-input swaps

`swap_exact_in` computes the output from the live reserves and current fee, then
refuses the trade unless it clears the caller's floor:

```rust
// Quote against current state, then accept 1% of drift.
let quoted = pool.get_amount_out(false, 1_000)?;
let min_amount_out = quoted * 99 / 100;

let received = pool.swap_exact_in(trader, false, 1_000, min_amount_out)?;
assert!(received >= min_amount_out);
```

If the price moves between the quote and execution such that the output would
fall below `min_amount_out`, the call returns `Error::SlippageExceeded` and no
tokens move.

Passing `min_amount_out = 0` disables the check. Do that only when any fill is
genuinely acceptable.

### Quoting

let out = pool.get_amount_out(buy_a, amount_in)?;  // output for a given input
let needed = pool.get_amount_in(buy_a, amount_out)?; // input for a given output
let (reserve_a, reserve_b) = pool.get_reserves()?;

Both quotes are read-only and only describe the state they were read from. Derive
a bound from them; do not assume the swap will match them. Rounding always
resolves in the pool's favour, so `get_amount_in(get_amount_out(x))` may come
back slightly above `x`.

### Exact-output swaps

For `swap`, the output is fixed by the caller and delivered exactly or not at
all, so `in_max` is the meaningful bound — a minimum-output parameter would be
satisfied by construction. Set `in_max` to the quoted input plus your tolerance:

let quoted_in = pool.get_amount_in(false, 900)?;
pool.swap(trader, false, 900, quoted_in * 101 / 100)?;

### Swap errors

- `Error::SlippageExceeded`: the bound you set was not met (`min_amount_out` for
  `swap_exact_in`, `in_max` for `swap`). Expected under normal price movement;
  re-quote and retry.
- `Error::InvalidAmount`: `amount_in` was zero or negative, or `min_amount_out`
  was negative.
- `Error::InsufficientLiquidity`: the reserves cannot support the trade, or the
  input was too small to buy a single unit of output.
- `Error::Paused`: swaps are currently paused by the guard.
#### Deposit Flow with LP Fee

1. User deposits `amount_a` and `amount_b` tokens
2. Contract calculates `gross_shares` using AMM formula
3. LP fee is deducted: `fee_shares = gross_shares * lp_fee_bps / 10000`
4. User receives `net_shares = gross_shares - fee_shares`
5. Full deposit stays in reserves, but only `net_shares` are minted
6. Result: Reserve value per share increases for existing LPs

**Example** (5 bps fee):
- Deposit: 1,000 tokenA + 1,000 tokenB
- Gross shares: 1,000 (sqrt(1000 * 1000))
- Fee: 1,000 * 5 / 10,000 = 0.5 shares
- Net shares minted: 999.5 shares

#### Withdrawal Flow with LP Fee

1. User burns `share_amount` LP shares
2. Contract calculates `gross_amount_a` and `gross_amount_b` proportionally
3. LP fee is deducted from each token:
   - `fee_amount_a = gross_amount_a * lp_fee_bps / 10000`
   - `fee_amount_b = gross_amount_b * lp_fee_bps / 10000`
4. User receives `net_amount_a` and `net_amount_b`
5. Fees remain in reserves, increasing value for remaining LP shares

- Burn: 1,000 shares (1% of 100,000 total)
- Gross payout: 1,000 tokenA + 1,000 tokenB
- Fee: 1,000 * 5 / 10,000 = 0.5 of each token
- Net payout: 999.5 tokenA + 999.5 tokenB

#### Economic Impact

**JIT Liquidity Penalty**:
- Round-trip cost: ~10 bps (0.10%) with 5 bps fee
- Deposit + immediate withdrawal = net loss
- Makes JIT liquidity unprofitable for small fee capture

**Long-term LP Benefit**:
- Fees compound in reserves over time
- Each JIT cycle increases reserve value per share
- Long-term LPs earn passive yield from JIT penalties

#### Events

**LP Deposit Fee Event**:
pub struct LpDepositFeeEvent {
    pub depositor: Address,
    pub gross_shares: i128,
    pub fee_shares: i128,
    pub net_shares: i128,
}
Topic: `("lp_fee", "deposit")`

**LP Withdrawal Fee Event**:
pub struct LpWithdrawFeeEvent {
    pub withdrawer: Address,
    pub gross_amount_a: i128,
    pub gross_amount_b: i128,
    pub fee_amount_a: i128,
    pub fee_amount_b: i128,
    pub net_amount_a: i128,
    pub net_amount_b: i128,
Topic: `("lp_fee", "withdraw")`

### 3. Emergency Pause Controls
- Granular pause controls for deposit, withdrawal, swap, transfer operations
- Multi-signature admin support
- Emergency pause all functionality

### 4. Dynamic Trading Fees
- Base trading fee configurable by admin
- Optional oracle-based dynamic fee adjustment
- Volatility-based fee tiers

### 5. ERC-20 Compatible LP Tokens
- `transfer`, `approve`, `transferFrom` for LP shares
- Standard token interface (name, symbol, decimals, balance, allowance)

## Core Functions

### Initialization
pub fn initialize(e: Env, admin: Address, token_a: Address, token_b: Address) -> Result<(), Error>

### Liquidity Management
// Deposit tokens, receive LP shares (minus LP fee)
pub fn deposit(e: Env, to: Address, amount_a: i128, amount_b: i128) -> Result<i128, Error>

// Burn LP shares, receive tokens (minus LP fee)
pub fn withdraw(e: Env, to: Address, share_amount: i128) -> Result<(i128, i128), Error>

### Trading
// Swap tokens with slippage protection
pub fn swap(e: Env, to: Address, buy_a: bool, out: i128, in_max: i128) -> Result<i128, Error>

### Admin Functions
// Set trading fee (swap fee)
pub fn set_fee(e: Env, fee_bps: i128) -> Result<(), Error>

// Set LP deposit/withdrawal fee
pub fn set_lp_fee_bps(e: Env, fee_bps: i128) -> Result<(), Error>

// Configure fee oracle for dynamic adjustment
pub fn configure_fee_oracle(e: Env, oracle: Address, base_fee_bps: i128, timelock_ledgers: u32) -> Result<(), Error>

// Emergency controls
pub fn guard_pause(e: Env, admin: Address, operation: u32, paused: bool) -> Result<(), Error>
pub fn emergency_pause(e: Env, approvers: Vec<Address>) -> Result<(), Error>
pub fn resume(e: Env, approvers: Vec<Address>) -> Result<(), Error>

## Storage Keys

| Key | Type | Description |
|-----|------|-------------|
| `Pool` | PoolState | Main pool state (tokens, reserves, shares, fees, admin) |
| `Balance(Address)` | i128 | Per-user LP share balance |
| `Allowance(AllowanceDataKey)` | AllowanceValue | ERC-20 allowances |
| `LpFeeBps` | i128 | LP deposit/withdrawal fee rate (basis points) |
| `Admin` | Address | Primary admin address |
| `Guard` | GuardState | Emergency pause state and admin list |
| `Oracle` | OracleConfig | Dynamic fee oracle configuration |
| `PendingFeeUpdate` | PendingFeeUpdate | Timelocked fee update |

## Constants

// Trading fees
pub const MAX_FEE_BPS: i128 = 100;              // 1% max trading fee
pub const DEFAULT_BASE_FEE_BPS: i128 = 30;      // 0.3% default trading fee

// LP fees
pub const DEFAULT_LP_FEE_BPS: i128 = 5;         // 0.05% default LP fee
pub const MAX_LP_FEE_BPS: i128 = 100;           // 1% max LP fee

## Error Codes

| Code | Name | Description |
|------|------|-------------|
| 1 | AlreadyInitialized | Contract already initialized |
| 2 | InsufficientLiquidity | Not enough liquidity in pool |
| 3 | SlippageExceeded | Swap slippage tolerance exceeded |
| 4 | InsufficientShares | User doesn't have enough LP shares |
| 5 | NotInitialized | Contract not initialized |
| 6 | InsufficientBalance | Insufficient token balance |
| 7 | Unauthorized | Caller not authorized |
| 8 | InsufficientAllowance | Insufficient token allowance |
| 9 | InvalidFee | Fee rate out of bounds |
| 10 | OracleNotConfigured | Fee oracle not configured |
| 11 | InvalidOraclePrice | Invalid price from oracle |
| 12 | TimelockNotElapsed | Timelock period not elapsed |
| 13 | NoPendingFeeUpdate | No pending fee update exists |
| 14 | Paused | Operation is paused |

## Testing

Run comprehensive test suite:
```bash
cargo test --package liquidity_pool

Key test scenarios:
- ✅ LP fee calculation accuracy
- ✅ JIT liquidity round-trip cost
- ✅ Share value compounding for long-term LPs
- ✅ Admin authorization and bounds checking
- ✅ Event emission verification
- ✅ Zero fee edge case
- ✅ Maximum fee edge case

## Security Considerations

1. **Admin Controls**: LP fee can only be set by authorized admin
2. **Fee Bounds**: LP fee capped at 100 bps (1%) to prevent excessive extraction
3. **Arithmetic Safety**: All fee calculations use checked arithmetic
4. **Emergency Pause**: Admin can pause deposits/withdrawals in case of issues
5. **Event Transparency**: All fee deductions are logged via events

## Deployment

1. Deploy contract
2. Initialize with admin and token addresses
3. (Optional) Configure LP fee rate via `set_lp_fee_bps`
4. (Optional) Configure trading fee oracle for dynamic adjustment
5. (Optional) Add additional guard admins for multi-sig emergency controls

## Upgrade Considerations

The LP fee feature is backward compatible:
- Existing deployments without LP fee will use 0 bps (no fee) as default until explicitly configured
- No breaking changes to existing function signatures
- New storage key (`LpFeeBps`) does not conflict with existing keys

## References

- [Uniswap V3 LP Fee Discussion](https://gov.uniswap.org/t/uni-should-become-an-oracle-token/11988)
- [JIT Liquidity Problem](https://www.paradigm.xyz/2021/06/uniswap-v3-the-universal-amm#jit-liquidity)
- [Stellar Soroban Documentation](https://soroban.stellar.org/)
