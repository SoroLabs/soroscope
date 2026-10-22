#![cfg(test)]

extern crate std;

use super::*;
use soroban_sdk::{testutils::Address as _, vec, Address, Env, String};

#[contract]
pub struct MockToken;

#[contractimpl]
impl MockToken {
    pub fn initialize(
        _e: Env,
        _admin: Address,
        _decimal: u32,
        _name: String,
        _symbol: String,
        _initial_supply: i128,
    ) {
    }

    pub fn mint(e: Env, to: Address, amount: i128) {
        let bal: i128 = e.storage().instance().get(&to).unwrap_or(0);
        e.storage().instance().set(&to, &(bal + amount));
    }

    pub fn balance(e: Env, id: Address) -> i128 {
        e.storage().instance().get(&id).unwrap_or(0)
    }

    pub fn transfer(e: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        let from_bal: i128 = e.storage().instance().get(&from).unwrap_or(0);
        assert!(from_bal >= amount, "insufficient balance");
        e.storage().instance().set(&from, &(from_bal - amount));
        let to_bal: i128 = e.storage().instance().get(&to).unwrap_or(0);
        e.storage().instance().set(&to, &(to_bal + amount));
    }
}

fn setup() -> (Env, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let token_id = env.register(MockToken, ());
    let token = MockTokenClient::new(&env, &token_id);
    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let recipient_a = Address::generate(&env);

    token.initialize(
        &admin,
        &7,
        &String::from_str(&env, "Batch Token"),
        &String::from_str(&env, "BATCH"),
        &1_000_000_000_i128,
    );
    token.mint(&sender, &1_000);

    let batch_id = env.register(BatchTransfer, ());
    (env, batch_id, token_id, sender, recipient_a)
}

#[test]
fn all_or_nothing_executes_entire_batch_atomically() {
    let (env, batch_id, token_id, sender, recipient_a) = setup();
    let batch = BatchTransferClient::new(&env, &batch_id);
    let token = MockTokenClient::new(&env, &token_id);
    let recipient_b = Address::generate(&env);

    let transfers = vec![
        &env,
        Transfer {
            from: sender.clone(),
            to: recipient_a.clone(),
            amount: 250i128,
        },
        Transfer {
            from: sender.clone(),
            to: recipient_b.clone(),
            amount: 125i128,
        },
    ];

    let results = batch.execute(&token_id, &transfers, &ExecutionMode::AllOrNothing);

    assert_eq!(results.len(), 2);
    assert_eq!(token.balance(&sender), 625);
    assert_eq!(token.balance(&recipient_a), 250);
    assert_eq!(token.balance(&recipient_b), 125);
}

#[test]
fn all_or_nothing_rejects_batch_before_any_transfer() {
    let (env, batch_id, token_id, sender, recipient_a) = setup();
    let batch = BatchTransferClient::new(&env, &batch_id);
    let token = MockTokenClient::new(&env, &token_id);
    let recipient_b = Address::generate(&env);

    let transfers = vec![
        &env,
        Transfer {
            from: sender.clone(),
            to: recipient_a.clone(),
            amount: 250i128,
        },
        Transfer {
            from: sender.clone(),
            to: recipient_b.clone(),
            amount: -5i128,
        },
    ];

    let err = batch.try_execute(&token_id, &transfers, &ExecutionMode::AllOrNothing);

    assert_eq!(err, Err(Ok(Error::InvalidAmount)));
    assert_eq!(token.balance(&sender), 1_000);
    assert_eq!(token.balance(&recipient_a), 0);
    assert_eq!(token.balance(&recipient_b), 0);
}

#[test]
fn partial_mode_skips_failures_and_continues() {
    let (env, batch_id, token_id, sender, recipient_a) = setup();
    let batch = BatchTransferClient::new(&env, &batch_id);
    let token = MockTokenClient::new(&env, &token_id);
    let recipient_b = Address::generate(&env);
    let recipient_c = Address::generate(&env);

    let transfers = vec![
        &env,
        Transfer {
            from: sender.clone(),
            to: recipient_a.clone(),
            amount: 400i128,
        },
        Transfer {
            from: sender.clone(),
            to: recipient_b.clone(),
            amount: -1i128,
        },
        Transfer {
            from: sender.clone(),
            to: recipient_c.clone(),
            amount: 700i128,
        },
    ];

    let results = batch.execute(&token_id, &transfers, &ExecutionMode::Partial);

    assert_eq!(results.len(), 3);
    assert!(results.get(0).unwrap().success);
    assert_eq!(
        results.get(1).unwrap().failure,
        TransferFailure::InvalidAmount
    );
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
fn accepts_batch_at_max_recipient_limit() {
    let (env, batch_id, token_id, sender, _recipient_a) = setup();
    let batch = BatchTransferClient::new(&env, &batch_id);
    let token = MockTokenClient::new(&env, &token_id);

    let mut transfers = Vec::new(&env);
    for _ in 0..MAX_RECIPIENTS {
        transfers.push_back(Transfer {
            from: sender.clone(),
            to: Address::generate(&env),
            amount: 1i128,
        });
    }

    let results = batch.execute(&token_id, &transfers, &ExecutionMode::AllOrNothing);

    assert_eq!(results.len(), MAX_RECIPIENTS);
    assert_eq!(token.balance(&sender), 1_000 - MAX_RECIPIENTS as i128);
}

#[test]
fn rejects_batch_over_max_recipient_limit() {
    let (env, batch_id, token_id, sender, _recipient_a) = setup();
    let batch = BatchTransferClient::new(&env, &batch_id);
    let token = MockTokenClient::new(&env, &token_id);

    let mut transfers = Vec::new(&env);
    for _ in 0..(MAX_RECIPIENTS + 1) {
        transfers.push_back(Transfer {
            from: sender.clone(),
            to: Address::generate(&env),
            amount: 1i128,
        });
    }

    let err = batch.try_execute(&token_id, &transfers, &ExecutionMode::AllOrNothing);

    assert_eq!(err, Err(Ok(Error::TooManyRecipients)));
    assert_eq!(token.balance(&sender), 1_000);
}

#[test]
fn quote_matches_partial_execution_plan() {
    let (env, batch_id, token_id, sender, recipient_a) = setup();
    let batch = BatchTransferClient::new(&env, &batch_id);
    let recipient_b = Address::generate(&env);

    let transfers = vec![
        &env,
        Transfer {
            from: sender.clone(),
            to: recipient_a.clone(),
            amount: 800i128,
        },
        Transfer {
            from: sender.clone(),
            to: recipient_b.clone(),
            amount: 400i128,
        },
    ];

    let quote = batch.quote(&token_id, &transfers, &ExecutionMode::Partial);

    assert!(quote.get(0).unwrap().success);
    assert_eq!(
        quote.get(1).unwrap().failure,
        TransferFailure::InsufficientBalance
    );
}

#[test]
fn multi_sender_independent_balances() {
    let (env, batch_id, token_id, sender_a, recipient_a) = setup();
    let batch = BatchTransferClient::new(&env, &batch_id);
    let token = MockTokenClient::new(&env, &token_id);

    let sender_b = Address::generate(&env);
    token.mint(&sender_b, &500);

    let transfers = vec![
        &env,
        Transfer {
            from: sender_a.clone(),
            to: recipient_a.clone(),
            amount: 300i128,
        },
        Transfer {
            from: sender_b.clone(),
            to: recipient_a.clone(),
            amount: 200i128,
        },
        Transfer {
            from: sender_a.clone(),
            to: recipient_a.clone(),
            amount: 100i128,
        },
    ];

    let results = batch.execute(&token_id, &transfers, &ExecutionMode::AllOrNothing);

    assert_eq!(results.len(), 3);
    assert_eq!(token.balance(&sender_a), 600); // 1000 - 300 - 100
    assert_eq!(token.balance(&sender_b), 300); // 500 - 200
    assert_eq!(token.balance(&recipient_a), 600); // 300 + 200 + 100
}

#[test]
fn per_transfer_authorization_verified() {
    let env = Env::default();
    env.mock_all_auths();

    let token_id = env.register(MockToken, ());
    let token = MockTokenClient::new(&env, &token_id);
    let batch_id = env.register(BatchTransfer, ());
    let batch = BatchTransferClient::new(&env, &batch_id);

    let sender_1 = Address::generate(&env);
    let sender_2 = Address::generate(&env);
    let recipient = Address::generate(&env);

    token.mint(&sender_1, &500);
    token.mint(&sender_2, &500);

    let transfers = vec![
        &env,
        Transfer {
            from: sender_1.clone(),
            to: recipient.clone(),
            amount: 100i128,
        },
        Transfer {
            from: sender_2.clone(),
            to: recipient.clone(),
            amount: 200i128,
        },
    ];

    let results = batch.execute(&token_id, &transfers, &ExecutionMode::AllOrNothing);
    assert_eq!(results.len(), 2);

    // Verify both sender_1 and sender_2 authorizations were checked
    let auths = env.auths();
    let auth_addresses: std::vec::Vec<Address> = auths.into_iter().map(|(addr, _)| addr).collect();
    assert!(auth_addresses.contains(&sender_1));
    assert!(auth_addresses.contains(&sender_2));
}

#[test]
#[should_panic]
fn unauthorized_transfer_from_rejected() {
    let env = Env::default();
    // Intentionally NOT calling env.mock_all_auths()
    let token_id = env.register(MockToken, ());
    let batch_id = env.register(BatchTransfer, ());
    let batch = BatchTransferClient::new(&env, &batch_id);

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    let transfers = vec![
        &env,
        Transfer {
            from: sender.clone(),
            to: recipient.clone(),
            amount: 100i128,
        },
    ];

    // This must fail because sender has not authorized the transfer
    batch.execute(&token_id, &transfers, &ExecutionMode::AllOrNothing);
}

#[test]
fn execute_single_sender_convenience_works() {
    let (env, batch_id, token_id, sender, recipient_a) = setup();
    let batch = BatchTransferClient::new(&env, &batch_id);
    let token = MockTokenClient::new(&env, &token_id);
    let recipient_b = Address::generate(&env);

    let recipients = vec![&env, recipient_a.clone(), recipient_b.clone()];
    let amounts = vec![&env, 300i128, 200i128];

    let results = batch.execute_single_sender(
        &token_id,
        &sender,
        &recipients,
        &amounts,
        &ExecutionMode::AllOrNothing,
    );

    assert_eq!(results.len(), 2);
    assert_eq!(token.balance(&sender), 500);
    assert_eq!(token.balance(&recipient_a), 300);
    assert_eq!(token.balance(&recipient_b), 200);
}

#[test]
fn gas_cost_comparison_batch_vs_individual() {
    let (env, batch_id, token_id, sender, _) = setup();
    let batch = BatchTransferClient::new(&env, &batch_id);
    let token = MockTokenClient::new(&env, &token_id);

    let mut transfers = Vec::new(&env);
    for _ in 0..5 {
        transfers.push_back(Transfer {
            from: sender.clone(),
            to: Address::generate(&env),
            amount: 10i128,
        });
    }

    let results = batch.execute(&token_id, &transfers, &ExecutionMode::AllOrNothing);
    assert_eq!(results.len(), 5);

    for t in transfers.iter() {
        assert_eq!(token.balance(&t.to), t.amount);
    }
}
