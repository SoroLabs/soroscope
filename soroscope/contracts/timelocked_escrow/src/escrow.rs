use crate::storage_types::{DataKey, Error, EscrowConfig};
use soroban_sdk::Env;

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

pub fn require_active(config: &EscrowConfig) -> Result<(), Error> {
    if config.is_released || config.is_cancelled {
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
