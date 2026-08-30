use soroban_sdk::{contract, contractimpl, Env};

#[contract]
pub struct ContractB;

#[contractimpl]
impl ContractB {
    pub fn ping(_env: Env, x: u32) -> u32 {
        x + 1
    }
}
