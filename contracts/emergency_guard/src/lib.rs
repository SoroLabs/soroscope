#![no_std]

#[cfg(feature = "contract")]
use soroban_sdk::{contract, contractimpl};
use soroban_sdk::{contracterror, contracttype, log, Address, Env, String, Vec};
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, log, Address, Env, String, Vec,
};
use soroban_sdk::{contracterror, contracttype, Address, Env, Vec};

/// Granular pause types using bitmask for efficient storage.
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
    pub const STAKE: u32 = 1 << 7;
    /// Pause reward claims independently of the global paused flag
    pub const CLAIM_REWARDS: u32 = 1 << 8;
    /// Pause borrow / flash loan operations
    pub const BORROW: u32 = 1 << 8;

    pub fn new(value: u32) -> Self {
        PauseType(value)
    }

    #[inline(always)]
    pub fn is_paused(&self, operation: u32) -> bool {
        (self.0 & operation) != 0
    }

    #[inline(always)]
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

/// Storage keys for emergency guard state.
#[contracttype]
pub enum GuardDataKey {
    PauseState,
    Admins,
    Guardians,
    SignatureThreshold,
    /// Addresses allowed to trigger an emergency pause (but not unpause or manage roles).
    Guardians,
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
    GuardianNotFound = 7,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[cfg_attr(feature = "contract", contract)]
pub struct EmergencyGuard;

#[cfg_attr(feature = "contract", contractimpl)]
impl EmergencyGuard {
    /// Initialize with a set of admins (multi-sig threshold) and an initial guardian.
    ///
    /// - `admins`    – addresses that form the multi-sig quorum; can unpause and manage roles.
    /// - `threshold` – how many admin signatures are required for privileged operations.
    /// - `guardian`  – address that may trigger an emergency pause unilaterally.
    pub fn initialize(
        env: Env,
        admins: Vec<Address>,
        threshold: u32,
        guardian: Address,
    /// Initialize the emergency guard with a list of admins and required threshold.
    /// Guardians default to the same set as admins so existing integrations keep working.
    pub fn initialize(env: Env, admins: Vec<Address>, threshold: u32) -> Result<(), GuardError> {
        Self::initialize_with_roles(env, admins.clone(), admins, threshold)
    }

    /// Initialize the emergency guard with distinct admin and guardian roles.
    pub fn initialize_with_roles(
        guardians: Vec<Address>,
    ) -> Result<(), GuardError> {
        if env.storage().instance().has(&GuardDataKey::Admins) {
            return Err(GuardError::AlreadyInitialized);
        }
        if threshold == 0 || threshold > admins.len() || threshold > guardians.len() {
            return Err(GuardError::InvalidThreshold);
        }

        let mut guardians = Vec::new(&env);
        guardians.push_back(guardian);

        env.storage().instance().set(&GuardDataKey::Admins, &admins);
        env.storage().instance().set(&GuardDataKey::SignatureThreshold, &threshold);
        env.storage().instance().set(&GuardDataKey::PauseState, &PauseType::new(0));
        env.storage().instance().set(&GuardDataKey::Guardians, &guardians);
        env.storage()
            .instance()
            .set(&GuardDataKey::Guardians, &guardians);
            .set(&GuardDataKey::SignatureThreshold, &threshold);
            .set(&GuardDataKey::PauseState, &PauseType::new(0));
        emit_guard_initialized(&env, &admins, threshold);
        Ok(())
    }

    // ── Pause queries ──────────────────────────────────────────────────────────

    pub fn is_paused(env: Env, operation: u32) -> bool {
        Self::is_paused_ref(&env, operation)
    }

    #[inline(always)]
    pub fn is_paused_ref(env: &Env, operation: u32) -> bool {
        let mask: u32 = env
            .storage()
            .instance()
            .get(&GuardDataKey::PauseState)
            .map(|s: PauseType| s.as_u32())
            .unwrap_or(0);
        (mask & operation) != 0
    }

    pub fn ensure_not_paused(env: &Env, operation: u32) {
        if Self::is_paused_ref(env, operation) {
            panic!("operation paused");
        }
    }

    pub fn get_pause_state(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&GuardDataKey::PauseState)
            .map(|s: PauseType| s.as_u32())
            .unwrap_or(0)
    }

    // ── Guardian: pause only ───────────────────────────────────────────────────

    /// Pause a specific operation. Can be called by any single guardian OR any single admin.
    pub fn set_pause(
        env: Env,
        caller: Address,
        operation: u32,
        paused: bool,
    ) -> Result<(), GuardError> {
        caller.require_auth();
        let is_admin = Self::is_admin_internal(&env, &caller);
        let is_guardian = Self::is_guardian_internal(&env, &caller);

        if !is_admin && !is_guardian {
    /// Set pause state for a specific operation (guardians only).
        guardian: Address,
        guardian.require_auth();
        if !Self::is_guardian_internal(&env, &guardian) {
            return Err(GuardError::Unauthorized);
        }

        // Guardians can only pause, not unpause individual operations.
        if is_guardian && !is_admin && !paused {
            return Err(GuardError::Unauthorized);
        }

        let mut state: PauseType = env
            .storage()
            .instance()
            .get(&GuardDataKey::PauseState)
            .unwrap_or(PauseType::new(0));
        state.set_paused(operation, paused);
        emit_pause_state_changed(&env, &admin, operation, paused);
        env.storage()
            .instance()
            .set(&GuardDataKey::PauseState, &state);

        emit_guard_event(
            &env,
            EmergencyGuardEvent {
                action: EmergencyGuardAction::PauseSet,
                admin: Some(guardian.clone()),
                operation,
                paused,
                threshold: Self::get_threshold(env.clone()),
                admin_count: Self::get_admins(env.clone()).len(),
                approver_count: 1,
            },
        );
        log!(
            "Pause state updated: op={}, paused={}",
            paused
        emit_pause_state_changed(&env, &guardian, operation, paused);
        Ok(())
    }

    /// Emergency pause all operations (requires guardian multi-sig approval).
    pub fn emergency_pause(env: Env, approvers: Vec<Address>) -> Result<(), GuardError> {
        Self::check_guardian_multi_sig(&env, &approvers)?;
        let mut state = PauseType::new(0);
        state.pause_all();
        env.storage()
            .instance()
            .set(&GuardDataKey::PauseState, &state);

        emit_guard_event(
            &env,
            EmergencyGuardEvent {
                action: EmergencyGuardAction::EmergencyPause,
                admin: None,
                operation: u32::MAX,
                paused: true,
                threshold: Self::get_threshold(env.clone()),
                admin_count: Self::get_admins(env.clone()).len(),
                approver_count: approvers.len(),
            },
        );
        emit_emergency_paused_all(&env, &approvers);
        Ok(())
    }

    /// Resume all operations (requires admin multi-sig approval).
    pub fn resume(env: Env, approvers: Vec<Address>) -> Result<(), GuardError> {
        Self::check_multi_sig(&env, &approvers)?;
        let state = PauseType::new(0);
        env.storage()
            .instance()
            .set(&GuardDataKey::PauseState, &PauseType::new(0));
            .set(&GuardDataKey::PauseState, &state);
        Self::check_admin_multi_sig(&env, &approvers)?;

        emit_guard_event(
            &env,
            EmergencyGuardEvent {
                action: EmergencyGuardAction::Resume,
                admin: None,
                operation: u32::MAX,
                paused: false,
                threshold: Self::get_threshold(env.clone()),
                admin_count: Self::get_admins(env.clone()).len(),
                approver_count: approvers.len(),
            },
        );
        emit_resumed_all(&env, &approvers);
=======
        env.storage().instance().set(&GuardDataKey::PauseState, &state);
        Ok(())
    }

    /// Emergency pause all operations. Can be triggered by a single guardian OR by a single admin.
    pub fn emergency_pause(env: Env, caller: Address) -> Result<(), GuardError> {
        caller.require_auth();
        if !Self::is_guardian_internal(&env, &caller) && !Self::is_admin_internal(&env, &caller) {
            return Err(GuardError::Unauthorized);
        }
        let mut state = PauseType::new(0);
        state.pause_all();
        env.storage().instance().set(&GuardDataKey::PauseState, &state);
        Ok(())
    }

    // ── Admin: unpause and role management ────────────────────────────────────

    /// Resume (unpause) all operations. Requires multi-sig from admins.
    pub fn resume(env: Env, approvers: Vec<Address>) -> Result<(), GuardError> {
        Self::check_multi_sig(&env, &approvers)?;
        env.storage().instance().set(&GuardDataKey::PauseState, &PauseType::new(0));
>>>>>>> e2e49c2 (feature(emergency-rbac): separate Guardian and Admin roles in EmergencyGuard)
        Ok(())
    }

    /// Add an admin. Requires multi-sig from existing admins.
    pub fn add_admin(
        env: Env,
        approvers: Vec<Address>,
        new_admin: Address,
    ) -> Result<(), GuardError> {
        Self::check_admin_multi_sig(&env, &approvers)?;
        let mut admins = Self::get_admins(env.clone());
        if !admins.iter().any(|a| a == new_admin) {
            admins.push_back(new_admin);
            env.storage().instance().set(&GuardDataKey::Admins, &admins);
            emit_guard_event(
                &env,
                EmergencyGuardEvent {
                    action: EmergencyGuardAction::AdminAdded,
                    admin: Some(new_admin.clone()),
                    operation: 0,
                    paused: false,
                    threshold: Self::get_threshold(env.clone()),
                    admin_count: admins.len(),
                    approver_count: approvers.len(),
                },
            );
            emit_admin_added(&env, &approvers, &new_admin);
=======
>>>>>>> main
        }
        Ok(())
    }

    /// Remove an admin. Requires multi-sig; cannot drop below threshold.
    pub fn remove_admin(
        env: Env,
        approvers: Vec<Address>,
        admin: Address,
    ) -> Result<(), GuardError> {
        Self::check_admin_multi_sig(&env, &approvers)?;
        let admins = Self::get_admins(env.clone());
        let threshold = Self::get_threshold(env.clone());
        if admins.len() <= threshold {
            return Err(GuardError::InvalidThreshold);
        }
        let mut new_admins = Vec::new(&env);
        let mut found = false;
        for a in admins.iter() {
            if a == admin {
                found = true;
            } else {
                new_admins.push_back(a);
            }
        }
        if !found {
            return Err(GuardError::AdminNotFound);
        }
        env.storage().instance().set(&GuardDataKey::Admins, &new_admins);

        env.storage()
            .instance()
            .set(&GuardDataKey::Admins, &new_admins);
        emit_guard_event(
            &env,
            EmergencyGuardEvent {
                action: EmergencyGuardAction::AdminRemoved,
                admin: Some(admin.clone()),
                operation: 0,
                paused: false,
                threshold,
                admin_count: new_admins.len(),
                approver_count: approvers.len(),
            },
        );
        emit_admin_removed(&env, &approvers, &admin);
        Ok(())
    }

    /// Atomically swap one admin for another. Requires multi-sig.
    pub fn rotate_admin(
        env: Env,
        approvers: Vec<Address>,
        old_admin: Address,
        new_admin: Address,
    ) -> Result<(), GuardError> {
        Self::check_multi_sig(&env, &approvers)?;
        let threshold = Self::get_threshold(env.clone());
        Self::check_admin_multi_sig(&env, &approvers)?;
        let admins = Self::get_admins(env.clone());
        let threshold = Self::get_threshold(env.clone());
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

        if new_admins.len() < threshold {
        if (new_admins.len() as u32) < threshold {
            return Err(GuardError::InvalidThreshold);
        }
        env.storage().instance().set(&GuardDataKey::Admins, &new_admins);

        env.storage()
            .instance()
            .set(&GuardDataKey::Admins, &new_admins);
        emit_guard_event(
            &env,
            EmergencyGuardEvent {
                action: EmergencyGuardAction::AdminRotated,
                admin: Some(new_admin.clone()),
                operation: 0,
                paused: false,
                threshold,
                admin_count: new_admins.len(),
                approver_count: approvers.len(),
            },
        );
        log!(&env, "Admin rotated: {} to {}", old_admin, new_admin);
=======
>>>>>>> main
        Ok(())
    }

    // ── Guardian management (admin-only) ──────────────────────────────────────

    /// Add a guardian. Requires multi-sig from admins.
    pub fn add_guardian(
        env: Env,
        approvers: Vec<Address>,
        new_guardian: Address,
    ) -> Result<(), GuardError> {
        Self::check_multi_sig(&env, &approvers)?;
        let mut guardians = Self::get_guardians(env.clone());
        if !guardians.iter().any(|g| g == new_guardian) {
            guardians.push_back(new_guardian);
            env.storage().instance().set(&GuardDataKey::Guardians, &guardians);
        }
        Ok(())
    }

    /// Remove a guardian. Requires multi-sig from admins.
    pub fn remove_guardian(
        env: Env,
        approvers: Vec<Address>,
        guardian: Address,
    ) -> Result<(), GuardError> {
        Self::check_multi_sig(&env, &approvers)?;
        let guardians = Self::get_guardians(env.clone());
        let mut new_guardians = Vec::new(&env);
        let mut found = false;
        for g in guardians.iter() {
            if g == guardian {
                found = true;
            } else {
                new_guardians.push_back(g);
            }
        }
        if !found {
            return Err(GuardError::GuardianNotFound);
        }
        env.storage().instance().set(&GuardDataKey::Guardians, &new_guardians);
        Ok(())
    }

    // ── Accessors ─────────────────────────────────────────────────────────────

    pub fn get_admins(env: Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&GuardDataKey::Admins)
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Get list of current guardians.
    pub fn get_guardians(env: Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&GuardDataKey::Guardians)
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Get required signature threshold.
    pub fn get_threshold(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&GuardDataKey::SignatureThreshold)
            .unwrap_or(0)
    }

    pub fn get_guardians(env: Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&GuardDataKey::Guardians)
            .unwrap_or_else(|| Vec::new(&env))
    }

    pub fn is_admin(env: Env, addr: Address) -> bool {
        Self::is_admin_internal(&env, &addr)
    }

    pub fn is_guardian(env: Env, addr: Address) -> bool {
        Self::is_guardian_internal(&env, &addr)
    }

    /// Check if an address is a guardian.
    pub fn is_guardian_public(env: Env, addr: Address) -> bool {

    /// Public wrapper to validate approvers against the stored threshold.
    pub fn validate_multi_sig(env: Env, approvers: Vec<Address>) -> Result<(), GuardError> {
        Self::check_admin_multi_sig(&env, &approvers)
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

    fn is_guardian_internal(env: &Env, addr: &Address) -> bool {
        let guardians: Vec<Address> = env
            .storage()
            .instance()
            .get(&GuardDataKey::Guardians)
            .unwrap_or_else(|| Vec::new(env));
        guardians.iter().any(|g| g == *addr)
    }

        guardians.iter().any(|a| a == *addr)

    /// Verify that `approvers` contains at least `threshold` distinct valid admins,
    /// each having provided their authorization.
    pub(crate) fn check_multi_sig(env: &Env, approvers: &Vec<Address>) -> Result<(), GuardError> {
        Self::check_admin_multi_sig(env, approvers)
    }

    pub(crate) fn check_admin_multi_sig(
        env: &Env,
        approvers: &Vec<Address>,
    ) -> Result<(), GuardError> {
        Self::check_role_multi_sig(env, approvers, |env, addr| {
            Self::is_admin_internal(env, addr)
        })
    }

    pub(crate) fn check_guardian_multi_sig(
        env: &Env,
        approvers: &Vec<Address>,
    ) -> Result<(), GuardError> {
        Self::check_role_multi_sig(env, approvers, |env, addr| {
            Self::is_guardian_internal(env, addr)
        })
    }

    fn check_role_multi_sig<F>(
        env: &Env,
        approvers: &Vec<Address>,
        is_member: F,
    ) -> Result<(), GuardError>
    where
        F: Fn(&Env, &Address) -> bool,
    {
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
            if is_member(env, &addr) {
                addr.require_auth();
                valid += 1;
            } else {
                return Err(GuardError::Unauthorized);
            }
        }

        if valid < threshold {
            Err(GuardError::InsufficientSignatures)
        } else {
            Ok(())
        }
    }

}

/// Default implementation of EmergencyGuardTrait using static methods
pub struct DefaultEmergencyGuard;

impl EmergencyGuardTrait for DefaultEmergencyGuard {
    /// Check if an operation is paused. Returns Err if paused.
    fn check_not_paused(env: &Env, operation: u32) -> Result<(), GuardError> {
        let pause_state: PauseType = env
            .storage()
            .instance()
            .get(&GuardDataKey::PauseState)
            .unwrap_or(PauseType::new(0));

        if pause_state.is_paused(operation) {
            Err(GuardError::Paused)
        } else {
            Ok(())
        }
    }

    /// Get current pause state
    fn get_pause_state(env: &Env) -> u32 {
        let pause_state: PauseType = env
            .storage()
            .instance()
            .get(&GuardDataKey::PauseState)
            .unwrap_or(PauseType::new(0));
        pause_state.0
    }

    /// Set pause state for a specific operation (guardians preferred, admins as fallback)
    fn set_pause_state(env: &Env, operation: u32, paused: bool) -> Result<(), GuardError> {
        let mut pause_state: PauseType = env
            .storage()
            .instance()
            .get(&GuardDataKey::PauseState)
            .unwrap_or(PauseType::new(0));

        pause_state.set_paused(operation, paused);
        env.storage()
            .instance()
            .set(&GuardDataKey::PauseState, &pause_state);

        log!(
            env,
            "Pause state updated: op={}, paused={}",
            operation,
            paused
        );
        Ok(())
    }

    /// Unpause a specific operation (guardians preferred, admins as fallback)
    fn unpause(env: &Env, operation: u32) -> Result<(), GuardError> {
        Self::set_pause_state(env, operation, false)
    }

    /// Unpause all operations (guardians preferred, admins as fallback)
    fn unpause_all(env: &Env) -> Result<(), GuardError> {
        let guardians = EmergencyGuard::get_guardians(env.clone());
        let actor = guardians.get(0).or_else(|| EmergencyGuard::get_admins(env.clone()).get(0));
        if let Some(actor) = actor {
            EmergencyGuard::set_pause(env.clone(), actor, u32::MAX, false)
        } else {
            Err(GuardError::Unauthorized)
        }
    }

    /// Emergency pause all operations (requires guardian multi-sig approval)
    fn emergency_pause_all(env: &Env, approvers: Vec<Address>) -> Result<(), GuardError> {
        EmergencyGuard::check_guardian_multi_sig(env, &approvers)?;

        let mut pause_state = PauseType::new(0);
        pause_state.pause_all();

        env.storage()
            .instance()
            .set(&GuardDataKey::PauseState, &pause_state);

        log!(env, "Emergency pause all activated");
        Ok(())
    }

    /// Resume all operations (unpause all) - requires admin multi-sig approval
    fn resume_all(env: &Env, approvers: Vec<Address>) -> Result<(), GuardError> {
        EmergencyGuard::check_admin_multi_sig(env, &approvers)?;

        let pause_state = PauseType::new(0);
        env.storage()
            .instance()
            .set(&GuardDataKey::PauseState, &pause_state);

        log!(env, "All operations resumed (unpaused)");
        Ok(())
    }

    /// Initialize emergency guard with admins and threshold
    fn init_guard(env: &Env, admins: Vec<Address>, threshold: u32) -> Result<(), GuardError> {
        if env.storage().instance().has(&GuardDataKey::Admins) {
            return Err(GuardError::AlreadyInitialized);
        }

        // Verify threshold is valid
        if threshold == 0 || threshold > admins.len() {
            return Err(GuardError::InvalidThreshold);
        }

        // Store admins
        env.storage().instance().set(&GuardDataKey::Admins, &admins);

        // Store threshold
        env.storage()
            .instance()
            .set(&GuardDataKey::SignatureThreshold, &threshold);

        // Initialize pause state to 0 (nothing paused)
        env.storage()
            .instance()
            .set(&GuardDataKey::PauseState, &PauseType::new(0));

        Ok(())
    }

    /// Add new admin (multi-sig required)
    fn add_admin(env: &Env, approvers: Vec<Address>, new_admin: Address) -> Result<(), GuardError> {
        EmergencyGuard::check_multi_sig(env, &approvers)?;

        let mut admins = Self::get_admins(env);
        if !admins.iter().any(|a| a == new_admin) {
            admins.push_back(new_admin.clone());
            env.storage().instance().set(&GuardDataKey::Admins, &admins);
            log!(env, "Admin added: {}", new_admin);
        }

        Ok(())
    }

    /// Remove admin (multi-sig required)
    fn remove_admin(env: &Env, approvers: Vec<Address>, admin: Address) -> Result<(), GuardError> {
        EmergencyGuard::check_multi_sig(env, &approvers)?;

        let admins = Self::get_admins(env);
        let threshold = Self::get_threshold(env);

        if admins.len() <= threshold {
            return Err(GuardError::InvalidThreshold);
        }

        let mut new_admins = Vec::new(env);
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
        log!(env, "Admin removed: {}", admin);
        Ok(())
    }

    /// Get list of current admins
    fn get_admins(env: &Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&GuardDataKey::Admins)
            .unwrap_or_else(|| Vec::new(env))
    }

    /// Get required signature threshold
    fn get_threshold(env: &Env) -> u32 {
        env.storage()
            .instance()
            .get(&GuardDataKey::SignatureThreshold)
            .unwrap_or(0)
    }

    /// Check if address is an admin
    fn is_admin(env: &Env, addr: Address) -> bool {
        let admins: Vec<Address> = env
            .storage()
            .instance()
            .get(&GuardDataKey::Admins)
            .unwrap_or_else(|| Vec::new(env));

        admins.iter().any(|a| a == addr)
    }

    /// Rotate admin (multi-sig required)
    fn rotate_admin(
        env: &Env,
        approvers: Vec<Address>,
        old_admin: Address,
        new_admin: Address,
    ) -> Result<(), GuardError> {
        EmergencyGuard::check_multi_sig(env, &approvers)?;

        let admins = Self::get_admins(env);
        let threshold = Self::get_threshold(env);

        let mut found = false;
        let mut new_admins = Vec::new(env);
        for a in admins.iter() {
            if a == old_admin {
                found = true;
            } else if a != new_admin {
                new_admins.push_back(a);
            }
        }

        if !found {
            return Err(GuardError::AdminNotFound);
        }

        new_admins.push_back(new_admin.clone());

        if new_admins.len() < threshold {
            return Err(GuardError::InvalidThreshold);
        }

        env.storage()
            .instance()
            .set(&GuardDataKey::Admins, &new_admins);
        log!(env, "Admin rotated: {} to {}", old_admin, new_admin);
        Ok(())
    }
}

/// Extension methods for unpause operations
impl DefaultEmergencyGuard {
    /// Unpause a specific operation (uses set_pause_state internally)
    pub fn unpause(env: &Env, operation: u32) -> Result<(), GuardError> {
        Self::set_pause_state(env, operation, false)
    }

    /// Unpause all operations (single-admin helper; same effect as `unpause_all`)
    pub fn unpause_all_emergency(env: &Env) -> Result<(), GuardError> {
        let pause_state = PauseType::new(0);
        env.storage()
            .instance()
            .set(&GuardDataKey::PauseState, &pause_state);

        log!(env, "All operations unpaused");
        Ok(())
    }

    /// Check if a specific operation is paused
    pub fn is_operation_paused(env: &Env, operation: u32) -> bool {
        let pause_state: PauseType = env
            .storage()
            .instance()
            .get(&GuardDataKey::PauseState)
            .unwrap_or(PauseType::new(0));

        pause_state.is_paused(operation)
    }

    /// Pause a specific operation
    pub fn pause(env: &Env, operation: u32) -> Result<(), GuardError> {
        Self::set_pause_state(env, operation, true)
    }

    /// Public wrapper to validate a set of approvers against the stored threshold.
    pub fn validate_multi_sig(env: Env, approvers: Vec<Address>) -> Result<(), GuardError> {
        EmergencyGuard::check_multi_sig(&env, &approvers)
    }
}

/// Standard emergency-guard surface for host contracts embedding `EmergencyGuard` storage.
pub trait TokenEmergencyGuardTrait {
    fn guard_pause(e: Env, admin: Address, operation: u32, paused: bool) -> Result<(), GuardError>;
    fn guard_unpause(e: Env, approvers: Vec<Address>) -> Result<(), GuardError>;
    fn guard_is_paused(e: Env, operation: u32) -> bool;
    fn emergency_pause_all(e: Env, approvers: Vec<Address>) -> Result<(), GuardError>;
    fn resume_all(e: Env, approvers: Vec<Address>) -> Result<(), GuardError>;
    fn guard_add_admin(
        e: Env,
        approvers: Vec<Address>,
        new_admin: Address,
    ) -> Result<(), GuardError>;
    fn guard_remove_admin(
        e: Env,
        approvers: Vec<Address>,
        admin: Address,
    ) -> Result<(), GuardError>;
    fn guard_rotate_admin(
        e: Env,
        approvers: Vec<Address>,
        old_admin: Address,
        new_admin: Address,
    ) -> Result<(), GuardError>;
    fn guard_admins(e: Env) -> Vec<Address>;
    fn guard_threshold(e: Env) -> u32;
    fn guard_pause_state(e: Env) -> u32;
=======
>>>>>>> e2e49c2 (feature(emergency-rbac): separate Guardian and Admin roles in EmergencyGuard)
}

#[cfg(test)]
mod test;

pub trait EmergencyGuardTrait {
    fn check_not_paused(env: &Env, operation: u32) -> Result<(), GuardError>;
    fn get_pause_state(env: &Env) -> u32;
    fn set_pause_state(env: &Env, operation: u32, paused: bool) -> Result<(), GuardError>;
    fn unpause(env: &Env, operation: u32) -> Result<(), GuardError>;
    fn unpause_all(env: &Env) -> Result<(), GuardError>;
    fn emergency_pause_all(env: &Env, approvers: Vec<Address>) -> Result<(), GuardError>;
    fn resume_all(env: &Env, approvers: Vec<Address>) -> Result<(), GuardError>;
    fn init_guard(env: &Env, admins: Vec<Address>, threshold: u32) -> Result<(), GuardError>;
    fn add_admin(env: &Env, approvers: Vec<Address>, new_admin: Address) -> Result<(), GuardError>;
    fn remove_admin(env: &Env, approvers: Vec<Address>, admin: Address) -> Result<(), GuardError>;
    fn rotate_admin(
        env: &Env,
        approvers: Vec<Address>,
        old_admin: Address,
        new_admin: Address,
    ) -> Result<(), GuardError>;
    fn get_admins(env: &Env) -> Vec<Address>;
    fn get_threshold(env: &Env) -> u32;
    fn is_admin(env: &Env, addr: Address) -> bool;
}

pub struct DefaultEmergencyGuard;

impl DefaultEmergencyGuard {
    pub fn check_not_paused(env: &Env, operation: u32) -> Result<(), GuardError> {
        if EmergencyGuard::is_paused(env.clone(), operation) {
            Err(GuardError::Paused)
        } else {
            Ok(())
        }
    }
    pub fn get_pause_state(env: &Env) -> u32 {
        EmergencyGuard::get_pause_state(env.clone())
    }
    pub fn set_pause_state(env: &Env, operation: u32, paused: bool) -> Result<(), GuardError> {
        let guardians = EmergencyGuard::get_guardians(env.clone());
        let actor = guardians.get(0).or_else(|| EmergencyGuard::get_admins(env.clone()).get(0));
        if let Some(actor) = actor {
            EmergencyGuard::set_pause(env.clone(), actor, operation, paused)
        } else {
            Err(GuardError::Unauthorized)
        }
    }
    pub fn unpause(env: &Env, operation: u32) -> Result<(), GuardError> {
        let guardians = EmergencyGuard::get_guardians(env.clone());
        let actor = guardians.get(0).or_else(|| EmergencyGuard::get_admins(env.clone()).get(0));
        if let Some(actor) = actor {
            EmergencyGuard::set_pause(env.clone(), actor, operation, false)
        } else {
            Err(GuardError::Unauthorized)
        }
    }
    pub fn unpause_all(env: &Env) -> Result<(), GuardError> {
        let guardians = EmergencyGuard::get_guardians(env.clone());
        let actor = guardians.get(0).or_else(|| EmergencyGuard::get_admins(env.clone()).get(0));
        if let Some(actor) = actor {
            EmergencyGuard::set_pause(env.clone(), actor, u32::MAX, false)
        } else {
            Err(GuardError::Unauthorized)
        }
    }
    pub fn emergency_pause_all(env: &Env, approvers: Vec<Address>) -> Result<(), GuardError> {
        EmergencyGuard::emergency_pause(env.clone(), approvers)
    }
    pub fn resume_all(env: &Env, approvers: Vec<Address>) -> Result<(), GuardError> {
        EmergencyGuard::resume(env.clone(), approvers)
    }
    pub fn init_guard(env: &Env, admins: Vec<Address>, threshold: u32) -> Result<(), GuardError> {
        EmergencyGuard::initialize(env.clone(), admins, threshold)
    }
    pub fn add_admin(
        env: &Env,
        approvers: Vec<Address>,
        new_admin: Address,
    ) -> Result<(), GuardError> {
        EmergencyGuard::add_admin(env.clone(), approvers, new_admin)
    }
    pub fn remove_admin(
        env: &Env,
        approvers: Vec<Address>,
        admin: Address,
    ) -> Result<(), GuardError> {
        EmergencyGuard::remove_admin(env.clone(), approvers, admin)
    }
    pub fn rotate_admin(
        env: &Env,
        approvers: Vec<Address>,
        old_admin: Address,
        new_admin: Address,
    ) -> Result<(), GuardError> {
        EmergencyGuard::rotate_admin(env.clone(), approvers, old_admin, new_admin)
    }
    pub fn get_admins(env: &Env) -> Vec<Address> {
        EmergencyGuard::get_admins(env.clone())
    }
    pub fn get_threshold(env: &Env) -> u32 {
        EmergencyGuard::get_threshold(env.clone())
    }
    pub fn is_admin(env: &Env, addr: Address) -> bool {
        EmergencyGuard::is_admin_public(env.clone(), addr)
    }
}
=======
>>>>>>> main
