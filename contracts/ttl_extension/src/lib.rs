#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Bytes, Env, Symbol,
};

const KEY: Symbol = symbol_short!("KEY");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum StorageHeavyError {
    NotFound = 1,
    InvalidPayload = 2,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    BatchFieldA(Address),
    BatchFieldB(Address),
    BatchFieldC(Address),
    BatchFieldD(Address),
    CompactBatch(Address),
}

/// Encodes `(a, b, c, d)` into a compact 16-byte `Bytes` buffer using little-endian integers.
pub fn encode_compact_batch(env: &Env, a: u32, b: i32, c: u32, d: u32) -> Bytes {
    let mut raw = [0u8; 16];
    raw[0..4].copy_from_slice(&a.to_le_bytes());
    raw[4..8].copy_from_slice(&b.to_le_bytes());
    raw[8..12].copy_from_slice(&c.to_le_bytes());
    raw[12..16].copy_from_slice(&d.to_le_bytes());
    Bytes::from_slice(env, &raw)
}

/// Decodes a compact `Bytes` buffer back into `(a, b, c, d)`.
pub fn decode_compact_batch(bytes: &Bytes) -> Result<(u32, i32, u32, u32), StorageHeavyError> {
    if bytes.len() != 16 {
        return Err(StorageHeavyError::InvalidPayload);
    }

    let mut raw = [0u8; 16];
    bytes.copy_into_slice(&mut raw);

    let a = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
    let b = i32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]);
    let c = u32::from_le_bytes([raw[8], raw[9], raw[10], raw[11]]);
    let d = u32::from_le_bytes([raw[12], raw[13], raw[14], raw[15]]);

    Ok((a, b, c, d))
}

#[contract]
pub struct TtlExtension;

#[contractimpl]
impl TtlExtension {
    pub fn extend_ttl(env: Env, threshold: u32, extend_to: u32) {
        let instance_ttl = env.storage().instance().get_ttl();
        if instance_ttl < threshold {
            env.storage().instance().extend_ttl(threshold, extend_to);
        }
        let persistent_ttl = env.storage().persistent().get_ttl(&KEY);
        if persistent_ttl < threshold {
            env.storage()
                .persistent()
                .extend_ttl(&KEY, threshold, extend_to);
        }
    }

    pub fn write(env: Env, value: u32) {
        env.storage().persistent().set(&KEY, &value);
    }

    pub fn persistent_ttl(env: Env) -> u32 {
        env.storage().persistent().get_ttl(&KEY)
    }

    pub fn instance_ttl(env: Env) -> u32 {
        env.storage().instance().get_ttl()
    }

    /// Writes a 4-field record across separate storage keys.
    pub fn write_batch_keys(env: Env, owner: Address, a: u32, b: i32, c: u32, d: u32) {
        let storage = env.storage().persistent();
        storage.set(&DataKey::BatchFieldA(owner.clone()), &a);
        storage.set(&DataKey::BatchFieldB(owner.clone()), &b);
        storage.set(&DataKey::BatchFieldC(owner.clone()), &c);
        storage.set(&DataKey::BatchFieldD(owner), &d);
    }

    /// Writes a 4-field record as a single compact `Bytes` entry.
    pub fn write_compact_bytes(env: Env, owner: Address, a: u32, b: i32, c: u32, d: u32) -> Bytes {
        let packed = encode_compact_batch(&env, a, b, c, d);
        env.storage()
            .persistent()
            .set(&DataKey::CompactBatch(owner), &packed);
        packed
    }

    /// Reads and decodes the compact `Bytes` entry for `owner`.
    pub fn read_compact_bytes(
        env: Env,
        owner: Address,
    ) -> Result<(u32, i32, u32, u32), StorageHeavyError> {
        let bytes: Bytes = env
            .storage()
            .persistent()
            .get(&DataKey::CompactBatch(owner))
            .ok_or(StorageHeavyError::NotFound)?;
        decode_compact_batch(&bytes)
    }
}

#[cfg(test)]
mod test;