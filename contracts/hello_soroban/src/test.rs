#![cfg(test)]

use super::*;
use soroban_sdk::Env;

#[test]
fn test() {
    let env = Env::default();
    let contract_id = env.register_contract(None, HelloContract);
    let client = HelloContractClient::new(&env, &contract_id);

    let words = client.hello(&symbol_short!("Dev"));
    std::println!("Contract Output: {:?}", words);
    assert_eq!(
        words,
        vec![&env, symbol_short!("Hello"), symbol_short!("Dev"),]
    );
}
