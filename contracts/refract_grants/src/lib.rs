#![no_std]
//! # RefractGrants — Velocity-Guarded Grant Disbursement Contract
//!
//! A grants contract with a two-step disbursement model:
//!
//! 1. **Approve** — the admin pre-approves a maximum grant amount for a
//!    recipient.  This is the program-level budget/cap.
//! 2. **Payout** — the admin (or the recipient themselves, if the grant is
//!    approved) draws down the approved amount.  Each payout is gated by the
//!   [`payout_velocity`] guard, enforcing per-recipient per-epoch caps,
//!   minimum spacing, and a rolling-window limit.
//!
//! This complements the program-level budget cap by limiting *how fast* funds
//! leave the contract to any single address, not merely *how much* the program
//! is allowed to spend in total.

use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, Symbol, Vec};

use emergency_guard::{DefaultEmergencyGuard, EmergencyGuard, EmergencyGuardTrait};
use payout_velocity::{PayoutVelocity, VelocityConfig, VelocityError};
pub use soroscope_error_codes::ContractError;

// ── Pause bits ────────────────────────────────────────────────────────────────
const PAUSE_PAYOUT: u32 = 1 << 10;

const TTL_LEDGERS: u32 = 10_000;

// ── Storage types ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct GrantsConfig {
    pub admin: Address,
    pub payout_token: Address,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Config,
    /// recipient → approved grant amount remaining.
    ApprovedGrant(Address),
    /// recipient → total originally approved (for reporting).
    TotalApproved(Address),
}

// ── Event structs ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct FundEvent {
    pub funder: Address,
    pub amount: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ApproveGrantEvent {
    pub recipient: Address,
    pub amount: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PayoutEvent {
    pub recipient: Address,
    pub amount: i128,
}

// ── Error mapping ─────────────────────────────────────────────────────────────

fn map_velocity_err(err: VelocityError) -> ContractError {
    match err {
        VelocityError::EpochCapExceeded
        | VelocityError::MinSpacingNotMet
        | VelocityError::RollingWindowCapExceeded => ContractError::RateLimited,
        VelocityError::InvalidAmount => ContractError::InvalidAmount,
        VelocityError::InvalidConfig => ContractError::InvalidInput,
        VelocityError::NotInitialized => ContractError::NotInitialized,
    }
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct RefractGrants;

#[contractimpl]
impl RefractGrants {
    /// Initialise the grants contract.
    pub fn initialize(
        e: Env,
        admin: Address,
        payout_token: Address,
        velocity_config: VelocityConfig,
    ) -> Result<(), ContractError> {
        if e.storage().instance().has(&DataKey::Config) {
            return Err(ContractError::AlreadyInitialized);
        }

        let config = GrantsConfig {
            admin: admin.clone(),
            payout_token,
        };
        e.storage().instance().set(&DataKey::Config, &config);

        PayoutVelocity::initialize(&e, velocity_config).map_err(map_velocity_err)?;

        EmergencyGuard::initialize(e.clone(), Vec::from_array(&e, [admin]), 1)
            .map_err(|_| ContractError::AlreadyInitialized)?;

        e.storage().instance().extend_ttl(TTL_LEDGERS, TTL_LEDGERS);
        Ok(())
    }

    // ── Funding ───────────────────────────────────────────────────────────────

    /// Deposit payout tokens into the grants pool (admin only).
    pub fn fund(e: Env, funder: Address, amount: i128) -> Result<(), ContractError> {
        if amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }
        funder.require_auth();

        let config = Self::get_config(e.clone())?;

        token::Client::new(&e, &config.payout_token).transfer(
            &funder,
            &e.current_contract_address(),
            &amount,
        );

        e.storage().instance().extend_ttl(TTL_LEDGERS, TTL_LEDGERS);

        e.events().publish(
            (Symbol::new(&e, "fund"), funder.clone()),
            FundEvent { funder, amount },
        );

        Ok(())
    }

    // ── Grant approval ────────────────────────────────────────────────────────

    /// Pre-approve a grant of `amount` for `recipient` (admin only).
    /// This is the program-level budget cap; velocity limits still apply at
    /// payout time.
    pub fn approve_grant(
        e: Env,
        admin: Address,
        recipient: Address,
        amount: i128,
    ) -> Result<(), ContractError> {
        admin.require_auth();

        if amount < 0 {
            return Err(ContractError::InvalidAmount);
        }

        let key = DataKey::ApprovedGrant(recipient.clone());
        let total_key = DataKey::TotalApproved(recipient.clone());

        e.storage().persistent().set(&key, &amount);
        e.storage()
            .persistent()
            .extend_ttl(&key, TTL_LEDGERS, TTL_LEDGERS);
        e.storage().persistent().set(&total_key, &amount);
        e.storage()
            .persistent()
            .extend_ttl(&total_key, TTL_LEDGERS, TTL_LEDGERS);

        e.storage().instance().extend_ttl(TTL_LEDGERS, TTL_LEDGERS);

        e.events().publish(
            (Symbol::new(&e, "approve_grant"), recipient.clone()),
            ApproveGrantEvent { recipient, amount },
        );

        Ok(())
    }

    /// Returns the remaining approved grant amount for `recipient`.
    pub fn get_approved_grant(e: Env, recipient: Address) -> i128 {
        e.storage()
            .persistent()
            .get(&DataKey::ApprovedGrant(recipient))
            .unwrap_or(0)
    }

    /// Returns the originally approved total for `recipient`.
    pub fn get_total_approved(e: Env, recipient: Address) -> i128 {
        e.storage()
            .persistent()
            .get(&DataKey::TotalApproved(recipient))
            .unwrap_or(0)
    }

    // ── Payout ────────────────────────────────────────────────────────────────

    /// Disburse `amount` from the approved grant to `recipient`.
    /// The caller must be the admin.  Velocity controls are enforced.
    pub fn payout(
        e: Env,
        admin: Address,
        recipient: Address,
        amount: i128,
    ) -> Result<(), ContractError> {
        DefaultEmergencyGuard::check_not_paused(&e, PAUSE_PAYOUT)
            .map_err(|_| ContractError::Paused)?;

        admin.require_auth();

        if amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        let config = Self::get_config(e.clone())?;

        // Check approved grant has sufficient remaining.
        let grant_key = DataKey::ApprovedGrant(recipient.clone());
        let approved: i128 = e
            .storage()
            .persistent()
            .get(&grant_key)
            .unwrap_or(0);
        if approved < amount {
            return Err(ContractError::GrantNotApproved);
        }

        // Check pool has sufficient tokens.
        let pool_balance =
            token::Client::new(&e, &config.payout_token).balance(&e.current_contract_address());
        if pool_balance < amount {
            return Err(ContractError::InsufficientFunds);
        }

        // Enforce velocity controls.
        PayoutVelocity::check_and_record(&e, &recipient, amount).map_err(map_velocity_err)?;

        // Reduce the approved grant.
        e.storage()
            .persistent()
            .set(&grant_key, &(approved - amount));
        e.storage()
            .persistent()
            .extend_ttl(&grant_key, TTL_LEDGERS, TTL_LEDGERS);

        // Transfer tokens to recipient.
        token::Client::new(&e, &config.payout_token).transfer(
            &e.current_contract_address(),
            &recipient,
            &amount,
        );

        e.storage().instance().extend_ttl(TTL_LEDGERS, TTL_LEDGERS);

        e.events().publish(
            (Symbol::new(&e, "payout"), recipient.clone()),
            PayoutEvent { recipient, amount },
        );

        Ok(())
    }

    // ── Velocity configuration ────────────────────────────────────────────────

    /// Update the velocity configuration (admin only).
    pub fn set_velocity_config(
        e: Env,
        admin: Address,
        config: VelocityConfig,
    ) -> Result<(), ContractError> {
        admin.require_auth();
        PayoutVelocity::set_config(&e, config).map_err(map_velocity_err)
    }

    /// Read the current velocity configuration.
    pub fn get_velocity_config(e: Env) -> Option<VelocityConfig> {
        PayoutVelocity::get_config(&e)
    }

    // ── Velocity introspection ────────────────────────────────────────────────

    pub fn get_recipient_epoch_outflow(e: Env, recipient: Address) -> i128 {
        PayoutVelocity::get_recipient_epoch_outflow(&e, &recipient, e.ledger().sequence())
    }

    pub fn get_recipient_last_payout(e: Env, recipient: Address) -> u32 {
        PayoutVelocity::get_recipient_last_payout(&e, &recipient)
    }

    pub fn get_rolling_window_total(e: Env) -> i128 {
        PayoutVelocity::get_rolling_window_total(&e)
    }

    // ── Views ──────────────────────────────────────────────────────────────────

    pub fn get_config(e: Env) -> Result<GrantsConfig, ContractError> {
        e.storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(ContractError::NotInitialized)
    }

    pub fn get_pool_balance(e: Env) -> i128 {
        let config = match Self::get_config(e.clone()) {
            Ok(c) => c,
            Err(_) => return 0,
        };
        token::Client::new(&e, &config.payout_token).balance(&e.current_contract_address())
    }

    // ── Emergency-guard passthrough ────────────────────────────────────────────

    pub fn set_pause(
        e: Env,
        admin: Address,
        operation: u32,
        paused: bool,
    ) -> Result<(), ContractError> {
        EmergencyGuard::set_pause(e, admin, operation, paused)
            .map_err(|_| ContractError::Paused)
    }

    pub fn emergency_pause_all(e: Env, approvers: Vec<Address>) -> Result<(), ContractError> {
        EmergencyGuard::emergency_pause(e, approvers).map_err(|_| ContractError::Paused)
    }

    pub fn resume_all(e: Env, approvers: Vec<Address>) -> Result<(), ContractError> {
        EmergencyGuard::resume(e, approvers).map_err(|_| ContractError::Paused)
    }

    pub fn get_pause_state(e: Env) -> u32 {
        DefaultEmergencyGuard::get_pause_state(&e)
    }

    pub fn is_paused(e: Env, operation: u32) -> bool {
        EmergencyGuard::is_paused(e, operation)
    }

    pub fn get_admins(e: Env) -> Vec<Address> {
        EmergencyGuard::get_admins(e)
    }

    pub fn get_threshold(e: Env) -> u32 {
        EmergencyGuard::get_threshold(e)
    }
}

#[cfg(test)]
mod test;
