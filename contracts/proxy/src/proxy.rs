use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, BytesN, Env, IntoVal, Symbol, Val,
    Vec,
};

pub const TIMELOCK_DELAY: u64 = 172_800; // 48 hours in seconds (48 * 60 * 60)

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingUpgrade {
    pub new_implementation: Address,
    pub eta: u64,
}

#[contracttype]
pub enum DataKey {
    Admin,
    Implementation,
    Counter,
    Storage(BytesN<32>),
    PendingUpgrade,
}

#[contract]
pub struct Proxy;

#[contractimpl]
impl Proxy {
    pub fn initialize(env: Env, admin: Address, implementation: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Proxy already initialized");
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::Implementation, &implementation);
        env.storage().instance().set(&DataKey::Counter, &0i32);
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Admin).unwrap()
    }

    pub fn get_implementation(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Implementation)
            .unwrap()
    }

    /// Returns pending upgrade details if one exists
    pub fn get_pending_upgrade(env: Env) -> Option<PendingUpgrade> {
        env.storage().persistent().get(&DataKey::PendingUpgrade)
    pub fn upgrade_to(env: Env, implementation: Address) {
        let admin = Self::get_admin(env.clone());
        admin.require_auth();
        env.storage()
            .instance()
            .set(&DataKey::Implementation, &implementation);
    }

    /// Step 1: Propose an upgrade with a 48-hour timelock delay
    pub fn propose_upgrade(env: Env, new_implementation: Address) {
        let admin = Self::get_admin(env.clone());
        admin.require_auth();

        let eta = env.ledger().timestamp() + TIMELOCK_DELAY;
        let proposal = PendingUpgrade {
            new_implementation: new_implementation.clone(),
            eta,
        };

        env.storage()
            .persistent()
            .set(&DataKey::PendingUpgrade, &proposal);

        env.events()
            .publish((symbol_short!("propose"),), (new_implementation, eta));
    }

    /// Step 2: Execute the pending upgrade once 48 hours have passed
    pub fn execute_upgrade(env: Env) {
        let admin = Self::get_admin(env.clone());
        admin.require_auth();

        let proposal: PendingUpgrade = env
            .storage()
            .persistent()
            .get(&DataKey::PendingUpgrade)
            .expect("No pending upgrade proposal found");

        if env.ledger().timestamp() < proposal.eta {
            panic!("Timelock delay of 48 hours has not elapsed");
        }

        env.storage()
            .persistent()
            .set(&DataKey::Implementation, &proposal.new_implementation);

        env.storage().persistent().remove(&DataKey::PendingUpgrade);

        env.events()
            .publish((symbol_short!("upgraded"),), proposal.new_implementation);
    }

    /// Step 3: Admin can cancel a pending upgrade
    pub fn cancel_upgrade(env: Env) {
        let admin = Self::get_admin(env.clone());
        admin.require_auth();

        env.storage().persistent().remove(&DataKey::PendingUpgrade);

        env.events().publish((symbol_short!("cancel"),), ());
    }

    /// Legacy upgrade method kept for backward compatibility (delegates to proposal flow)
    pub fn upgrade_to(env: Env, implementation: Address) {
        Self::propose_upgrade(env, implementation);
    }

    /// Execute the pending upgrade (after timelock) and immediately call a method on the new implementation.
    /// The upgrade must have already been proposed via `propose_upgrade` and the 48-hour
    /// timelock must have elapsed before calling this.
    pub fn upgrade_to_and_call(
        env: Env,
        implementation: Address,
        method: Symbol,
        args: Vec<Val>,
    ) -> Val {
        let admin = Self::get_admin(env.clone());
        admin.require_auth();

        let proposal: PendingUpgrade = env
            .storage()
            .persistent()
            .get(&DataKey::PendingUpgrade)
            .expect("No pending upgrade proposal – call propose_upgrade first");

        if proposal.new_implementation != implementation {
            panic!("Implementation does not match the pending proposal");
        }

        if env.ledger().timestamp() < proposal.eta {
            panic!("Timelock delay of 48 hours has not elapsed");
        }

        env.storage()
            .persistent()
            .set(&DataKey::Implementation, &proposal.new_implementation);
        env.storage().persistent().remove(&DataKey::PendingUpgrade);

        env.events()
            .publish((symbol_short!("upgraded"),), proposal.new_implementation);

            .instance()
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
            .instance()
            .get(&DataKey::Counter)
            .unwrap_or(0)
    }

    pub fn set_value(env: Env, value: i32) {
        env.storage().instance().set(&DataKey::Counter, &value);
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
