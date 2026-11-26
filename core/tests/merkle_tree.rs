#[path = "../src/merkle_tree.rs"]
mod merkle_tree;

use merkle_tree::MerkleTree;

#[test]
fn test_merkle() {
    let leaves = vec![vec![0u8; 32], vec![1u8; 32], vec![2u8; 32], vec![3u8; 32]];
    let mut tree = MerkleTree::new(4);
    tree.build(leaves).unwrap();
    assert_eq!(tree.leaf_count(), 4);
    for i in 0..4 {
        let proof = tree.generate_proof(i).unwrap();
        assert!(MerkleTree::verify_proof(&proof, &tree.root()));
    }
}

#[test]
fn test_empty() {
    let mut tree = MerkleTree::new(4);
    assert!(tree.build(vec![]).is_err());
}
