//! Merkle Tree implementation for SoroScope state commitments.
//!
//! Leaves are SHA-256 hashed to form leaf nodes. Internal nodes are produced by
//! sorting each pair of child hashes (min || max) before concatenating and hashing,
//! making proofs order-independent.

use hex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// One step in a Merkle inclusion proof.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProofNode {
    /// The sibling hash at this level.
    pub hash: [u8; 32],
    /// Whether the current leaf path node is the left child.
    pub is_left: bool,
}

/// A Merkle inclusion proof for a single leaf.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    /// The index of the leaf in the original tree.
    pub leaf_index: usize,
    /// The raw leaf payload.
    pub leaf: Vec<u8>,
    /// The leaf hash used to start verification.
    pub leaf_hash: [u8; 32],
    /// The root hash that this proof commits to.
    pub root: [u8; 32],
    /// The path of sibling nodes from the leaf to the root.
    pub proof: Vec<ProofNode>,
}

impl MerkleProof {
    /// Verify the proof against the claimed root.
    pub fn verify(&self) -> bool {
        let mut current = if self.leaf_hash == [0u8; 32] {
            MerkleTree::hash_leaf(&self.leaf)
        } else {
            self.leaf_hash
        };

        for node in &self.proof {
            current = if node.is_left {
                MerkleTree::hash_pair(&current, &node.hash)
            } else {
                MerkleTree::hash_pair(&node.hash, &current)
            };
        }

        current == self.root
    }
}

/// A binary Merkle Tree implementation with SHA-256 hashing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleTree {
    /// The root hash of the tree.
    pub root: [u8; 32],
    /// The configured maximum number of levels.
    pub levels: usize,
    /// The number of leaves that were built into the tree.
    pub leaf_count: usize,
    /// Raw leaf data, preserved for proof generation.
    data_leaves: Vec<Vec<u8>>,
    /// All layers of the tree from leaves to the root.
    nodes: Vec<Vec<[u8; 32]>>,
}

impl MerkleTree {
    /// Create a new empty Merkle tree with a maximum depth.
    pub fn new(levels: usize) -> Self {
        MerkleTree {
            root: [0u8; 32],
            levels,
            leaf_count: 0,
            data_leaves: Vec::new(),
            nodes: Vec::new(),
        }
    }

    /// Build the tree from raw leaf bytes.
    pub fn build(&mut self, leaves: Vec<Vec<u8>>) -> Result<(), &'static str> {
        if leaves.is_empty() {
            return Err("Cannot build a Merkle tree from zero leaves.");
        }

        let max_leaves = if self.levels >= 64 {
            usize::MAX
        } else {
            1usize.checked_shl(self.levels as u32).unwrap_or(usize::MAX)
        };

        if leaves.len() > max_leaves {
            return Err("Leaf count exceeds configured tree capacity.");
        }

        let leaf_hashes: Vec<[u8; 32]> = leaves.iter().map(|leaf| Self::hash_leaf(leaf)).collect();
        self.nodes = Self::build_levels(leaf_hashes);
        self.root = *self
            .nodes
            .last()
            .and_then(|level| level.first())
            .ok_or("Failed to calculate tree root.")?;
        self.data_leaves = leaves;
        self.leaf_count = self.data_leaves.len();

        Ok(())
    }

    /// Generate an inclusion proof for the leaf at leaf_index.
    pub fn generate_proof(&self, leaf_index: usize) -> Result<MerkleProof, &'static str> {
        if self.nodes.is_empty() {
            return Err("Cannot generate proof before building the tree.");
        }

        if leaf_index >= self.leaf_count {
            return Err("Leaf index is out of bounds.");
        }

        let mut path_index = leaf_index;
        let mut proof = Vec::new();

        for level in 0..self.nodes.len() - 1 {
            let level_nodes = &self.nodes[level];
            let sibling_index = if path_index % 2 == 0 {
                path_index + 1
            } else {
                path_index - 1
            };

            let sibling_hash = if sibling_index < level_nodes.len() {
                level_nodes[sibling_index]
            } else {
                level_nodes[path_index]
            };

            proof.push(ProofNode {
                hash: sibling_hash,
                is_left: path_index % 2 == 0,
            });

            path_index /= 2;
        }

        Ok(MerkleProof {
            leaf_index,
            leaf: self.data_leaves[leaf_index].clone(),
            leaf_hash: self.nodes[0][leaf_index],
            root: self.root,
            proof,
        })
    }

    /// Verify a proof for the provided root.
    pub fn verify_proof(proof: &MerkleProof, root: &[u8; 32]) -> bool {
        let mut current = if proof.leaf_hash == [0u8; 32] {
            Self::hash_leaf(&proof.leaf)
        } else {
            proof.leaf_hash
        };

        for node in &proof.proof {
            current = if node.is_left {
                Self::hash_pair(&current, &node.hash)
            } else {
                Self::hash_pair(&node.hash, &current)
            };
        }

        current == *root
    }

    /// Return the root hash as a hex string.
    pub fn get_root_hex(&self) -> String {
        hex::encode(self.root)
    }

    /// Return the number of leaves in the tree.
    pub fn leaf_count(&self) -> usize {
        self.leaf_count
    }

    /// Build from hex-encoded leaf values.
    pub fn from_hex_strings(hex_leaves: Vec<String>) -> Result<Self, &'static str> {
        let leaves: Result<Vec<Vec<u8>>, &'static str> = hex_leaves
            .into_iter()
            .map(|hex| hex::decode(&hex).map_err(|_| "Invalid hex encoding in leaf data."))
            .collect();

        let leaves = leaves?;
        let mut tree = MerkleTree::new(32);
        tree.build(leaves)?;
        Ok(tree)
    }

    fn hash_leaf(data: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let digest = hasher.finalize();
        digest.into()
    }

    fn hash_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        if left <= right {
            hasher.update(left);
            hasher.update(right);
        } else {
            hasher.update(right);
            hasher.update(left);
        }
        let digest = hasher.finalize();
        digest.into()
    }

    fn build_levels(leaf_hashes: Vec<[u8; 32]>) -> Vec<Vec<[u8; 32]>> {
        let mut levels = Vec::new();
        let mut current_level = leaf_hashes;

        loop {
            levels.push(current_level.clone());
            if current_level.len() == 1 {
                break;
            }

            let mut next_level = Vec::new();
            let mut i = 0;
            while i < current_level.len() {
                let left = &current_level[i];
                let right = if i + 1 < current_level.len() { &current_level[i + 1] } else { left };
                next_level.push(Self::hash_pair(left, right));
                i += 2;
            }

            current_level = next_level;
        }

        levels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tree(leaves: &[&str]) -> MerkleTree {
        let mut tree = MerkleTree::new(32);
        let data: Vec<Vec<u8>> = leaves.iter().map(|s| s.as_bytes().to_vec()).collect();
        tree.build(data).expect("tree builds");
        tree
    }

    #[test]
    fn builds_tree_and_generates_valid_proofs_for_each_leaf() {
        let tree = make_tree(&["a", "b", "c", "d", "e"]);

        for index in 0..tree.leaf_count() {
            let proof = tree.generate_proof(index).expect("proof exists");
            assert_eq!(proof.leaf_index, index);
            assert_eq!(proof.root, tree.root);
            assert!(proof.verify());
            assert!(MerkleTree::verify_proof(&proof, &tree.root));
        }
    }

    #[test]
    fn test_build_empty_returns_error() {
        let mut tree = MerkleTree::new(32);
        assert!(tree.build(vec![]).is_err());
    }

    #[test]
    fn test_generate_proof_before_build_returns_error() {
        let tree = MerkleTree::new(32);
        assert!(tree.generate_proof(0).is_err());
    }

    #[test]
    fn test_generate_proof_out_of_range_returns_error() {
        let tree = make_tree(&["a", "b"]);
        assert!(tree.generate_proof(2).is_err());
    }

    #[test]
    fn test_generate_proof_single_leaf_has_no_nodes() {
        let tree = make_tree(&["solo"]);
        let proof = tree.generate_proof(0).unwrap();
        assert!(proof.proof.is_empty());
    }

    #[test]
    fn test_verify_proof_valid_two_leaves() {
        let tree = make_tree(&["alice", "bob"]);
        let proof = tree.generate_proof(0).unwrap();
        assert!(MerkleTree::verify_proof(&proof, &tree.root));
    }

    #[test]
    fn test_generate_and_verify_proof_for_random_leaf_indexes() {
        let mut tree = MerkleTree::new(32);
        let leaves: Vec<Vec<u8>> = (0..16).map(|i| format!("leaf-{i}").into_bytes()).collect();
        tree.build(leaves).expect("tree builds");

        let mut seed = 12345u64;
        let indices: Vec<usize> = (0..8)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                (seed % 16) as usize
            })
            .collect();

        for index in indices {
            let proof = tree.generate_proof(index).expect("proof exists");
            assert_eq!(proof.leaf_index, index);
            assert!(proof.verify());
            assert!(MerkleTree::verify_proof(&proof, &tree.root));
        }
    }

    #[test]
    fn test_verify_proof_valid_odd_leaves() {
        let tree = make_tree(&["a", "b", "c"]);
        for i in 0..tree.leaf_count() {
            let proof = tree.generate_proof(i).unwrap();
            assert!(MerkleTree::verify_proof(&proof, &tree.root));
        }
    }

    #[test]
    fn test_verify_proof_tampered_leaf_fails() {
        let tree = make_tree(&["alice", "bob"]);
        let mut proof = tree.generate_proof(0).unwrap();
        proof.leaf_hash[0] ^= 0xff;
        assert!(!proof.verify());
    }

    #[test]
    fn test_verify_proof_tampered_sibling_fails() {
        let tree = make_tree(&["alice", "bob"]);
        let mut proof = tree.generate_proof(0).unwrap();
        proof.proof[0].hash = [0xFFu8; 32];
        assert!(!MerkleTree::verify_proof(&proof, &tree.root));
    }

    #[test]
    fn test_verify_proof_wrong_root_fails() {
        let tree = make_tree(&["alice", "bob"]);
        let proof = tree.generate_proof(0).unwrap();
        let wrong_root = [0xABu8; 32];
        assert!(!MerkleTree::verify_proof(&proof, &wrong_root));
    }

    #[test]
    fn test_verify_proof_cross_tree_fails() {
        let tree1 = make_tree(&["a", "b"]);
        let tree2 = make_tree(&["c", "d"]);
        let proof = tree1.generate_proof(0).unwrap();
        assert!(!MerkleTree::verify_proof(&proof, &tree2.root));
    }

    #[test]
    fn test_merkle_tree_from_hex_strings() {
        let hex_leaves = vec![hex::encode(b"data1"), hex::encode(b"data2")];
        let tree = MerkleTree::from_hex_strings(hex_leaves).expect("Failed to build tree");
        assert_eq!(tree.leaf_count(), 2);
        assert!(!tree.get_root_hex().is_empty());
    }

    #[test]
    fn test_verify_proof_for_large_tree_indices() {
        let tree = make_tree(&["w", "x", "y", "z", "p", "q", "r", "s"]);
        for index in 0..tree.leaf_count() {
            let proof = tree.generate_proof(index).unwrap();
            assert!(MerkleTree::verify_proof(&proof, &tree.root));
        }
    }

    #[test]
    fn test_get_root_hex_length() {
        let tree = make_tree(&["hello", "world"]);
        assert_eq!(tree.get_root_hex().len(), 64);
    }

    #[test]
    fn test_tree_deterministic_root() {
        let data = vec![b"test".to_vec(), b"data".to_vec()];
        let mut t1 = MerkleTree::new(32);
        let mut t2 = MerkleTree::new(32);
        t1.build(data.clone()).unwrap();
        t2.build(data).unwrap();
        assert_eq!(t1.root, t2.root);
    }

    // ── Even and odd leaf count tests with reference values ────────────────

    #[test]
    fn test_even_leaf_count_root_matches_reference() {
        // Reference computed with SHA-256 + sorted hash_pair (identical algorithm).
        // Leaves: ["a", "b", "c", "d"] — 4 leaves (even).
        // Expected root: 4c6aae040ffada3d02598207b8485fcbe161c03f4cb3f660e4d341e7496ff3b2
        let tree = make_tree(&["a", "b", "c", "d"]);
        let expected = "4c6aae040ffada3d02598207b8485fcbe161c03f4cb3f660e4d341e7496ff3b2";
        assert_eq!(tree.get_root_hex(), expected);
    }

    #[test]
    fn test_odd_leaf_count_root_matches_reference() {
        // Leaves: ["a", "b", "c"] — 3 leaves (odd).
        // The last leaf is paired with itself when building the next level.
        // Expected root: b1da020d217b348265d6578cdfe4cc717bb79b5deaffce7fc167180e9e1ec8c6
        let tree = make_tree(&["a", "b", "c"]);
        let expected = "b1da020d217b348265d6578cdfe4cc717bb79b5deaffce7fc167180e9e1ec8c6";
        assert_eq!(tree.get_root_hex(), expected);
    }

    #[test]
    fn test_single_leaf_root_matches_reference() {
        // Single leaf: SHA-256("solo") with no pairing.
        // Expected root: 5364f2f2fc4f54e9d47ad29cfb08ef430c8153394bf2a0dff5cbe77a0ffef861
        let tree = make_tree(&["solo"]);
        let expected = "5364f2f2fc4f54e9d47ad29cfb08ef430c8153394bf2a0dff5cbe77a0ffef861";
        assert_eq!(tree.get_root_hex(), expected);
    }

    #[test]
    fn test_two_leaf_root_matches_reference() {
        // Leaves: ["alice", "bob"] — 2 leaves (even).
        // Expected root: cb57721dc3aa8df0eef91989560b053a86be98131f45650bd1c3955e0167ef17
        let tree = make_tree(&["alice", "bob"]);
        let expected = "cb57721dc3aa8df0eef91989560b053a86be98131f45650bd1c3955e0167ef17";
        assert_eq!(tree.get_root_hex(), expected);
    }

    #[test]
    fn test_five_leaf_root_matches_reference() {
        // Leaves: ["a", "b", "c", "d", "e"] — 5 leaves (odd).
        // Expected root: df947ef1b6dda4cb4ef081afd68f255104ccaab2661f2047d2f1a05c5440076f
        let tree = make_tree(&["a", "b", "c", "d", "e"]);
        let expected = "df947ef1b6dda4cb4ef081afd68f255104ccaab2661f2047d2f1a05c5440076f";
        assert_eq!(tree.get_root_hex(), expected);
    }

    #[test]
    fn test_even_leaves_all_proofs_valid() {
        // 4 leaves — all proofs must verify against the known root.
        let tree = make_tree(&["a", "b", "c", "d"]);
        for i in 0..tree.leaf_count() {
            let proof = tree.generate_proof(i).unwrap();
            assert!(proof.verify(), "proof for leaf {i} failed");
        }
    }

    #[test]
    fn test_odd_leaves_all_proofs_valid() {
        // 3 leaves — includes the "duplicate-last" padding case.
        let tree = make_tree(&["a", "b", "c"]);
        for i in 0..tree.leaf_count() {
            let proof = tree.generate_proof(i).unwrap();
            assert!(proof.verify(), "proof for leaf {i} failed");
        }
    }
}

// ── Property-based fuzz tests for MerkleTree ──────────────────────────────────
//
// Run with: cargo test (as part of the normal test suite)
// For libfuzzer-based fuzzing, see core/fuzz/fuzz_targets/merkle_build.rs
//
// To run with higher iteration counts:
//   PROPTEST_CASES=10000 cargo test fuzz_
#[cfg(test)]
mod fuzz_tests {
    use super::*;
    use proptest::prelude::*;

    fn arb_leaf() -> impl Strategy<Value = Vec<u8>> {
        prop::collection::vec(any::<u8>(), 0..=256)
    }

    fn arb_leaves(min: usize, max: usize) -> impl Strategy<Value = Vec<Vec<u8>>> {
        prop::collection::vec(arb_leaf(), min..=max)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(500))]

        /// Builder must never panic on any non-empty input.
        #[test]
        fn fuzz_build_never_panics(leaves in arb_leaves(1, 64)) {
            let mut tree = MerkleTree::new(32);
            let _ = tree.build(leaves);
        }

        /// Empty input must return an error, never panic.
        #[test]
        fn fuzz_empty_input_returns_err(_dummy in any::<u8>()) {
            let mut tree = MerkleTree::new(32);
            prop_assert!(tree.build(vec![]).is_err());
        }

        /// Every proof produced by the builder must verify against the tree root.
        #[test]
        fn fuzz_all_proofs_verify(leaves in arb_leaves(1, 32)) {
            let mut tree = MerkleTree::new(32);
            tree.build(leaves).expect("build succeeds");
            for i in 0..tree.leaf_count() {
                let proof = tree.generate_proof(i).expect("proof exists");
                prop_assert!(proof.verify(), "proof for leaf {i} failed");
                prop_assert!(MerkleTree::verify_proof(&proof, &tree.root));
            }
        }

        /// Root must be deterministic: same leaves always produce the same root.
        #[test]
        fn fuzz_build_is_deterministic(leaves in arb_leaves(1, 32)) {
            let mut t1 = MerkleTree::new(32);
            let mut t2 = MerkleTree::new(32);
            t1.build(leaves.clone()).unwrap();
            t2.build(leaves).unwrap();
            prop_assert_eq!(t1.root, t2.root);
        }

        /// A tampered leaf hash must cause proof.verify() to return false.
        #[test]
        fn fuzz_tampered_leaf_invalidates_proof(leaves in arb_leaves(2, 32)) {
            let mut tree = MerkleTree::new(32);
            tree.build(leaves).unwrap();
            let mut proof = tree.generate_proof(0).unwrap();
            proof.leaf_hash[0] ^= 0xff;
            prop_assert!(!proof.verify());
        }

        /// A tampered sibling hash must cause verify_proof to return false.
        #[test]
        fn fuzz_tampered_sibling_invalidates_proof(leaves in arb_leaves(2, 32)) {
            let mut tree = MerkleTree::new(32);
            tree.build(leaves).unwrap();
            let mut proof = tree.generate_proof(0).unwrap();
            if !proof.proof.is_empty() {
                proof.proof[0].hash = [0xFFu8; 32];
                prop_assert!(!MerkleTree::verify_proof(&proof, &tree.root));
            }
        }

        /// All leaves in a tree of power-of-two size must produce valid proofs.
        #[test]
        fn fuzz_power_of_two_leaf_counts_valid(
            log2_size in 1usize..=6usize,
            seed in any::<u64>(),
        ) {
            let size = 1 << log2_size;
            let leaves: Vec<Vec<u8>> = (0..size)
                .map(|i| {
                    let mut v = seed.to_le_bytes().to_vec();
                    v.extend_from_slice(&(i as u64).to_le_bytes());
                    v
                })
                .collect();
            let mut tree = MerkleTree::new(32);
            tree.build(leaves).unwrap();
            for i in 0..tree.leaf_count() {
                let proof = tree.generate_proof(i).unwrap();
                prop_assert!(MerkleTree::verify_proof(&proof, &tree.root));
            }
        }

        /// Duplicate leaves must not cause panics and must produce a valid tree.
        #[test]
        fn fuzz_duplicate_leaves_no_panic(
            leaf in arb_leaf(),
            count in 2usize..=16usize,
        ) {
            let leaves = vec![leaf; count];
            let mut tree = MerkleTree::new(32);
            if tree.build(leaves).is_ok() {
                prop_assert_eq!(tree.leaf_count(), count);
            }
        }

        /// Very large individual leaves (up to 64 KiB) must not cause panics.
        #[test]
        fn fuzz_large_leaf_values(
            leaf in prop::collection::vec(any::<u8>(), 0..=65536usize),
        ) {
            let mut tree = MerkleTree::new(32);
            let _ = tree.build(vec![leaf]);
        }

        /// hex-string round-trip: encode leaves to hex, build via from_hex_strings,
        /// result must equal direct build.
        #[test]
        fn fuzz_hex_roundtrip(leaves in arb_leaves(1, 16)) {
            let hex_leaves: Vec<String> = leaves.iter().map(hex::encode).collect();
            let tree_via_hex = MerkleTree::from_hex_strings(hex_leaves);
            let mut tree_direct = MerkleTree::new(32);
            tree_direct.build(leaves).unwrap();
            match tree_via_hex {
                Ok(t) => prop_assert_eq!(t.root, tree_direct.root),
                Err(_) => {}
            }
        }
    }
}
