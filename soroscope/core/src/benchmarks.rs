use crate::simulation_service::{SimulationMetric, SimulationService};
use sha2::{Digest, Sha256};
use soroban_sdk::{
    testutils::Address as _, Address, Bytes, Env, IntoVal, String, Symbol, Val, Vec,
};
use std::fs;
use std::path::PathBuf;

pub async fn run_token_benchmark(
    wasm_path: PathBuf,
    simulation_service: &SimulationService,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Loading contract from: {:?}", wasm_path);
    let wasm = fs::read(wasm_path)?;
    let code_hash = format!("{:x}", Sha256::digest(&wasm));

    let env = Env::default();
    env.mock_all_auths();

    // Register contract
    let wasm_bytes = Bytes::from_slice(&env, &wasm);
    #[allow(deprecated)]
    let _contract_id = env.register_contract_wasm(None, wasm_bytes);
    let contract_id = env.register(&*wasm, ());

    // Initialize
    let admin = Address::generate(&env);
    let token_name = String::from_str(&env, "Benchmark Token");
    let token_symbol = String::from_str(&env, "BNCH");

    println!("Invoking initialize...");
    let args: Vec<Val> = Vec::from_array(
        &env,
        [
            admin.to_val(),
            7u32.into_val(&env),
            token_name.to_val(),
            token_symbol.to_val(),
        ],
    );
    let _res: Val = env.invoke_contract(&contract_id, &Symbol::new(&env, "initialize"), args);

    // Create users
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    // Mint
    println!("Invoking mint...");
    // Measure instructions before
    env.cost_estimate().budget().reset_unlimited();
    let start_cpu = env.cost_estimate().budget().cpu_instruction_cost();
    let start_mem = env.cost_estimate().budget().memory_bytes_cost();

    let args: Vec<Val> = Vec::from_array(&env, [user1.to_val(), 1000i128.into_val(&env)]);
    let _res: Val = env.invoke_contract(&contract_id, &Symbol::new(&env, "mint"), args);

    let end_cpu = env.cost_estimate().budget().cpu_instruction_cost();
    let end_mem = env.cost_estimate().budget().memory_bytes_cost();
    let ledger_footprint = end_mem.saturating_sub(start_mem);

    println!("Mint Stats:");
    println!("  CPU Instructions: {}", end_cpu - start_cpu);
    println!("  Memory Bytes: {}", end_mem - start_mem);
    println!("  Ledger Footprint Proxy: {}", ledger_footprint);

    let mint_metric = SimulationMetric {
        contract: "token".to_string(),
        method: "mint".to_string(),
        code_hash: code_hash.clone(),
        cpu_instructions: end_cpu.saturating_sub(start_cpu),
        ram_bytes: end_mem.saturating_sub(start_mem),
        ledger_footprint,
    };
    let mint_analysis = simulation_service.record_and_analyze(mint_metric).await?;
    if mint_analysis.has_historical_baseline {
        println!(
            "Historical comparison for mint: alert_triggered={} outliers={}",
            mint_analysis.alert_triggered,
            mint_analysis.outliers.len()
        );
    } else {
        println!("No historical baseline available for mint yet.");
    }

    // Transfer
    println!("Invoking transfer...");
    env.cost_estimate().budget().reset_unlimited();
    let start_cpu = env.cost_estimate().budget().cpu_instruction_cost();
    let start_mem = env.cost_estimate().budget().memory_bytes_cost();

    let args: Vec<Val> = Vec::from_array(
        &env,
        [user1.to_val(), user2.to_val(), 200i128.into_val(&env)],
    );
    let _res: Val = env.invoke_contract(&contract_id, &Symbol::new(&env, "transfer"), args);

    let end_cpu = env.cost_estimate().budget().cpu_instruction_cost();
    let end_mem = env.cost_estimate().budget().memory_bytes_cost();
    let ledger_footprint = end_mem.saturating_sub(start_mem);

    println!("Transfer Stats:");
    println!("  CPU Instructions: {}", end_cpu - start_cpu);
    println!("  Memory Bytes: {}", end_mem - start_mem);
    println!("  Ledger Footprint Proxy: {}", ledger_footprint);

    let transfer_metric = SimulationMetric {
        contract: "token".to_string(),
        method: "transfer".to_string(),
        code_hash,
        cpu_instructions: end_cpu.saturating_sub(start_cpu),
        ram_bytes: end_mem.saturating_sub(start_mem),
        ledger_footprint,
    };
    let transfer_analysis = simulation_service
        .record_and_analyze(transfer_metric)
        .await?;
    if transfer_analysis.has_historical_baseline {
        println!(
            "Historical comparison for transfer: alert_triggered={} outliers={}",
            transfer_analysis.alert_triggered,
            transfer_analysis.outliers.len()
        );
    } else {
        println!("No historical baseline available for transfer yet.");
    }

    Ok(())
}
