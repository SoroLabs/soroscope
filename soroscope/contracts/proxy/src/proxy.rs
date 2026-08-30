use soroban_sdk::{
    contract, contractimpl, contracttype, Address, BytesN, Env, IntoVal, Symbol, Val, Vec,
};

#[contracttype]
pub enum DataKey {
    Admin,
    Implementation,
    Counter,
    Storage(BytesN<32>),
}

#[contract]
pub struct Proxy;

#[contractimpl]
impl Proxy {
    pub fn initialize(env: Env, admin: Address, implementation: Address) {
        if env.storage().persistent().has(&DataKey::Admin) {
            panic!("Proxy already initialized");
        }

        env.storage().persistent().set(&DataKey::Admin, &admin);
        env.storage()
            .persistent()
            .set(&DataKey::Implementation, &implementation);
        env.storage().persistent().set(&DataKey::Counter, &0i32);
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage().persistent().get(&DataKey::Admin).unwrap()
    }

    pub fn get_implementation(env: Env) -> Address {
        env.storage()
            .persistent()
            .get(&DataKey::Implementation)
            .unwrap()
    }

    pub fn upgrade_to(env: Env, implementation: Address) {
        let admin = Self::get_admin(env.clone());
        admin.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::Implementation, &implementation);
    }

    pub fn upgrade_to_and_call(
        env: Env,
        implementation: Address,
        method: Symbol,
        args: Vec<Val>,
    ) -> Val {
        let admin = Self::get_admin(env.clone());
        admin.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::Implementation, &implementation);
        Self::delegate_call(env, method, args)
    }

    pub fn delegate_call(env: Env, method: Symbol, args: Vec<Val>) -> Val {
        let implementation = Self::get_implementation(env.clone());
        env.invoke_contract(&implementation, &method, args)
    }

    pub fn increment(env: Env, amount: i32) -> i32 {
        let current = Self::get_value(env.clone());
        let method = Symbol::new(&env, "calculate");
        let args: Vec<Val> = Vec::from_array(&env, [current.into_val(&env), amount.into_val(&env)]);
        let next: i32 = env.invoke_contract(&Self::get_implementation(env.clone()), &method, args);
        Self::set_value(env, next);
        next
    }

    pub fn get_value(env: Env) -> i32 {
        env.storage()
            .persistent()
            .get(&DataKey::Counter)
            .unwrap_or(0)
    }

    pub fn set_value(env: Env, value: i32) {
        env.storage().persistent().set(&DataKey::Counter, &value);
    }

    pub fn set_storage(env: Env, key: BytesN<32>, value: Val) {
        let admin = Self::get_admin(env.clone());
        admin.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::Storage(key), &value);
    }

    pub fn get_storage(env: Env, key: BytesN<32>) -> Option<Val> {
        env.storage().persistent().get(&DataKey::Storage(key))
    }
}
