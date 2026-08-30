//! Fuzz target: verify that proof generation and verification are panic-free
//! under arbitrary inputs.
//!
//! Run with:
//!   cargo fuzz run merkle_proof
//!
//! Strategy: build a tree from the first half of the input, then use the
//! second half as raw bytes to construct a synthetic proof and attempt
//! verification. Neither path may panic.
#![no_main]

use libfuzzer_sys::fuzz_target;
use soroscope_core::merkle_tree::{MerkleTree, MerkleProof, ProofNode};

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 {
        return;
    }

    let split = data.len() / 2;
    let (tree_input, proof_input) = data.split_at(split);

    // Build a real tree from the first half.
    let leaves: Vec<Vec<u8>> = tree_input
        .split(|&b| b == 0x00)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_vec())
        .collect();

    if leaves.is_empty() {
        return;
    }

    let mut tree = MerkleTree::new(32);
    if tree.build(leaves).is_err() {
        return;
    }

    // Valid proof path — must never panic.
    for i in 0..tree.leaf_count() {
        if let Ok(proof) = tree.generate_proof(i) {
            let _ = proof.verify();
            let _ = MerkleTree::verify_proof(&proof, &tree.root);
        }
    }

    // Synthetic proof from raw bytes — verify must not panic even on garbage.
    if proof_input.len() >= 32 {
        let mut leaf_hash = [0u8; 32];
        leaf_hash.copy_from_slice(&proof_input[..32]);

        let mut proof_nodes = Vec::new();
        let mut cursor = &proof_input[32..];
        while cursor.len() >= 33 {
            let mut sibling = [0u8; 32];
            sibling.copy_from_slice(&cursor[..32]);
            let is_left = cursor[32] & 1 == 1;
            proof_nodes.push(ProofNode { hash: sibling, is_left });
            cursor = &cursor[33..];
        }

        let synthetic = MerkleProof {
            leaf_index: 0,
            leaf: b"fuzz".to_vec(),
            leaf_hash,
            root: tree.root,
            proof: proof_nodes,
        };
        let _ = synthetic.verify();
        let _ = MerkleTree::verify_proof(&synthetic, &tree.root);
    }
});
