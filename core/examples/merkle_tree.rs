#[path = "../src/merkle_tree.rs"]
mod merkle_tree;

fn main() {
    let leaves: Vec<[u8; 32]> = vec![
        [0u8; 32],
        [1u8; 32],
        [2u8; 32],
        [3u8; 32],
    ];
    let tree = merkle_tree::MerkleTree::new(leaves).unwrap();
    println!("Root: {?:}", tree.root());
    let proof = tree.generate_proof(0).unwrap();
    assert!(merkle_tree::MerkleTree::verify_proof(&proof));
    println!("Proof verified for leaf 0");
}