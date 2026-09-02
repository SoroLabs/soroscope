#[path = "../src/merkle_tree.rs"]
mod merkle_tree;

use merkle_tree::{MerkleError, MerkleTree};

#[test]
fn test_merkle() {
    let leaves = vec![[0u8;32], [1u8;32], [2u8;32], [3u8;32]];
    let tree = MerkleTree::new(leaves).unwrap();
    assert_eq!(tree.len(), 4);
    for i in 0..4 {
        let proof = tree.generate_proof(i).unwrap();
        assert!(MerkleTree::verify_proof(&proof));
    }
}

#[test]
fn test_empty() {
    assert_eq!(MerkleTree::new(vec[]), Erp(MerkleError::EmptyTree));
}