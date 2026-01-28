#![cfg(test)]

use super::*;
use soroban_sdk::{Env, Vec};

#[test]
fn test_benchmarks_run_successfully() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CpuHeavyContract);
    let client = CpuHeavyContractClient::new(&env, &contract_id);

    // Test Fibonacci
    let fib_res = client.fibonacci_iterative(&20);
    assert_eq!(fib_res, 6765);

    // Test Prime Counting
    let prime_count = client.count_primes(&100);
    assert_eq!(prime_count, 25);

    // Test Bubble Sort
    let unsorted = Vec::from_array(&env, [5, 3, 8, 1]);
    let sorted = client.bubble_sort(&unsorted);
    assert_eq!(sorted, Vec::from_array(&env, [1, 3, 5, 8]));

    // Test Nested Loop
    let loop_res = client.nested_loop_burn(&10, &10);
    assert!(loop_res > 0);
}

#[test]
fn test_combined_benchmark() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CpuHeavyContract);
    let client = CpuHeavyContractClient::new(&env, &contract_id);

    let results = client.combined_benchmark(&100, &20, &50);
    // Combined returns results for Fibonacci and Prime counting
    assert_eq!(results.len(), 2); 
}

#[test]
#[should_panic(expected = "input too large")]
fn test_safety_limits() {
    let env = Env::default();
    let contract_id = env.register_contract(None, CpuHeavyContract);
    let client = CpuHeavyContractClient::new(&env, &contract_id);

    client.fibonacci_iterative(&60_000);
}