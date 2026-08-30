#![cfg(test)]

use super::*;
use soroban_sdk::{Env, Vec};

fn setup(env: &Env) -> CpuHeavyContractClient<'_> {
    let contract_id = env.register(CpuHeavyContract, ());
    CpuHeavyContractClient::new(env, &contract_id)
}

/// Builds a descending `[n-1, ..., 0]` list, the worst case for bubble sort.
fn descending(env: &Env, n: u32) -> Vec<u32> {
    let mut v = Vec::new(env);
    for i in (0..n).rev() {
        v.push_back(i);
    }
    v
}

// ── Happy paths ──────────────────────────────────────────────────────────────

#[test]
fn test_benchmarks_run_successfully() {
    let env = Env::default();
    let client = setup(&env);

    assert_eq!(client.fibonacci_iterative(&20), 6765);
    assert_eq!(client.count_primes(&100), 25);
    assert_eq!(
        client.bubble_sort(&Vec::from_array(&env, [5, 3, 8, 1])),
        Vec::from_array(&env, [1, 3, 5, 8])
    );
    assert!(client.nested_loop_burn(&10, &10) > 0);
}

#[test]
fn test_combined_benchmark() {
    let env = Env::default();
    let client = setup(&env);

    let results = client.combined_benchmark(&100, &20, &50);
    // Combined returns results for Fibonacci and prime counting.
    assert_eq!(results.len(), 2);
    assert_eq!(results.get(1).unwrap(), 15);
}

#[test]
fn test_bubble_sort_handles_degenerate_lengths() {
    let env = Env::default();
    let client = setup(&env);

    assert_eq!(client.bubble_sort(&Vec::new(&env)), Vec::new(&env));
    assert_eq!(
        client.bubble_sort(&Vec::from_array(&env, [7])),
        Vec::from_array(&env, [7])
    );
}

// ── Guards accept inputs exactly at the cap ──────────────────────────────────

#[test]
fn test_inputs_at_the_cap_are_accepted() {
    let env = Env::default();
    let client = setup(&env);

    // Each of these is the largest value the contract promises to serve. They
    // must succeed, otherwise the cap is advertising work it cannot deliver.
    client.fibonacci_iterative(&MAX_FIB);
    client.count_primes(&MAX_PRIME);
    client.bubble_sort(&descending(&env, MAX_SORT));
    client.nested_loop_burn(&MAX_LOOP_OPS, &1);
    client.combined_benchmark(&MAX_COMBINED_FIB, &MAX_COMBINED_SORT, &MAX_COMBINED_PRIME);
}

/// The whole point of the caps: the most expensive legal call must finish
/// inside the default network budget rather than aborting the host.
///
/// This measures the *native* build, so it accounts for host-call cost (which
/// is what makes `bubble_sort` quadratically expensive) but not for metered
/// WASM instructions. The WASM figures behind each constant are recorded in
/// `lib.rs`.
#[test]
fn test_worst_case_call_stays_within_default_budget() {
    let env = Env::default();
    let client = setup(&env);

    env.cost_estimate().budget().reset_default();
    client.bubble_sort(&descending(&env, MAX_SORT));

    let budget = env.cost_estimate().budget();
    assert!(
        budget.cpu_instruction_cost() < 100_000_000,
        "bubble_sort at MAX_SORT burned {} CPU instructions",
        budget.cpu_instruction_cost()
    );
    assert!(
        budget.memory_bytes_cost() < 41_943_040,
        "bubble_sort at MAX_SORT allocated {} memory bytes",
        budget.memory_bytes_cost()
    );
}

// ── Guards reject inputs past the cap with a clean error code ────────────────

#[test]
fn test_fibonacci_rejects_oversized_input() {
    let env = Env::default();
    let client = setup(&env);

    assert_eq!(
        client.try_fibonacci_iterative(&(MAX_FIB + 1)),
        Err(Ok(Error::FibonacciInputTooLarge))
    );
    assert_eq!(
        client.try_fibonacci_iterative(&u32::MAX),
        Err(Ok(Error::FibonacciInputTooLarge))
    );
}

#[test]
fn test_bubble_sort_rejects_oversized_input() {
    let env = Env::default();
    let client = setup(&env);

    assert_eq!(
        client.try_bubble_sort(&descending(&env, MAX_SORT + 1)),
        Err(Ok(Error::SortInputTooLarge))
    );
}

#[test]
fn test_count_primes_rejects_oversized_input() {
    let env = Env::default();
    let client = setup(&env);

    assert_eq!(
        client.try_count_primes(&(MAX_PRIME + 1)),
        Err(Ok(Error::PrimeLimitTooLarge))
    );
}

#[test]
fn test_nested_loop_burn_rejects_oversized_input() {
    let env = Env::default();
    let client = setup(&env);

    assert_eq!(
        client.try_nested_loop_burn(&(MAX_LOOP_OPS + 1), &1),
        Err(Ok(Error::LoopOpsTooLarge))
    );
    // Neither factor is individually huge, but the product is.
    assert_eq!(
        client.try_nested_loop_burn(&1_000, &1_000),
        Err(Ok(Error::LoopOpsTooLarge))
    );
}

/// `outer * inner` overflows `u32`, so a naive `outer * inner` guard would wrap
/// to a small number and wave the call through. `saturating_mul` must not.
#[test]
fn test_nested_loop_burn_guard_survives_multiplication_overflow() {
    let env = Env::default();
    let client = setup(&env);

    assert_eq!(
        client.try_nested_loop_burn(&65_536, &65_536),
        Err(Ok(Error::LoopOpsTooLarge))
    );
    assert_eq!(
        client.try_nested_loop_burn(&u32::MAX, &u32::MAX),
        Err(Ok(Error::LoopOpsTooLarge))
    );
}

#[test]
fn test_combined_benchmark_rejects_each_oversized_argument() {
    let env = Env::default();
    let client = setup(&env);

    let over = [
        (MAX_COMBINED_FIB + 1, 10, 100),
        (100, MAX_COMBINED_SORT + 1, 100),
        (100, 10, MAX_COMBINED_PRIME + 1),
    ];
    for (fib_n, sort_size, prime_limit) in over {
        assert_eq!(
            client.try_combined_benchmark(&fib_n, &sort_size, &prime_limit),
            Err(Ok(Error::CombinedInputTooLarge))
        );
    }
}

/// An over-cap call must fail cleanly rather than trapping, so it has to leave
/// the budget essentially untouched — that is what distinguishes a guard from a
/// host budget abort.
#[test]
fn test_rejected_call_does_no_work() {
    let env = Env::default();
    let client = setup(&env);

    env.cost_estimate().budget().reset_default();
    let _ = client.try_count_primes(&(MAX_PRIME + 1));

    assert!(
        env.cost_estimate().budget().cpu_instruction_cost() < 1_000_000,
        "rejected call burned {} CPU instructions before failing",
        env.cost_estimate().budget().cpu_instruction_cost()
    );
}

/// Error discriminants are part of the contract's public surface: the SoroScope
/// UI decodes them by number, so reordering the enum would silently change what
/// users see.
#[test]
fn test_error_codes_are_stable() {
    assert_eq!(Error::FibonacciInputTooLarge as u32, 1);
    assert_eq!(Error::SortInputTooLarge as u32, 2);
    assert_eq!(Error::PrimeLimitTooLarge as u32, 3);
    assert_eq!(Error::LoopOpsTooLarge as u32, 4);
    assert_eq!(Error::CombinedInputTooLarge as u32, 5);
}
