#![no_std]

use soroban_sdk::{
    contracterror, contracttype, Address, Env, String, Vec,
};
#[cfg(feature = "contract")]
use soroban_sdk::{contract, contractimpl};

/// Granular pause types using bitmask for efficient storage
#[contracttype]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PauseType(u32);

impl PauseType {
    pub const SWAP: u32 = 1 << 0;
    pub const DEPOSIT: u32 = 1 << 1;
    pub const WITHDRAW: u32 = 1 << 2;
    pub const TRANSFER: u32 = 1 << 3;
    pub const MINT: u32 = 1 << 4;
    pub const BURN: u32 = 1 << 5;
    pub const CREATE_PAIR: u32 = 1 << 6;

    pub fn new(value: u32) -> Self {
        PauseType(value)
    }

    pub fn is_paused(&self, operation: u32) -> bool {
        (self.0 & operation) != 0
    }

    pub fn set_paused(&mut self, operation: u32, paused: bool) {
        if paused {
            self.0 |= operation;
        } else {
            self.0 &= !operation;
        }
    }

    pub fn pause_all(&mut self) {
        self.0 = u32::MAX;
    }

    pub fn unpause_all(&mut self) {
        self.0 = 0;
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }
}

/// Data keys for emergency guard storage
#[contracttype]
pub enum GuardDataKey {
    PauseState,
    Admins,
    SignatureThreshold,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[repr(u32)]
pub enum GuardError {
    NotInitialized = 0,
    Unauthorized = 1,
    Paused = 2,
    InsufficientSignatures = 3,
    InvalidThreshold = 4,
    AdminNotFound = 5,
    AlreadyInitialized = 6,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuardInitializedEvent {
    pub admins: Vec<Address>,
    pub threshold: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PauseStateChangedEvent {
    pub admin: Address,
    pub operation: u32,
    pub paused: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmergencyPausedEvent {
    pub approvers: Vec<Address>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResumedEvent {
    pub approvers: Vec<Address>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminAddedEvent {
    pub approvers: Vec<Address>,
    pub new_admin: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminRemovedEvent {
    pub approvers: Vec<Address>,
    pub admin: Address,
}

const EVENT_INIT_GUARD: &str = "emergency_guard_initialized";
const EVENT_SET_PAUSE: &str = "emergency_guard_pause_state_changed";
const EVENT_EMERGENCY_PAUSE_ALL: &str = "emergency_guard_emergency_paused_all";
const EVENT_RESUME_ALL: &str = "emergency_guard_resumed_all";
const EVENT_ADD_ADMIN: &str = "emergency_guard_admin_added";
const EVENT_REMOVE_ADMIN: &str = "emergency_guard_admin_removed";

pub fn emit_guard_initialized(e: &Env, admins: &Vec<Address>, threshold: u32) {
    e.events().publish(
        (String::from_str(e, EVENT_INIT_GUARD),),
        GuardInitializedEvent {
            admins: admins.clone(),
            threshold,
        },
    );
}

pub fn emit_pause_state_changed(e: &Env, admin: &Address, operation: u32, paused: bool) {
    e.events().publish(
        (String::from_str(e, EVENT_SET_PAUSE), admin.clone()),
        PauseStateChangedEvent {
            admin: admin.clone(),
            operation,
            paused,
        },
    );
}

pub fn emit_emergency_paused_all(e: &Env, approvers: &Vec<Address>) {
    e.events().publish(
        (String::from_str(e, EVENT_EMERGENCY_PAUSE_ALL),),
        EmergencyPausedEvent {
            approvers: approvers.clone(),
        },
    );
}

pub fn emit_resumed_all(e: &Env, approvers: &Vec<Address>) {
    e.events().publish(
        (String::from_str(e, EVENT_RESUME_ALL),),
        ResumedEvent {
            approvers: approvers.clone(),
        },
    );
}

pub fn emit_admin_added(e: &Env, approvers: &Vec<Address>, new_admin: &Address) {
    e.events().publish(
        (String::from_str(e, EVENT_ADD_ADMIN), new_admin.clone()),
        AdminAddedEvent {
            approvers: approvers.clone(),
            new_admin: new_admin.clone(),
        },
    );
}

pub fn emit_admin_removed(e: &Env, approvers: &Vec<Address>, admin: &Address) {
    e.events().publish(
        (String::from_str(e, EVENT_REMOVE_ADMIN), admin.clone()),
        AdminRemovedEvent {
            approvers: approvers.clone(),
            admin: admin.clone(),
        },
    );
}

#[cfg_attr(feature = "contract", contract)]
pub struct EmergencyGuard;

#[cfg_attr(feature = "contract", contractimpl)]
impl EmergencyGuard {
    /// Initialize the emergency guard with a list of admins and required threshold.
    pub fn initialize(env: Env, admins: Vec<Address>, threshold: u32) -> Result<(), GuardError> {
        if env.storage().instance().has(&GuardDataKey::Admins) {
            return Err(GuardError::AlreadyInitialized);
        }
        if threshold == 0 || threshold > admins.len() {
            return Err(GuardError::InvalidThreshold);
        }
        env.storage().instance().set(&GuardDataKey::Admins, &admins);
        env.storage()
            .instance()
            .set(&GuardDataKey::SignatureThreshold, &threshold);
        env.storage()
            .instance()
            .set(&GuardDataKey::PauseState, &PauseType::new(0));
        emit_guard_initialized(&env, &admins, threshold);
        Ok(())
    }

    /// Returns the raw pause-state bitmask.
    pub fn get_pause_state(env: Env) -> u32 {
        let state: PauseType = env
            .storage()
            .instance()
            .get(&GuardDataKey::PauseState)
            .unwrap_or(PauseType::new(0));
        state.as_u32()
    }

    /// Check if an operation is paused.
    pub fn is_paused(env: Env, operation: u32) -> bool {
        let state: PauseType = env
            .storage()
            .instance()
            .get(&GuardDataKey::PauseState)
            .unwrap_or(PauseType::new(0));
        state.is_paused(operation)
    }

    /// Set pause state for a specific operation (any single admin can do this).
    pub fn set_pause(
        env: Env,
        admin: Address,
        operation: u32,
        paused: bool,
    ) -> Result<(), GuardError> {
        admin.require_auth();
        if !Self::is_admin_internal(&env, &admin) {
            return Err(GuardError::Unauthorized);
        }
        let mut state: PauseType = env
            .storage()
            .instance()
            .get(&GuardDataKey::PauseState)
            .unwrap_or(PauseType::new(0));
        state.set_paused(operation, paused);
        env.storage()
            .instance()
            .set(&GuardDataKey::PauseState, &state);
        emit_pause_state_changed(&env, &admin, operation, paused);
        Ok(())
    }

    /// Emergency pause all operations (requires multi-sig approval).
    pub fn emergency_pause(env: Env, approvers: Vec<Address>) -> Result<(), GuardError> {
        Self::check_multi_sig(&env, &approvers)?;
        let mut state = PauseType::new(0);
        state.pause_all();
        env.storage()
            .instance()
            .set(&GuardDataKey::PauseState, &state);
        emit_emergency_paused_all(&env, &approvers);
        Ok(())
    }

    /// Resume all operations (requires multi-sig approval).
    pub fn resume(env: Env, approvers: Vec<Address>) -> Result<(), GuardError> {
        Self::check_multi_sig(&env, &approvers)?;
        env.storage()
            .instance()
            .set(&GuardDataKey::PauseState, &PauseType::new(0));
        emit_resumed_all(&env, &approvers);
        Ok(())
    }

    /// Add new admin (multi-sig required).
    pub fn add_admin(
        env: Env,
        approvers: Vec<Address>,
        new_admin: Address,
    ) -> Result<(), GuardError> {
        Self::check_multi_sig(&env, &approvers)?;
        let mut admins = Self::get_admins(env.clone());
        if !admins.iter().any(|a| a == new_admin) {
            admins.push_back(new_admin.clone());
            env.storage().instance().set(&GuardDataKey::Admins, &admins);
            emit_admin_added(&env, &approvers, &new_admin);
        }
        Ok(())
    }

    /// Remove admin (multi-sig required).
    pub fn remove_admin(
        env: Env,
        approvers: Vec<Address>,
        admin: Address,
    ) -> Result<(), GuardError> {
        Self::check_multi_sig(&env, &approvers)?;
        let admins = Self::get_admins(env.clone());
        let threshold = Self::get_threshold(env.clone());
        if admins.len() <= threshold {
            return Err(GuardError::InvalidThreshold);
        }
        let mut new_admins = Vec::new(&env);
        let mut found = false;
        for a in admins.iter() {
            if a != admin {
                new_admins.push_back(a);
            } else {
                found = true;
            }
        }
        if !found {
            return Err(GuardError::AdminNotFound);
        }
        env.storage()
            .instance()
            .set(&GuardDataKey::Admins, &new_admins);
        emit_admin_removed(&env, &approvers, &admin);
        Ok(())
    }

    /// Rotate admin (multi-sig required).
    pub fn rotate_admin(
        env: Env,
        approvers: Vec<Address>,
        old_admin: Address,
        new_admin: Address,
    ) -> Result<(), GuardError> {
        Self::check_multi_sig(&env, &approvers)?;
        let admins = Self::get_admins(env.clone());
        let mut new_admins = Vec::new(&env);
        let mut found = false;
        for a in admins.iter() {
            if a == old_admin {
                found = true;
                if !new_admins.iter().any(|x| x == new_admin) {
                    new_admins.push_back(new_admin.clone());
                }
            } else if !new_admins.iter().any(|x| x == a) {
                new_admins.push_back(a);
            }
        }
        if !found {
            return Err(GuardError::AdminNotFound);
        }
        env.storage()
            .instance()
            .set(&GuardDataKey::Admins, &new_admins);
        Ok(())
    }

    /// Get list of current admins.
    pub fn get_admins(env: Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&GuardDataKey::Admins)
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Get required signature threshold.
    pub fn get_threshold(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&GuardDataKey::SignatureThreshold)
            .unwrap_or(0)
    }

    /// Check if an address is an admin.
    pub fn is_admin_public(env: Env, addr: Address) -> bool {
        Self::is_admin_internal(&env, &addr)
    }

    /// Public wrapper to validate approvers against the stored threshold.
    pub fn validate_multi_sig(env: Env, approvers: Vec<Address>) -> Result<(), GuardError> {
        Self::check_multi_sig(&env, &approvers)
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn is_admin_internal(env: &Env, addr: &Address) -> bool {
        let admins: Vec<Address> = env
            .storage()
            .instance()
            .get(&GuardDataKey::Admins)
            .unwrap_or_else(|| Vec::new(env));
        admins.iter().any(|a| a == *addr)
    }

    /// Verify that `approvers` contains at least `threshold` distinct valid admins,
    /// each having provided their authorization.
    pub(crate) fn check_multi_sig(env: &Env, approvers: &Vec<Address>) -> Result<(), GuardError> {
        let threshold: u32 = env
            .storage()
            .instance()
            .get(&GuardDataKey::SignatureThreshold)
            .ok_or(GuardError::NotInitialized)?;

        if approvers.len() < threshold {
            return Err(GuardError::InsufficientSignatures);
        }

        let mut valid = 0u32;
        let mut seen = Vec::new(env);
        for addr in approvers.iter() {
            if seen.iter().any(|a| a == addr) {
                continue;
            }
            seen.push_back(addr.clone());
            if Self::is_admin_internal(env, &addr) {
                addr.require_auth();
                valid += 1;
            }
        }

        if valid < threshold {
            Err(GuardError::InsufficientSignatures)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod test;

