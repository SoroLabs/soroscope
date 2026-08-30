use soroban_sdk::{contract, contractimpl, Env};

#[contract]
pub struct ProxyLogicV1;

#[contractimpl]
impl ProxyLogicV1 {
    pub fn calculate(_env: Env, a: i32, b: i32) -> i32 {
        a + b
    }

    pub fn version(_env: Env) -> u32 {
        1
    }
}
