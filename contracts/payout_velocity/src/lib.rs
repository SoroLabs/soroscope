#![no_std]
//! # Payout Velocity Controls
//!
//! Reusable velocity-guard library for disbursement contracts.
//!
//! Provides three independent, configurable rate-limits that are checked and
//! recorded atomically inside [`PayoutVelocity::check_and_record`]:
//!
//! 1. **Per-recipient per-epoch outflow cap** — limits how much a single
//!    address can receive within one epoch (`epoch_length` ledgers).
//! 2. **Minimum spacing** — enforces a minimum number of ledgers between
//!    consecutive payouts to the same recipient.
//! 3. **Rolling-window cap** — limits the *total* outflow (across all
//!    recipients) inside a rolling window of the last N ledgers.
//!
//! Each limit can be individually disabled by setting its threshold to zero,
//! allowing contracts to pick only the controls they need.
//!
//! The crate mirrors the embedding pattern used by `emergency_guard`:
//! contracts call static methods on [`PayoutVelocity`] and map
//! [`VelocityError`] to their own error enum.

use soroban_sdk::{contracterror, contracttype, Address, Env, Vec};

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[repr(u32)]
pub enum VelocityError {
    NotInitialized = 0,
    EpochCapExceeded = 1,
    MinSpacingNotMet = 2,
    RollingWindowCapExceeded = 3,
    InvalidConfig = 4,
    InvalidAmount = 5,
}

// ── Configuration ─────────────────────────────────────────────────────────────

/// All three velocity limits in one struct.  A threshold of `0` disables the
/// corresponding control.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VelocityConfig {
    /// Ledgers per epoch.  `0` disables per-epoch tracking.
    pub epoch_length: u32,
    /// Maximum cumulative payout to a single recipient within one epoch.
    /// `0` = unlimited.
    pub max_per_recipient_per_epoch: i128,
    /// Minimum ledgers that must elapse between two payouts to the same
    /// recipient.  `0` = no minimum spacing.
    pub min_spacing_ledgers: u32,
    /// Width of the rolling-window in ledgers.  `0` disables the window.
    pub rolling_window_ledgers: u32,
    /// Maximum total payout (all recipients) inside the rolling window.
    /// `0` = unlimited.
    pub max_per_rolling_window: i128,
}

impl Default for VelocityConfig {
    fn default() -> Self {
        Self {
            epoch_length: 0,
            max_per_recipient_per_epoch: 0,
            min_spacing_ledgers: 0,
            rolling_window_ledgers: 0,
            max_per_rolling_window: 0,
        }
    }
}

// ── Storage types ────────────────────────────────────────────────────────────

/// A single payout record stored in the rolling-window log.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowRecord {
    pub ledger: u32,
    pub amount: i128,
}

/// Storage keys used by the velocity guard.  Contracts that embed
/// `PayoutVelocity` share the contract's own storage namespace; the keys
/// below are namespaced under the `Velocity` prefix to avoid collisions.
#[contracttype]
#[derive(Clone)]
pub enum VelocityKey {
    Config,
    /// `(recipient, epoch)` → cumulative outflow this epoch.
    RecipientEpochOutflow(Address, u32),
    /// `recipient` → last ledger a payout was sent.
    RecipientLastPayout(Address),
    /// Ordered log of recent payouts for rolling-window accounting.
    RollingWindowRecords,
}

// ── Velocity guard ───────────────────────────────────────────────────────────

/// Static helper analogous to `EmergencyGuard`.  Contracts embed the guard by
/// calling [`PayoutVelocity::initialize`] during their own `initialize` and
/// [`PayoutVelocity::check_and_record`] at every payout entrypoint.
pub struct PayoutVelocity;

impl PayoutVelocity {
    // ── Lifecycle ────────────────────────────────────────────────────────────

    /// Store the velocity configuration.  Call once during contract init.
    pub fn initialize(e: &Env, config: VelocityConfig) -> Result<(), VelocityError> {
        if e.storage().instance().has(&VelocityKey::Config) {
            return Err(VelocityError::NotInitialized);
        }
        Self::validate_config(&config)?;
        e.storage().instance().set(&VelocityKey::Config, &config);
        e.storage()
            .instance()
            .set(&VelocityKey::RollingWindowRecords, &Vec::<WindowRecord>::new(e));
        Ok(())
    }

    /// Replace the configuration (admin-gated by the calling contract).
    pub fn set_config(e: &Env, config: VelocityConfig) -> Result<(), VelocityError> {
        Self::validate_config(&config)?;
        e.storage().instance().set(&VelocityKey::Config, &config);
        Ok(())
    }

    /// Read the active configuration, or `None` if uninitialized.
    pub fn get_config(e: &Env) -> Option<VelocityConfig> {
        e.storage().instance().get(&VelocityKey::Config)
    }

    // ── Core check-and-record ────────────────────────────────────────────────

    /// Validate all three velocity limits for `amount` to `recipient`, then
    /// record the payout so subsequent calls see the updated state.
    ///
    /// Returns `Ok(())` when the payout is within all limits, or the first
    /// `Err(VelocityError)` encountered.  On error no state is mutated.
    pub fn check_and_record(
        e: &Env,
        recipient: &Address,
        amount: i128,
    ) -> Result<(), VelocityError> {
        if amount <= 0 {
            return Err(VelocityError::InvalidAmount);
        }

        let config = Self::get_config(e).ok_or(VelocityError::NotInitialized)?;
        let current_ledger = e.ledger().sequence();

        // ── Phase 1: validate ───────────────────────────────────────────────

        // 1a. Per-recipient per-epoch cap
        let mut epoch_outflow: i128 = 0;
        if config.epoch_length > 0 && config.max_per_recipient_per_epoch > 0 {
            let epoch = current_ledger / config.epoch_length;
            let key = VelocityKey::RecipientEpochOutflow(recipient.clone(), epoch);
            epoch_outflow = e.storage().persistent().get(&key).unwrap_or(0);
            let new_total = epoch_outflow
                .checked_add(amount)
                .ok_or(VelocityError::EpochCapExceeded)?;
            if new_total > config.max_per_recipient_per_epoch {
                return Err(VelocityError::EpochCapExceeded);
            }
        }

        // 1b. Minimum spacing between payouts to the same recipient
        if config.min_spacing_ledgers > 0 {
            let key = VelocityKey::RecipientLastPayout(recipient.clone());
            let last: u32 = e.storage().persistent().get(&key).unwrap_or(0);
            if last > 0 && current_ledger.saturating_sub(last) < config.min_spacing_ledgers {
                return Err(VelocityError::MinSpacingNotMet);
            }
        }

        // 1c. Rolling-window cap
        let mut pruned_records: Option<Vec<WindowRecord>> = None;
        if config.rolling_window_ledgers > 0 && config.max_per_rolling_window > 0 {
            let records_key = VelocityKey::RollingWindowRecords;
            let records: Vec<WindowRecord> = e
                .storage()
                .instance()
                .get(&records_key)
                .unwrap_or(Vec::new(e));

            let cutoff = current_ledger.saturating_sub(config.rolling_window_ledgers);
            let mut window_total: i128 = 0;
            let mut kept = Vec::new(e);
            for i in 0..records.len() {
                let record = records.get(i).unwrap();
                if record.ledger > cutoff {
                    window_total = window_total
                        .checked_add(record.amount)
                        .unwrap_or(window_total);
                    kept.push_back(record);
                }
            }

            let new_window_total = window_total
                .checked_add(amount)
                .ok_or(VelocityError::RollingWindowCapExceeded)?;
            if new_window_total > config.max_per_rolling_window {
                return Err(VelocityError::RollingWindowCapExceeded);
            }

            pruned_records = Some(kept);
        }

        // ── Phase 2: record ──────────────────────────────────────────────────

        // 2a. Epoch outflow
        if config.epoch_length > 0 && config.max_per_recipient_per_epoch > 0 {
            let epoch = current_ledger / config.epoch_length;
            let key = VelocityKey::RecipientEpochOutflow(recipient.clone(), epoch);
            e.storage()
                .persistent()
                .set(&key, &(epoch_outflow + amount));
            e.storage()
                .persistent()
                .extend_ttl(&key, 10_000, 10_000);
        }

        // 2b. Last payout ledger
        if config.min_spacing_ledgers > 0 {
            let key = VelocityKey::RecipientLastPayout(recipient.clone());
            e.storage().persistent().set(&key, &current_ledger);
            e.storage()
                .persistent()
                .extend_ttl(&key, 10_000, 10_000);
        }

        // 2c. Rolling-window record (append to pruned list)
        if let Some(mut kept) = pruned_records {
            kept.push_back(WindowRecord {
                ledger: current_ledger,
                amount,
            });
            e.storage()
                .instance()
                .set(&VelocityKey::RollingWindowRecords, &kept);
        }

        Ok(())
    }

    // ── Introspection helpers ────────────────────────────────────────────────

    /// Cumulative outflow to `recipient` in the epoch containing `ledger`.
    pub fn get_recipient_epoch_outflow(e: &Env, recipient: &Address, ledger: u32) -> i128 {
        let config = match Self::get_config(e) {
            Some(c) if c.epoch_length > 0 => c,
            _ => return 0,
        };
        let epoch = ledger / config.epoch_length;
        let key = VelocityKey::RecipientEpochOutflow(recipient.clone(), epoch);
        e.storage().persistent().get(&key).unwrap_or(0)
    }

    /// Last ledger a payout was sent to `recipient` (0 = never).
    pub fn get_recipient_last_payout(e: &Env, recipient: &Address) -> u32 {
        let key = VelocityKey::RecipientLastPayout(recipient.clone());
        e.storage().persistent().get(&key).unwrap_or(0)
    }

    /// Total outflow inside the current rolling window (after pruning).
    pub fn get_rolling_window_total(e: &Env) -> i128 {
        let config = match Self::get_config(e) {
            Some(c) if c.rolling_window_ledgers > 0 => c,
            _ => return 0,
        };
        let current_ledger = e.ledger().sequence();
        let cutoff = current_ledger.saturating_sub(config.rolling_window_ledgers);
        let records: Vec<WindowRecord> = e
            .storage()
            .instance()
            .get(&VelocityKey::RollingWindowRecords)
            .unwrap_or(Vec::new(e));
        let mut total: i128 = 0;
        for i in 0..records.len() {
            let r = records.get(i).unwrap();
            if r.ledger > cutoff {
                total += r.amount;
            }
        }
        total
    }

    // ── Internal ──────────────────────────────────────────────────────────────

    fn validate_config(config: &VelocityConfig) -> Result<(), VelocityError> {
        if config.max_per_recipient_per_epoch < 0
            || config.max_per_rolling_window < 0
        {
            return Err(VelocityError::InvalidConfig);
        }
        if config.epoch_length == 0 && config.max_per_recipient_per_epoch > 0 {
            return Err(VelocityError::InvalidConfig);
        }
        Ok(())
    }
}

#[cfg(test)]
mod test;
