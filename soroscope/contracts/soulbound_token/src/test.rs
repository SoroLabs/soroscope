use crate::contract::{SoulboundToken, SoulboundTokenClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_mint_and_balance() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SoulboundToken, ());
    let client = SoulboundTokenClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);

    client.initialize(
        &admin,
        &0,
        &String::from_str(&env, "Soulbound Token"),
        &String::from_str(&env, "SBT"),
    );

    client.mint(&user1);
    assert_eq!(client.balance(&user1), 1);
}

#[test]
#[should_panic(expected = "soulbound tokens cannot be transferred")]
fn test_transfer_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SoulboundToken, ());
    let client = SoulboundTokenClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    client.initialize(
        &admin,
        &0,
        &String::from_str(&env, "Soulbound Token"),
        &String::from_str(&env, "SBT"),
    );

    client.mint(&user1);
    client.transfer(&user1, &user2, &1);
}

#[test]
fn test_admin_transfer() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SoulboundToken, ());
    let client = SoulboundTokenClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    client.initialize(
        &admin,
        &0,
        &String::from_str(&env, "Soulbound Token"),
        &String::from_str(&env, "SBT"),
    );

    client.mint(&user1);
    assert_eq!(client.balance(&user1), 1);
    assert_eq!(client.balance(&user2), 0);

    client.admin_transfer(&user1, &user2);
    assert_eq!(client.balance(&user1), 0);
    assert_eq!(client.balance(&user2), 1);
}

#[test]
fn test_burn() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SoulboundToken, ());
    let client = SoulboundTokenClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);

    client.initialize(
        &admin,
        &0,
        &String::from_str(&env, "Soulbound Token"),
        &String::from_str(&env, "SBT"),
    );

    client.mint(&user1);
    assert_eq!(client.balance(&user1), 1);

    client.burn(&user1);
    assert_eq!(client.balance(&user1), 0);
}

#[test]
#[should_panic(expected = "cannot hold more than one soulbound token")]
fn test_mint_twice_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(SoulboundToken, ());
    let client = SoulboundTokenClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);

    client.initialize(
        &admin,
        &0,
        &String::from_str(&env, "Soulbound Token"),
        &String::from_str(&env, "SBT"),
    );

    client.mint(&user1);
    client.mint(&user1);
}