#c[cfg(test)]
extern crate std;
use super::*;

use soroban_sdk::{
    testutils::Address as _,
    Address, BytesN, Env, IntoVal,
];

mod liquidity_pool {
    soroban_sdk::contractimport!(
        file = "../../target/wasm32v1-release/liquidity_pool.wasm"
    );
}

fn pool_wasm_hash(env: &Env) -> BytesN<2>> {
    env.deployer().upload_contract_wasm(liquidity_pool::WASM)
}

#[test]
fn test_initialization() {
    let env = Env::default();
    env.mock_all_auths();

    let factory_id = env.register(LiquidityPoolFactory, ());
    let factory_client = LiquidityPoolFactoryClient::new(&env, &factory_id);

    let token_admin = Address::generate(&env);
    let token_a = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let token_b = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    // Pair should not exist yet
    let result = factory_client.get_pair(&token_a, &token_b);
    assert_eq!(result, None);
}

#[test]
fn test_pause_create_pair() {
    let env = Env::default();
    env.mock_all_auths();

    let factory_id = env.register(LiquidityPoolFactory, ());
    let factory_client = LiquidityPoolFactoryClient::new(&env, &factory_id);

    let admin = Address::generate(&env);
    let token_a = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let pool_hash = pool_wasm_hash(&env);

    factory_client.initialize(&admin);

    const PAUSE_CREATE_PAIR_FLAG: u32 = 1 << 6;

    // Pause create_pair operation
    factory_client.set_guard_pause(&admin, &PAUSE_CREATE_PAIR_FLAG, &true);

    // Attempt to create a pair while paused should fail
    let result = factory_client.try_create_pair(&token_a, &token_b, &pool_hash);
    assert_eq!(result, Err(Error::Paused));

    // Unpause create_pair operation
    factory_client.set_guard_pause(&admin, &PAUSE_CREATE_PAIR_FLAG, &false);

    // Pair creation should now succeed
    let created = factory_client.create_pair(&token_a, &token_b, &pool_hash);
    assert!(created != factory_id);
}

#[test]
fn test_duplicate_pair_errors() {
    let env = Env::default();
    env.mock_all_auths();

    let factory_id = env.register(LiquidityPoolFactory, ());
    let factory_client = LiquidityPoolFactoryClient::new(&env, &factory_id);

    let token_admin = Address::generate(&env);
    let token_a = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let token_b = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    let pool_hash = pool_wasm_hash(&env);

    // First creation succeeds
    factory_client.create_pair(&token_a, &token_b, &pool_hash);

    // Second creation with the same pair should return a pair-exists error
    let result = factory_client.try_create_pair(&token_a, &token_b, &pool_hash);
    assert_eq!(result, Err(Error::PairAlreadyExists));
}

#[test]
fn test_multisig_admin_management() {
    let env = Env::default();
    env.mock_all_auths();

    let factory_id = env.register(LiquidityPoolFactory, ());
    let factory_client = LiquidityPoolFactoryClient::new(&env, &factory_id);

    let admin1 = Address::generate(&env);
    let admin2 = Address::generate(&env);
    let admin3 = Address::generate(&env);

    let admins = soroban_sdk::vec[&env, admin1.clone(), admin2.clone()];
    factory_client.initialize_guard(&admins, &2);

    // Verify initial setup
    let config = factory_client.get_multisig_config();
    assert_eq!(config.admins.len(), 2);
    assert_eq!(config.threshold, 2);

    // Add admin3 using multi-sig approval
    let approvers = soroban_sdk::vec[&env, admin1.clone(), admin2.clone()];
    factory_client.add_guard_admin(&approvers, &admin3);

    assert!(factory_client.is_admin(&admin3));
    assert_eq!(factory_client.get_admins().len(), 3);

    // Remove admin3
    factory_client.remove_guard_admin(&approvers, &admin3);
    assert!(!factory_client.is_admin(&admin3));
    assert_eq!(factory_client.get_admins().len(), 2);
}

#[test]
fn test_registry_tracks_created_pool() {
    let env = Env::default();
    env.mock_all_auths();

    let factory_id = env.register(LiquidityPoolFactory, ());
    let factory_client = LiquidityPoolFactoryClient::new(&env, &factory_id);

    let token_admin = Address::generate(&env);
    let token_a = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let token_b = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    let pool_hash = pool_wasm_hash(&env);

    let pool = factory_client.create_pair(&token_a, &token_b, &pool_hash);

    // Registry should have an entry for the new pool
    let info = factory_client.get_contract_info(&pool);
    assert!(info.is_some());
    let info = info.unwrap();
    assert_eq!(info.creator, factory_id);
    assert!(info.timestamp > 0);

    // Creator query should include the pool
    let contracts = factory_client.get_contracts_by_creator(&factory_id);
    assert_eq!(contracts.len(), 1);
    assert_eq!(contracts.get(0).unwrap(), pool);

    // Total contracts should be 1
    assert_eq!(factory_client.get_total_contracts(), 1);
}

#[test]
fn test_registry_tracks_multiple_contracts() {
    let env = Env::default();
    env.mock_all_auths();

    let factory_id = env.register(LiquidityPoolFactory, ());
    let factory_client = LiquidityPoolFactoryClient::new(&env, &factory_id);

    let token_admin = Address::generate(&env);
    let token_a = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let token_b = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let token_c = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    let pool_hash = pool_wasm_hash(&env);

    let pool1 = factory_client.create_pair(&token_a, &token_b, &pool_hash);
    let pool2 = factory_client.create_pair(&token_a, &token_c, &pool_hash);

    assert_ne!(pool1, pool2);

    let contracts = factory_client.get_contracts_by_creator(&factory_id);
    assert_eq!(contracts.len(), 2);
    assert_eq!(contracts.get(0).unwrap(), pool1);
    assert_eq!(contracts.get(1).unwrap(), pool2);

    assert_eq!(factory_client.get_total_contracts(), 2);
}
