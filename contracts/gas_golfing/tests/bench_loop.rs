use gas_golfing::{GasGolfingContract, GasGolfingContractClient};
use soroban_sdk::Env;

fn bench_loop(n: u32) -> (u64, u64) {
    let env = Env::default();
    let contract_id = env.register(GasGolfingContract, ());
    let client = GasGolfingContractClient::new(&env, &contract_id);

    let standard_result = client.standard_loop(&n);
    let standard_cpu_instructions = env.cost_estimate().budget().cpu_instruction_cost();

    let unrolled_result = client.unrolled_loop(&n);
    let unrolled_cpu_instructions = env.cost_estimate().budget().cpu_instruction_cost();

    assert_eq!(
        standard_result, unrolled_result,
        "loop results differ for n={n}"
    );
    (standard_cpu_instructions, unrolled_cpu_instructions)
}

#[test]
fn loop_variants_return_the_same_result_and_instruction_counts() {
    for n in [0, 1, 3, 4, 5, 31, 1_000] {
        let (standard_cpu, unrolled_cpu) = bench_loop(n);

        println!("[gas-golfing] n={n} standard_cpu={standard_cpu} unrolled_cpu={unrolled_cpu}");
        assert!(standard_cpu > 0, "standard loop was not metered for n={n}");
        assert!(unrolled_cpu > 0, "unrolled loop was not metered for n={n}");
    }
}
