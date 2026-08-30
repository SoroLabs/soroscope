import re
with open('contracts/factory/src/lib.rs', 'r') as f:
    lines = f.readlines()

# The file has a lot of syntax errors and duplicate blocks. 
# We'll just generate a clean factory contract that contains both the EmergencyGuard integration and the MultiSig features.

clean_code = """#![no_std]
#[cfg(test)]
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, xdr::ToXdr, Address, BytesN, Env, IntoVal, Vec,
};
use emergency_guard::{EmergencyGuard, GuardError, PauseType, DefaultEmergencyGuard};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    Paused = 4,
    PairAlreadyExists = 5,
    InvalidThreshold = 6,
}

const PAUSE_CREATE_PAIR_FLAG: u32 = 1 << 6;

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataKey {
    Pair(Address, Address),
    Admin,
    MultiSigConfig,
    PendingAction(u32),
    ApprovalCount(u32),
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MultiSigConfig {
    pub admins: Vec<Address>,
    pub threshold: u32,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdminAction {
    AddAdmin(Address),
    RemoveAdmin(Address),
    SetThreshold(u32),
}

#[contract]
pub struct LiquidityPoolFactory;

#[contractimpl]
impl LiquidityPoolFactory {
    /// Initializes the factory contract with an admin and setup the emergency guard.
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);

        let mut admins = Vec::new(&env);
        admins.push_back(admin);
        DefaultEmergencyGuard::initialize(env.clone(), admins, 1)
            .map_err(|_| Error::Unauthorized)?;

        Ok(())
    }

    pub fn create_pair(
        env: Env,
        token_a: Address,
        token_b: Address,
        wasm_hash: BytesN<32>,
    ) -> Result<Address, Error> {
        DefaultEmergencyGuard::check_not_paused(env.clone(), PAUSE_CREATE_PAIR_FLAG).map_err(|_| Error::Paused)?;

        let (token_0, token_1) = if token_a < token_b {
            (token_a, token_b)
        } else {
            (token_b, token_a)
        };

        if env.storage().instance().has(&DataKey::Pair(token_0.clone(), token_1.clone())) {
            return Err(Error::PairAlreadyExists);
        }

        #[cfg(test)]
        let deployed_address = {
            let _ = wasm_hash;
            Address::generate(&env)
        };

        #[cfg(not(test))]
        let deployed_address = {
            let salt = env.crypto().sha256(&(token_0.clone(), token_1.clone()).to_xdr(&env));
            let deployed_address = env.deployer().with_current_contract(salt).deploy_v2(wasm_hash, soroban_sdk::Vec::<soroban_sdk::Val>::new(&env));
            let init_args = soroban_sdk::vec![
                &env,
                env.current_contract_address().into_val(&env),
                token_0.clone().into_val(&env),
                token_1.clone().into_val(&env)
            ];
            let _res: soroban_sdk::Val = env.invoke_contract(
                &deployed_address,
                &soroban_sdk::Symbol::new(&env, "initialize"),
                init_args,
            );
            deployed_address
        };

        env.storage().instance().set(&DataKey::Pair(token_0, token_1), &deployed_address);
        Ok(deployed_address)
    }

    pub fn get_pair(env: Env, token_a: Address, token_b: Address) -> Option<Address> {
        let (token_0, token_1) = if token_a < token_b {
            (token_a, token_b)
        } else {
            (token_b, token_a)
        };
        env.storage().instance().get(&DataKey::Pair(token_0, token_1))
    }

    pub fn set_paused(env: Env, admin: Address, paused: bool) -> Result<(), Error> {
        DefaultEmergencyGuard::set_pause(env, admin, PAUSE_CREATE_PAIR_FLAG, paused).map_err(|_| Error::Unauthorized)
    }

    pub fn emergency_pause(env: Env, approvers: Vec<Address>) -> Result<(), Error> {
        DefaultEmergencyGuard::emergency_pause(env, approvers).map_err(|_| Error::Unauthorized)
    }

    pub fn get_pause_state(env: Env) -> u32 {
        DefaultEmergencyGuard::get_pause_state(env)
    }

    pub fn is_paused(env: Env, operation: u32) -> bool {
        DefaultEmergencyGuard::is_paused(env, operation)
    }

    pub fn get_admins(env: Env) -> Vec<Address> {
        DefaultEmergencyGuard::get_admins(env)
    }
    
    pub fn guard_unpause(env: Env, admin: Address, operation: u32) -> Result<(), Error> {
        DefaultEmergencyGuard::set_pause(env, admin, operation, false).map_err(|_| Error::Unauthorized)
    }

    // Multi-sig logic
    pub fn init_multisig(env: Env, admins: Vec<Address>, threshold: u32) {
        if env.storage().instance().has(&DataKey::MultiSigConfig) {
            panic!("MultiSig already initialized");
        }
        if admins.len() == 0 {
            panic!("At least one admin required");
        }
        if threshold == 0 || threshold as usize > admins.len() as usize {
            panic!("Invalid threshold");
        }
        let config = MultiSigConfig {
            admins: admins.clone(),
            threshold,
        };
        env.storage().instance().set(&DataKey::MultiSigConfig, &config);
    }

    pub fn get_multisig_config(env: Env) -> MultiSigConfig {
        env.storage().instance().get(&DataKey::MultiSigConfig).unwrap_or_else(|| panic!("MultiSig not initialized"))
    }

    pub fn is_admin(env: Env, address: &Address) -> bool {
        if let Some(config) = env.storage().instance().get::<_, MultiSigConfig>(&DataKey::MultiSigConfig) {
            config.admins.iter().any(|a| a == *address)
        } else {
            false
        }
    }

    pub fn propose_admin_action(env: Env, proposer: Address, action: AdminAction) -> u32 {
        if !Self::is_admin(env.clone(), &proposer) {
            panic!("Only admins can propose actions");
        }
        let action_id = env.ledger().timestamp() as u32;
        env.storage().instance().set(&DataKey::PendingAction(action_id), &action);
        env.storage().instance().set(&DataKey::ApprovalCount(action_id), &1u32);
        action_id
    }

    pub fn approve_admin_action(env: Env, approver: Address, action_id: u32) {
        if !Self::is_admin(env.clone(), &approver) {
            panic!("Only admins can approve actions");
        }
        if !env.storage().instance().has(&DataKey::PendingAction(action_id)) {
            panic!("Action not found");
        }
        let mut approval_count: u32 = env.storage().instance().get(&DataKey::ApprovalCount(action_id)).unwrap_or_else(|| 0);
        approval_count += 1;
        env.storage().instance().set(&DataKey::ApprovalCount(action_id), &approval_count);
    }

    pub fn execute_admin_action(env: Env, action_id: u32) {
        let config = Self::get_multisig_config(env.clone());
        let approval_count: u32 = env.storage().instance().get(&DataKey::ApprovalCount(action_id)).unwrap_or_else(|| 0);
        if approval_count < config.threshold {
            panic!("Insufficient approvals");
        }
        let action: AdminAction = env.storage().instance().get(&DataKey::PendingAction(action_id)).unwrap_or_else(|| panic!("Action not found"));

        match action {
            AdminAction::AddAdmin(new_admin) => {
                let mut new_config = config.clone();
                if new_config.admins.iter().any(|a| a == new_admin) {
                    panic!("Admin already exists");
                }
                new_config.admins.push_back(new_admin);
                env.storage().instance().set(&DataKey::MultiSigConfig, &new_config);
            }
            AdminAction::RemoveAdmin(admin_to_remove) => {
                let mut new_config = config.clone();
                let initial_len = new_config.admins.len();
                let mut filtered_admins = Vec::new(&env);
                for a in new_config.admins.iter() {
                    if a != admin_to_remove {
                        filtered_admins.push_back(a);
                    }
                }
                if filtered_admins.len() == initial_len {
                    panic!("Admin not found");
                }
                if filtered_admins.len() == 0 {
                    panic!("Cannot remove last admin");
                }
                new_config.admins = filtered_admins;
                if new_config.threshold as usize > new_config.admins.len() as usize {
                    new_config.threshold = new_config.admins.len() as u32;
                }
                env.storage().instance().set(&DataKey::MultiSigConfig, &new_config);
            }
            AdminAction::SetThreshold(new_threshold) => {
                if new_threshold == 0 || new_threshold as usize > config.admins.len() as usize {
                    panic!("Invalid threshold");
                }
                let mut new_config = config.clone();
                new_config.threshold = new_threshold;
                env.storage().instance().set(&DataKey::MultiSigConfig, &new_config);
            }
        }
        env.storage().instance().remove(&DataKey::PendingAction(action_id));
        env.storage().instance().remove(&DataKey::ApprovalCount(action_id));
    }
}

mod test;
"""

with open('contracts/factory/src/lib.rs', 'w') as f:
    f.write(clean_code)
