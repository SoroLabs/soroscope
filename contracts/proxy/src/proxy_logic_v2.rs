use soroban_sdk::{contract, contractimpl, Address, Env};

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

    /// Returns the caller forwarded by the proxy after authenticating it.
    ///
    /// The host reports the proxy as this contract's invoker, so the original
    /// user must be supplied explicitly by `Proxy::delegate_call`.
    pub fn whoami(_env: Env, caller: Address) -> Address {
        caller.require_auth();
        caller
    }
}
