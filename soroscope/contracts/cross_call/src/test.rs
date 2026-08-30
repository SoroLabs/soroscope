use super::{ContractA, ContractAClient, ContractB};
use soroban_sdk::{Address, Env};
use soroban_sdk::testutils::Address as _;

#[test]
fn test_cross_contract_call() {
    let env = Env::default();
    
    let contract_b_id = Address::generate(&env);
    env.register_at(&contract_b_id, ContractB, ());

    let contract_a_id = Address::generate(&env);
    env.register_at(&contract_a_id, ContractA, ());

    let client_a = ContractAClient::new(&env, &contract_a_id);
    let result = client_a.call_b(&contract_b_id, &41);

    assert_eq!(result, 42);
}
