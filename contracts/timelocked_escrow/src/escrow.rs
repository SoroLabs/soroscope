use crate::storage_types::{DataKey, Error, EscrowConfig};
use soroban_sdk::{Address, Env};

pub fn has_config(e: &Env) -> bool {
    e.storage().instance().has(&DataKey::Config)
}

pub fn read_config(e: &Env) -> Result<EscrowConfig, Error> {
    e.storage()
        .instance()
        .get(&DataKey::Config)
        .ok_or(Error::NotInitialized)
}

pub fn write_config(e: &Env, config: &EscrowConfig) {
    e.storage().instance().set(&DataKey::Config, config);
}

pub fn read_admin(e: &Env) -> Result<Address, Error> {
    e.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(Error::NotInitialized)
}

pub fn write_admin(e: &Env, admin: &Address) {
    e.storage().instance().set(&DataKey::Admin, admin);
}

pub fn read_admin_release_vote(e: &Env) -> Option<u32> {
    e.storage().instance().get(&DataKey::AdminReleaseVote)
}

pub fn write_admin_release_vote(e: &Env, ledger: u32) {
    e.storage().instance().set(&DataKey::AdminReleaseVote, &ledger);
}

pub fn clear_admin_release_vote(e: &Env) {
    e.storage().instance().remove(&DataKey::AdminReleaseVote);
}

pub fn require_active(config: &EscrowConfig) -> Result<(), Error> {
    if config.is_released || config.is_recovered || config.is_cancelled {
        return Err(Error::AlreadyFinalized);
    }
    Ok(())
}

pub fn require_funded(config: &EscrowConfig) -> Result<(), Error> {
    if config.amount == 0 {
        return Err(Error::NoDeposit);
    }
    Ok(())
}
