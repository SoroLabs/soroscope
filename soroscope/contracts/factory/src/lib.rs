#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, xdr::ToXdr, Address, BytesN, Env, IntoVal, Vec,
};
use emergency_guard::{EmergencyGuard, GuardError, PauseType};

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
    Pair(Address, Address),
    Admin,
    PauseState,
}

#[contract]
pub struct LiquidityPoolFactory;

#[contractimpl]
impl LiquidityPoolFactory {
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);

        let mut admins = Vec::new(&env);
        admins.push_back(admin);
        EmergencyGuard::initialize(env.clone(), admins, 1)
            .map_err(|_| Error::Unauthorized)?;

        Ok(())
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

        if env.storage().instance().has(&DataKey::Pair(token_0.clone(), token_1.clone())) {
            return Err(Error::PairAlreadyExists);
        }

        let salt = env.crypto().sha256(&(token_0.clone(), token_1.clone()).to_xdr(&env));
        let deployed_address = env.deployer().with_current_contract(salt).deploy_v2(wasm_hash, soroban_sdk::Vec::<soroban_sdk::Val>::new(&env));
        let init_args = soroban_sdk::vec![
            &env,
            env.current_contract_address().into_val(&env),
            token_0.clone().into_val(&env),
            token_1.clone().into_val(&env)
        ];
        let _res: soroban_sdk::Val = env.invoke_contract(
            &deployed_address,
            &soroban_sdk::Symbol::new(&env, "initialize"),
            init_args,
        );

        env.storage().instance().set(&DataKey::Pair(token_0, token_1), &deployed_address);
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

    pub fn set_operation_paused(env: Env, admin: Address, operation: u32, paused: bool) -> Result<(), Error> {
        EmergencyGuard::set_pause(env, admin, operation, paused)
            .map_err(map_guard_err)
    }

    pub fn is_paused(env: Env, operation: u32) -> bool {
        EmergencyGuard::is_paused(env, operation)
    }

    pub fn get_pause_state(env: Env) -> u32 {
        EmergencyGuard::get_pause_state(env)
    }

    pub fn get_admins(env: Env) -> Vec<Address> {
        EmergencyGuard::get_admins(env)
    }

    pub fn add_admin(env: Env, approvers: Vec<Address>, new_admin: Address) -> Result<(), Error> {
        EmergencyGuard::add_admin(env, approvers, new_admin).map_err(map_guard_err)
    }

    pub fn remove_admin(env: Env, approvers: Vec<Address>, admin: Address) -> Result<(), Error> {
        EmergencyGuard::remove_admin(env, approvers, admin).map_err(map_guard_err)
    }

    pub fn emergency_pause(env: Env, approvers: Vec<Address>) -> Result<(), Error> {
        EmergencyGuard::emergency_pause(env, approvers).map_err(map_guard_err)
    }

    pub fn resume(env: Env, approvers: Vec<Address>) -> Result<(), Error> {
        EmergencyGuard::resume(env, approvers).map_err(map_guard_err)
    }
}

fn map_guard_err(err: GuardError) -> Error {
    match err {
        GuardError::Paused => Error::Paused,
        GuardError::NotInitialized => Error::NotInitialized,
        GuardError::Unauthorized
        | GuardError::InsufficientSignatures
        | GuardError::AdminNotFound
        | GuardError::InvalidThreshold => Error::Unauthorized,
        GuardError::AlreadyInitialized => Error::AlreadyInitialized,
    }
}

mod test;
