use super::{ContractA, ContractAClient, ContractB, Error, MAX_DEPTH};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env};

fn setup(env: &Env) -> (Address, Address, ContractAClient<'_>) {
    let contract_b_id = Address::generate(env);
    env.register_at(&contract_b_id, ContractB, ());

    let contract_a_id = Address::generate(env);
    env.register_at(&contract_a_id, ContractA, ());

    let client_a = ContractAClient::new(env, &contract_a_id);
    (contract_a_id, contract_b_id, client_a)
}

#[test]
fn test_cross_contract_call() {
    let env = Env::default();
    let (_, contract_b_id, client_a) = setup(&env);

    let result = client_a.call_b(&contract_b_id, &41);

    assert_eq!(result, 42);
}

#[test]
fn test_bench_cross_hop_depth() {
    let env = Env::default();
    let (_, contract_b_id, client_a) = setup(&env);

    assert_eq!(client_a.bench_cross_hop_depth(&contract_b_id, &4), 4);
    assert_eq!(
        client_a.bench_cross_hop_depth(&contract_b_id, &MAX_DEPTH),
        MAX_DEPTH
    );
}

#[test]
fn test_bench_cross_hop_depth_rejects_excessive_depth() {
    let env = Env::default();
    let (_, contract_b_id, client_a) = setup(&env);

    assert_eq!(
        client_a.try_bench_cross_hop_depth(&contract_b_id, &(MAX_DEPTH + 1)),
        Err(Ok(Error::DepthExceeded))
    );
    assert_eq!(
        client_a.try_bench_cross_hop_depth(&contract_b_id, &u32::MAX),
        Err(Ok(Error::DepthExceeded))
    );
}

#[test]
fn test_bench_cross_hop_depth_rejects_self_invocation() {
    let env = Env::default();
    let (contract_a_id, _, client_a) = setup(&env);

    assert_eq!(
        client_a.try_bench_cross_hop_depth(&contract_a_id, &1),
        Err(Ok(Error::SelfInvocation))
    );
    assert_eq!(
        client_a.try_call_b(&contract_a_id, &41),
        Err(Ok(Error::SelfInvocation))
    );
}
