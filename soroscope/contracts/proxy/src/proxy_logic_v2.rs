use soroban_sdk::{contract, contractimpl, Env};

#[contract]
pub struct ProxyLogicV2;

#[contractimpl]
impl ProxyLogicV2 {
    pub fn calculate(_env: Env, a: i32, b: i32) -> i32 {
        a + b + 10
    }

    pub fn version(_env: Env) -> u32 {
        2
    }
}
