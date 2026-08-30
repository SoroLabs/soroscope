#![no_std]
use soroban_sdk::{contract, contractimpl, Bytes, Env, Symbol, Vec, Map};

#[cfg(test)]
mod test;

#[contract]
pub struct StorageHeavyContract;

#[contractimpl]
impl StorageHeavyContract {
    /// Writes a single large entry to persistent storage.
    pub fn write_persistent(env: Env, key: Symbol, data: Bytes) {
        env.storage().persistent().set(&key, &data);
    }

    /// Writes a single large entry to temporary storage.
    pub fn write_temporary(env: Env, key: Symbol, data: Bytes) {
        env.storage().temporary().set(&key, &data);
    }

    /// Reads a large entry from persistent storage.
    pub fn read_persistent(env: Env, key: Symbol) -> Bytes {
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or(Bytes::new(&env))
    }

    /// Reads a large entry from temporary storage.
    pub fn read_temporary(env: Env, key: Symbol) -> Bytes {
        env.storage()
            .temporary()
            .get(&key)
            .unwrap_or(Bytes::new(&env))
    }

    /// Extends the lifetime of a persistent storage entry.
    ///
    /// The TTL is changed to `extend_to` ledgers only when its current value is
    /// below `threshold`. This method intentionally requires no authorization,
    /// allowing any caller to pay the network rent needed to keep an entry
    /// available.
    pub fn extend_ttl(env: Env, key: Symbol, threshold: u32, extend_to: u32) {
        env.storage()
            .persistent()
            .extend_ttl(&key, threshold, extend_to);
    }
    /// Batch-write to persistent storage.
    /// Demonstrates the cost of N separate ledger-entry writes.
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

    /// Batch-write to temporary storage.
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

    /// Batch-read from persistent storage.
    ///
    /// Complements `batch_write_persistent` to demonstrate the grouped-access
    /// pattern: a single invocation pays one base transaction fee and one
    /// ledger-footprint entry per key, rather than N separate invocations each
    /// paying their own base fee.  Compare this against calling `read_persistent`
    /// N times to see the per-call overhead savings.
    pub fn batch_read_persistent(env: Env, keys: Vec<Symbol>) -> Vec<Bytes> {
        let mut results = Vec::new(&env);
        for i in 0..keys.len() {
            let value = env
                .storage()
                .persistent()
                .get(&keys.get(i).unwrap())
                .unwrap_or(Bytes::new(&env));
            results.push_back(value);
        }
        results
    }

    /// Batch-read from temporary storage.
    ///
    /// Same grouped-access pattern as `batch_read_persistent` but for
    /// temporary storage entries.
    pub fn batch_read_temporary(env: Env, keys: Vec<Symbol>) -> Vec<Bytes> {
        let mut results = Vec::new(&env);
        for i in 0..keys.len() {
            let value = env
                .storage()
                .temporary()
                .get(&keys.get(i).unwrap())
                .unwrap_or(Bytes::new(&env));
            results.push_back(value);
        }
        results
    }

    /// Write separate boolean values using individual keys in persistent storage.
    pub fn write_separate_booleans(env: Env, keys: Vec<Symbol>, values: Vec<bool>) {
        if keys.len() != values.len() {
            panic!("Keys and values must have the same length");
        }
        for i in 0..keys.len() {
            env.storage()
                .persistent()
                .set(&keys.get(i).unwrap(), &values.get(i).unwrap());
        }
    }

    /// Read separate boolean values using individual keys from persistent storage.
    pub fn read_separate_booleans(env: Env, keys: Vec<Symbol>) -> Vec<bool> {
        let mut results = Vec::new(&env);
        for i in 0..keys.len() {
            let value = env
                .storage()
                .persistent()
                .get(&keys.get(i).unwrap())
                .unwrap_or(false);
            results.push_back(value);
        }
        results
    }

    /// Write packed boolean values as bits in a single u32 entry in persistent storage.
    pub fn write_packed_booleans(env: Env, key: Symbol, values: Vec<bool>) {
        if values.len() > 32 {
            panic!("Cannot pack more than 32 boolean values into a u32");
        }
        let mut mask: u32 = 0;
        for i in 0..values.len() {
            if values.get(i).unwrap() {
                mask |= 1 << i;
            }
        }
        env.storage().persistent().set(&key, &mask);
    }

    /// Read packed boolean values from a single u32 entry in persistent storage.
    pub fn read_packed_booleans(env: Env, key: Symbol, len: u32) -> Vec<bool> {
        if len > 32 {
            panic!("Cannot unpack more than 32 boolean values from a u32");
        }
        let mask: u32 = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(0u32);
        
        let mut results = Vec::new(&env);
        for i in 0..len {
            results.push_back((mask & (1 << i)) != 0);
        }
        results
    }

    /// Update a single boolean flag in separate storage slots.
    pub fn update_separate_boolean(env: Env, key: Symbol, value: bool) {
        env.storage().persistent().set(&key, &value);
    }

    /// Update a single boolean flag in a packed u32 storage slot.
    pub fn update_packed_boolean(env: Env, key: Symbol, flag_idx: u32, value: bool) {
        if flag_idx >= 32 {
            panic!("Flag index out of range for u32");
        }
        let mut mask: u32 = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(0u32);

        if value {
            mask |= 1 << flag_idx;
        } else {
            mask &= !(1 << flag_idx);
        }
        env.storage().persistent().set(&key, &mask);
    }

    /// Write boolean values using a Map<u32, bool> in persistent storage.
    pub fn write_map_booleans(env: Env, key: Symbol, values: Vec<bool>) {
        let mut map: Map<u32, bool> = Map::new(&env);
        for i in 0..values.len() {
            map.set(i, values.get(i).unwrap());
        }
        env.storage().persistent().set(&key, &map);
    }

    /// Read boolean values using a Map<u32, bool> from persistent storage.
    pub fn read_map_booleans(env: Env, key: Symbol, len: u32) -> Vec<bool> {
        let map: Map<u32, bool> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Map::new(&env));
        
        let mut results = Vec::new(&env);
        for i in 0..len {
            results.push_back(map.get(i).unwrap_or(false));
        }
        results
    }

    /// Update a single boolean flag in a Map<u32, bool> stored under a single key.
    pub fn update_map_boolean(env: Env, key: Symbol, flag_idx: u32, value: bool) {
        let mut map: Map<u32, bool> = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| Map::new(&env));
        map.set(flag_idx, value);
        env.storage().persistent().set(&key, &map);
    }
}

