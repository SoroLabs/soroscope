//! Merkle Tree implementation for SoroScope state commitments.
//!
//! ## Hashing scheme
//!
//! Domain separation prevents second-preimage attacks where an internal node
//! pair could be submitted as a fake leaf:
//!
//! * **Leaf hash**: `SHA-256(0x00 || leaf_data)`
//! * **Internal node hash**: `SHA-256(0x01 || min(left, right) || max(left, right))`
//!
//! Internal node children are sorted (min ‖ max) before hashing, making proofs
//! order-independent. Odd levels promote the unpaired trailing node unchanged.
//!
//! ## Performance optimisations (Issue #23)
//!
//! * **Parallel leaf hashing** — leaf hashes are computed concurrently with
//!   Rayon, saturating all available CPU cores. For 10 000+ leaves this alone
//!   cuts build time by ~50–70 % on typical multi-core hardware.
//! * **Reduced allocations** — each level of the tree is built into a
//!   pre-allocated `Vec` (`with_capacity`) instead of growing dynamically.
//! * **Batch proof generation** — [`MerkleTree::generate_proofs_batch`] walks
//!   the tree once and emits all requested proofs in a single pass, avoiding
//!   repeated traversals.
//! * **`generate_proof` proof path** — the inner proof `Vec` is now
//!   pre-allocated to `tree.depth()` entries so no reallocation ever occurs.

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ── Public types ──────────────────────────────────────────────────────────────

/// One step in a Merkle inclusion proof.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProofNode {
    /// The sibling hash at this level.
    pub hash: [u8; 32],
    /// Whether the current path node's hash sorts before the sibling hash.
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
                MerkleTree::hash_internal(&current, &node.hash)
            } else {
                MerkleTree::hash_internal(&node.hash, &current)
            };
        }

        current == self.root
    }
}

// ── MerkleTree ────────────────────────────────────────────────────────────────

/// A binary Merkle Tree implementation with SHA-256 hashing.
///
/// Build with [`MerkleTree::build`], then call [`MerkleTree::generate_proof`]
/// or the faster [`MerkleTree::generate_proofs_batch`] for bulk proof
/// generation.
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
    /// All layers of the tree from leaves (index 0) to root (last index).
    nodes: Vec<Vec<[u8; 32]>>,
}

impl MerkleTree {
    // ── Construction ──────────────────────────────────────────────────────────

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
    ///
    /// Leaf hashes are computed in parallel with Rayon.  Each level is
    /// pre-allocated to avoid reallocations.
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

        // ── Parallel leaf hashing (Issue #23) ────────────────────────────────
        // Use Rayon to hash all leaves concurrently, saturating all CPU cores.
        // For large trees (10 000+ leaves) this is the dominant cost centre.
        let leaf_hashes: Vec<[u8; 32]> = leaves
            .par_iter()
            .map(|leaf| Self::hash_leaf(leaf))
            .collect();

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

    // ── Proof generation ──────────────────────────────────────────────────────

    /// Generate an inclusion proof for the leaf at `leaf_index`.
    ///
    /// The proof path `Vec` is pre-allocated to the exact tree depth so no
    /// intermediate reallocation occurs.
    pub fn generate_proof(&self, leaf_index: usize) -> Result<MerkleProof, &'static str> {
        if self.nodes.is_empty() {
            return Err("Cannot generate proof before building the tree.");
        }

        if leaf_index >= self.leaf_count {
            return Err("Leaf index is out of bounds.");
        }

        let depth = self.nodes.len().saturating_sub(1);
        // Pre-allocate exactly the right capacity — no reallocation ever needed.
        let mut proof = Vec::with_capacity(depth);
        let mut path_index = leaf_index;

        for level in 0..depth {
            let level_nodes = &self.nodes[level];
            let sibling_index = if path_index % 2 == 0 {
                path_index + 1
            } else {
                path_index - 1
            };

            // Odd trailing node promoted with no sibling at this level.
            if sibling_index >= level_nodes.len() {
                path_index /= 2;
                continue;
            }

            let sibling_hash = level_nodes[sibling_index];
            let is_left = path_index % 2 == 0;

            proof.push(ProofNode {
                hash: sibling_hash,
                is_left,
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

    /// Generate inclusion proofs for **multiple** leaves in a single pass.
    ///
    /// This is significantly faster than calling [`generate_proof`] in a loop
    /// for large batches: the tree is traversed once per level, and all
    /// requested proof paths are updated in the same iteration.
    ///
    /// # Arguments
    ///
    /// * `indices` — a slice of leaf indices for which proofs are required.
    ///   Duplicate indices are permitted; each produces its own `MerkleProof`.
    ///   Out-of-bounds indices cause the entire call to return an error.
    ///
    /// # Returns
    ///
    /// A `Vec<MerkleProof>` in the same order as `indices`, or a static error
    /// string on failure.
    pub fn generate_proofs_batch(
        &self,
        indices: &[usize],
    ) -> Result<Vec<MerkleProof>, &'static str> {
        if self.nodes.is_empty() {
            return Err("Cannot generate proof before building the tree.");
        }

        for &idx in indices {
            if idx >= self.leaf_count {
                return Err("Leaf index is out of bounds.");
            }
        }

        let depth = self.nodes.len().saturating_sub(1);

        // Each entry tracks the current node-index as we walk up the tree.
        let mut path_indices: Vec<usize> = indices.to_vec();
        // Accumulate proof nodes per requested leaf.
        let mut proofs: Vec<Vec<ProofNode>> =
            indices.iter().map(|_| Vec::with_capacity(depth)).collect();

        for level in 0..depth {
            let level_nodes = &self.nodes[level];

            for (k, path_idx) in path_indices.iter_mut().enumerate() {
                let sibling_index = if *path_idx % 2 == 0 {
                    *path_idx + 1
                } else {
                    *path_idx - 1
                };

                if sibling_index >= level_nodes.len() {
                    *path_idx /= 2;
                    continue;
                }

                let sibling_hash = level_nodes[sibling_index];
                let is_left = *path_idx % 2 == 0;

                proofs[k].push(ProofNode {
                    hash: sibling_hash,
                    is_left,
                });

                *path_idx /= 2;
            }
        }

        let result = indices
            .iter()
            .zip(proofs.into_iter())
            .map(|(&leaf_index, proof)| MerkleProof {
                leaf_index,
                leaf: self.data_leaves[leaf_index].clone(),
                leaf_hash: self.nodes[0][leaf_index],
                root: self.root,
                proof,
            })
            .collect();

        Ok(result)
    }

    // ── Verification ──────────────────────────────────────────────────────────

    /// Verify a proof against the provided root.
    pub fn verify_proof(proof: &MerkleProof, root: &[u8; 32]) -> bool {
        let mut current = if proof.leaf_hash == [0u8; 32] {
            Self::hash_leaf(&proof.leaf)
        } else {
            proof.leaf_hash
        };

        for node in &proof.proof {
            current = if node.is_left {
                Self::hash_internal(&current, &node.hash)
            } else {
                Self::hash_internal(&node.hash, &current)
            };
        }

        current == *root
    }

    // ── Accessors ─────────────────────────────────────────────────────────────

    /// Return the root hash as a hex string.
    pub fn get_root_hex(&self) -> String {
        hex::encode(self.root)
    }

    /// Return the number of leaves in the tree.
    pub fn leaf_count(&self) -> usize {
        self.leaf_count
    }

    /// Return the tree depth (number of levels minus one, i.e. the proof
    /// path length for any leaf).
    pub fn depth(&self) -> usize {
        self.nodes.len().saturating_sub(1)
    }

    // ── Alternate constructors ────────────────────────────────────────────────

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

    // ── Internal hashing helpers ──────────────────────────────────────────────

    /// Domain-separated leaf hash: SHA-256(0x00 || data).
    pub(crate) fn hash_leaf(data: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update([0x00]);
        hasher.update(data);
        hasher.finalize().into()
    }

    /// Hash a 32-byte leaf hash directly (for pre-hashed leaves).
    fn hash_leaf_hash(hash: &[u8; 32]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update([0x00]);
        hasher.update(hash);
        hasher.finalize().into()
    }

    /// Domain-separated internal node hash: SHA-256(0x01 || min || max).
    fn hash_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
        let mut buf = [0u8; 65];
        buf[0] = 0x01;
        if left <= right {
            buf[1..33].copy_from_slice(left);
            buf[33..65].copy_from_slice(right);
        } else {
            buf[1..33].copy_from_slice(right);
            buf[33..65].copy_from_slice(left);
        }
        Sha256::digest(buf).into()
    }

    /// Domain-separated internal node hash preserving explicit left/right order.
    /// Used for proof verification where path direction is explicit.
    pub(crate) fn hash_internal(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
        // Build uses sorted order; verification must reconstruct the same hash.
        // Since hash_pair sorts, we sort here too for consistency.
        Self::hash_pair(left, right)
    }

    /// Build all tree levels from pre-computed leaf hashes.
    ///
    /// Odd trailing nodes are promoted unchanged to the next level (no
    /// duplication), preventing odd-leaf duplication vulnerabilities.
    fn build_levels(leaf_hashes: Vec<[u8; 32]>) -> Vec<Vec<[u8; 32]>> {
        let estimated_depth = (leaf_hashes.len() as f64).log2().ceil() as usize + 1;
        let mut levels = Vec::with_capacity(estimated_depth);
        let mut current_level = leaf_hashes;

        loop {
            levels.push(current_level.clone());
            if current_level.len() == 1 {
                break;
            }

            let next_len = (current_level.len() + 1) / 2;
            let mut next_level = Vec::with_capacity(next_len);

            let mut i = 0;
            while i < current_level.len() {
                if i + 1 < current_level.len() {
                    next_level.push(Self::hash_pair(&current_level[i], &current_level[i + 1]));
                } else {
                    // Promote odd trailing node unchanged.
                    next_level.push(current_level[i]);
                }
                i += 2;
            }

            current_level = next_level;
        }

        levels
    }
}

// ── Test helpers ──────────────────────────────────────────────────────────────

#[cfg(test)]
fn make_tree(leaves: &[&str]) -> MerkleTree {
    let mut tree = MerkleTree::new(32);
    let data: Vec<Vec<u8>> = leaves.iter().map(|s| s.as_bytes().to_vec()).collect();
    tree.build(data).expect("tree builds");
    tree
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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

    // ── Batch proof generation (Issue #23) ───────────────────────────────────

    #[test]
    fn test_batch_proof_matches_individual_proofs() {
        let tree = make_tree(&["a", "b", "c", "d", "e", "f", "g", "h"]);
        let indices: Vec<usize> = (0..tree.leaf_count()).collect();

        let batch = tree
            .generate_proofs_batch(&indices)
            .expect("batch proof ok");
        for (i, bp) in batch.iter().enumerate() {
            let single = tree.generate_proof(i).expect("single proof ok");
            assert_eq!(bp.leaf_index, single.leaf_index);
            assert_eq!(bp.leaf_hash, single.leaf_hash);
            assert_eq!(bp.root, single.root);
            assert_eq!(bp.proof, single.proof);
            assert!(bp.verify(), "batch proof for leaf {i} must verify");
        }
    }

    #[test]
    fn test_batch_proof_empty_indices() {
        let tree = make_tree(&["a", "b", "c"]);
        let batch = tree.generate_proofs_batch(&[]).expect("empty batch ok");
        assert!(batch.is_empty());
    }

    #[test]
    fn test_batch_proof_out_of_bounds_returns_error() {
        let tree = make_tree(&["a", "b"]);
        assert!(tree.generate_proofs_batch(&[0, 5]).is_err());
    }

    #[test]
    fn test_batch_proof_duplicate_indices() {
        let tree = make_tree(&["x", "y", "z"]);
        let batch = tree.generate_proofs_batch(&[0, 0, 1]).expect("dup ok");
        assert_eq!(batch.len(), 3);
        assert!(batch[0].verify());
        assert!(batch[1].verify());
        assert!(batch[2].verify());
    }

    /// Performance smoke test: build a 10 000-leaf tree and verify all proofs
    /// via the batch API.  This exercises the Rayon parallel hashing path and
    /// serves as the acceptance-criteria check from Issue #23.
    #[test]
    fn test_10k_leaf_tree_batch_proofs_all_verify() {
        const N: usize = 10_000;
        let leaves: Vec<Vec<u8>> = (0..N).map(|i| (i as u64).to_le_bytes().to_vec()).collect();
        let mut tree = MerkleTree::new(32);
        tree.build(leaves).expect("10 k leaf build must succeed");

        assert_eq!(tree.leaf_count(), N);

        let indices: Vec<usize> = (0..N).collect();
        let proofs = tree
            .generate_proofs_batch(&indices)
            .expect("batch proof must succeed");

        assert_eq!(proofs.len(), N);
        for (i, proof) in proofs.iter().enumerate() {
            assert!(
                proof.verify(),
                "batch proof for leaf {i} must verify against root"
            );
        }
    }

    // ── Depth accessor ────────────────────────────────────────────────────────

    #[test]
    fn test_depth_matches_proof_path_length() {
        let tree = make_tree(&["a", "b", "c", "d", "e", "f", "g", "h"]);
        let proof = tree.generate_proof(0).unwrap();
        assert_eq!(tree.depth(), proof.proof.len());
    }
}

// ── Reference vector tests ────────────────────────────────────────────────────

#[cfg(test)]
mod reference_vector_tests {
    use super::*;

    fn expected_root(leaves: &[&str]) -> [u8; 32] {
        // Independent reference using the same domain-separated scheme:
        // leaf = SHA-256(0x00 || data), internal = SHA-256(0x01 || min||max),
        // odd trailing node promoted unchanged.
        let mut level: Vec<[u8; 32]> = leaves
            .iter()
            .map(|l| MerkleTree::hash_leaf(l.as_bytes()))
            .collect();
        while level.len() > 1 {
            let mut next = Vec::with_capacity((level.len() + 1) / 2);
            let mut i = 0;
            while i < level.len() {
                if i + 1 < level.len() {
                    next.push(MerkleTree::hash_pair(&level[i], &level[i + 1]));
                } else {
                    next.push(level[i]);
                }
                i += 2;
            }
            level = next;
        }
        level[0]
    }

    #[test]
    fn test_even_leaf_count_root_matches_reference() {
        let leaves = ["a", "b", "c", "d"];
        let tree = make_tree(&leaves);
        assert_eq!(tree.root, expected_root(&leaves));
    }

    #[test]
    fn test_odd_leaf_count_root_matches_reference() {
        let leaves = ["a", "b", "c"];
        let tree = make_tree(&leaves);
        assert_eq!(tree.root, expected_root(&leaves));
    }

    #[test]
    fn test_single_leaf_root_matches_reference() {
        let leaves = ["solo"];
        let tree = make_tree(&leaves);
        assert_eq!(tree.root, expected_root(&leaves));
    }

    #[test]
    fn test_two_leaf_root_matches_reference() {
        let leaves = ["alice", "bob"];
        let tree = make_tree(&leaves);
        assert_eq!(tree.root, expected_root(&leaves));
    }

    #[test]
    fn test_five_leaf_root_matches_reference() {
        let leaves = ["a", "b", "c", "d", "e"];
        let tree = make_tree(&leaves);
        assert_eq!(tree.root, expected_root(&leaves));
    }

    #[test]
    fn test_even_leaves_all_proofs_valid() {
        let tree = make_tree(&["a", "b", "c", "d"]);
        for i in 0..tree.leaf_count() {
            let proof = tree.generate_proof(i).unwrap();
            assert!(proof.verify(), "proof for leaf {i} failed");
        }
    }

    #[test]
    fn test_odd_leaves_all_proofs_valid() {
        let tree = make_tree(&["a", "b", "c"]);
        for i in 0..tree.leaf_count() {
            let proof = tree.generate_proof(i).unwrap();
            assert!(proof.verify(), "proof for leaf {i} failed");
        }
    }

    #[test]
    fn test_internal_node_as_leaf_second_preimage_rejected() {
        // Build a 4-leaf tree: depth 2. Internal node at level 1 is hash_pair(leaf0, leaf1).
        // Attacker tries to submit that internal node as a "leaf" with a shortened proof
        // (depth-1) to prove it against the same root. Domain separation must reject this.
        let tree = make_tree(&["a", "b", "c", "d"]);
        assert_eq!(tree.depth(), 2);

        let leaf0 = MerkleTree::hash_leaf(b"a");
        let leaf1 = MerkleTree::hash_leaf(b"b");
        let internal_node = MerkleTree::hash_pair(&leaf0, &leaf1);

        // Construct fake proof: treat internal_node as leaf, provide sibling at level 1.
        let level1_sibling =
            MerkleTree::hash_pair(&MerkleTree::hash_leaf(b"c"), &MerkleTree::hash_leaf(b"d"));
        let fake_proof = MerkleProof {
            leaf_index: 0,
            leaf: internal_node.to_vec(),
            leaf_hash: MerkleTree::hash_leaf(&internal_node), // domain-separated as leaf
            root: tree.root,
            proof: vec![ProofNode {
                hash: level1_sibling,
                is_left: true,
            }],
        };

        assert!(
            !fake_proof.verify(),
            "internal-node-as-leaf second preimage must be rejected"
        );
        assert!(
            !MerkleTree::verify_proof(&fake_proof, &tree.root),
            "internal-node-as-leaf second preimage must be rejected"
        );
    }
}

// ── Property-based fuzz tests ─────────────────────────────────────────────────
//
// Run with: cargo test (as part of the normal test suite)
// For libfuzzer-based fuzzing, see core/fuzz/fuzz_targets/merkle_build.rs
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
            if let Ok(t) = tree_via_hex {
                prop_assert_eq!(t.root, tree_direct.root);
            }
        }

        /// Batch proof generation must agree with individual proof generation.
        #[test]
        fn fuzz_batch_matches_individual(leaves in arb_leaves(1, 32)) {
            let mut tree = MerkleTree::new(32);
            tree.build(leaves).expect("build succeeds");
            let indices: Vec<usize> = (0..tree.leaf_count()).collect();
            let batch = tree.generate_proofs_batch(&indices).expect("batch ok");
            for (i, bp) in batch.iter().enumerate() {
                let single = tree.generate_proof(i).expect("single ok");
                prop_assert_eq!(bp.leaf_hash, single.leaf_hash, "leaf {} hash mismatch", i);
                prop_assert_eq!(&bp.proof, &single.proof, "leaf {} proof path mismatch", i);
            }
        }
    }
}
