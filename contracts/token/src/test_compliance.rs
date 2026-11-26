#![cfg(test)]

//! SEP-41 compliance controls: supply cap enforcement, account freeze flags
//! and admin clawback.

use crate::contract::{Token, TokenClient};
use crate::error::Error;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn setup(env: &Env, max_supply: i128) -> TokenClient<'_> {
    let contract_id = env.register(Token, ());
    let client = TokenClient::new(env, &contract_id);
    let admin = Address::generate(env);
    client.initialize(
        &admin,
        &7,
        &String::from_str(env, "Test Token"),
        &String::from_str(env, "TEST"),
        &max_supply,
    );
    client
}

/// Extracts the contract error code from a failed invocation. Functions that
/// report failures with `panic_with_error!` surface the code, not a typed
/// `Error` value, through the generated client.
fn contract_error_code<T, C>(
    result: Result<Result<T, C>, Result<soroban_sdk::Error, soroban_sdk::InvokeError>>,
) -> u32 {
    match result {
        Err(Ok(err)) => err.get_code(),
        _ => panic!("expected the invocation to fail with a contract error"),
    }
}

/// Minting above `max_supply` reverts with `Error::SupplyCapExceeded`.
#[test]
fn test_mint_above_max_supply_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let client = setup(&env, 1_000);
    let user = Address::generate(&env);

    client.mint(&user, &1_000);
    assert_eq!(client.total_supply(), 1_000);
    assert_eq!(client.max_supply(), 1_000);

    let result = client.try_mint(&user, &1);
    assert_eq!(contract_error_code(result), Error::SupplyCapExceeded as u32);

    // Supply and balances are untouched by the rejected mint.
    assert_eq!(client.total_supply(), 1_000);
    assert_eq!(client.balance(&user), 1_000);
}

/// A frozen account cannot send tokens; re-authorizing restores transfers.
#[test]
fn test_frozen_account_cannot_send() {
    let env = Env::default();
    env.mock_all_auths();
    let client = setup(&env, 1_000_000);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    client.mint(&alice, &500);
    assert!(client.authorized(&alice));

    client.set_authorized(&alice, &false);
    assert!(!client.authorized(&alice));

    let result = client.try_transfer(&alice, &bob, &100);
    assert_eq!(contract_error_code(result), Error::AccountFrozen as u32);
    assert_eq!(client.balance(&alice), 500);
    assert_eq!(client.balance(&bob), 0);

    client.set_authorized(&alice, &true);
    assert!(client.authorized(&alice));
    client.transfer(&alice, &bob, &100);
    assert_eq!(client.balance(&alice), 400);
    assert_eq!(client.balance(&bob), 100);
}

/// A frozen account cannot receive tokens.
#[test]
fn test_frozen_account_cannot_receive() {
    let env = Env::default();
    env.mock_all_auths();
    let client = setup(&env, 1_000_000);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    client.mint(&alice, &500);
    client.set_authorized(&bob, &false);

    let result = client.try_transfer(&alice, &bob, &100);
    assert_eq!(contract_error_code(result), Error::AccountFrozen as u32);
    assert_eq!(client.balance(&alice), 500);
    assert_eq!(client.balance(&bob), 0);
}

/// Allowance spenders cannot move tokens out of a frozen account.
#[test]
fn test_frozen_account_cannot_transfer_from() {
    let env = Env::default();
    env.mock_all_auths();
    let client = setup(&env, 1_000_000);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let spender = Address::generate(&env);

    client.mint(&alice, &500);
    client.approve(&alice, &spender, &200, &100);
    client.set_authorized(&alice, &false);

    let result = client.try_transfer_from(&spender, &alice, &bob, &100);
    assert_eq!(contract_error_code(result), Error::AccountFrozen as u32);
    assert_eq!(client.balance(&alice), 500);
    assert_eq!(client.allowance(&alice, &spender), 200);
}

/// Admin clawback burns the requested balance and reduces total supply.
#[test]
fn test_clawback_reduces_balance_and_supply() {
    let env = Env::default();
    env.mock_all_auths();
    let client = setup(&env, 1_000_000);
    let user = Address::generate(&env);

    client.mint(&user, &500);
    client.clawback(&user, &200);

    assert_eq!(client.balance(&user), 300);
    assert_eq!(client.total_supply(), 300);
}

/// Clawback larger than the balance reverts without changing state.
#[test]
fn test_clawback_above_balance_reverts() {
    let env = Env::default();
    env.mock_all_auths();
    let client = setup(&env, 1_000_000);
    let user = Address::generate(&env);

    client.mint(&user, &500);
    let result = client.try_clawback(&user, &501);
    assert_eq!(result, Err(Ok(Error::InsufficientBalance)));
    assert_eq!(client.balance(&user), 500);
    assert_eq!(client.total_supply(), 500);
}

/// Clawback can still reach a frozen account (compliance seizure).
#[test]
fn test_clawback_works_on_frozen_account() {
    let env = Env::default();
    env.mock_all_auths();
    let client = setup(&env, 1_000_000);
    let user = Address::generate(&env);

    client.mint(&user, &500);
    client.set_authorized(&user, &false);
    client.clawback(&user, &500);

    assert_eq!(client.balance(&user), 0);
    assert_eq!(client.total_supply(), 0);
    assert!(!client.authorized(&user));
}
