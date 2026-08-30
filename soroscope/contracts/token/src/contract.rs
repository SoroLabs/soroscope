#![allow(dead_code)]

use crate::admin::{has_administrator, read_administrator, write_administrator};
use crate::allowance::{read_allowance, spend_allowance, write_allowance};
use crate::balance::{read_balance, receive_balance, spend_balance};
use crate::metadata::{read_decimal, read_name, read_symbol, write_metadata};
use emergency_guard::{EmergencyGuard, GuardError, PauseType};
use soroban_sdk::{contract, contractimpl, contracttype, vec, Address, Env, String, Vec};

fn ensure_not_paused(e: &Env, operation: u32) {
    if EmergencyGuard::is_paused(e.clone(), operation) {
        panic!("operation paused");
    }
}

pub trait TokenTrait {
    fn initialize(e: Env, admin: Address, decimal: u32, name: String, symbol: String);
    fn mint(e: Env, to: Address, amount: i128);
    fn set_admin(e: Env, new_admin: Address);
    fn allowance(e: Env, from: Address, spender: Address) -> i128;
    fn approve(e: Env, from: Address, spender: Address, amount: i128, expiration_ledger: u32);
    fn balance(e: Env, id: Address) -> i128;
    fn transfer(e: Env, from: Address, to: Address, amount: i128);
    fn transfer_from(e: Env, spender: Address, from: Address, to: Address, amount: i128);
    fn burn(e: Env, from: Address, amount: i128);
    fn burn_from(e: Env, spender: Address, from: Address, amount: i128);
    fn decimals(e: Env) -> u32;
    fn name(e: Env) -> String;
    fn symbol(e: Env) -> String;
    fn guard_pause(e: Env, admin: Address, operation: u32, paused: bool) -> Result<(), GuardError>;
    fn guard_unpause(e: Env, approvers: Vec<Address>) -> Result<(), GuardError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BurnEvent {
    pub burner: Address,
    pub target_account: Address,
    pub amount: i128,
}

#[contract]
pub struct Token;

#[contractimpl]
impl TokenTrait for Token {
    fn initialize(e: Env, admin: Address, decimal: u32, name: String, symbol: String) {
        if has_administrator(&e) {
            panic!("already initialized");
        }
        write_administrator(&e, &admin);
        write_metadata(&e, &name, &symbol, decimal);
        // Initialize EmergencyGuard with the token admin as the sole guard admin.
        // Pause state is stored in GuardDataKey::PauseState (instance storage bitmask).
        let admins: Vec<Address> = vec![&e, admin];
        EmergencyGuard::initialize(e, admins, 1).expect("failed to initialize emergency guard");
    }

    fn mint(e: Env, to: Address, amount: i128) {
        ensure_not_paused(&e, PauseType::MINT);
        let admin = read_administrator(&e);
        admin.require_auth();
        e.storage().instance().extend_ttl(100, 100);
        receive_balance(&e, to, amount);
    }

    fn set_admin(e: Env, new_admin: Address) {
        let admin = read_administrator(&e);
        admin.require_auth();
        e.storage().instance().extend_ttl(100, 100);
        // Use EmergencyGuard multi-sig to rotate the guard admin list, then update token admin.
        EmergencyGuard::rotate_admin(
            e.clone(),
            vec![&e, admin.clone()],
            admin,
            new_admin.clone(),
        )
        .expect("failed to rotate admin via EmergencyGuard");
        write_administrator(&e, &new_admin);
    }

    fn allowance(e: Env, from: Address, spender: Address) -> i128 {
        e.storage().instance().extend_ttl(100, 100);
        read_allowance(&e, from, spender).amount
    }

    fn approve(e: Env, from: Address, spender: Address, amount: i128, expiration_ledger: u32) {
        // Approvals follow the transfer pause flag.
        ensure_not_paused(&e, PauseType::TRANSFER);
        from.require_auth();
        e.storage().instance().extend_ttl(100, 100);
        write_allowance(&e, from, spender, amount, expiration_ledger);
    }

    fn balance(e: Env, id: Address) -> i128 {
        e.storage().instance().extend_ttl(100, 100);
        read_balance(&e, id)
    }

    fn transfer(e: Env, from: Address, to: Address, amount: i128) {
        // Issue #440: check PauseType::TRANSFER to block transfers during emergency.
        ensure_not_paused(&e, PauseType::TRANSFER);
        from.require_auth();
        e.storage().instance().extend_ttl(100, 100);
        spend_balance(&e, from, amount);
        receive_balance(&e, to, amount);
    }

    fn transfer_from(e: Env, spender: Address, from: Address, to: Address, amount: i128) {
        // Delegated transfers respect the same TRANSFER pause flag.
        ensure_not_paused(&e, PauseType::TRANSFER);
        spender.require_auth();
        e.storage().instance().extend_ttl(100, 100);
        spend_allowance(&e, from.clone(), spender, amount);
        spend_balance(&e, from, amount);
        receive_balance(&e, to, amount);
    }

    fn burn(e: Env, from: Address, amount: i128) {
        ensure_not_paused(&e, PauseType::BURN);
        from.require_auth();
        e.storage().instance().extend_ttl(100, 100);

        spend_balance(&e, from.clone(), amount);

        e.events().publish(
            (String::from_str(&e, "burn"), from.clone()),
            BurnEvent {
                burner: from.clone(),
                target_account: from,
                amount,
            },
        );
    }

    fn burn_from(e: Env, spender: Address, from: Address, amount: i128) {
        ensure_not_paused(&e, PauseType::BURN);
        spender.require_auth();
        e.storage().instance().extend_ttl(100, 100);

        spend_allowance(&e, from.clone(), spender.clone(), amount);
        spend_balance(&e, from.clone(), amount);

        e.events().publish(
            (String::from_str(&e, "burn"), from.clone()),
            BurnEvent {
                burner: spender,
                target_account: from,
                amount,
            },
        );
    }

    fn decimals(e: Env) -> u32 {
        read_decimal(&e)
    }

    fn name(e: Env) -> String {
        read_name(&e)
    }

    fn symbol(e: Env) -> String {
        read_symbol(&e)
    }

    fn guard_pause(e: Env, admin: Address, operation: u32, paused: bool) -> Result<(), GuardError> {
        EmergencyGuard::set_pause(e, admin, operation, paused)
    }

    fn guard_unpause(e: Env, approvers: Vec<Address>) -> Result<(), GuardError> {
        EmergencyGuard::resume(e, approvers)
    }
}

/// Additional guard-management helpers exposed on the token contract.
#[contractimpl]
impl Token {
    pub fn pause_minting(e: Env, admin: Address) {
        EmergencyGuard::set_pause(e, admin, PauseType::MINT, true)
            .expect("unauthorized: caller is not a guard admin");
    }

    pub fn resume_minting(e: Env, admin: Address) {
        EmergencyGuard::set_pause(e, admin, PauseType::MINT, false).expect("unauthorized");
    }

    pub fn pause_transfers(e: Env, admin: Address) {
        EmergencyGuard::set_pause(e, admin, PauseType::TRANSFER, true)
            .expect("unauthorized: caller is not a guard admin");
    }

    pub fn resume_transfers(e: Env, admin: Address) {
        EmergencyGuard::set_pause(e, admin, PauseType::TRANSFER, false).expect("unauthorized");
    }

    pub fn pause_burning(e: Env, admin: Address) {
        EmergencyGuard::set_pause(e, admin, PauseType::BURN, true)
            .expect("unauthorized: caller is not a guard admin");
    }

    pub fn resume_burning(e: Env, admin: Address) {
        EmergencyGuard::set_pause(e, admin, PauseType::BURN, false).expect("unauthorized");
    }

    pub fn emergency_pause_all(e: Env, approvers: Vec<Address>) {
        EmergencyGuard::emergency_pause(e, approvers)
            .expect("unauthorized or insufficient approvals");
    }

    pub fn resume_all(e: Env, approvers: Vec<Address>) {
        EmergencyGuard::resume(e, approvers).expect("unauthorized or insufficient approvals");
    }

    pub fn get_pause_state(e: Env) -> u32 {
        EmergencyGuard::get_pause_state(e)
    }

    pub fn is_operation_paused(e: Env, operation: u32) -> bool {
        EmergencyGuard::is_paused(e, operation)
    }

    pub fn get_guard_admins(e: Env) -> Vec<Address> {
        EmergencyGuard::get_admins(e)
    }

    pub fn get_guard_threshold(e: Env) -> u32 {
        EmergencyGuard::get_threshold(e)
    }
}
