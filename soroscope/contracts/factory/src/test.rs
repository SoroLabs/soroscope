#![cfg(test)]
extern crate std;
use super::*;

use emergency_guard::PauseType;
use soroban_sdk::{
    testutils::Address as _,
    vec, Address, Env,
};

fn setup_factory(env: &Env) -> (Address, LiquidityPoolFactoryClient<'_>) {
    env.mock_all_auths();
    let factory_id = env.register(LiquidityPoolFactory, ());
    let client = LiquidityPoolFactoryClient::new(env, &factory_id);
    (factory_id, client)
}

#[test]
fn test_initialization() {
    let env = Env::default();
    let (_, client) = setup_factory(&env);

    let admin = Address::generate(&env);
    assert!(client.try_initialize(&admin).is_ok());

    let admins = client.get_admins();
    assert_eq!(admins.len(), 1);
    assert_eq!(admins.get(0).unwrap(), admin);
}

#[test]
fn test_double_initialization_fails() {
    let env = Env::default();
    let (_, client) = setup_factory(&env);

    let admin = Address::generate(&env);
    assert!(client.try_initialize(&admin).is_ok());
    assert_eq!(client.try_initialize(&admin), Err(Ok(Error::AlreadyInitialized)));
}

#[test]
fn test_set_operation_paused() {
    let env = Env::default();
    let (_, client) = setup_factory(&env);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    assert!(!client.is_paused(&PauseType::CREATE_PAIR));

    client.set_operation_paused(&admin, &PauseType::CREATE_PAIR, &true);
    assert!(client.is_paused(&PauseType::CREATE_PAIR));

    client.set_operation_paused(&admin, &PauseType::CREATE_PAIR, &false);
    assert!(!client.is_paused(&PauseType::CREATE_PAIR));
}

#[test]
fn test_pause_independent_operations() {
    let env = Env::default();
    let (_, client) = setup_factory(&env);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    client.set_operation_paused(&admin, &PauseType::SWAP, &true);
    assert!(client.is_paused(&PauseType::SWAP));
    assert!(!client.is_paused(&PauseType::CREATE_PAIR));
}

#[test]
fn test_unauthorized_pause() {
    let env = Env::default();
    let (_, client) = setup_factory(&env);

    let admin = Address::generate(&env);
    let stranger = Address::generate(&env);
    client.initialize(&admin);

    assert_eq!(
        client.try_set_operation_paused(&stranger, &PauseType::CREATE_PAIR, &true),
        Err(Ok(Error::Unauthorized))
    );
}

#[test]
fn test_emergency_pause_and_resume() {
    let env = Env::default();
    let (_, client) = setup_factory(&env);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    let approvers = vec![&env, admin.clone()];
    client.emergency_pause(&approvers);

    assert!(client.is_paused(&PauseType::CREATE_PAIR));
    assert!(client.is_paused(&PauseType::SWAP));

    client.resume(&approvers);
    assert!(!client.is_paused(&PauseType::CREATE_PAIR));
    assert!(!client.is_paused(&PauseType::SWAP));
}

#[test]
fn test_add_remove_admin() {
    let env = Env::default();
    let (_, client) = setup_factory(&env);

    let admin1 = Address::generate(&env);
    let admin2 = Address::generate(&env);
    let new_admin = Address::generate(&env);

    client.initialize(&admin1);

    let admins = client.get_admins();
    assert_eq!(admins.len(), 1);

    let approvers = vec![&env, admin1.clone(), admin2.clone()];
    client.add_admin(&approvers, &new_admin);

    let admins = client.get_admins();
    assert_eq!(admins.len(), 2);

    client.remove_admin(&approvers, &new_admin);
    let admins = client.get_admins();
    assert_eq!(admins.len(), 1);
}

#[test]
fn test_get_pair() {
    let env = Env::default();
    let (_, client) = setup_factory(&env);

    let token_admin = Address::generate(&env);
    let token_a = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let token_b = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    let result = client.get_pair(&token_a, &token_b);
    assert_eq!(result, None);
}

#[test]
fn test_get_pair_reverse_order() {
    let env = Env::default();
    let (_, client) = setup_factory(&env);

    let token_admin = Address::generate(&env);
    let token_a = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let token_b = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    let result_a_b = client.get_pair(&token_a, &token_b);
    let result_b_a = client.get_pair(&token_b, &token_a);
    assert_eq!(result_a_b, result_b_a);
}

#[test]
fn test_get_pause_state() {
    let env = Env::default();
    let (_, client) = setup_factory(&env);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    assert_eq!(client.get_pause_state(), 0);

    client.set_operation_paused(&admin, &PauseType::CREATE_PAIR, &true);
    assert!(client.get_pause_state() & PauseType::CREATE_PAIR != 0);
}
