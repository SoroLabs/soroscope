//! Fuzz target: feed arbitrary byte slices into MerkleTree::build.
//!
//! Run with:
//!   cargo fuzz run merkle_build
//!
//! The fuzzer splits the input at 0x00 bytes to generate variable-length,
//! variable-count leaf vectors. The builder must never panic regardless of
//! the content, size, or number of leaves.
#![no_main]

use libfuzzer_sys::fuzz_target;
use soroscope_core::merkle_tree::MerkleTree;

fuzz_target!(|data: &[u8]| {
    // Split at null bytes to create a variable number of leaves.
    let leaves: Vec<Vec<u8>> = data.split(|&b| b == 0x00).map(|s| s.to_vec()).collect();

    if leaves.is_empty() {
        return;
    }

    let mut tree = MerkleTree::new(32);
    match tree.build(leaves) {
        Ok(()) => {
            for i in 0..tree.leaf_count() {
                if let Ok(proof) = tree.generate_proof(i) {
                    let _ = proof.verify();
                    let _ = MerkleTree::verify_proof(&proof, &tree.root);
                }
            }
            let _ = tree.get_root_hex();
        }
        Err(_) => {}
    }
});
