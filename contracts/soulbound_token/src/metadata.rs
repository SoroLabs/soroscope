use crate::storage_types::DataKey;
use soroban_sdk::{Address, Env, String};

pub fn read_decimal(e: &Env) -> u32 {
    let key = DataKey::Decimals;
    e.storage().instance().get(&key).unwrap()
}

pub fn write_decimal(e: &Env, d: u32) {
    let key = DataKey::Decimals;
    e.storage().instance().set(&key, &d);
}

pub fn read_name(e: &Env) -> String {
    let key = DataKey::Name;
    e.storage().instance().get(&key).unwrap()
}

pub fn write_name(e: &Env, name: &String) {
    let key = DataKey::Name;
    e.storage().instance().set(&key, name);
}

pub fn read_symbol(e: &Env) -> String {
    let key = DataKey::Symbol;
    e.storage().instance().get(&key).unwrap()
}

pub fn write_symbol(e: &Env, symbol: &String) {
    let key = DataKey::Symbol;
    e.storage().instance().set(&key, symbol);
}

pub fn read_identity_metadata(e: &Env, owner: Address) -> String {
    let key = DataKey::IdentityMetadata(owner);
    e.storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| String::from_str(e, ""))
}

pub fn write_identity_metadata(e: &Env, owner: Address, metadata: &String) {
    let key = DataKey::IdentityMetadata(owner);
    e.storage().persistent().set(&key, metadata);
}

pub fn remove_identity_metadata(e: &Env, owner: Address) {
    let key = DataKey::IdentityMetadata(owner);
    e.storage().persistent().remove(&key);
}
