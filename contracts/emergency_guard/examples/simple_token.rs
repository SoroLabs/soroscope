#![no_std]
use emergency_guard::{EmergencyGuard, PauseType};
use soroban_sdk::{contract, contractimpl, contracttype, vec, Address, Env, Vec};

#[contracttype]
pub enum DataKey {
    Admin,
    TotalSupply,
    Balance(Address),
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

    pub fn mint(env: Env, to: Address, amount: i128) {
        if EmergencyGuard::is_paused(env.clone(), PauseType::MINT) {
            panic!("Minting is paused");
        }
        // Check if minting is paused
        if EmergencyGuard::is_paused(env.clone(), PauseType::MINT) {
            panic!("Minting is paused");
        }

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

    pub fn burn(env: Env, from: Address, amount: i128) {
        if EmergencyGuard::is_paused(env.clone(), PauseType::BURN) {
            panic!("Burning is paused");
        }
        // Check if burning is paused
        if EmergencyGuard::is_paused(env.clone(), PauseType::BURN) {
            panic!("Burning is paused");
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
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not found");
        EmergencyGuard::set_pause(env, admin, PauseType::TRANSFER, true).expect("Unauthorized");
    }

    /// Resume transfers
    pub fn resume_transfers(env: Env) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not found");
        EmergencyGuard::set_pause(env, admin, PauseType::TRANSFER, false).expect("Unauthorized");
    }

    /// Pause only minting
    pub fn pause_minting(env: Env) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not found");
        EmergencyGuard::set_pause(env, admin, PauseType::MINT, true).expect("Unauthorized");
    }

    /// Resume minting
    pub fn resume_minting(env: Env) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not found");
        EmergencyGuard::set_pause(env, admin, PauseType::MINT, false).expect("Unauthorized");
    }

    /// Pause only burning
    pub fn pause_burning(env: Env) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not found");
        EmergencyGuard::set_pause(env, admin, PauseType::BURN, true).expect("Unauthorized");
    }

    /// Resume burning
    pub fn resume_burning(env: Env) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not found");
        EmergencyGuard::set_pause(env, admin, PauseType::BURN, false).expect("Unauthorized");
    }

    /// Emergency: pause all operations
    pub fn emergency_pause_all(env: Env) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not found");
        EmergencyGuard::emergency_pause(env.clone(), vec![&env, admin]).expect("Unauthorized");
    }

    /// Resume all operations
    pub fn resume_all(env: Env) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not found");
        EmergencyGuard::resume(env.clone(), vec![&env, admin]).expect("Unauthorized");
    }

    /// Get current pause state (bitmask)
    pub fn get_pause_state(env: Env) -> u32 {
        EmergencyGuard::get_pause_state(env)
    }

    pub fn is_paused(env: Env, operation: u32) -> bool {
        EmergencyGuard::is_paused(env, operation)
    }

    pub fn get_admins(env: Env) -> Vec<Address> {
        EmergencyGuard::get_admins(env)
    }

    pub fn get_threshold(env: Env) -> u32 {
        EmergencyGuard::get_threshold(env)
    }

    /// Add new admin (requires existing admin authorization)
    pub fn add_admin(env: Env, new_admin: Address) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not found");
        EmergencyGuard::add_admin(env.clone(), vec![&env, admin], new_admin)
            .expect("Unauthorized or threshold would be violated");
    }

    /// Remove admin
    pub fn remove_admin(env: Env, admin: Address) {
        let approver: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not found");
        EmergencyGuard::remove_admin(env.clone(), vec![&env, approver], admin)
            .expect("Unauthorized or threshold would be violated");
    }

    /// Rotate admin (current admin transfers authority to new admin)
    pub fn rotate_admin(env: Env, new_admin: Address) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Admin not found");
        EmergencyGuard::rotate_admin(env.clone(), vec![&env, admin.clone()], admin, new_admin)
            .expect("Unauthorized");
    }

    // ==== READ-ONLY FUNCTIONS ====

    /// Get token balance for an address
    pub fn balance(env: Env, addr: Address) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::Balance(addr))
            .unwrap_or(0)
    }

    pub fn total_supply(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0)
    }
}

#[cfg(not(target_family = "wasm"))]
fn main() {}
