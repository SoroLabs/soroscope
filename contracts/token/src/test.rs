#![cfg(test)]

use crate::contract::{Token, TokenClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

#[test]
fn test_mint_and_transfer() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, Token);
    let client = TokenClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    client.initialize(
        &admin,
        &7,
        &String::from_str(&env, "Test Token"),
        &String::from_str(&env, "TEST"),
    );

    client.mint(&user1, &1000);
    assert_eq!(client.balance(&user1), 1000);

    client.transfer(&user1, &user2, &200);
    assert_eq!(client.balance(&user1), 800);
    assert_eq!(client.balance(&user2), 200);
}

#[test]
fn test_allowance() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, Token);
    let client = TokenClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let spender = Address::generate(&env);

    client.initialize(
        &admin,
        &7,
        &String::from_str(&env, "Test Token"),
        &String::from_str(&env, "TEST"),
    );

    client.mint(&user1, &1000);

    client.approve(&user1, &spender, &500, &200);
    assert_eq!(client.allowance(&user1, &spender), 500);

    client.transfer_from(&spender, &user1, &spender, &200);
    assert_eq!(client.balance(&user1), 800);
    assert_eq!(client.balance(&spender), 200);
    assert_eq!(client.allowance(&user1, &spender), 300);
}
