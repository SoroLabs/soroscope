use soroban_sdk::{contract, contractimpl, Address, Env};

use crate::contract_b::ContractBClient;

#[contract]
pub struct ContractA;

#[contractimpl]
impl ContractA {
    pub fn call_b(env: Env, b_id: Address, x: u32) -> u32 {
        let client = ContractBClient::new(&env, &b_id);
        client.ping(&x)
    }
}
