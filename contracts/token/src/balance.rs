use crate::storage_types::DataKey;
use soroban_sdk::{Address, Env};

pub fn read_balance(e: &Env, addr: Address) -> i128 {
    let key = DataKey::Balance(addr);
    e.storage()
        .persistent()
        .get::<DataKey, i128>(&key)
        .unwrap_or_default()
}

fn write_balance(e: &Env, addr: Address, amount: i128) {
    let key = DataKey::Balance(addr);
    e.storage().persistent().set(&key, &amount);
}

pub fn receive_balance(e: &Env, addr: Address, amount: i128) {
    let balance = read_balance(e, addr.clone());
    write_balance(e, addr, balance + amount); // Assumes no overflow for this example, but production should check
}

pub fn spend_balance(e: &Env, addr: Address, amount: i128) {
    let balance = read_balance(e, addr.clone());
    if balance < amount {
        panic!("insufficient balance");
    }
    write_balance(e, addr, balance - amount);
}

// ── Supply helpers ────────────────────────────────────────────────────────

pub fn read_total_supply(e: &Env) -> i128 {
    let key = DataKey::TotalSupply;
    e.storage().instance().get(&key).unwrap_or_default()
}

pub fn write_total_supply(e: &Env, amount: i128) {
    let key = DataKey::TotalSupply;
    e.storage().instance().set(&key, &amount);
}

pub fn has_max_supply(e: &Env) -> bool {
    let key = DataKey::MaxSupply;
    e.storage().instance().has(&key)
}

pub fn read_max_supply(e: &Env) -> i128 {
    let key = DataKey::MaxSupply;
    e.storage().instance().get(&key).unwrap()
}

pub fn write_max_supply(e: &Env, amount: i128) {
    let key = DataKey::MaxSupply;
    e.storage().instance().set(&key, &amount);
}
