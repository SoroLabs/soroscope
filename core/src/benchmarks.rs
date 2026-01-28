use soroban_sdk::{testutils::Address as _, xdr::ScVal, Address, Bytes, Env, IntoVal, String, Symbol, Val, Vec};
use std::fs;
use std::path::PathBuf;

pub fn run_token_benchmark(wasm_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    println!("Loading contract from: {:?}", wasm_path);
    let wasm = fs::read(wasm_path)?;

    let env = Env::default();
    env.mock_all_auths();

    // Register contract
    let wasm_bytes = Bytes::from_slice(&env, &wasm);
    let contract_id = env.register_contract_wasm(None, wasm_bytes);
    
    // Initialize
    let admin = Address::generate(&env);
    let token_name = String::from_str(&env, "Benchmark Token");
    let token_symbol = String::from_str(&env, "BNCH");
    
    println!("Invoking initialize...");
    let args: Vec<Val> = Vec::from_array(&env, [admin.to_val(), 7u32.into_val(&env), token_name.to_val(), token_symbol.to_val()]);
    let _res: Val = env.invoke_contract(
        &contract_id, 
        &Symbol::new(&env, "initialize"), 
        args
    );
    
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
    let _res: Val = env.invoke_contract(
        &contract_id,
        &Symbol::new(&env, "mint"),
        args
    );

    let end_cpu = env.cost_estimate().budget().cpu_instruction_cost();
    let end_mem = env.cost_estimate().budget().memory_bytes_cost();

    println!("Mint Stats:");
    println!("  CPU Instructions: {}", end_cpu - start_cpu);
    println!("  Memory Bytes: {}", end_mem - start_mem);

    // Transfer
    println!("Invoking transfer...");
    env.cost_estimate().budget().reset_unlimited();
    let start_cpu = env.cost_estimate().budget().cpu_instruction_cost();
    let start_mem = env.cost_estimate().budget().memory_bytes_cost();

    let args: Vec<Val> = Vec::from_array(&env, [user1.to_val(), user2.to_val(), 200i128.into_val(&env)]);
    let _res: Val = env.invoke_contract(
        &contract_id,
        &Symbol::new(&env, "transfer"),
        args
    );

    let end_cpu = env.cost_estimate().budget().cpu_instruction_cost();
    let end_mem = env.cost_estimate().budget().memory_bytes_cost();

    println!("Transfer Stats:");
    println!("  CPU Instructions: {}", end_cpu - start_cpu);
    println!("  Memory Bytes: {}", end_mem - start_mem);

    Ok(())
}
