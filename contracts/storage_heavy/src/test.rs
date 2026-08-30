extern crate std;

use super::*;
use soroban_sdk::{
    symbol_short,
    testutils::{storage::Persistent as _, Ledger as _, LedgerInfo},
    Bytes, Env, Symbol, Vec,
};
use std::println;


#[test]
fn test_storage() {
    let env = Env::default();
    let contract_id = env.register(StorageHeavyContract, ());
    let client = StorageHeavyContractClient::new(&env, &contract_id);

    let key = symbol_short!("data");
    let mut data = [0u8; 1000];
    for (i, item) in data.iter_mut().enumerate() {
        *item = (i % 256) as u8;
    }
    let data_bytes = Bytes::from_slice(&env, &data);

    // Test Persistent
    client.write_persistent(&key, &data_bytes);
    assert_eq!(client.read_persistent(&key), data_bytes);

    // Test Temporary
    client.write_temporary(&key, &data_bytes);
    assert_eq!(client.read_temporary(&key), data_bytes);
}

#[test]
fn test_extend_ttl_for_persistent_entry() {
    let env = Env::default();
    env.ledger().set(LedgerInfo {
        timestamp: 0,
        protocol_version: 22,
        sequence_number: 1,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 10_000,
    });

    let contract_id = env.register(StorageHeavyContract, ());
    let client = StorageHeavyContractClient::new(&env, &contract_id);
    let key = symbol_short!("data");
    let data = Bytes::from_slice(&env, &[1, 2, 3]);

    client.write_persistent(&key, &data);
    let initial_ttl = env.as_contract(&contract_id, || env.storage().persistent().get_ttl(&key));
    let extended_ttl = initial_ttl + 1_000;

    client.extend_ttl(&key, &(initial_ttl + 1), &extended_ttl);

    let ttl = env.as_contract(&contract_id, || env.storage().persistent().get_ttl(&key));
    assert_eq!(ttl, extended_ttl);
    assert_eq!(client.read_persistent(&key), data);
}

#[test]
fn test_extend_ttl_respects_threshold() {
    let env = Env::default();
    let contract_id = env.register(StorageHeavyContract, ());
    let client = StorageHeavyContractClient::new(&env, &contract_id);
    let key = symbol_short!("data");
    let data = Bytes::from_slice(&env, &[1, 2, 3]);

    client.write_persistent(&key, &data);
    let initial_ttl = env.as_contract(&contract_id, || env.storage().persistent().get_ttl(&key));

    client.extend_ttl(&key, &(initial_ttl - 1), &(initial_ttl + 1_000));

    let ttl = env.as_contract(&contract_id, || env.storage().persistent().get_ttl(&key));
    assert_eq!(ttl, initial_ttl);
}
#[test]
fn test_batch_storage() {
    let env = Env::default();
    let contract_id = env.register(StorageHeavyContract, ());
    let client = StorageHeavyContractClient::new(&env, &contract_id);

    let mut keys = Vec::new(&env);
    let mut data_points = Vec::new(&env);

    keys.push_back(symbol_short!("k1"));
    keys.push_back(symbol_short!("k2"));
    keys.push_back(symbol_short!("k3"));

    data_points.push_back(Bytes::from_slice(&env, &[1u8; 100]));
    data_points.push_back(Bytes::from_slice(&env, &[2u8; 100]));
    data_points.push_back(Bytes::from_slice(&env, &[3u8; 100]));

    client.batch_write_persistent(&keys, &data_points);

    assert_eq!(
        client.read_persistent(&symbol_short!("k1")),
        Bytes::from_slice(&env, &[1u8; 100])
    );
    assert_eq!(
        client.read_persistent(&symbol_short!("k2")),
        Bytes::from_slice(&env, &[2u8; 100])
    );
    assert_eq!(
        client.read_persistent(&symbol_short!("k3")),
        Bytes::from_slice(&env, &[3u8; 100])
    );
}

#[test]
fn test_bitwise_storage_benchmark() {
    let env = Env::default();
    let contract_id = env.register(StorageHeavyContract, ());
    let client = StorageHeavyContractClient::new(&env, &contract_id);

    // Create 10 keys and values to write
    let mut keys = Vec::new(&env);
    let mut values = Vec::new(&env);
    for i in 0..10 {
        let key_str = std::format!("k{}", i);
        keys.push_back(Symbol::new(&env, &key_str));
        values.push_back(i % 2 == 0);
    }
    let packed_key = Symbol::new(&env, "packed");

    // --- Benchmark Write ---
    env.cost_estimate().budget().reset_unlimited();
    let start_cpu_sep_write = env.cost_estimate().budget().cpu_instruction_cost();
    let start_mem_sep_write = env.cost_estimate().budget().memory_bytes_cost();
    client.write_separate_booleans(&keys, &values);
    let end_cpu_sep_write = env.cost_estimate().budget().cpu_instruction_cost();
    let end_mem_sep_write = env.cost_estimate().budget().memory_bytes_cost();

    let sep_write_cpu = end_cpu_sep_write.saturating_sub(start_cpu_sep_write);
    let sep_write_mem = end_mem_sep_write.saturating_sub(start_mem_sep_write);

    env.cost_estimate().budget().reset_unlimited();
    let start_cpu_pack_write = env.cost_estimate().budget().cpu_instruction_cost();
    let start_mem_pack_write = env.cost_estimate().budget().memory_bytes_cost();
    client.write_packed_booleans(&packed_key, &values);
    let end_cpu_pack_write = env.cost_estimate().budget().cpu_instruction_cost();
    let end_mem_pack_write = env.cost_estimate().budget().memory_bytes_cost();

    let pack_write_cpu = end_cpu_pack_write.saturating_sub(start_cpu_pack_write);
    let pack_write_mem = end_mem_pack_write.saturating_sub(start_mem_pack_write);

    println!("================ STORAGE WRITE BENCHMARK (10 booleans) ================");
    println!("Separate storage slots: CPU = {} instructions, Memory = {} bytes", sep_write_cpu, sep_write_mem);
    println!("Packed (bitwise) slot : CPU = {} instructions, Memory = {} bytes", pack_write_cpu, pack_write_mem);
    println!("Savings: {:.2}% CPU, {:.2}% Memory", 
             (sep_write_cpu.saturating_sub(pack_write_cpu) as f64 / sep_write_cpu as f64) * 100.0,
             (sep_write_mem.saturating_sub(pack_write_mem) as f64 / sep_write_mem as f64) * 100.0);
    println!("======================================================================");

    // --- Benchmark Read ---
    env.cost_estimate().budget().reset_unlimited();
    let start_cpu_sep_read = env.cost_estimate().budget().cpu_instruction_cost();
    let start_mem_sep_read = env.cost_estimate().budget().memory_bytes_cost();
    let _sep_res = client.read_separate_booleans(&keys);
    let end_cpu_sep_read = env.cost_estimate().budget().cpu_instruction_cost();
    let end_mem_sep_read = env.cost_estimate().budget().memory_bytes_cost();

    let sep_read_cpu = end_cpu_sep_read.saturating_sub(start_cpu_sep_read);
    let sep_read_mem = end_mem_sep_read.saturating_sub(start_mem_sep_read);

    env.cost_estimate().budget().reset_unlimited();
    let start_cpu_pack_read = env.cost_estimate().budget().cpu_instruction_cost();
    let start_mem_pack_read = env.cost_estimate().budget().memory_bytes_cost();
    let _pack_res = client.read_packed_booleans(&packed_key, &10);
    let end_cpu_pack_read = env.cost_estimate().budget().cpu_instruction_cost();
    let end_mem_pack_read = env.cost_estimate().budget().memory_bytes_cost();

    let pack_read_cpu = end_cpu_pack_read.saturating_sub(start_cpu_pack_read);
    let pack_read_mem = end_mem_pack_read.saturating_sub(start_mem_pack_read);

    println!("================ STORAGE READ BENCHMARK (10 booleans) ================");
    println!("Separate storage slots: CPU = {} instructions, Memory = {} bytes", sep_read_cpu, sep_read_mem);
    println!("Packed (bitwise) slot : CPU = {} instructions, Memory = {} bytes", pack_read_cpu, pack_read_mem);
    println!("Savings: {:.2}% CPU, {:.2}% Memory", 
             (sep_read_cpu.saturating_sub(pack_read_cpu) as f64 / sep_read_cpu as f64) * 100.0,
             (sep_read_mem.saturating_sub(pack_read_mem) as f64 / sep_read_mem as f64) * 100.0);
    println!("======================================================================");

    // --- Benchmark Update (modify 1 flag) ---
    let update_key = keys.get(4).unwrap();
    
    env.cost_estimate().budget().reset_unlimited();
    let start_cpu_sep_update = env.cost_estimate().budget().cpu_instruction_cost();
    let start_mem_sep_update = env.cost_estimate().budget().memory_bytes_cost();
    client.update_separate_boolean(&update_key, &true);
    let end_cpu_sep_update = env.cost_estimate().budget().cpu_instruction_cost();
    let end_mem_sep_update = env.cost_estimate().budget().memory_bytes_cost();

    let sep_update_cpu = end_cpu_sep_update.saturating_sub(start_cpu_sep_update);
    let sep_update_mem = end_mem_sep_update.saturating_sub(start_mem_sep_update);

    env.cost_estimate().budget().reset_unlimited();
    let start_cpu_pack_update = env.cost_estimate().budget().cpu_instruction_cost();
    let start_mem_pack_update = env.cost_estimate().budget().memory_bytes_cost();
    client.update_packed_boolean(&packed_key, &4, &true);
    let end_cpu_pack_update = env.cost_estimate().budget().cpu_instruction_cost();
    let end_mem_pack_update = env.cost_estimate().budget().memory_bytes_cost();

    let pack_update_cpu = end_cpu_pack_update.saturating_sub(start_cpu_pack_update);
    let pack_update_mem = end_mem_pack_update.saturating_sub(start_mem_pack_update);

    println!("================ STORAGE UPDATE BENCHMARK (1 flag) ================");
    println!("Separate storage slots: CPU = {} instructions, Memory = {} bytes", sep_update_cpu, sep_update_mem);
    println!("Packed (bitwise) slot : CPU = {} instructions, Memory = {} bytes", pack_update_cpu, pack_update_mem);
    println!("Savings: {:.2}% CPU, {:.2}% Memory", 
             (sep_update_cpu.saturating_sub(pack_update_cpu) as f64 / sep_update_cpu as f64) * 100.0,
             (sep_update_mem.saturating_sub(pack_update_mem) as f64 / sep_update_mem as f64) * 100.0);
    println!("======================================================================");

    // Make sure the packed / bitwise updates actually work as expected
    let read_vals = client.read_packed_booleans(&packed_key, &10);
    assert!(read_vals.get(4).unwrap()); // index 4 should be true now

    // Assert that packed writing/reading/updating is cheaper
    assert!(pack_write_cpu < sep_write_cpu);
    assert!(pack_write_mem < sep_write_mem);
    assert!((sep_write_cpu - pack_write_cpu) * 100 / sep_write_cpu >= 50);

    assert!(pack_read_cpu < sep_read_cpu);
    assert!(pack_read_mem < sep_read_mem);
    assert!((sep_read_cpu - pack_read_cpu) * 100 / sep_read_cpu >= 50);

    // For single flag update, packed might or might not be cheaper in CPU instructions (since packed does a read-modify-write, whereas separate does a direct write).
    // Let's assert that packed update is at least valid, and verify its cpu instruction difference.
}

#[test]
fn test_bitmask_vs_map_benchmark() {
    let env = Env::default();
    let contract_id = env.register(StorageHeavyContract, ());
    let client = StorageHeavyContractClient::new(&env, &contract_id);

    // Create 10 values to write
    let mut values = Vec::new(&env);
    for i in 0..10 {
        values.push_back(i % 2 == 0);
    }
    let key_bitmask = Symbol::new(&env, "bitmask");
    let key_map = Symbol::new(&env, "map");

    // --- Benchmark Write ---
    env.cost_estimate().budget().reset_unlimited();
    let start_cpu_bitmask_write = env.cost_estimate().budget().cpu_instruction_cost();
    let start_mem_bitmask_write = env.cost_estimate().budget().memory_bytes_cost();
    client.write_packed_booleans(&key_bitmask, &values);
    let end_cpu_bitmask_write = env.cost_estimate().budget().cpu_instruction_cost();
    let end_mem_bitmask_write = env.cost_estimate().budget().memory_bytes_cost();

    let bitmask_write_cpu = end_cpu_bitmask_write.saturating_sub(start_cpu_bitmask_write);
    let bitmask_write_mem = end_mem_bitmask_write.saturating_sub(start_mem_bitmask_write);

    env.cost_estimate().budget().reset_unlimited();
    let start_cpu_map_write = env.cost_estimate().budget().cpu_instruction_cost();
    let start_mem_map_write = env.cost_estimate().budget().memory_bytes_cost();
    client.write_map_booleans(&key_map, &values);
    let end_cpu_map_write = env.cost_estimate().budget().cpu_instruction_cost();
    let end_mem_map_write = env.cost_estimate().budget().memory_bytes_cost();

    let map_write_cpu = end_cpu_map_write.saturating_sub(start_cpu_map_write);
    let map_write_mem = end_mem_map_write.saturating_sub(start_mem_map_write);

    println!("================ WRITE BENCHMARK (Bitmask vs Map) ================");
    println!("Map storage    : CPU = {} instructions, Memory = {} bytes", map_write_cpu, map_write_mem);
    println!("Bitmask storage: CPU = {} instructions, Memory = {} bytes", bitmask_write_cpu, bitmask_write_mem);
    println!("Savings: {:.2}% CPU, {:.2}% Memory", 
             (map_write_cpu.saturating_sub(bitmask_write_cpu) as f64 / map_write_cpu as f64) * 100.0,
             (map_write_mem.saturating_sub(bitmask_write_mem) as f64 / map_write_mem as f64) * 100.0);
    println!("==================================================================");

    // --- Benchmark Read ---
    env.cost_estimate().budget().reset_unlimited();
    let start_cpu_bitmask_read = env.cost_estimate().budget().cpu_instruction_cost();
    let start_mem_bitmask_read = env.cost_estimate().budget().memory_bytes_cost();
    let _bitmask_res = client.read_packed_booleans(&key_bitmask, &10);
    let end_cpu_bitmask_read = env.cost_estimate().budget().cpu_instruction_cost();
    let end_mem_bitmask_read = env.cost_estimate().budget().memory_bytes_cost();

    let bitmask_read_cpu = end_cpu_bitmask_read.saturating_sub(start_cpu_bitmask_read);
    let bitmask_read_mem = end_mem_bitmask_read.saturating_sub(start_mem_bitmask_read);

    env.cost_estimate().budget().reset_unlimited();
    let start_cpu_map_read = env.cost_estimate().budget().cpu_instruction_cost();
    let start_mem_map_read = env.cost_estimate().budget().memory_bytes_cost();
    let _map_res = client.read_map_booleans(&key_map, &10);
    let end_cpu_map_read = env.cost_estimate().budget().cpu_instruction_cost();
    let end_mem_map_read = env.cost_estimate().budget().memory_bytes_cost();

    let map_read_cpu = end_cpu_map_read.saturating_sub(start_cpu_map_read);
    let map_read_mem = end_mem_map_read.saturating_sub(start_mem_map_read);

    println!("================ READ BENCHMARK (Bitmask vs Map) ================");
    println!("Map storage    : CPU = {} instructions, Memory = {} bytes", map_read_cpu, map_read_mem);
    println!("Bitmask storage: CPU = {} instructions, Memory = {} bytes", bitmask_read_cpu, bitmask_read_mem);
    println!("Savings: {:.2}% CPU, {:.2}% Memory", 
             (map_read_cpu.saturating_sub(bitmask_read_cpu) as f64 / map_read_cpu as f64) * 100.0,
             (map_read_mem.saturating_sub(bitmask_read_mem) as f64 / map_read_mem as f64) * 100.0);
    println!("=================================================================");

    // --- Benchmark Update ---
    env.cost_estimate().budget().reset_unlimited();
    let start_cpu_bitmask_update = env.cost_estimate().budget().cpu_instruction_cost();
    let start_mem_bitmask_update = env.cost_estimate().budget().memory_bytes_cost();
    client.update_packed_boolean(&key_bitmask, &4, &true);
    let end_cpu_bitmask_update = env.cost_estimate().budget().cpu_instruction_cost();
    let end_mem_bitmask_update = env.cost_estimate().budget().memory_bytes_cost();

    let bitmask_update_cpu = end_cpu_bitmask_update.saturating_sub(start_cpu_bitmask_update);
    let bitmask_update_mem = end_mem_bitmask_update.saturating_sub(start_mem_bitmask_update);

    env.cost_estimate().budget().reset_unlimited();
    let start_cpu_map_update = env.cost_estimate().budget().cpu_instruction_cost();
    let start_mem_map_update = env.cost_estimate().budget().memory_bytes_cost();
    client.update_map_boolean(&key_map, &4, &true);
    let end_cpu_map_update = env.cost_estimate().budget().cpu_instruction_cost();
    let end_mem_map_update = env.cost_estimate().budget().memory_bytes_cost();

    let map_update_cpu = end_cpu_map_update.saturating_sub(start_cpu_map_update);
    let map_update_mem = end_mem_map_update.saturating_sub(start_mem_map_update);

    println!("================ UPDATE BENCHMARK (Bitmask vs Map) ================");
    println!("Map storage    : CPU = {} instructions, Memory = {} bytes", map_update_cpu, map_update_mem);
    println!("Bitmask storage: CPU = {} instructions, Memory = {} bytes", bitmask_update_cpu, bitmask_update_mem);
    println!("Savings: {:.2}% CPU, {:.2}% Memory", 
             (map_update_cpu.saturating_sub(bitmask_update_cpu) as f64 / map_update_cpu as f64) * 100.0,
             (map_update_mem.saturating_sub(bitmask_update_mem) as f64 / map_update_mem as f64) * 100.0);
    println!("==================================================================");

    // Assert that bitmask storage is more efficient than map storage in CPU instructions
    assert!(bitmask_write_cpu < map_write_cpu);
    assert!(bitmask_read_cpu < map_read_cpu);
    assert!(bitmask_update_cpu < map_update_cpu);
    
    // CPU savings should be significant (Write >= 30%, Read >= 10%, Update >= 10%)
    assert!((map_write_cpu - bitmask_write_cpu) * 100 / map_write_cpu >= 30);
    assert!((map_read_cpu - bitmask_read_cpu) * 100 / map_read_cpu >= 10);
    assert!((map_update_cpu - bitmask_update_cpu) * 100 / map_update_cpu >= 10);
}

