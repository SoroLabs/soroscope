#![allow(dead_code)]
use crate::storage_types::{DataKey, TokenMetadata};
use soroban_sdk::{Env, String};

fn load_metadata(e: &Env) -> TokenMetadata {
    e.storage().instance().get(&DataKey::Metadata).unwrap()
}

fn save_metadata(e: &Env, meta: &TokenMetadata) {
    e.storage().instance().set(&DataKey::Metadata, meta);
}

/// Write all three metadata fields in a single storage entry.
pub fn write_metadata(e: &Env, name: &String, symbol: &String, decimals: u32) {
    save_metadata(
        e,
        &TokenMetadata {
            name: name.clone(),
            symbol: symbol.clone(),
            decimals,
        },
    );
}

pub fn read_decimal(e: &Env) -> u32 {
    load_metadata(e).decimals
}

pub fn read_name(e: &Env) -> String {
    load_metadata(e).name
}

pub fn read_symbol(e: &Env) -> String {
    load_metadata(e).symbol
}

// Legacy single-field writers kept for compatibility with admin set_* paths.
pub fn write_decimal(e: &Env, d: u32) {
    let mut meta = load_metadata(e);
    meta.decimals = d;
    save_metadata(e, &meta);
}

pub fn write_name(e: &Env, name: &String) {
    let mut meta = load_metadata(e);
    meta.name = name.clone();
    save_metadata(e, &meta);
}

pub fn write_symbol(e: &Env, symbol: &String) {
    let mut meta = load_metadata(e);
    meta.symbol = symbol.clone();
    save_metadata(e, &meta);
}
