#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, String, Vec};

use emergency_guard::{DefaultEmergencyGuard, EmergencyGuard, EmergencyGuardTrait, PauseType};
pub use soroscope_error_codes::ContractError;
use soroscope_math::Fixed;

pub const SCALE: i128 = 1_000_000_000_000_000_000; // 18 decimals

// ── Storage Keys ──────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Config,
    UserState(Address),
    TotalStaked,
    EpochSnapshot(u32),
    PenaltyFeeBps,
}

// ── Configuration Struct ──────────────────────────────────────

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct StakingConfig {
    pub owner: Address,
    pub staking_token: Address,
    pub reward_token: Address,
    pub initial_rate: Fixed,           // r0 — emission rate at epoch 0
    pub epoch_decay_percent: Fixed,    // percentage reduction per epoch (e.g. 0.1 = 10%)
    pub epoch_length: u32,             // blocks per epoch
    pub start_block: u32,
    pub is_paused: bool,
}

// ── Epoch Snapshot ────────────────────────────────────────────

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct EpochSnapshot {
    pub rate: Fixed,
    pub start_block: u32,
}

// ── User Staking State ────────────────────────────────────────

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct UserStakingState {
    pub staked_amount: i128,
    pub accrued_rewards: i128,
    pub last_update_block: u32,
}

// ── Event Structs ─────────────────────────────────────────────

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct StakeEvent {
    pub user: Address,
    pub amount: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct WithdrawEvent {
    pub user: Address,
    pub amount: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ClaimEvent {
    pub user: Address,
    pub amount: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct EmergencyWithdrawEvent {
    pub user: Address,
    pub amount: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PausedEvent {
    pub paused: bool,
}

// ── Helper Math Functions ─────────────────────────────────────

fn fixed_pow_int(base: Fixed, mut exp: u32) -> Result<Fixed, ContractError> {
    let mut temp = base;
    let mut ans = Fixed::ONE;
    while exp > 0 {
        if exp & 1 == 1 {
            ans = ans.mul(temp).map_err(|_| ContractError::Overflow)?;
        }
        temp = temp.mul(temp).map_err(|_| ContractError::Overflow)?;
        exp >>= 1;
    }
    Ok(ans)
}

fn mul_div(a: i128, b: i128, d: i128) -> Option<i128> {
    if d == 0 {
        return None;
    }
    let a_abs = a.unsigned_abs();
    let b_abs = b.unsigned_abs();
    let d_abs = d.unsigned_abs();

    let (res_abs, overflow) = mul_div_u128(a_abs, b_abs, d_abs);
    if overflow || res_abs > (i128::MAX as u128) {
        return None;
    }

    let res = res_abs as i128;
    if (a < 0) ^ (b < 0) ^ (d < 0) {
        Some(-res)
    } else {
        Some(res)
    }
}

fn mul_div_u128(a: u128, b: u128, d: u128) -> (u128, bool) {
    if let Some(prod) = a.checked_mul(b) {
        return (prod / d, false);
    }
    let a_low = a & 0xFFFFFFFFFFFFFFFF;
    let a_high = a >> 64;
    let b_low = b & 0xFFFFFFFFFFFFFFFF;
    let b_high = b >> 64;
    let p0 = a_low * b_low;
    let p1 = a_low * b_high;
    let p2 = a_high * b_low;
    let p3 = a_high * b_high;
    let mid = (p1 & 0xFFFFFFFFFFFFFFFF) + (p2 & 0xFFFFFFFFFFFFFFFF) + (p0 >> 64);
    let high = p3 + (p1 >> 64) + (p2 >> 64) + (mid >> 64);
    let low = (mid << 64) | (p0 & 0xFFFFFFFFFFFFFFFF);
    if high >= d {
        return (0, true);
    }
    let mut quotient = 0u128;
    let mut remainder = high;
    for i in (0..128).rev() {
        remainder = (remainder << 1) | ((low >> i) & 1);
        if remainder >= d {
            remainder -= d;
            quotient |= 1 << i;
        }
    }
    (quotient, false)
}

fn multiply_amount(amount: i128, multiplier: Fixed) -> Result<i128, ContractError> {
    mul_div(amount, multiplier.0, SCALE).ok_or(ContractError::Overflow)
}

// ── Epoch Helpers ─────────────────────────────────────────────

fn epoch_for_block(start_block: u32, epoch_length: u32, block: u32) -> u32 {
    if block <= start_block {
        return 0;
    }
    (block - start_block) / epoch_length
}

fn epoch_start_block(start_block: u32, epoch_length: u32, epoch: u32) -> u32 {
    start_block + epoch * epoch_length
}

fn compute_epoch_rate(
    initial_rate: &Fixed,
    epoch_decay_percent: &Fixed,
    epoch: u32,
) -> Result<Fixed, ContractError> {
    if epoch == 0 {
        return Ok(*initial_rate);
    }
    // rate = initial_rate * (1 - epoch_decay_percent)^epoch
    let base = Fixed::ONE
        .sub(*epoch_decay_percent)
        .map_err(|_| ContractError::Overflow)?;
    if base.0 < 0 || base.0 > SCALE {
        return Err(ContractError::InvalidInput);
    }
    let decay_factor = fixed_pow_int(base, epoch)?;
    initial_rate.mul(decay_factor).map_err(|_| ContractError::Overflow)
}

fn ensure_epoch_snapshots(
    e: &Env,
    config: &StakingConfig,
    up_to_block: u32,
) -> Result<(), ContractError> {
    let up_to_epoch = epoch_for_block(config.start_block, config.epoch_length, up_to_block);
    // Find the highest epoch already snapshotted
    let mut epoch = 0u32;
    while epoch <= up_to_epoch {
        let key = DataKey::EpochSnapshot(epoch);
        if !e.storage().instance().has(&key) {
            let rate = compute_epoch_rate(&config.initial_rate, &config.epoch_decay_percent, epoch)?;
            let start = epoch_start_block(config.start_block, config.epoch_length, epoch);
            e.storage().instance().set(&key, &EpochSnapshot { rate, start_block: start });
        }
        epoch += 1;
    }
    Ok(())
}

fn get_epoch_rate(e: &Env, epoch: u32) -> Result<Fixed, ContractError> {
    let key = DataKey::EpochSnapshot(epoch);
    let snapshot: EpochSnapshot = e
        .storage()
        .instance()
        .get(&key)
        .ok_or(ContractError::NotInitialized)?;
    Ok(snapshot.rate)
}

// ── Compounding Multiplier Calculation ────────────────────────

fn calculate_multiplier(
    e: &Env,
    config: &StakingConfig,
    t1: u32,
    t2: u32,
) -> Result<Fixed, ContractError> {
    if t2 <= t1 {
        return Ok(Fixed::ONE);
    }

    let t_start = config.start_block;
    let t1_eff = t1.max(t_start);
    let t2_eff = t2.max(t_start);

    if t2_eff <= t1_eff {
        return Ok(Fixed::ONE);
    }

    ensure_epoch_snapshots(e, config, t2_eff)?;

    let e1 = epoch_for_block(t_start, config.epoch_length, t1_eff);
    let e2 = epoch_for_block(t_start, config.epoch_length, t2_eff);

    let mut mult = Fixed::ONE;

    for epoch in e1..=e2 {
        let rate = get_epoch_rate(e, epoch)?;
        let epoch_start = epoch_start_block(t_start, config.epoch_length, epoch);
        let epoch_end = epoch_start + config.epoch_length;
        let overlap_start = t1_eff.max(epoch_start);
        let overlap_end = t2_eff.min(epoch_end);

        if overlap_end > overlap_start {
            let blocks = (overlap_end - overlap_start) as i128;
            let blocks_fixed = Fixed::from_int(blocks).map_err(|_| ContractError::Overflow)?;
            let exponent = rate.mul(blocks_fixed).map_err(|_| ContractError::Overflow)?;
            let factor = exponent.exp().map_err(|_| ContractError::Overflow)?;
            mult = mult.mul(factor).map_err(|_| ContractError::Overflow)?;
        }
    }

    Ok(mult)
}

// ── Contract Implementation ───────────────────────────────────

#[contract]
pub struct StakingRewards;

#[contractimpl]
impl StakingRewards {
    /// Initializes the staking rewards contract with the config.
    pub fn initialize(
        e: Env,
        owner: Address,
        staking_token: Address,
        reward_token: Address,
        initial_rate: i128,
        epoch_decay_percent: i128,
        epoch_length: u32,
        start_block: u32,
    ) -> Result<(), ContractError> {
        if e.storage().instance().has(&DataKey::Config) {
            return Err(ContractError::AlreadyInitialized);
        }

        if epoch_decay_percent < 0 || epoch_decay_percent > SCALE {
            return Err(ContractError::InvalidInput);
        }

        if epoch_length == 0 {
            return Err(ContractError::InvalidInput);
        }

        if initial_rate < 0 {
            return Err(ContractError::InvalidInput);
        }

        let config = StakingConfig {
            owner: owner.clone(),
            staking_token,
            reward_token,
            initial_rate: Fixed(initial_rate),
            epoch_decay_percent: Fixed(epoch_decay_percent),
            epoch_length,
            start_block,
            is_paused: false,
        };

        // Create epoch 0 snapshot
        let rate0 = compute_epoch_rate(&config.initial_rate, &config.epoch_decay_percent, 0)?;
        e.storage().instance().set(
            &DataKey::EpochSnapshot(0),
            &EpochSnapshot { rate: rate0, start_block },
        );

        e.storage().instance().set(&DataKey::Config, &config);
        e.storage().instance().set(&DataKey::TotalStaked, &0i128);
        e.storage().instance().extend_ttl(10000, 10000);

        // Initialize the embedded EmergencyGuard so granular pause checks
        // (e.g. PauseType::CLAIM_REWARDS) can be toggled by the owner.
        // Threshold of 1 means the single owner can trigger any pause.
        let admins = soroban_sdk::vec![&e, config.owner.clone()];
        EmergencyGuard::initialize(e, admins, 1).map_err(|_| ContractError::AlreadyInitialized)?;

        Ok(())
    }

    /// Stakes primary tokens in the contract.
    pub fn stake(e: Env, user: Address, amount: i128) -> Result<(), ContractError> {
        // Check if staking is paused using granular pause control
        DefaultEmergencyGuard::check_not_paused(&e, PauseType::STAKE)
            .map_err(|_| ContractError::Paused)?;

        if amount <= 0 {
            return Err(ContractError::InvalidInput);
        }

        user.require_auth();

        let config = Self::get_config(e.clone())?;
        let mut state = Self::update_user_rewards_internal(&e, &config, &user)?;

        // Transfer staking tokens from user to contract
        token::Client::new(&e, &config.staking_token).transfer(
            &user,
            &e.current_contract_address(),
            &amount,
        );

        state.staked_amount = state
            .staked_amount
            .checked_add(amount)
            .ok_or(ContractError::Overflow)?;

        // Update total staked
        let mut total_staked: i128 = e.storage().instance().get(&DataKey::TotalStaked).unwrap_or(0);
        total_staked = total_staked
            .checked_add(amount)
            .ok_or(ContractError::Overflow)?;
        e.storage().instance().set(&DataKey::TotalStaked, &total_staked);

        e.storage()
            .persistent()
            .set(&DataKey::UserState(user.clone()), &state);
        e.storage()
            .persistent()
            .extend_ttl(&DataKey::UserState(user.clone()), 10000, 10000);
        e.storage().instance().extend_ttl(10000, 10000);

        e.events().publish(
            (String::from_str(&e, "stake"), user.clone()),
            StakeEvent { user, amount },
        );

        Ok(())
    }

    /// Withdraws staked principal tokens.
    pub fn withdraw(e: Env, user: Address, amount: i128) -> Result<(), ContractError> {
        // Check if staking is paused using granular pause control
        DefaultEmergencyGuard::check_not_paused(&e, PauseType::STAKE)
            .map_err(|_| ContractError::Paused)?;

        if amount <= 0 {
            return Err(ContractError::InvalidInput);
        }

        user.require_auth();

        let config = Self::get_config(e.clone())?;
        let mut state = Self::update_user_rewards_internal(&e, &config, &user)?;

        if state.staked_amount < amount {
            return Err(ContractError::InsufficientBalance);
        }

        state.staked_amount = state
            .staked_amount
            .checked_sub(amount)
            .ok_or(ContractError::Overflow)?;

        // Update total staked
        let mut total_staked: i128 = e.storage().instance().get(&DataKey::TotalStaked).unwrap_or(0);
        total_staked = total_staked
            .checked_sub(amount)
            .ok_or(ContractError::Overflow)?;
        e.storage().instance().set(&DataKey::TotalStaked, &total_staked);

        if state.staked_amount == 0 && state.accrued_rewards == 0 {
            e.storage()
                .persistent()
                .remove(&DataKey::UserState(user.clone()));
        } else {
            e.storage()
                .persistent()
                .set(&DataKey::UserState(user.clone()), &state);
            e.storage()
                .persistent()
                .extend_ttl(&DataKey::UserState(user.clone()), 10000, 10000);
        }
        e.storage().instance().extend_ttl(10000, 10000);

        // Transfer staking tokens back to user
        token::Client::new(&e, &config.staking_token).transfer(
            &e.current_contract_address(),
            &user,
            &amount,
        );

        e.events().publish(
            (String::from_str(&e, "withdraw"), user.clone()),
            WithdrawEvent { user, amount },
        );

        Ok(())
    }

    /// Claims accrued rewards.
    pub fn claim(e: Env, user: Address) -> Result<i128, ContractError> {
        // Check if staking is paused using granular pause control
        DefaultEmergencyGuard::check_not_paused(&e, PauseType::STAKE)
            .map_err(|_| ContractError::Paused)?;

        if EmergencyGuard::is_paused(e.clone(), PauseType::CLAIM_REWARDS) {
            return Err(ContractError::Paused);
        }

        user.require_auth();

        let config = Self::get_config(e.clone())?;
        let mut state = Self::update_user_rewards_internal(&e, &config, &user)?;
        let reward_amount = state.accrued_rewards;

        if reward_amount <= 0 {
            return Ok(0);
        }

        state.accrued_rewards = 0;

        if state.staked_amount == 0 {
            e.storage()
                .persistent()
                .remove(&DataKey::UserState(user.clone()));
        } else {
            e.storage()
                .persistent()
                .set(&DataKey::UserState(user.clone()), &state);
            e.storage()
                .persistent()
                .extend_ttl(&DataKey::UserState(user.clone()), 10000, 10000);
        }
        e.storage().instance().extend_ttl(10000, 10000);

        // Transfer reward tokens to user
        token::Client::new(&e, &config.reward_token).transfer(
            &e.current_contract_address(),
            &user,
            &reward_amount,
        );

        e.events().publish(
            (String::from_str(&e, "claim"), user.clone()),
            ClaimEvent {
                user,
                amount: reward_amount,
            },
        );

        Ok(reward_amount)
    }

    /// Emergency withdraw: pulls all principal stakings and forfeits all rewards.
    /// Operates even when paused or if the reward token pool is completely dry.
    pub fn emergency_withdraw(e: Env, user: Address) -> Result<i128, ContractError> {
        user.require_auth();

        let config = Self::get_config(e.clone())?;
        let state_key = DataKey::UserState(user.clone());

        if !e.storage().persistent().has(&state_key) {
            return Ok(0);
        }

        let state: UserStakingState = e.storage().persistent().get(&state_key).unwrap();
        let staked_amount = state.staked_amount;

        // Update total staked
        let mut total_staked: i128 = e.storage().instance().get(&DataKey::TotalStaked).unwrap_or(0);
        total_staked = total_staked
            .checked_sub(staked_amount)
            .ok_or(ContractError::Overflow)?;
        e.storage()
            .instance()
            .set(&DataKey::TotalStaked, &total_staked);

        if staked_amount <= 0 {
            return Ok(0);
        }

        // Calculate emergency unbonding penalty fee if configured
        let penalty_fee_bps: u32 = e.storage().instance().get(&DataKey::PenaltyFeeBps).unwrap_or(0);
        let penalty = (staked_amount as u128 * penalty_fee_bps as u128 / 10000) as i128;
        let payout = staked_amount - penalty;

        // Wipe user state entirely (forfeiting rewards)
        e.storage().persistent().remove(&state_key);
        e.storage().instance().extend_ttl(10000, 10000);

        // Transfer payout of staking tokens back to user
        if payout > 0 {
            token::Client::new(&e, &config.staking_token).transfer(
                &e.current_contract_address(),
                &user,
                &payout,
            );
        }

        e.events().publish(
            (String::from_str(&e, "emergency_withdraw"), user.clone()),
            EmergencyWithdrawEvent {
                user,
                amount: payout,
            },
        );

        Ok(payout)
    }

    /// Sets emergency unbonding penalty fee in basis points (owner only, max 10000 = 100%).
    pub fn set_penalty_fee(e: Env, fee_bps: u32) -> Result<(), ContractError> {
        let config = Self::get_config(e.clone())?;
        config.owner.require_auth();

        if fee_bps > 10000 {
            return Err(ContractError::InvalidInput);
        }

        e.storage().instance().set(&DataKey::PenaltyFeeBps, &fee_bps);
        Ok(())
    }

    /// Gets emergency unbonding penalty fee in basis points.
    pub fn get_penalty_fee(e: Env) -> u32 {
        e.storage().instance().get(&DataKey::PenaltyFeeBps).unwrap_or(0)
    }

    /// Pause staking operations (admin only).
    pub fn pause_staking(e: Env) -> Result<(), ContractError> {
        let config = Self::get_config(e.clone())?;
        config.owner.require_auth();

        DefaultEmergencyGuard::set_pause_state(&e, PauseType::STAKE, true)
            .map_err(|_| ContractError::Paused)?;

        e.events().publish(
            (String::from_str(&e, "pause_staking"),),
            PausedEvent { paused: true },
        );

        Ok(())
    }

    /// Resume staking operations (admin only).
    pub fn resume_staking(e: Env) -> Result<(), ContractError> {
        let config = Self::get_config(e.clone())?;
        config.owner.require_auth();

        DefaultEmergencyGuard::set_pause_state(&e, PauseType::STAKE, false)
            .map_err(|_| ContractError::Paused)?;

        e.events().publish(
            (String::from_str(&e, "resume_staking"),),
            PausedEvent { paused: false },
        );

        Ok(())
    }

    /// Emergency pause all operations (requires multi-sig approval).
    pub fn emergency_pause_all(e: Env, approvers: Vec<Address>) -> Result<(), ContractError> {
        DefaultEmergencyGuard::emergency_pause_all(&e, approvers)
            .map_err(|_| ContractError::Paused)?;

        e.events().publish(
            (String::from_str(&e, "emergency_pause_all"),),
            PausedEvent { paused: true },
        );

        Ok(())
    }

    /// Resume all paused operations (requires multi-sig approval).
    pub fn resume_all(e: Env, approvers: Vec<Address>) -> Result<(), ContractError> {
        DefaultEmergencyGuard::resume_all(&e, approvers).map_err(|_| ContractError::Paused)?;

        e.events().publish(
            (String::from_str(&e, "resume_all"),),
            PausedEvent { paused: false },
        );

        Ok(())
    }

    /// Get current pause state.
    pub fn get_pause_state(e: Env) -> u32 {
        DefaultEmergencyGuard::get_pause_state(&e)
    }

    /// Check if staking is paused.
    pub fn is_staking_paused(e: Env) -> bool {
        let state = DefaultEmergencyGuard::get_pause_state(&e);
        let pause_type = PauseType::new(state);
        pause_type.is_paused(PauseType::STAKE)
    }

    /// Get list of admins.
    pub fn get_admins(e: Env) -> Vec<Address> {
        DefaultEmergencyGuard::get_admins(&e)
    }

    /// Get multi-sig threshold.
    pub fn get_threshold(e: Env) -> u32 {
        DefaultEmergencyGuard::get_threshold(&e)
    }

    /// Add new admin (multi-sig required).
    pub fn add_admin(
        e: Env,
        approvers: Vec<Address>,
        new_admin: Address,
    ) -> Result<(), ContractError> {
        DefaultEmergencyGuard::add_admin(&e, approvers, new_admin)
            .map_err(|_| ContractError::Paused)
    }

    /// Remove admin (multi-sig required).
    pub fn remove_admin(
        e: Env,
        approvers: Vec<Address>,
        admin: Address,
    ) -> Result<(), ContractError> {
        DefaultEmergencyGuard::remove_admin(&e, approvers, admin).map_err(|_| ContractError::Paused)
    }

    /// Rotate admin (multi-sig required).
    pub fn rotate_admin(
        e: Env,
        approvers: Vec<Address>,
        old_admin: Address,
        new_admin: Address,
    ) -> Result<(), ContractError> {
        DefaultEmergencyGuard::rotate_admin(&e, approvers, old_admin, new_admin)
            .map_err(|_| ContractError::Paused)
    }

    /// Granularly pause or unpause the claim_rewards operation (owner only).
    /// This is independent of the global `is_paused` flag and uses the
    /// embedded EmergencyGuard bitmask (PauseType::CLAIM_REWARDS).
    pub fn set_claim_rewards_paused(e: Env, paused: bool) -> Result<(), ContractError> {
        let config = Self::get_config(e.clone())?;
        // `EmergencyGuard::set_pause` performs the ownership auth check itself,
        // so we pass the owner through directly to avoid double-auth failures
        // when the same signer is reused within the same transaction.
        EmergencyGuard::set_pause(e, config.owner, PauseType::CLAIM_REWARDS, paused)
            .map_err(|_| ContractError::Paused)
    }

    // ── View Functions ──────────────────────────────────────────

    /// Returns the staked principal balance of the user.
    pub fn get_staked_balance(e: Env, user: Address) -> i128 {
        let state_key = DataKey::UserState(user);
        if e.storage().persistent().has(&state_key) {
            let state: UserStakingState = e.storage().persistent().get(&state_key).unwrap();
            state.staked_amount
        } else {
            0
        }
    }

    /// Returns the accrued rewards saved during the last update.
    pub fn get_accrued_rewards(e: Env, user: Address) -> i128 {
        let state_key = DataKey::UserState(user);
        if e.storage().persistent().has(&state_key) {
            let state: UserStakingState = e.storage().persistent().get(&state_key).unwrap();
            state.accrued_rewards
        } else {
            0
        }
    }

    /// Returns the real-time pending rewards (accrued + interest accumulated since last update).
    pub fn get_pending_rewards(e: Env, user: Address) -> i128 {
        let config_res = Self::get_config(e.clone());
        if config_res.is_err() {
            return 0;
        }
        let config = config_res.unwrap();
        let state_key = DataKey::UserState(user);

        if !e.storage().persistent().has(&state_key) {
            return 0;
        }

        let state: UserStakingState = e.storage().persistent().get(&state_key).unwrap();
        let t_curr = e.ledger().sequence();

        if state.staked_amount > 0 && t_curr > state.last_update_block {
            // Time-based reward calculation: V_new = V_old * multiplier, where
            // multiplier = exp(integral of reward rate over time). Rewards are
            // computed as R_new = V_new - staked_amount to avoid rounding errors.
            let multiplier_res = calculate_multiplier(&e, &config, state.last_update_block, t_curr);
            if let Ok(multiplier) = multiplier_res {
                let v_old_res = state.staked_amount.checked_add(state.accrued_rewards);
                if let Some(v_old) = v_old_res {
                    let v_new_res = multiply_amount(v_old, multiplier);
                    if let Ok(v_new) = v_new_res {
                        let r_new_res = v_new.checked_sub(state.staked_amount);
                        if let Some(r_new) = r_new_res {
                            return r_new;
                        }
                    }
                }
            }
        }

        state.accrued_rewards
    }

    /// Returns the contract's configuration.
    pub fn get_config(e: Env) -> Result<StakingConfig, ContractError> {
        e.storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(ContractError::NotInitialized)
    }

    // ── Internal Helpers ────────────────────────────────────────

    fn update_user_rewards_internal(
        e: &Env,
        config: &StakingConfig,
        user: &Address,
    ) -> Result<UserStakingState, ContractError> {
        let state_key = DataKey::UserState(user.clone());
        let mut state = if e.storage().persistent().has(&state_key) {
            e.storage().persistent().get(&state_key).unwrap()
        } else {
            UserStakingState {
                staked_amount: 0,
                accrued_rewards: 0,
                last_update_block: e.ledger().sequence().max(config.start_block),
            }
        };

        let t_curr = e.ledger().sequence();

        if state.staked_amount > 0 && t_curr > state.last_update_block {
            // Time-based reward calculation: V_new = V_old * multiplier, where
            // multiplier = exp(integral of reward rate over time). Rewards are
            // computed as R_new = V_new - staked_amount to avoid rounding errors.
            let multiplier = calculate_multiplier(e, config, state.last_update_block, t_curr)?;

            // Virtual Balance V = S + R
            let v_old = state
                .staked_amount
                .checked_add(state.accrued_rewards)
                .ok_or(ContractError::Overflow)?;

            // V_new = v_old * multiplier
            let v_new = multiply_amount(v_old, multiplier)?;

            // R_new = V_new - S
            let r_new = v_new
                .checked_sub(state.staked_amount)
                .ok_or(ContractError::Overflow)?;

            state.accrued_rewards = r_new;
        }

        state.last_update_block = t_curr.max(config.start_block);
        Ok(state)
    }
}
mod test;
