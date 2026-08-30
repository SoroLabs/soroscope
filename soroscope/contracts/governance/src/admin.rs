use crate::storage_types::DataKey;
use soroban_sdk::{Address, Env};

pub fn has_administrator(e: &Env) -> bool {
    let key = DataKey::Config;
    e.storage().instance().has(&key)
}

pub fn read_administrator(e: &Env) -> Address {
    let key = DataKey::Config;
    let config: crate::storage_types::GovernanceConfig = e.storage().instance().get(&key).unwrap();
    config.admin
}

pub fn write_administrator(e: &Env, id: &Address) {
    let mut config = read_config(e);
    config.admin = id.clone();
    write_config(e, &config);
}

pub fn read_config(e: &Env) -> crate::storage_types::GovernanceConfig {
    let key = DataKey::Config;
    e.storage().instance().get(&key).unwrap_or_else(|| {
        // Default config
        crate::storage_types::GovernanceConfig {
            admin: Address::generate(e),
            voting_period: 7 * 24 * 60 * 60, // 7 days
            timelock_delay: 2 * 24 * 60 * 60, // 2 days
            quorum_percentage: 10, // 10%
            proposal_count: 0,
        }
    })
}

pub fn write_config(e: &Env, config: &crate::storage_types::GovernanceConfig) {
    let key = DataKey::Config;
    e.storage().instance().set(&key, config);
}