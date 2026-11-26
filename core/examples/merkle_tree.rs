#[path = "../src/merkle_tree.rs"]
mod merkle_tree;

fn main() {
    let leaves: Vec<Vec<u8>> = vec![
        vec![0u8; 32],
        vec![1u8; 32],
        vec![2u8; 32],
        vec![3u8; 32],
    ];
    let mut tree = merkle_tree::MerkleTree::new(4);
    tree.build(leaves).unwrap();
    println!("Root: {:?}", tree.root());
    let proof = tree.generate_proof(0).unwrap();
    assert!(merkle_tree::MerkleTree::verify_proof(&proof, &tree.root()));
    println!("Proof verified for leaf 0");
}