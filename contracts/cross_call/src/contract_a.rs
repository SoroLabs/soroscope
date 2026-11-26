use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

use crate::contract_b::ContractBClient;
use crate::{Error, MAX_DEPTH};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    ReentrancyLock,
}

fn lock(env: &Env) -> Result<(), Error> {
    if env
        .storage()
        .instance()
        .get(&DataKey::ReentrancyLock)
        .unwrap_or(false)
    {
        return Err(Error::Reentrancy);
    }
    env.storage()
        .instance()
        .set(&DataKey::ReentrancyLock, &true);
    Ok(())
}

fn unlock(env: &Env) {
    env.storage()
        .instance()
        .set(&DataKey::ReentrancyLock, &false);
}

#[contract]
pub struct ContractA;

#[contractimpl]
impl ContractA {
    pub fn call_b(env: Env, b_id: Address, x: u32) -> Result<u32, Error> {
        if b_id == env.current_contract_address() {
            return Err(Error::SelfInvocation);
        }
        lock(&env)?;
        let client = ContractBClient::new(&env, &b_id);
        let res = client.ping(&x);
        unlock(&env);
        Ok(res)
    }

    /// Deterministic cross-contract benchmark: performs `depth` hops between
    /// this contract and `b_id` and returns the number of hops completed.
    pub fn bench_cross_hop_depth(env: Env, b_id: Address, depth: u32) -> Result<u32, Error> {
        if depth > MAX_DEPTH {
            return Err(Error::DepthExceeded);
        }
        if b_id == env.current_contract_address() {
            return Err(Error::SelfInvocation);
        }
        let mut hops = 0;
        for _ in 0..depth {
            hops = ContractBClient::new(&env, &b_id).ping(&hops);
        }
        Ok(hops)
    }
}
