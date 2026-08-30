#![cfg(test)]

use super::*;
use soroban_sdk::Env;

#[test]
fn test() {
    let env = Env::default();
    let contract_id = env.register(HelloContract, ());
    let client = HelloContractClient::new(&env, &contract_id);

    let words = client.hello(&symbol_short!("Dev"));
    assert_eq!(
        words,
        vec![&env, symbol_short!("Hello"), symbol_short!("Dev"),]
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
