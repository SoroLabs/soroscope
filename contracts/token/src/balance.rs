use soroban_sdk::{Address, Env};
use crate::storage_types::DataKey;

pub fn read_balance(e: &Env, addr: Address) -> i128 {
    let key = DataKey::Balance(addr);
    match e.storage().persistent().get::<DataKey, i128>(&key) {
        Some(balance) => balance,
        None => 0,
    }
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
