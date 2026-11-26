use serde::Serialize;
use soroscope_core::merkle_tree::MerkleTree;
use std::fs::File;
use std::io::Write;
use std::time::Instant;

#[derive(Serialize)]
struct BenchmarkResult {
    leaf_count: usize,
    build_time_ms: u128,
    proof_generation_time_ms: u128,
    verification_time_ms: u128,
}

#[derive(Serialize)]
struct BenchmarkReport {
    results: Vec<BenchmarkResult>,
}

fn main() {
    let sizes = vec![100, 1000, 10000];
    let mut results = Vec::new();

    println!("| Leaves | Build Time (ms) | Proof Gen Time (ms) | Verify Time (ms) |");
    println!("|--------|-----------------|---------------------|------------------|");

    for size in sizes {
        let mut leaves = Vec::with_capacity(size);
        for i in 0..size {
            let mut leaf = [0u8; 32];
            let bytes = (i as u64).to_le_bytes();
            leaf[0..8].copy_from_slice(&bytes);
            leaves.push(leaf);
        }

        let start = Instant::now();
        let tree = MerkleTree::new(leaves).unwrap();
        let build_time = start.elapsed().as_millis();

        let start = Instant::now();
        let proof = tree.generate_proof(0).unwrap();
        let proof_time = start.elapsed().as_millis();

        let start = Instant::now();
        let valid = MerkleTree::verify_proof(&proof);
        let verify_time = start.elapsed().as_millis();

        assert!(valid, "Proof must be valid");

        println!(
            "| {:<6} | {:<15} | {:<19} | {:<16} |",
            size, build_time, proof_time, verify_time
        );

        results.push(BenchmarkResult {
            leaf_count: size,
            build_time_ms: build_time,
            proof_generation_time_ms: proof_time,
            verification_time_ms: verify_time,
        });
    }

    let report = BenchmarkReport { results };
    let json = serde_json::to_string_pretty(&report).unwrap();
    let mut file = File::create("benchmark_reports.json").unwrap();
    file.write_all(json.as_bytes()).unwrap();

    println!("\nJSON report written to benchmark_reports.json");
}
