#![no_std]
use emergency_guard::{EmergencyGuard, PauseType};
use soroban_sdk::{contract, contractimpl, contracttype, vec, Address, Env};

#[contracttype]
pub enum DataKey {
    Admin,
    TotalSupply,
    Balance(Address),
    Allowance(AllowanceKey),
}

#[contracttype]
pub struct AllowanceKey {
    from: Address,
    to: Address,
}

#[contract]
pub struct SimpleToken;

#[contractimpl]
impl SimpleToken {
    pub fn initialize(env: Env, admin: Address, initial_supply: i128) {
        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::TotalSupply, &initial_supply);
        env.storage()
            .instance()
            .set(&DataKey::Balance(admin.clone()), &initial_supply);

        let admins = vec![&env, admin];
        EmergencyGuard::initialize(env.clone(), admins, 1).expect("Failed to init guard");
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        if EmergencyGuard::is_paused(env.clone(), PauseType::TRANSFER) {
            panic!("Transfers are paused");
        }

        from.require_auth();

        let balance: i128 = env
            .storage()
            .instance()
            .get(&DataKey::Balance(from.clone()))
            .unwrap_or(0);
        assert!(balance >= amount, "Insufficient balance");

        env.storage()
            .instance()
            .set(&DataKey::Balance(from.clone()), &(balance - amount));

        let to_balance: i128 = env
            .storage()
            .instance()
            .get(&DataKey::Balance(to.clone()))
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::Balance(to), &(to_balance + amount));
    }

    /// Mint tokens (blocked if MINT pause is active)
    pub fn mint(env: Env, to: Address, amount: i128) {
        // Check if minting is paused
        EmergencyGuard::check_not_paused(&env, PauseType::MINT).expect("Minting is paused");

        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not found");

        admin.require_auth();

        let balance: i128 = env
            .storage()
            .instance()
            .get(&DataKey::Balance(to.clone()))
            .unwrap_or(0);

        env.storage()
            .instance()
            .set(&DataKey::Balance(to), &(balance + amount));

        let supply: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0);

        env.storage()
            .instance()
            .set(&DataKey::TotalSupply, &(supply + amount));
    }

    /// Burn tokens (blocked if BURN pause is active)
    pub fn burn(env: Env, from: Address, amount: i128) {
        // Check if burning is paused
        EmergencyGuard::check_not_paused(&env, PauseType::BURN).expect("Burning is paused");

        from.require_auth();

        let balance: i128 = env
            .storage()
            .instance()
            .get(&DataKey::Balance(from.clone()))
            .unwrap_or(0);

        assert!(balance >= amount, "Insufficient balance");

        env.storage()
            .instance()
            .set(&DataKey::Balance(from), &(balance - amount));

        let supply: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0);

        env.storage()
            .instance()
            .set(&DataKey::TotalSupply, &(supply - amount));
    }

    // ==== EMERGENCY GUARD FUNCTIONS ====

    /// Pause only transfers (minting and burning still work)
    pub fn pause_transfers(env: Env) {
        EmergencyGuard::set_pause_state(&env, PauseType::TRANSFER, true)
            .expect("Unauthorized");
    }

    /// Resume transfers
    pub fn resume_transfers(env: Env) {
        EmergencyGuard::unpause(&env, PauseType::TRANSFER)
            .expect("Unauthorized");
    }

    /// Pause only minting
    pub fn pause_minting(env: Env) {
        EmergencyGuard::set_pause_state(&env, PauseType::MINT, true).expect("Unauthorized");
    }

    /// Resume minting
    pub fn resume_minting(env: Env) {
        EmergencyGuard::unpause(&env, PauseType::MINT).expect("Unauthorized");
    }

    /// Pause only burning
    pub fn pause_burning(env: Env) {
        EmergencyGuard::set_pause_state(&env, PauseType::BURN, true).expect("Unauthorized");
    }

    /// Resume burning
    pub fn resume_burning(env: Env) {
        EmergencyGuard::unpause(&env, PauseType::BURN).expect("Unauthorized");
    }

    /// Emergency: pause all operations
    pub fn emergency_pause_all(env: Env) {
        EmergencyGuard::emergency_pause_all(&env).expect("Unauthorized");
    }

    /// Resume all operations
    pub fn resume_all(env: Env) {
        EmergencyGuard::unpause_all(&env).expect("Unauthorized");
    }

    /// Get current pause state (bitmask)
    pub fn get_pause_state(env: Env) -> u32 {
        EmergencyGuard::get_pause_state(&env)
    }

    /// Check if specific operation is paused
    pub fn is_paused(env: Env, operation: u32) -> bool {
        let state = EmergencyGuard::get_pause_state(&env);
        let pause_type = PauseType::new(state);
        pause_type.is_paused(operation)
    }

    /// Get list of admins
    pub fn get_admins(env: Env) -> Vec<Address> {
        EmergencyGuard::get_admins(&env)
    }

    /// Get multi-sig threshold
    pub fn get_threshold(env: Env) -> u32 {
        EmergencyGuard::get_threshold(&env)
    }

    /// Add new admin (requires existing admin authorization)
    pub fn add_admin(env: Env, new_admin: Address) {
        EmergencyGuard::add_admin(&env, new_admin)
            .expect("Unauthorized or threshold would be violated");
    }

    /// Remove admin
    pub fn remove_admin(env: Env, admin: Address) {
        EmergencyGuard::remove_admin(&env, admin)
            .expect("Unauthorized or threshold would be violated");
    }

    /// Rotate admin (current admin transfers authority to new admin)
    pub fn rotate_admin(env: Env, new_admin: Address) {
        EmergencyGuard::rotate_admin(&env, new_admin).expect("Unauthorized");
    }

    // ==== READ-ONLY FUNCTIONS ====

    /// Get token balance for an address
    pub fn balance(env: Env, addr: Address) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::Balance(addr))
            .unwrap_or(0)
    }

    /// Get total supply
    pub fn total_supply(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0)
    }
}

#[cfg(not(target_family = "wasm"))]
fn main() {}
