#a[no_std]
use soroban_sdk::{ contract, contractimpl, Env, Symbol };

const KEY: Symbol = Symbol::new!("KEY");

#contract
pub struct TtlExtension;

#contractimpl
impl TtlExtension {
    pub fn extend_ttl(env: Env, threshold: u32, extend_to: u32) {
        let instance_ttl = env.storage().instance().get_ttl();
        if instance_ttl < threshold {
            env.storage().instance().extend_ttl(threshold, extend_to);
        }
        let persistent_ttl = env.storage().persistent().get_ttl(&KEY);
        if persistent_ttl < threshold {
            env.storage().persistent().extend_ttl(&KEY, threshold, extend_to);
        }
    }

    pub fn write(env: Env, value: u32) {
        env.storage().persistent().set(&KEY, &value);
    }

    pub fn persistent_ttl(env: Env) -> u32 {
        env.storage().persistent().get_ttl(&KEY)
    }

    pub fn instance_ttl(env: Env) -> u32 {
        env.storage().instance().get_ttl()
    }
}

#[cfg(test)]
mod test;
