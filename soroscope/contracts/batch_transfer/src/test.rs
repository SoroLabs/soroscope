#![cfg(test)]

extern crate std;

use super::*;
use soroban_sdk::{testutils::Address as _, vec, Address, Env, String};
use token_contract::{Token, TokenClient};

fn setup() -> (Env, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let token_id = env.register(Token, ());
    let token = TokenClient::new(&env, &token_id);
    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let recipient_a = Address::generate(&env);
    let recipient_b = Address::generate(&env);

    token.initialize(
        &admin,
        &7,
        &String::from_str(&env, "Batch Token"),
        &String::from_str(&env, "BATCH"),
    );
    token.mint(&sender, &1_000);

    let batch_id = env.register(BatchTransfer, ());
    (env, batch_id, token_id, sender, recipient_a)
}

#[test]
fn all_or_nothing_executes_entire_batch_atomically() {
    let (env, batch_id, token_id, sender, recipient_a) = setup();
    let batch = BatchTransferClient::new(&env, &batch_id);
    let token = TokenClient::new(&env, &token_id);
    let recipient_b = Address::generate(&env);

    let recipients = vec![&env, recipient_a.clone(), recipient_b.clone()];
    let amounts = vec![&env, 250i128, 125i128];

    let results = batch.execute(
        &token_id,
        &sender,
        &recipients,
        &amounts,
        &ExecutionMode::AllOrNothing,
    );

    assert_eq!(results.len(), 2);
    assert_eq!(token.balance(&sender), 625);
    assert_eq!(token.balance(&recipient_a), 250);
    assert_eq!(token.balance(&recipient_b), 125);
}

#[test]
fn all_or_nothing_rejects_batch_before_any_transfer() {
    let (env, batch_id, token_id, sender, recipient_a) = setup();
    let batch = BatchTransferClient::new(&env, &batch_id);
    let token = TokenClient::new(&env, &token_id);
    let recipient_b = Address::generate(&env);

    let recipients = vec![&env, recipient_a.clone(), recipient_b.clone()];
    let amounts = vec![&env, 250i128, -5i128];

    let err = batch.try_execute(
        &token_id,
        &sender,
        &recipients,
        &amounts,
        &ExecutionMode::AllOrNothing,
    );

    assert_eq!(err, Err(Ok(Error::InvalidAmount)));
    assert_eq!(token.balance(&sender), 1_000);
    assert_eq!(token.balance(&recipient_a), 0);
    assert_eq!(token.balance(&recipient_b), 0);
}

#[test]
fn partial_mode_skips_failures_and_continues() {
    let (env, batch_id, token_id, sender, recipient_a) = setup();
    let batch = BatchTransferClient::new(&env, &batch_id);
    let token = TokenClient::new(&env, &token_id);
    let recipient_b = Address::generate(&env);
    let recipient_c = Address::generate(&env);

    let recipients = vec![
        &env,
        recipient_a.clone(),
        recipient_b.clone(),
        recipient_c.clone()
    ];
    let amounts = vec![&env, 400i128, -1i128, 700i128];

    let results = batch.execute(
        &token_id,
        &sender,
        &recipients,
        &amounts,
        &ExecutionMode::Partial,
    );

    assert_eq!(results.len(), 3);
    assert_eq!(results.get(0).unwrap().success, true);
    assert_eq!(results.get(1).unwrap().failure, TransferFailure::InvalidAmount);
    assert_eq!(
        results.get(2).unwrap().failure,
        TransferFailure::InsufficientBalance
    );
    assert_eq!(token.balance(&sender), 600);
    assert_eq!(token.balance(&recipient_a), 400);
    assert_eq!(token.balance(&recipient_b), 0);
    assert_eq!(token.balance(&recipient_c), 0);
}

#[test]
fn quote_matches_partial_execution_plan() {
    let (env, batch_id, token_id, sender, recipient_a) = setup();
    let batch = BatchTransferClient::new(&env, &batch_id);
    let recipient_b = Address::generate(&env);

    let recipients = vec![&env, recipient_a.clone(), recipient_b.clone()];
    let amounts = vec![&env, 800i128, 400i128];

    let quote = batch.quote(
        &token_id,
        &sender,
        &recipients,
        &amounts,
        &ExecutionMode::Partial,
    );

    assert_eq!(quote.get(0).unwrap().success, true);
    assert_eq!(
        quote.get(1).unwrap().failure,
        TransferFailure::InsufficientBalance
    );
}
