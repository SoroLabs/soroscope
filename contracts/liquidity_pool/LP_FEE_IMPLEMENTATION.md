# LP Deposit/Withdrawal Fee Implementation Guide

## Overview
This document describes the implementation of configurable liquidity deposit/withdrawal fees for the liquidity pool contract to mitigate just-in-time (JIT) liquidity attacks.

## Problem Statement
Rapid liquidity deposit/withdrawal cycles allow actors to capture trading fees without incurring inventory risk, diluting returns for long-term LPs.

## Solution
Introduce a configurable LP fee (default 5 bps / 0.05%) charged during LP token minting (deposit) and burning (withdrawal). The fee stays within pool reserves, boosting the underlying value per LP token.

## Implementation Changes

### 1. Event Structures (Add after line 155)

```rust
/// Event emitted when LP deposit fee is charged (on minting)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LpDepositFeeEvent {
    pub depositor: Address,
    pub gross_shares: i128,
    pub fee_shares: i128,
    pub net_shares: i128,
}

/// Event emitted when LP withdrawal fee is charged (on burning)
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

### 2. Constants (Add to constants section around line 366-370)

```rust
/// Default LP fee (5 bps = 0.05%)
pub const DEFAULT_LP_FEE_BPS: i128 = 5;
/// Maximum allowed LP fee (100 bps = 1%)
pub const MAX_LP_FEE_BPS: i128 = 100;
```

### 3. Storage Key (Add to DataKey enum)

Add `LpFeeBps` to the DataKey enum to store the LP fee rate.

### 4. Initialize Method (Modify to set default LP fee)

In the `initialize` function, add:
```rust
e.storage().instance().set(&DataKey::LpFeeBps, &DEFAULT_LP_FEE_BPS);
```

### 5. Admin Method to Configure LP Fee

```rust
/// Set the LP deposit/withdrawal fee rate (in basis points).
/// Only callable by admin. Fee must be between 0 and MAX_LP_FEE_BPS (100 bps = 1%).
pub fn set_lp_fee_bps(e: Env, fee_bps: i128) -> Result<(), Error> {
    let admin = load_admin(&e)?;
    admin.require_auth();
    
    if fee_bps < 0 || fee_bps > MAX_LP_FEE_BPS {
        return Err(Error::InvalidFee);
    }
    
    e.storage().instance().set(&DataKey::LpFeeBps, &fee_bps);
    Ok(())
}

/// Get the current LP deposit/withdrawal fee rate (in basis points).
pub fn get_lp_fee_bps(e: Env) -> i128 {
    e.storage()
        .instance()
        .get::<_, i128>(&DataKey::LpFeeBps)
        .unwrap_or(DEFAULT_LP_FEE_BPS)
}
```

### 6. Modified Deposit Function

The deposit function calculates shares, then deducts the LP fee:

```rust
pub fn deposit(e: Env, to: Address, amount_a: i128, amount_b: i128) -> Result<i128, Error> {
    require_not_paused(&e, PauseType::DEPOSIT)?;
    to.require_auth();

    let mut pool = load_pool(&e)?;
    let client_a = soroban_sdk::token::Client::new(&e, &pool.token_a);
    let client_b = soroban_sdk::token::Client::new(&e, &pool.token_b);
    
    // Transfer tokens to contract
    client_a.transfer(&to, &e.current_contract_address(), &amount_a);
    client_b.transfer(&to, &e.current_contract_address(), &amount_b);

    // Calculate gross shares (before fee)
    let gross_shares = if pool.total_shares == 0 {
        sqrt(amount_a.checked_mul(amount_b).ok_or(Error::InsufficientLiquidity)?)
    } else {
        let share_a = amount_a.checked_mul(pool.total_shares).ok_or(Error::InsufficientLiquidity)? / pool.reserve_a;
        let share_b = amount_b.checked_mul(pool.total_shares).ok_or(Error::InsufficientLiquidity)? / pool.reserve_b;
        if share_a < share_b { share_a } else { share_b }
    };

    // Apply LP deposit fee
    let lp_fee_bps: i128 = e.storage().instance().get(&DataKey::LpFeeBps).unwrap_or(DEFAULT_LP_FEE_BPS);
    let fee_shares = (gross_shares * lp_fee_bps) / 10000;
    let net_shares = gross_shares - fee_shares;

    if net_shares <= 0 {
        return Err(Error::InsufficientLiquidity);
    }

    // Update user balance with net shares (fee stays in pool reserves)
    let user_key = DataKey::Balance(to.clone());
    let current = e.storage().persistent().get::<_, i128>(&user_key).unwrap_or(0);
    e.storage().persistent().set(&user_key, &(current + net_shares));
    e.storage().persistent().extend_ttl(&user_key, 100, 100);

    // Update pool state (note: reserves get full deposit, but only net_shares minted)
    pool.total_shares += net_shares;
    pool.reserve_a += amount_a;
    pool.reserve_b += amount_b;
    save_pool(&e, &pool);

    // Emit LP fee event
    e.events().publish(
        (String::from_str(&e, "lp_fee"), String::from_str(&e, "deposit")),
        LpDepositFeeEvent {
            depositor: to.clone(),
            gross_shares,
            fee_shares,
            net_shares,
        },
    );

    // Emit deposit event
    e.events().publish(
        (String::from_str(&e, "deposit"), to.clone()),
        DepositEvent {
            user: to,
            amount_a,
            amount_b,
            shares_minted: net_shares,
        },
    );

    Ok(net_shares)
}
```

### 7. Modified Withdraw Function

The withdraw function calculates asset payouts, then deducts the LP fee:

```rust
pub fn withdraw(e: Env, to: Address, share_amount: i128) -> Result<(i128, i128), Error> {
    require_not_paused(&e, PauseType::WITHDRAW)?;
    to.require_auth();

    let mut pool = load_pool(&e)?;
    let user_key = DataKey::Balance(to.clone());
    let current = e.storage().persistent().get::<_, i128>(&user_key).unwrap_or(0);
    
    if share_amount > current {
        return Err(Error::InsufficientShares);
    }

    if pool.total_shares <= 0 {
        return Err(Error::InsufficientLiquidity);
    }

    // Calculate gross payouts (before fee)
    let gross_amount_a = share_amount * pool.reserve_a / pool.total_shares;
    let gross_amount_b = share_amount * pool.reserve_b / pool.total_shares;

    // Apply LP withdrawal fee
    let lp_fee_bps: i128 = e.storage().instance().get(&DataKey::LpFeeBps).unwrap_or(DEFAULT_LP_FEE_BPS);
    let fee_amount_a = (gross_amount_a * lp_fee_bps) / 10000;
    let fee_amount_b = (gross_amount_b * lp_fee_bps) / 10000;
    let net_amount_a = gross_amount_a - fee_amount_a;
    let net_amount_b = gross_amount_b - fee_amount_b;

    // Update user balance
    e.storage().persistent().set(&user_key, &(current - share_amount));
    e.storage().persistent().extend_ttl(&user_key, 100, 100);

    // Update pool state (fees stay in reserves, reducing reserve proportionally to net payout)
    pool.total_shares -= share_amount;
    pool.reserve_a -= net_amount_a;
    pool.reserve_b -= net_amount_b;
    let token_a = pool.token_a.clone();
    let token_b = pool.token_b.clone();
    save_pool(&e, &pool);

    // Transfer net amounts to user
    soroban_sdk::token::Client::new(&e, &token_a).transfer(
        &e.current_contract_address(),
        &to,
        &net_amount_a,
    );
    soroban_sdk::token::Client::new(&e, &token_b).transfer(
        &e.current_contract_address(),
        &to,
        &net_amount_b,
    );

    // Emit LP fee event
    e.events().publish(
        (String::from_str(&e, "lp_fee"), String::from_str(&e, "withdraw")),
        LpWithdrawFeeEvent {
            withdrawer: to.clone(),
            gross_amount_a,
            gross_amount_b,
            fee_amount_a,
            fee_amount_b,
            net_amount_a,
            net_amount_b,
        },
    );

    // Emit withdraw event
    e.events().publish(
        (String::from_str(&e, "withdraw"), to.clone()),
        WithdrawEvent {
            user: to,
            shares_burned: share_amount,
            amount_a: net_amount_a,
            amount_b: net_amount_b,
        },
    );

    Ok((net_amount_a, net_amount_b))
}
```

## Testing Requirements

1. **Fee Calculation Accuracy Test**: Verify net_shares and net_payout correctly deduct configured basis points
2. **Reserve Value Compounding Test**: Confirm JIT deposit+immediate withdrawal yields net loss, increasing share value for remaining LPs
3. **Admin Authorization Test**: Test set_lp_fee_bps requires admin auth and rejects invalid fee rates
4. **Boundary Tests**: Test 0 bps (no fee) and MAX_LP_FEE_BPS (100 bps = 1%) edge cases
5. **Event Emission Test**: Verify LpDepositFeeEvent and LpWithdrawFeeEvent are emitted with correct values

## Economics

- **Default Fee**: 5 bps (0.05%) on both deposit and withdrawal
- **Round-trip Cost**: 10 bps (0.10%) for immediate deposit + withdrawal
- **Benefit**: Fees remain in pool reserves, compounding value for long-term LPs
- **Discouragement**: Makes JIT liquidity unprofitable for small fee capture scenarios
