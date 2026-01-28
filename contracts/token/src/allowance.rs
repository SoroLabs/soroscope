use crate::storage_types::{AllowanceDataKey, AllowanceValue, DataKey};
use soroban_sdk::{Address, Env};

pub fn read_allowance(e: &Env, from: Address, spender: Address) -> AllowanceValue {
    let key = DataKey::Allowance(AllowanceDataKey { from, spender });
    match e.storage().temporary().get::<_, AllowanceValue>(&key) {
        Some(allowance) => allowance,
        None => AllowanceValue {
            amount: 0,
            expiration_ledger: 0,
        },
    }
}

pub fn write_allowance(e: &Env, from: Address, spender: Address, amount: i128, expiration_ledger: u32) {
    let key = DataKey::Allowance(AllowanceDataKey { from, spender });
    let allowance = AllowanceValue {
        amount,
        expiration_ledger,
    };

    if amount > 0 {
        e.storage().temporary().set(&key, &allowance);
        // In newer Soroban versions, we might need to extend TTL, but for this basic logic we just set it.
        // Assuming standard bump logic handles it or it's manual.
    } else {
        e.storage().temporary().remove(&key);
    }
}

pub fn spend_allowance(e: &Env, from: Address, spender: Address, amount: i128) {
    let allowance = read_allowance(e, from.clone(), spender.clone());
    if allowance.amount < amount {
        panic!("insufficient allowance");
    }
    if allowance.expiration_ledger < e.ledger().sequence() {
        panic!("allowance expired");
    }
    write_allowance(e, from, spender, allowance.amount - amount, allowance.expiration_ledger);
}
