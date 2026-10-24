#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env};

#[cfg(test)]
mod test;

const BPS: i128 = 10_000;
const PRICE_SCALE: i128 = 10_000_000;
const LEDGERS_PER_YEAR: i128 = 6_307_200;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InvalidAmount = 3,
    InvalidConfig = 4,
    PositionNotFound = 5,
    PositionHealthy = 6,
    Undercollateralized = 7,
    InsufficientCollateral = 8,
    InsufficientStableBalance = 9,
    OracleNotConfigured = 10,
    InvalidOraclePrice = 11,
    LiquidationUnavailable = 12,
    MathOverflow = 13,
    VolatilityNotAvailable = 14,
    VolatilityTooHigh = 15,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Position {
    pub collateral_amount: i128,
    pub debt_amount: i128,
    pub last_accrual_ledger: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiskParams {
    pub min_collateral_ratio_bps: i128,
    pub liquidation_incentive_bps: i128,
    pub volatility_config: VolatilityConfig,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterestRateModel {
    pub base_rate_bps: i128,
    pub slope1_bps: i128,
    pub slope2_bps: i128,
    pub optimal_utilization_bps: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiquidationQuote {
    pub repay_amount: i128,
    pub collateral_to_seize: i128,
    pub incentive_amount: i128,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    CollateralToken,
    Oracle,
    TotalCollateral,
    TotalDebt,
    TotalStableSupply,
    ProtocolCollateralReserves,
    TotalBadDebt,
    RiskParams,
    InterestRateModel,
    VolatilityConfig,
    Position(Address),
    StableBalance(Address),
    /// Stability fee rate in basis points per 1,000 ledgers (bpm).
    /// E.g. 5 means 0.05 % per 1,000 ledgers.  Zero means no stability fee.
    StabilityFeeRate,
}

#[soroban_sdk::contractclient(name = "PriceOracleClient")]
pub trait PriceOracle {
    fn latest_price(e: Env) -> i128;
    fn volatility_bps(e: Env) -> i128;
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VolatilityConfig {
    pub low_volatility_threshold_bps: i128,
    pub high_volatility_threshold_bps: i128,
    pub base_collateral_floor_bps: i128,
    pub max_collateral_floor_bps: i128,
}

/// Parameters passed to `initialize`.  Grouping avoids the 10-argument limit
/// imposed by the Soroban contract ABI.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitParams {
    pub min_collateral_ratio_bps: i128,
    pub liquidation_incentive_bps: i128,
    pub interest_rate_model: InterestRateModel,
    pub volatility_config: VolatilityConfig,
}

fn checked_add(a: i128, b: i128) -> Result<i128, Error> {
    a.checked_add(b).ok_or(Error::MathOverflow)
}

fn checked_sub(a: i128, b: i128) -> Result<i128, Error> {
    a.checked_sub(b).ok_or(Error::MathOverflow)
}

fn checked_mul(a: i128, b: i128) -> Result<i128, Error> {
    a.checked_mul(b).ok_or(Error::MathOverflow)
}

fn require_admin(env: &Env) -> Result<Address, Error> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(Error::NotInitialized)?;
    admin.require_auth();
    Ok(admin)
}

fn read_risk_params(env: &Env) -> Result<RiskParams, Error> {
    env.storage()
        .instance()
        .get(&DataKey::RiskParams)
        .ok_or(Error::NotInitialized)
}

fn read_interest_model(env: &Env) -> Result<InterestRateModel, Error> {
    env.storage()
        .instance()
        .get(&DataKey::InterestRateModel)
        .ok_or(Error::NotInitialized)
}

fn read_position(env: &Env, user: Address) -> Position {
    env.storage().instance().extend_ttl(5_000_000, 5_000_000);
    env.storage()
        .persistent()
        .get(&DataKey::Position(user))
        .unwrap_or(Position {
            collateral_amount: 0,
            debt_amount: 0,
            last_accrual_ledger: env.ledger().sequence(),
        })
}

fn write_position(env: &Env, user: Address, position: &Position) {
    let key = DataKey::Position(user);
    env.storage().persistent().set(&key, position);
    env.storage().persistent().extend_ttl(&key, 5_000_000, 5_000_000);
}

fn read_stable_balance(env: &Env, user: Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::StableBalance(user))
        .unwrap_or(0)
}

fn write_stable_balance(env: &Env, user: Address, amount: i128) {
    let key = DataKey::StableBalance(user);
    env.storage().persistent().set(&key, &amount);
    env.storage().persistent().extend_ttl(&key, 5_000_000, 5_000_000);
}

fn oracle_price(env: &Env) -> Result<i128, Error> {
    let oracle: Address = env
        .storage()
        .instance()
        .get(&DataKey::Oracle)
        .ok_or(Error::OracleNotConfigured)?;
    let price = PriceOracleClient::new(env, &oracle).latest_price();
    if price <= 0 {
        return Err(Error::InvalidOraclePrice);
    }
    Ok(price)
}

fn oracle_volatility_bps(env: &Env) -> Result<i128, Error> {
    let oracle: Address = env
        .storage()
        .instance()
        .get(&DataKey::Oracle)
        .ok_or(Error::OracleNotConfigured)?;
    let volatility = PriceOracleClient::new(env, &oracle).volatility_bps();
    if volatility < 0 {
        return Err(Error::VolatilityNotAvailable);
    }
    Ok(volatility)
}

fn collateral_value(price: i128, collateral_amount: i128) -> Result<i128, Error> {
    Ok(checked_mul(collateral_amount, price)? / PRICE_SCALE)
}

fn collateral_ratio_bps(
    price: i128,
    collateral_amount: i128,
    debt_amount: i128,
) -> Result<i128, Error> {
    if debt_amount <= 0 {
        return Ok(i128::MAX);
    }

    let numerator = checked_mul(collateral_amount, price)?;
    let scaled_numerator = checked_mul(numerator, BPS)?;
    let denominator = checked_mul(debt_amount, PRICE_SCALE)?;
    let ratio = scaled_numerator / denominator;
    let remainder = scaled_numerator % denominator;
    let remainder_twice = remainder.checked_mul(2).ok_or(Error::MathOverflow)?;

    Ok(if remainder_twice >= denominator {
        checked_add(ratio, 1)?
    } else {
        ratio
    })
}

fn total_debt(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::TotalDebt)
        .unwrap_or(0)
}

fn total_collateral(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::TotalCollateral)
        .unwrap_or(0)
}

fn utilization_bps(env: &Env, price: i128) -> Result<i128, Error> {
    let collateral_value_total = collateral_value(price, total_collateral(env))?;
    if collateral_value_total <= 0 {
        return Ok(0);
    }
    let util = checked_mul(total_debt(env), BPS)? / collateral_value_total;
    Ok(if util > BPS { BPS } else { util })
}

fn borrow_rate_bps(model: &InterestRateModel, utilization: i128) -> Result<i128, Error> {
    if utilization <= model.optimal_utilization_bps {
        let variable = checked_mul(utilization, model.slope1_bps)? / model.optimal_utilization_bps;
        checked_add(model.base_rate_bps, variable)
    } else {
        let base = checked_add(model.base_rate_bps, model.slope1_bps)?;
        let excess_util = utilization - model.optimal_utilization_bps;
        let tail =
            checked_mul(excess_util, model.slope2_bps)? / (BPS - model.optimal_utilization_bps);
        checked_add(base, tail)
    }
}

fn accrue_position(env: &Env, user: Address) -> Result<Position, Error> {
    // Keep the contract instance alive for a generous period on every access.
    env.storage().instance().extend_ttl(5_000_000, 5_000_000);

    let mut position = read_position(env, user.clone());
    let now = env.ledger().sequence();
    if position.debt_amount <= 0 || position.last_accrual_ledger >= now {
        position.last_accrual_ledger = now;
        write_position(env, user, &position);
        return Ok(position);
    }

    let elapsed = (now - position.last_accrual_ledger) as i128;
    let model = read_interest_model(env)?;
    let price = oracle_price(env)?;
    let utilization = utilization_bps(env, price)?;
    let rate_bps = borrow_rate_bps(&model, utilization)?;

    let interest = checked_mul(position.debt_amount, rate_bps)?
        .checked_mul(elapsed)
        .ok_or(Error::MathOverflow)?
        / BPS
        / LEDGERS_PER_YEAR;

    if interest > 0 {
        position.debt_amount = checked_add(position.debt_amount, interest)?;
        let new_total_debt = checked_add(total_debt(env), interest)?;
        env.storage()
            .instance()
            .set(&DataKey::TotalDebt, &new_total_debt);
        let new_supply = checked_add(
            env.storage()
                .instance()
                .get(&DataKey::TotalStableSupply)
                .unwrap_or(0),
            interest,
        )?;
        env.storage()
            .instance()
            .set(&DataKey::TotalStableSupply, &new_supply);
    }

    // Apply stability fee on top of (already-updated) debt.
    position = apply_stability_fee(env, position)?;

    position.last_accrual_ledger = now;
    write_position(env, user, &position);
    Ok(position)
}

/// Reads the configured stability fee rate (bpm).  Returns 0 if not set.
fn read_stability_fee_rate(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::StabilityFeeRate)
        .unwrap_or(0)
}

/// Computes the stability fee that would accrue on `position` up to `now`
/// without mutating any state.  Returns the fee amount (may be 0).
fn compute_stability_fee(env: &Env, position: &Position) -> Result<i128, Error> {
    let rate_bpm = read_stability_fee_rate(env);
    if rate_bpm <= 0 || position.debt_amount <= 0 {
        return Ok(0);
    }
    let now = env.ledger().sequence();
    if position.last_accrual_ledger >= now {
        return Ok(0);
    }
    let elapsed = (now - position.last_accrual_ledger) as i128;
    // fee = debt * rate_bpm * elapsed / (BPS * 1_000)
    // Using integer arithmetic; division last to preserve precision.
    position
        .debt_amount
        .checked_mul(rate_bpm)
        .and_then(|v| v.checked_mul(elapsed))
        .map(|v| v / BPS / 1_000)
        .ok_or(Error::MathOverflow)
}

/// Accrues the stability fee onto `position.debt_amount` and updates global
/// TotalDebt / TotalStableSupply.  Returns the updated position (not written
/// to storage — callers are responsible for persisting it).
fn apply_stability_fee(env: &Env, mut position: Position) -> Result<Position, Error> {
    let fee = compute_stability_fee(env, &position)?;
    if fee > 0 {
        position.debt_amount = checked_add(position.debt_amount, fee)?;
        let new_total_debt = checked_add(total_debt(env), fee)?;
        env.storage()
            .instance()
            .set(&DataKey::TotalDebt, &new_total_debt);
        let new_supply: i128 = checked_add(
            env.storage()
                .instance()
                .get(&DataKey::TotalStableSupply)
                .unwrap_or(0),
            fee,
        )?;
        env.storage()
            .instance()
            .set(&DataKey::TotalStableSupply, &new_supply);
    }
    Ok(position)
}

fn quote_liquidation_for_position(
    env: &Env,
    position: &Position,
    requested_repay_amount: i128,
) -> Result<LiquidationQuote, Error> {
    if requested_repay_amount <= 0 {
        return Err(Error::InvalidAmount);
    }

    let params = read_risk_params(env)?;
    let price = oracle_price(env)?;
    let ratio = collateral_ratio_bps(price, position.collateral_amount, position.debt_amount)?;
    if ratio >= params.min_collateral_ratio_bps {
        return Err(Error::PositionHealthy);
    }

    let collateral_value_total = collateral_value(price, position.collateral_amount)?;
    let max_coverable_repay =
        checked_mul(collateral_value_total, BPS)? / (BPS + params.liquidation_incentive_bps);
    let repay_amount = requested_repay_amount
        .min(position.debt_amount)
        .min(max_coverable_repay);
    if repay_amount <= 0 {
        return Err(Error::LiquidationUnavailable);
    }

    let seize_value = checked_mul(repay_amount, BPS + params.liquidation_incentive_bps)? / BPS;
    let mut collateral_to_seize = checked_mul(seize_value, PRICE_SCALE)? / price;
    if collateral_to_seize > position.collateral_amount {
        collateral_to_seize = position.collateral_amount;
    }
    let incentive_amount = checked_sub(seize_value, repay_amount)?;

    Ok(LiquidationQuote {
        repay_amount,
        collateral_to_seize,
        incentive_amount,
    })
}

fn dynamic_collateral_floor(env: &Env) -> Result<i128, Error> {
    let params = read_risk_params(env)?;
    let volatility = oracle_volatility_bps(env)?;
    let config = &params.volatility_config;

    // If volatility is below low threshold, use base floor
    if volatility <= config.low_volatility_threshold_bps {
        return Ok(config.base_collateral_floor_bps);
    }

    // If volatility is above high threshold, use max floor
    if volatility >= config.high_volatility_threshold_bps {
        return Ok(config.max_collateral_floor_bps);
    }

    // Linear interpolation between base and max floor
    let volatility_range =
        config.high_volatility_threshold_bps - config.low_volatility_threshold_bps;
    let volatility_above_low = volatility - config.low_volatility_threshold_bps;
    let floor_range = config.max_collateral_floor_bps - config.base_collateral_floor_bps;

    let additional_floor = checked_mul(floor_range, volatility_above_low)? / volatility_range;
    checked_add(config.base_collateral_floor_bps, additional_floor)
}

fn ensure_safe(env: &Env, position: &Position) -> Result<(), Error> {
    let price = oracle_price(env)?;
    let params = read_risk_params(env)?;
    let dynamic_floor = dynamic_collateral_floor(env)?;
    let ratio = collateral_ratio_bps(price, position.collateral_amount, position.debt_amount)?;

    // Use the higher of static min ratio or dynamic floor
    let required_ratio = if params.min_collateral_ratio_bps > dynamic_floor {
        params.min_collateral_ratio_bps
    } else {
        dynamic_floor
    };

    if ratio < required_ratio {
        return Err(Error::Undercollateralized);
    }
    Ok(())
}

#[contract]
pub struct CdpContract;

#[contractimpl]
impl CdpContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        collateral_token: Address,
        oracle: Address,
        params: InitParams,
    ) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        let min_collateral_ratio_bps = params.min_collateral_ratio_bps;
        let liquidation_incentive_bps = params.liquidation_incentive_bps;
        let model = &params.interest_rate_model;
        let vc = &params.volatility_config;
        if min_collateral_ratio_bps <= BPS
            || liquidation_incentive_bps < 0
            || model.optimal_utilization_bps <= 0
            || model.optimal_utilization_bps >= BPS
            || vc.low_volatility_threshold_bps < 0
            || vc.high_volatility_threshold_bps <= vc.low_volatility_threshold_bps
            || vc.base_collateral_floor_bps < BPS
            || vc.max_collateral_floor_bps < vc.base_collateral_floor_bps
            || vc.max_collateral_floor_bps > BPS * 3
        {
            return Err(Error::InvalidConfig);
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::CollateralToken, &collateral_token);
        env.storage().instance().set(&DataKey::Oracle, &oracle);
        env.storage().instance().set(
            &DataKey::RiskParams,
            &RiskParams {
                min_collateral_ratio_bps,
                liquidation_incentive_bps,
                volatility_config: params.volatility_config,
            },
        );
        env.storage()
            .instance()
            .set(&DataKey::InterestRateModel, &params.interest_rate_model);
        env.storage()
            .instance()
            .set(&DataKey::TotalCollateral, &0i128);
        env.storage().instance().set(&DataKey::TotalDebt, &0i128);
        env.storage()
            .instance()
            .set(&DataKey::TotalStableSupply, &0i128);
        env.storage()
            .instance()
            .set(&DataKey::ProtocolCollateralReserves, &0i128);
        env.storage().instance().set(&DataKey::TotalBadDebt, &0i128);
        Ok(())
    }

    pub fn set_oracle(env: Env, oracle: Address) -> Result<(), Error> {
        require_admin(&env)?;
        env.storage().instance().set(&DataKey::Oracle, &oracle);
        Ok(())
    }

    /// Admin: set the per-position stability fee rate in basis points per 1,000 ledgers.
    /// Pass 0 to disable the stability fee.
    pub fn set_stability_fee_rate(env: Env, rate_bpm: i128) -> Result<(), Error> {
        require_admin(&env)?;
        if rate_bpm < 0 {
            return Err(Error::InvalidConfig);
        }
        env.storage()
            .instance()
            .set(&DataKey::StabilityFeeRate, &rate_bpm);
        Ok(())
    }

    /// Explicitly accrue outstanding stability fees (and borrow-rate interest) on
    /// a CDP position.  Can be called by anyone; useful for keepers.
    pub fn accrue_fees(env: Env, cdp_id: Address) -> Result<Position, Error> {
        accrue_position(&env, cdp_id)
    }

    /// Read-only preview of the stability fee that would accrue on `cdp_id`
    /// up to the current ledger without modifying state.  Returns 0 if the
    /// position has no debt or the fee rate is zero.
    pub fn stability_fee_preview(env: Env, cdp_id: Address) -> i128 {
        let position = read_position(&env, cdp_id);
        compute_stability_fee(&env, &position).expect("stability fee preview calculation overflow")
    }

    pub fn get_position(env: Env, user: Address) -> Position {
        read_position(&env, user)
    }

    pub fn stable_balance(env: Env, user: Address) -> i128 {
        read_stable_balance(&env, user)
    }

    pub fn total_debt(env: Env) -> i128 {
        total_debt(&env)
    }

    pub fn total_stable_supply(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::TotalStableSupply)
            .unwrap_or(0)
    }

    pub fn protocol_collateral_reserves(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::ProtocolCollateralReserves)
            .unwrap_or(0)
    }

    pub fn total_bad_debt(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::TotalBadDebt)
            .unwrap_or(0)
    }

    pub fn current_borrow_rate_bps(env: Env) -> Result<i128, Error> {
        let model = read_interest_model(&env)?;
        let price = oracle_price(&env)?;
        let util = utilization_bps(&env, price)?;
        borrow_rate_bps(&model, util)
    }

    pub fn collateral_ratio_bps(env: Env, user: Address) -> Result<i128, Error> {
        let position = read_position(&env, user);
        let price = oracle_price(&env)?;
        collateral_ratio_bps(price, position.collateral_amount, position.debt_amount)
    }

    pub fn dynamic_collateral_floor(env: Env) -> Result<i128, Error> {
        dynamic_collateral_floor(&env)
    }

    pub fn current_volatility_bps(env: Env) -> Result<i128, Error> {
        oracle_volatility_bps(&env)
    }

    pub fn deposit_collateral(env: Env, user: Address, amount: i128) -> Result<Position, Error> {
        user.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let collateral_token: Address = env
            .storage()
            .instance()
            .get(&DataKey::CollateralToken)
            .ok_or(Error::NotInitialized)?;
        let mut position = accrue_position(&env, user.clone())?;

        soroban_sdk::token::Client::new(&env, &collateral_token).transfer(
            &user,
            &env.current_contract_address(),
            &amount,
        );

        position.collateral_amount = checked_add(position.collateral_amount, amount)?;
        write_position(&env, user, &position);
        env.storage().instance().set(
            &DataKey::TotalCollateral,
            &checked_add(total_collateral(&env), amount)?,
        );
        Ok(position)
    }

    pub fn withdraw_collateral(env: Env, user: Address, amount: i128) -> Result<Position, Error> {
        user.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let collateral_token: Address = env
            .storage()
            .instance()
            .get(&DataKey::CollateralToken)
            .ok_or(Error::NotInitialized)?;
        let mut position = accrue_position(&env, user.clone())?;
        if amount > position.collateral_amount {
            return Err(Error::InsufficientCollateral);
        }

        position.collateral_amount = checked_sub(position.collateral_amount, amount)?;
        ensure_safe(&env, &position)?;

        write_position(&env, user.clone(), &position);
        env.storage().instance().set(
            &DataKey::TotalCollateral,
            &checked_sub(total_collateral(&env), amount)?,
        );
        soroban_sdk::token::Client::new(&env, &collateral_token).transfer(
            &env.current_contract_address(),
            &user,
            &amount,
        );
        Ok(position)
    }

    pub fn mint_stable(env: Env, user: Address, amount: i128) -> Result<Position, Error> {
        user.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let mut position = accrue_position(&env, user.clone())?;
        position.debt_amount = checked_add(position.debt_amount, amount)?;
        ensure_safe(&env, &position)?;

        let new_balance = checked_add(read_stable_balance(&env, user.clone()), amount)?;
        write_stable_balance(&env, user.clone(), new_balance);
        write_position(&env, user, &position);
        env.storage()
            .instance()
            .set(&DataKey::TotalDebt, &checked_add(total_debt(&env), amount)?);
        let total_supply = checked_add(Self::total_stable_supply(env.clone()), amount)?;
        env.storage()
            .instance()
            .set(&DataKey::TotalStableSupply, &total_supply);
        Ok(position)
    }

    pub fn repay(env: Env, user: Address, amount: i128) -> Result<Position, Error> {
        user.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let mut position = accrue_position(&env, user.clone())?;
        let balance = read_stable_balance(&env, user.clone());
        if balance <= 0 {
            return Err(Error::InsufficientStableBalance);
        }

        let repay_amount = if amount > balance { balance } else { amount };
        let repay_amount = if repay_amount > position.debt_amount {
            position.debt_amount
        } else {
            repay_amount
        };
        if repay_amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        position.debt_amount = checked_sub(position.debt_amount, repay_amount)?;
        write_stable_balance(&env, user.clone(), checked_sub(balance, repay_amount)?);
        write_position(&env, user, &position);
        env.storage().instance().set(
            &DataKey::TotalDebt,
            &checked_sub(total_debt(&env), repay_amount)?,
        );
        env.storage().instance().set(
            &DataKey::TotalStableSupply,
            &checked_sub(Self::total_stable_supply(env.clone()), repay_amount)?,
        );
        Ok(position)
    }

    pub fn transfer_stable(
        env: Env,
        from: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), Error> {
        from.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let from_balance = read_stable_balance(&env, from.clone());
        if from_balance < amount {
            return Err(Error::InsufficientStableBalance);
        }
        write_stable_balance(&env, from, checked_sub(from_balance, amount)?);
        write_stable_balance(
            &env,
            to.clone(),
            checked_add(read_stable_balance(&env, to), amount)?,
        );
        Ok(())
    }

    pub fn quote_liquidation(
        env: Env,
        borrower: Address,
        requested_repay_amount: i128,
    ) -> Result<LiquidationQuote, Error> {
        let position = accrue_position(&env, borrower)?;
        quote_liquidation_for_position(&env, &position, requested_repay_amount)
    }

    pub fn liquidate(
        env: Env,
        liquidator: Address,
        borrower: Address,
        requested_repay_amount: i128,
    ) -> Result<LiquidationQuote, Error> {
        liquidator.require_auth();
        let mut position = accrue_position(&env, borrower.clone())?;
        let quote = quote_liquidation_for_position(&env, &position, requested_repay_amount)?;
        let liquidator_balance = read_stable_balance(&env, liquidator.clone());
        if liquidator_balance < quote.repay_amount {
            return Err(Error::InsufficientStableBalance);
        }

        let collateral_token: Address = env
            .storage()
            .instance()
            .get(&DataKey::CollateralToken)
            .ok_or(Error::NotInitialized)?;
        position.debt_amount = checked_sub(position.debt_amount, quote.repay_amount)?;
        position.collateral_amount =
            checked_sub(position.collateral_amount, quote.collateral_to_seize)?;

        write_stable_balance(
            &env,
            liquidator.clone(),
            checked_sub(liquidator_balance, quote.repay_amount)?,
        );
        write_position(&env, borrower, &position);
        env.storage().instance().set(
            &DataKey::TotalDebt,
            &checked_sub(total_debt(&env), quote.repay_amount)?,
        );
        env.storage().instance().set(
            &DataKey::TotalStableSupply,
            &checked_sub(Self::total_stable_supply(env.clone()), quote.repay_amount)?,
        );
        env.storage().instance().set(
            &DataKey::TotalCollateral,
            &checked_sub(total_collateral(&env), quote.collateral_to_seize)?,
        );

        soroban_sdk::token::Client::new(&env, &collateral_token).transfer(
            &env.current_contract_address(),
            &liquidator,
            &quote.collateral_to_seize,
        );

        Ok(quote)
    }

    pub fn self_liquidate(env: Env, borrower: Address) -> Result<Position, Error> {
        let mut position = accrue_position(&env, borrower.clone())?;
        let params = read_risk_params(&env)?;
        let price = oracle_price(&env)?;
        let ratio = collateral_ratio_bps(price, position.collateral_amount, position.debt_amount)?;
        if ratio >= params.min_collateral_ratio_bps {
            return Err(Error::PositionHealthy);
        }

        let collateral_value_total = collateral_value(price, position.collateral_amount)?;
        let repay_coverable =
            checked_mul(collateral_value_total, BPS)? / (BPS + params.liquidation_incentive_bps);
        let bad_debt = if position.debt_amount > repay_coverable {
            position.debt_amount - repay_coverable
        } else {
            0
        };
        let debt_repaid = position.debt_amount - bad_debt;
        let seized_collateral = position.collateral_amount;

        env.storage().instance().set(
            &DataKey::ProtocolCollateralReserves,
            &checked_add(
                Self::protocol_collateral_reserves(env.clone()),
                seized_collateral,
            )?,
        );
        env.storage().instance().set(
            &DataKey::TotalBadDebt,
            &checked_add(Self::total_bad_debt(env.clone()), bad_debt)?,
        );
        env.storage().instance().set(
            &DataKey::TotalDebt,
            &checked_sub(total_debt(&env), debt_repaid)?,
        );
        env.storage().instance().set(
            &DataKey::TotalCollateral,
            &checked_sub(total_collateral(&env), seized_collateral)?,
        );

        position.collateral_amount = 0;
        position.debt_amount = 0;
        position.last_accrual_ledger = env.ledger().sequence();
        write_position(&env, borrower, &position);
        Ok(position)
    }
}
