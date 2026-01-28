#![no_std]
use soroban_sdk::{contract, contractimpl, Bytes, Env, Symbol, Vec};

#[cfg(test)]
mod test;

#[contract]
pub struct StorageHeavyContract;

#[contractimpl]
impl StorageHeavyContract {
    /// Writes a single large entry to persistent storage
    pub fn write_persistent(env: Env, key: Symbol, data: Bytes) {
        env.storage().persistent().set(&key, &data);
    }

    /// Writes a single large entry to temporary storage
    pub fn write_temporary(env: Env, key: Symbol, data: Bytes) {
        env.storage().temporary().set(&key, &data);
    }

    /// Reads a large entry from persistent storage
    pub fn read_persistent(env: Env, key: Symbol) -> Bytes {
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or(Bytes::new(&env))
    }

    /// Reads a large entry from temporary storage
    pub fn read_temporary(env: Env, key: Symbol) -> Bytes {
        env.storage()
            .temporary()
            .get(&key)
            .unwrap_or(Bytes::new(&env))
    }

    /// Batch write to persistent storage to test write limits
    pub fn batch_write_persistent(env: Env, keys: Vec<Symbol>, data_points: Vec<Bytes>) {
        if keys.len() != data_points.len() {
            panic!("Keys and data_points must have the same length");
        }
        for i in 0..keys.len() {
            env.storage()
                .persistent()
                .set(&keys.get(i).unwrap(), &data_points.get(i).unwrap());
        }
    }

    /// Batch write to temporary storage to test write limits
    pub fn batch_write_temporary(env: Env, keys: Vec<Symbol>, data_points: Vec<Bytes>) {
        if keys.len() != data_points.len() {
            panic!("Keys and data_points must have the same length");
        }
        for i in 0..keys.len() {
            env.storage()
                .temporary()
                .set(&keys.get(i).unwrap(), &data_points.get(i).unwrap());
        }
    }
}
