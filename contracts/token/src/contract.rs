use crate::admin::{has_administrator, read_administrator, write_administrator};
use crate::allowance::{read_allowance, spend_allowance, write_allowance};
use crate::balance::{
    read_balance, read_max_supply, read_total_supply, receive_balance, spend_balance,
    write_max_supply, write_total_supply,
};
use crate::metadata::{read_decimal, read_name, read_symbol, write_metadata};
use emergency_guard::{EmergencyGuard, GuardError, PauseType};
use soroban_sdk::{contract, contractimpl, contracttype, vec, Address, Env, String, Vec};

fn require_not_paused(e: &Env, operation: u32) {
    if EmergencyGuard::is_paused(e.clone(), operation) {
        panic!("operation paused");
    }
}

pub trait TokenTrait {
    fn initialize(e: Env, admin: Address, decimal: u32, name: String, symbol: String, guardian: Address);
    fn initialize(e: Env, admin: Address, decimal: u32, name: String, symbol: String, max_supply: i128);
    fn mint(e: Env, to: Address, amount: i128);
    fn set_admin(e: Env, new_admin: Address);
    fn guard_pause(e: Env, caller: Address, operation: u32, paused: bool) -> Result<(), GuardError>;
    fn emergency_pause(e: Env, caller: Address) -> Result<(), GuardError>;
    fn guard_resume(e: Env, approvers: Vec<Address>) -> Result<(), GuardError>;
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
    fn guard_admins(e: Env) -> Vec<Address>;
    fn guard_threshold(e: Env) -> u32;
    fn guard_is_paused(e: Env, operation: u32) -> bool;
    fn allowance(e: Env, from: Address, spender: Address) -> i128;
    fn approve(e: Env, from: Address, spender: Address, amount: i128, expiration_ledger: u32);
    fn balance(e: Env, id: Address) -> i128;
    fn transfer(e: Env, from: Address, to: Address, amount: i128);
    fn transfer_from(e: Env, spender: Address, from: Address, to: Address, amount: i128);
    fn burn(e: Env, from: Address, amount: i128);
    fn burn_from(e: Env, spender: Address, from: Address, amount: i128);
    fn total_supply(e: Env) -> i128;
    fn max_supply(e: Env) -> i128;
    fn decimals(e: Env) -> u32;
    fn name(e: Env) -> String;
    fn symbol(e: Env) -> String;
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
    fn initialize(e: Env, admin: Address, decimal: u32, name: String, symbol: String, guardian: Address) {
    fn initialize(e: Env, admin: Address, decimal: u32, name: String, symbol: String, max_supply: i128) {
        if has_administrator(&e) {
            panic!("already initialized");
        }
        write_administrator(&e, &admin);
        EmergencyGuard::initialize(e.clone(), vec![&e, admin.clone()], 1, guardian)
            .expect("failed to initialize emergency guard");
        write_metadata(&e, &name, &symbol, decimal);
        write_max_supply(&e, max_supply);
    }

    fn mint(e: Env, to: Address, amount: i128) {
        require_not_paused(&e, PauseType::MINT);
        let admin = read_administrator(&e);
        admin.require_auth();
        e.storage().instance().extend_ttl(100, 100);

        let supply = read_total_supply(&e);
        let max_supply = read_max_supply(&e);
        if supply.checked_add(amount).is_none() || supply + amount > max_supply {
            panic!("max supply exceeded");
        }

        receive_balance(&e, to, amount);
        write_total_supply(&e, supply + amount);
    }

    fn set_admin(e: Env, new_admin: Address) {
        let admin = read_administrator(&e);
        admin.require_auth();
        e.storage().instance().extend_ttl(100, 100);

        EmergencyGuard::add_admin(e.clone(), vec![&e, admin.clone()], new_admin.clone())
            .expect("failed to add token admin");
        EmergencyGuard::remove_admin(e.clone(), vec![&e, admin.clone()], admin)
            .expect("failed to remove old token admin");
        write_administrator(&e, &new_admin);
    }

    fn guard_pause(e: Env, caller: Address, operation: u32, paused: bool) -> Result<(), GuardError> {
        EmergencyGuard::set_pause(e, caller, operation, paused)
    }

    fn emergency_pause(e: Env, caller: Address) -> Result<(), GuardError> {
        EmergencyGuard::emergency_pause(e, caller)
    }

    fn guard_resume(e: Env, approvers: Vec<Address>) -> Result<(), GuardError> {
        EmergencyGuard::resume(e, approvers)
    }

    fn guard_add_admin(
        e: Env,
        approvers: Vec<Address>,
        new_admin: Address,
    ) -> Result<(), GuardError> {
        EmergencyGuard::add_admin(e, approvers, new_admin)
    }

    fn guard_remove_admin(
        e: Env,
        approvers: Vec<Address>,
        admin: Address,
    ) -> Result<(), GuardError> {
        EmergencyGuard::remove_admin(e, approvers, admin)
    }

    fn guard_admins(e: Env) -> Vec<Address> {
        EmergencyGuard::get_admins(e)
    }

    fn guard_threshold(e: Env) -> u32 {
        EmergencyGuard::get_threshold(e)
    }

    fn guard_is_paused(e: Env, operation: u32) -> bool {
        EmergencyGuard::is_paused(e, operation)
    }

    fn allowance(e: Env, from: Address, spender: Address) -> i128 {
        e.storage().instance().extend_ttl(100, 100);
        read_allowance(&e, from, spender).amount
    }

    fn approve(e: Env, from: Address, spender: Address, amount: i128, expiration_ledger: u32) {
        from.require_auth();
        e.storage().instance().extend_ttl(100, 100);

        write_allowance(&e, from, spender, amount, expiration_ledger);
    }

    fn balance(e: Env, id: Address) -> i128 {
        e.storage().instance().extend_ttl(100, 100);
        read_balance(&e, id)
    }

    fn transfer(e: Env, from: Address, to: Address, amount: i128) {
        require_not_paused(&e, PauseType::TRANSFER);
        from.require_auth();
        e.storage().instance().extend_ttl(100, 100);

        spend_balance(&e, from, amount);
        receive_balance(&e, to, amount);
    }

    fn transfer_from(e: Env, spender: Address, from: Address, to: Address, amount: i128) {
        require_not_paused(&e, PauseType::TRANSFER);
        spender.require_auth();
        e.storage().instance().extend_ttl(100, 100);

        spend_allowance(&e, from.clone(), spender, amount);
        spend_balance(&e, from, amount);
        receive_balance(&e, to, amount);
    }

    fn burn(e: Env, from: Address, amount: i128) {
        require_not_paused(&e, PauseType::BURN);
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
        spend_balance(&e, from, amount);
        write_total_supply(&e, read_total_supply(&e) - amount);
    }

    fn burn_from(e: Env, spender: Address, from: Address, amount: i128) {
        require_not_paused(&e, PauseType::BURN);
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
        spend_allowance(&e, from.clone(), spender, amount);
        spend_balance(&e, from, amount);
        write_total_supply(&e, read_total_supply(&e) - amount);
    }

    fn total_supply(e: Env) -> i128 {
        e.storage().instance().extend_ttl(100, 100);
        read_total_supply(&e)

    fn max_supply(e: Env) -> i128 {
        read_max_supply(&e)
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
}
