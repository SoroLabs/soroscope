#![no_std]

use emergency_guard::{
    DefaultEmergencyGuard, EmergencyGuard, EmergencyGuardTrait, GuardDataKey, GuardError, PauseType,
};
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, xdr::ToXdr, Address, BytesN, Env,
    IntoVal, Vec,
};

#[cfg(test)]
use soroban_sdk::testutils::Address as _;

pub trait EmergencyGuardTrait {
    fn check_not_paused(env: &Env, operation: u32) -> Result<(), GuardError>;
    fn get_pause_state(env: &Env) -> u32;
    fn set_pause_state(env: &Env, operation: u32, paused: bool) -> Result<(), GuardError>;
    fn unpause(env: &Env, operation: u32) -> Result<(), GuardError>;
    fn unpause_all(env: &Env) -> Result<(), GuardError>;
    fn emergency_pause_all(env: &Env, approvers: Vec<Address>) -> Result<(), GuardError>;
    fn resume_all(env: &Env, approvers: Vec<Address>) -> Result<(), GuardError>;
    fn init_guard(env: &Env, admins: Vec<Address>, threshold: u32) -> Result<(), GuardError>;
    fn add_admin(env: &Env, approvers: Vec<Address>, new_admin: Address) -> Result<(), GuardError>;
    fn remove_admin(env: &Env, approvers: Vec<Address>, admin: Address) -> Result<(), GuardError>;
    fn rotate_admin(
        env: &Env,
        approvers: Vec<Address>,
        old_admin: Address,
        new_admin: Address,
    ) -> Result<(), GuardError>;
    fn get_admins(env: &Env) -> Vec<Address>;
    fn get_threshold(env: &Env) -> u32;
    fn is_admin(env: &Env, addr: Address) -> bool;
}

pub struct DefaultEmergencyGuard;

impl DefaultEmergencyGuard {
    pub fn check_not_paused(env: &Env, operation: u32) -> Result<(), GuardError> {
        let pause_state: PauseType = env
            .storage()
            .instance()
            .get(&GuardDataKey::PauseState)
            .unwrap_or(PauseType::new(0));

        if pause_state.is_paused(operation) {
            Err(GuardError::Paused)
        } else {
            Ok(())
        }
    }

    pub fn get_pause_state(env: &Env) -> u32 {
        let pause_state: PauseType = env
            .storage()
            .instance()
            .get(&GuardDataKey::PauseState)
            .unwrap_or(PauseType::new(0));
        pause_state.as_u32()
    }

    pub fn set_pause_state(env: &Env, operation: u32, paused: bool) -> Result<(), GuardError> {
        let mut pause_state: PauseType = env
            .storage()
            .instance()
            .get(&GuardDataKey::PauseState)
            .unwrap_or(PauseType::new(0));

        pause_state.set_paused(operation, paused);
        env.storage()
            .instance()
            .set(&GuardDataKey::PauseState, &pause_state);

        Ok(())
    }

    pub fn unpause(env: &Env, operation: u32) -> Result<(), GuardError> {
        Self::set_pause_state(env, operation, false)
    }

    pub fn unpause_all(env: &Env) -> Result<(), GuardError> {
        let pause_state = PauseType::new(0);
        env.storage()
            .instance()
            .set(&GuardDataKey::PauseState, &pause_state);

        Ok(())
    }

    pub fn emergency_pause_all(env: &Env, approvers: Vec<Address>) -> Result<(), GuardError> {
        EmergencyGuard::validate_multi_sig(env.clone(), approvers.clone())?;

        let mut pause_state = PauseType::new(0);
        pause_state.pause_all();

        env.storage()
            .instance()
            .set(&GuardDataKey::PauseState, &pause_state);

        Ok(())
    }

    pub fn resume_all(env: &Env, approvers: Vec<Address>) -> Result<(), GuardError> {
        EmergencyGuard::validate_multi_sig(env.clone(), approvers.clone())?;

        let pause_state = PauseType::new(0);
        env.storage()
            .instance()
            .set(&GuardDataKey::PauseState, &pause_state);

        Ok(())
    }

    pub fn init_guard(env: &Env, admins: Vec<Address>, threshold: u32) -> Result<(), GuardError> {
        if env.storage().instance().has(&GuardDataKey::Admins) {
            return Err(GuardError::AlreadyInitialized);
        }

        if threshold == 0 || threshold > admins.len() as u32 {
            return Err(GuardError::InvalidThreshold);
        }

        env.storage().instance().set(&GuardDataKey::Admins, &admins);
        env.storage()
            .instance()
            .set(&GuardDataKey::SignatureThreshold, &threshold);
        env.storage()
            .instance()
            .set(&GuardDataKey::PauseState, &PauseType::new(0));

        Ok(())
    }

    pub fn add_admin(env: &Env, approvers: Vec<Address>, new_admin: Address) -> Result<(), GuardError> {
        EmergencyGuard::validate_multi_sig(env.clone(), approvers.clone())?;

        let mut admins = Self::get_admins(env);
        if !admins.iter().any(|a| a == new_admin) {
            admins.push_back(new_admin.clone());
            env.storage().instance().set(&GuardDataKey::Admins, &admins);
        }

        Ok(())
    }

    pub fn remove_admin(env: &Env, approvers: Vec<Address>, admin: Address) -> Result<(), GuardError> {
        EmergencyGuard::validate_multi_sig(env.clone(), approvers.clone())?;

        let admins = Self::get_admins(env);
        let threshold = Self::get_threshold(env);

        if admins.len() as u32 <= threshold {
            return Err(GuardError::InvalidThreshold);
        }

        let mut new_admins = Vec::new(env);
        let mut found = false;
        for a in admins.iter() {
            if a != admin {
                new_admins.push_back(a);
            } else {
                found = true;
            }
        }

        if !found {
            return Err(GuardError::AdminNotFound);
        }

        env.storage().instance().set(&GuardDataKey::Admins, &new_admins);
        Ok(())
    }

    pub fn rotate_admin(
        env: &Env,
        approvers: Vec<Address>,
        old_admin: Address,
        new_admin: Address,
    ) -> Result<(), GuardError> {
        EmergencyGuard::validate_multi_sig(env.clone(), approvers.clone())?;

        let admins = Self::get_admins(env);
        let threshold = Self::get_threshold(env);

        let mut found = false;
        let mut new_admins = Vec::new(env);
        for a in admins.iter() {
            if a == old_admin {
                found = true;
            } else if a != new_admin {
                new_admins.push_back(a);
            }
        }

        if !found {
            return Err(GuardError::AdminNotFound);
        }

        new_admins.push_back(new_admin.clone());

        if (new_admins.len() as u32) < threshold {
            return Err(GuardError::InvalidThreshold);
        }

        env.storage().instance().set(&GuardDataKey::Admins, &new_admins);
        Ok(())
    }

    pub fn get_admins(env: &Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&GuardDataKey::Admins)
            .unwrap_or_else(|| Vec::new(env))
    }

    pub fn get_threshold(env: &Env) -> u32 {
        env.storage()
            .instance()
            .get(&GuardDataKey::SignatureThreshold)
            .unwrap_or(0)
    }

    pub fn is_admin(env: &Env, addr: Address) -> bool {
        let admins: Vec<Address> = env
            .storage()
            .instance()
            .get(&GuardDataKey::Admins)
            .unwrap_or_else(|| Vec::new(env));

        admins.iter().any(|a| a == addr)
    }
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    Paused = 4,
    PairAlreadyExists = 5,
    InvalidThreshold = 6,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataKey {
    Admin,
    Pair(Address, Address),
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MultisigConfig {
    pub admins: Vec<Address>,
    pub threshold: u32,
}

fn map_guard_err(err: GuardError) -> Error {
    match err {
        GuardError::AlreadyInitialized => Error::AlreadyInitialized,
        GuardError::InvalidThreshold => Error::InvalidThreshold,
        GuardError::NotInitialized => Error::NotInitialized,
        GuardError::InsufficientSignatures
        | GuardError::Unauthorized
        | GuardError::AdminNotFound => Error::Unauthorized,
        GuardError::Paused => Error::Paused,
    }
}

impl EmergencyGuardTrait for LiquidityPoolFactory {
    fn check_not_paused(env: &Env, operation: u32) -> Result<(), GuardError> {
        DefaultEmergencyGuard::check_not_paused(env, operation)
    }

    fn get_pause_state(env: &Env) -> u32 {
        DefaultEmergencyGuard::get_pause_state(env)
    }

    fn set_pause_state(env: &Env, operation: u32, paused: bool) -> Result<(), GuardError> {
        DefaultEmergencyGuard::set_pause_state(env, operation, paused)
    }

    fn unpause(env: &Env, operation: u32) -> Result<(), GuardError> {
        DefaultEmergencyGuard::unpause(env, operation)
    }

    fn unpause_all(env: &Env) -> Result<(), GuardError> {
        DefaultEmergencyGuard::unpause_all(env)
    }

    fn emergency_pause_all(env: &Env, approvers: Vec<Address>) -> Result<(), GuardError> {
        DefaultEmergencyGuard::emergency_pause_all(env, approvers)
    }

    fn resume_all(env: &Env, approvers: Vec<Address>) -> Result<(), GuardError> {
        DefaultEmergencyGuard::resume_all(env, approvers)
    }

    fn init_guard(env: &Env, admins: Vec<Address>, threshold: u32) -> Result<(), GuardError> {
        DefaultEmergencyGuard::init_guard(env, admins, threshold)
    }

    fn add_admin(env: &Env, approvers: Vec<Address>, new_admin: Address) -> Result<(), GuardError> {
        DefaultEmergencyGuard::add_admin(env, approvers, new_admin)
    }

    fn remove_admin(env: &Env, approvers: Vec<Address>, admin: Address) -> Result<(), GuardError> {
        DefaultEmergencyGuard::remove_admin(env, approvers, admin)
    }

    fn rotate_admin(
        env: &Env,
        approvers: Vec<Address>,
        old_admin: Address,
        new_admin: Address,
    ) -> Result<(), GuardError> {
        DefaultEmergencyGuard::rotate_admin(env, approvers, old_admin, new_admin)
    }

    fn get_admins(env: &Env) -> Vec<Address> {
        DefaultEmergencyGuard::get_admins(env)
    }

    fn get_threshold(env: &Env) -> u32 {
        DefaultEmergencyGuard::get_threshold(env)
    }

    fn is_admin(env: &Env, addr: Address) -> bool {
        DefaultEmergencyGuard::is_admin(env, addr)
    }
}

#[contract]
pub struct LiquidityPoolFactory;

#[contractimpl]
impl LiquidityPoolFactory {
    /// Initializes the factory contract with an admin and setups the emergency guard.
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);

        let mut admins = Vec::new(&env);
        admins.push_back(admin);

        EmergencyGuard::initialize(env, admins, 1).map_err(|_| Error::Unauthorized)
    }

    /// Convenience initializer for a multisig guard configuration.
    pub fn initialize_guard(
        env: Env,
        admins: Vec<Address>,
        threshold: u32,
    ) -> Result<(), GuardError> {
        EmergencyGuard::initialize(env, admins, threshold)
    }

    pub fn add_guard_admin(
        env: Env,
        approvers: Vec<Address>,
        new_admin: Address,
    ) -> Result<(), GuardError> {
        EmergencyGuard::add_admin(env, approvers, new_admin)
    }

    pub fn remove_guard_admin(
        env: Env,
        approvers: Vec<Address>,
        admin: Address,
    ) -> Result<(), GuardError> {
        EmergencyGuard::remove_admin(env, approvers, admin)
    }

    pub fn get_admins(env: Env) -> Vec<Address> {
        EmergencyGuard::get_admins(env)
    }

    pub fn get_threshold(env: Env) -> u32 {
        EmergencyGuard::get_threshold(env)
    }

    pub fn is_admin(env: Env, addr: Address) -> bool {
        EmergencyGuard::is_admin_public(env, addr)
    }

    pub fn set_guard_pause(
        env: Env,
        admin: Address,
        operation: u32,
        paused: bool,
    ) -> Result<(), GuardError> {
        EmergencyGuard::set_pause(env, admin, operation, paused)
    }

    pub fn guard_unpause(env: Env, admin: Address, operation: u32) -> Result<(), Error> {
        EmergencyGuard::set_pause(env, admin, operation, false).map_err(map_guard_err)
    }

    pub fn is_paused(env: Env, operation: u32) -> bool {
        EmergencyGuard::is_paused(env, operation)
    }

    pub fn get_pause_state(env: Env) -> u32 {
        EmergencyGuard::get_pause_state(env)
    }

    pub fn emergency_pause(env: Env, approvers: Vec<Address>) -> Result<(), Error> {
        EmergencyGuard::emergency_pause(env, approvers).map_err(map_guard_err)
    }

    pub fn resume_guard(env: Env, approvers: Vec<Address>) -> Result<(), GuardError> {
        EmergencyGuard::resume(env, approvers)
    }

    pub fn create_pair(
        env: Env,
        token_a: Address,
        token_b: Address,
        wasm_hash: BytesN<32>,
    ) -> Result<Address, Error> {
        if EmergencyGuard::is_paused(env.clone(), PauseType::CREATE_PAIR) {
            return Err(Error::Paused);
        }

        let (token_0, token_1) = if token_a < token_b {
            (token_a, token_b)
        } else {
            (token_b, token_a)
        };

        if env
            .storage()
            .instance()
            .has(&DataKey::Pair(token_0.clone(), token_1.clone()))
        {
            return Err(Error::PairAlreadyExists);
        }

        let salt = env
            .crypto()
            .sha256(&(token_0.clone(), token_1.clone()).to_xdr(&env));

        let deployed_address = env
            .deployer()
            .with_current_contract(salt)
            .deploy_v2(wasm_hash, Vec::<soroban_sdk::Val>::new(&env));

        let init_args = soroban_sdk::vec![
            &env,
            env.current_contract_address().into_val(&env),
            token_0.clone().into_val(&env),
            token_1.clone().into_val(&env),
        ];

        let _res: soroban_sdk::Val = env.invoke_contract(
            &deployed_address,
            &soroban_sdk::Symbol::new(&env, "initialize"),
            init_args,
        );

        env.storage()
            .instance()
            .set(&DataKey::Pair(token_0, token_1), &deployed_address);

        Ok(deployed_address)
    }

    pub fn get_pair(env: Env, token_a: Address, token_b: Address) -> Option<Address> {
        let (token_0, token_1) = if token_a < token_b {
            (token_a, token_b)
        } else {
            (token_b, token_a)
        };

        env.storage().instance().get(&DataKey::Pair(token_0, token_1))
    }

    pub fn get_multisig_config(env: Env) -> MultisigConfig {
        let admins = EmergencyGuard::get_admins(env.clone());
        let threshold = EmergencyGuard::get_threshold(env);
        MultisigConfig { admins, threshold }
    }
}

#[cfg(test)]
mod test;