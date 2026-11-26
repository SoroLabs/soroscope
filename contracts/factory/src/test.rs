#[cfg(test)]
extern crate std;
use super::*;

use soroban_sdk::{vec, Address, BytesN, Env};

mod liquidity_pool {
    soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/liquidity_pool.wasm");
}

fn pool_wasm_hash(env: &Env) -> BytesN<32> {
    env.deployer().upload_contract_wasm(liquidity_pool::WASM)
}

fn setup_factory() -> (
    Env,
    LiquidityPoolFactoryClient<'static>,
    Address,
    Address,
    Address,
    BytesN<32>,
) {
    let env = Env::default();
    env.mock_all_auths();

    let factory_id = env.register(LiquidityPoolFactory, ());
    let client = LiquidityPoolFactoryClient::new(&env, &factory_id);

    let admin = Address::generate(&env);
    let token_a = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let pool_hash = pool_wasm_hash(&env);

    client.initialize(&admin);
    client.approve_wasm_hash(&vec![&env, admin.clone()], &pool_hash);

    (env, client, admin, token_a, token_b, pool_hash)
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
    let (_env, factory_client, admin, token_a, token_b, pool_hash) = setup_factory();

    const PAUSE_CREATE_PAIR_FLAG: u32 = 1 << 6;

    // Pause create_pair operation
    factory_client.set_guard_pause(&admin, &PAUSE_CREATE_PAIR_FLAG, &true);

    // Attempt to create a pair while paused should fail
    let result = factory_client.try_create_pair(&token_a, &token_b, &pool_hash);
    assert_eq!(result, Err(Ok(Error::Paused)));

    // Unpause create_pair operation
    factory_client.set_guard_pause(&admin, &PAUSE_CREATE_PAIR_FLAG, &false);

    // Pair creation should now succeed
    let created = factory_client.create_pair(&token_a, &token_b, &pool_hash);
    assert_eq!(factory_client.get_pair(&token_a, &token_b), Some(created));
}

#[test]
fn test_duplicate_pair_errors() {
    let (_env, factory_client, _admin, token_a, token_b, pool_hash) = setup_factory();

    // First creation succeeds
    factory_client.create_pair(&token_a, &token_b, &pool_hash);

    // Second creation with the same pair should return a pool-exists error
    let result = factory_client.try_create_pair(&token_a, &token_b, &pool_hash);
    assert_eq!(result, Err(Ok(Error::PoolAlreadyExists)));
}

#[test]
fn test_reversed_tokens_resolve_to_same_canonical_pair() {
    let (_env, factory_client, _admin, token_a, token_b, pool_hash) = setup_factory();

    let created_ab = factory_client.create_pair(&token_a, &token_b, &pool_hash);
    assert_eq!(
        factory_client.get_pair(&token_b, &token_a),
        Some(created_ab.clone())
    );
    assert_eq!(
        factory_client.try_create_pair(&token_b, &token_a, &pool_hash),
        Err(Ok(Error::PoolAlreadyExists))
    );
}

#[test]
fn test_identical_tokens_rejected() {
    let (_env, factory_client, _admin, token_a, _token_b, pool_hash) = setup_factory();

    assert_eq!(
        factory_client.try_create_pair(&token_a, &token_a, &pool_hash),
        Err(Ok(Error::IdenticalTokens))
    );
}

#[test]
fn test_wasm_hash_whitelist_enforced() {
    let (env, factory_client, admin, token_a, token_b, pool_hash) = setup_factory();

    assert!(factory_client.is_wasm_hash_approved(&pool_hash));

    let other_hash = BytesN::from_array(&env, &[7u8; 32]);
    assert!(!factory_client.is_wasm_hash_approved(&other_hash));
    assert_eq!(
        factory_client.try_create_pair(&token_a, &token_b, &other_hash),
        Err(Ok(Error::WasmHashNotApproved))
    );

    let approvers = vec![&env, admin.clone()];
    factory_client.revoke_wasm_hash(&approvers, &pool_hash);
    assert!(!factory_client.is_wasm_hash_approved(&pool_hash));

    factory_client.approve_wasm_hash(&approvers, &pool_hash);
    assert!(factory_client.is_wasm_hash_approved(&pool_hash));

    let created = factory_client.create_pair(&token_a, &token_b, &pool_hash);
    assert_eq!(factory_client.get_pair(&token_b, &token_a), Some(created));
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

    let admins = soroban_sdk::vec![&env, admin1.clone(), admin2.clone()];
    factory_client.initialize_guard(&admins, &2);

    // Verify initial setup
    let config = factory_client.get_multisig_config();
    assert_eq!(config.admins.len(), 2);
    assert_eq!(config.threshold, 2);

    // Add admin3 using multi-sig approval
    let approvers = soroban_sdk::vec![&env, admin1.clone(), admin2.clone()];
    factory_client.add_guard_admin(&approvers, &admin3);

    assert!(factory_client.is_admin(&admin3));
    assert_eq!(factory_client.get_admins().len(), 3);

    // Remove admin3
    factory_client.remove_guard_admin(&approvers, &admin3);
    assert!(!factory_client.is_admin(&admin3));
    assert_eq!(factory_client.get_admins().len(), 2);
}
