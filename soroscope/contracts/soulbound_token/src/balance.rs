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
    if balance + amount > 1 {
        panic!("cannot hold more than one soulbound token");
    }
    write_balance(e, addr, balance + amount);
}

pub fn spend_balance(e: &Env, addr: Address, amount: i128) {
    let balance = read_balance(e, addr.clone());
    if balance < amount {
        panic!("insufficient balance");
    }
    write_balance(e, addr, balance - amount);
}