#![cfg(test)]

use super::*;
use soroban_sdk::Env;

#[test]
fn test_hello_returns_expected_greeting() {
    let env = Env::default();
    let contract_id = env.register(HelloContract, ());
    let client = HelloContractClient::new(&env, &contract_id);

    let greeting = client.hello(&symbol_short!("Dev"));

    assert_eq!(
        greeting,
        vec![&env, symbol_short!("Hello"), symbol_short!("Dev")]
    );
}

#[test]
fn test_hello_accepts_multiple_valid_symbols() {
    let env = Env::default();
    let contract_id = env.register(HelloContract, ());
    let client = HelloContractClient::new(&env, &contract_id);

    let greeting = client.hello(&symbol_short!("Alice"));
    assert_eq!(
        greeting,
        vec![&env, symbol_short!("Hello"), symbol_short!("Alice")]
    );

    let second_greeting = client.hello(&symbol_short!("Bob"));
    assert_eq!(
        second_greeting,
        vec![&env, symbol_short!("Hello"), symbol_short!("Bob")]
    );
}

#[test]
fn test_rejects_empty_symbol() {
    let env = Env::default();
    let result = std::panic::catch_unwind(|| {
        let _ = Symbol::new(&env, "");
    });

    assert!(result.is_err(), "empty symbols should be rejected");
}

#[test]
fn test_rejects_oversized_symbol() {
    let env = Env::default();
    let result = std::panic::catch_unwind(|| {
        let _ = Symbol::new(
            &env,
            "this-symbol-name-is-longer-than-thirty-two-characters",
        );
    });

    assert!(result.is_err(), "oversized symbols should be rejected");
}
