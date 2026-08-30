use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env};

use crate::contract_b::ContractBClient;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    Reentrancy = 1,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    ReentrancyLock,
}

fn lock(env: &Env) -> Result<(), Error> {
    if env.storage().instance().get(&DataKey::ReentrancyLock).unwrap_or(false) {
        return Err(Error::Reentrancy);
    }
    env.storage().instance().set(&DataKey::ReentrancyLock, &true);
    Ok(())
}

fn unlock(env: &Env) {
    env.storage().instance().set(&DataKey::ReentrancyLock, &false);
}

#[contract]
pub struct ContractA;

#[contractimpl]
impl ContractA {
    pub fn call_b(env: Env, b_id: Address, x: u32) -> Result<u32, Error> {
        lock(&env)?;
        let client = ContractBClient::new(&env, &b_id);
        let res = client.ping(&x);
        unlock(&env);
        Ok(res)
    }
}
