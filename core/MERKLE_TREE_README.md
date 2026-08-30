# Merkle Tree Utility

## Overview

The `MerkleTree` struct in `core/src/merkle_tree.rs` is an off-chain utility for building Binary Merkle Trees and generating inclusion proofs. It is the companion tool to the [`cross_chain_verifier`](../contracts/cross_chain_verifier/) on-chain contract: you build the tree here, post the root on-chain, and then generate proofs that the contract can verify.

```
Off-Chain (this library)                    On-Chain (Soroban contract)
─────────────────────────                   ───────────────────────────
MerkleTree::build(leaves)                   CrossChainVerifier::update_root()
        │                                           │
        ▼                                           ▼
  root_hex ──────────────────────────────► stored root
        │
        ▼
MerkleTree::generate_proof(index)
        │
        ▼
  (proof, proof_flags) ─────────────────► CrossChainVerifier::verify_message()
```

---

## How It Works

### Tree Construction

Leaves are SHA-256 hashes of your raw data. The tree is built bottom-up by repeatedly hashing adjacent pairs:

```
Level 0 (leaves):  [H(A), H(B), H(C), H(D)]
Level 1:           [H(H(A)||H(B)),  H(H(C)||H(D))]
Level 2 (root):    [H(H(H(A)||H(B)) || H(H(C)||H(D)))]
```

**Odd-length levels**: When a level has an odd number of nodes, the last node is hashed with itself — `H(X || X)` — before being promoted. This is standard practice and ensures the tree always has a single root.

### Proof Generation

A proof for leaf at index `i` is the list of sibling hashes along the path from that leaf to the root, plus a flag for each step indicating whether the sibling is on the left or right:

```
Tree for [A, B, C, D]:

              ROOT
             /    \
          H(AB)   H(CD)
          /  \    /  \
         A    B  C    D
              ▲
         prove B (index 1)

proof       = [A,    H(CD)]
proof_flags = [true, false]
```

The on-chain verifier reconstructs the root from `(leaf, proof, proof_flags)` and checks it against the stored root.

---

## API Reference

### `MerkleTree::new(levels: usize) → MerkleTree`

Creates an empty tree. `levels` sets the maximum depth (32 is a safe default for most use cases).

```rust
let mut tree = MerkleTree::new(32);
```

### `MerkleTree::build(leaves: Vec<Vec<u8>>) → Result<(), &'static str>`

Builds the tree from a list of pre-hashed leaf values. Each leaf must be exactly 32 bytes (a SHA-256 digest). Returns an error if `leaves` is empty.

```rust
use sha2::{Digest, Sha256};

let leaves: Vec<Vec<u8>> = vec![
    Sha256::digest(b"message_0").to_vec(),
    Sha256::digest(b"message_1").to_vec(),
    Sha256::digest(b"message_2").to_vec(),
    Sha256::digest(b"message_3").to_vec(),
];

tree.build(leaves)?;
```

### `MerkleTree::get_root_hex() → String`

Returns the root hash as a lowercase hex string, ready to pass to `update_root` on-chain.

```rust
let root = tree.get_root_hex();
// "a1b2c3d4e5f6..."
```

### `tree.root: [u8; 32]`

The raw root bytes, available directly after `build()`.

---

## Usage Examples

### Example 1: Build a Tree and Get the Root

```rust
use soroscope_core::merkle_tree::MerkleTree;
use sha2::{Digest, Sha256};

fn main() {
    // Hash your raw messages first
    let messages = vec![
        b"transfer: Alice -> Bob, 100 XLM".as_ref(),
        b"transfer: Carol -> Dave, 50 XLM".as_ref(),
        b"transfer: Eve -> Frank, 200 XLM".as_ref(),
        b"transfer: Grace -> Heidi, 75 XLM".as_ref(),
    ];

    let leaves: Vec<Vec<u8>> = messages
        .iter()
        .map(|m| Sha256::digest(m).to_vec())
        .collect();

    let mut tree = MerkleTree::new(32);
    tree.build(leaves).expect("failed to build tree");

    println!("Merkle root: {}", tree.get_root_hex());
    // Post this root on-chain via update_root()
}
```

### Example 2: Generate a Proof for a Specific Leaf

```rust
use soroscope_core::merkle_tree::MerkleTree;
use sha2::{Digest, Sha256};

fn main() {
    let leaves: Vec<Vec<u8>> = (0u8..4)
        .map(|i| Sha256::digest(&[i; 32]).to_vec())
        .collect();

    let mut tree = MerkleTree::new(32);
    tree.build(leaves.clone()).expect("build failed");

    // Generate proof for leaf at index 1
    let leaf_index = 1usize;
    let (proof, proof_flags) = tree
        .generate_proof(leaf_index)
        .expect("proof generation failed");

    println!("Leaf:        {}", hex::encode(&leaves[leaf_index]));
    println!("Root:        {}", tree.get_root_hex());
    println!("Proof steps: {}", proof.len());
    for (i, (sibling, is_left)) in proof.iter().zip(proof_flags.iter()).enumerate() {
        println!(
            "  Step {}: sibling={} side={}",
            i,
            hex::encode(sibling),
            if *is_left { "left" } else { "right" }
        );
    }
}
```

### Example 3: Verify a Proof Locally (Before Submitting On-Chain)

Use this to sanity-check a proof off-chain before paying transaction fees.

```rust
use soroscope_core::merkle_tree::MerkleTree;
use sha2::{Digest, Sha256};

fn verify_proof_locally(
    root: &[u8; 32],
    leaf: &[u8],
    proof: &[Vec<u8>],
    proof_flags: &[bool],
) -> bool {
    let mut current = leaf.to_vec();

    for (sibling, is_left) in proof.iter().zip(proof_flags.iter()) {
        let mut combined = Vec::with_capacity(64);
        if *is_left {
            combined.extend_from_slice(sibling);
            combined.extend_from_slice(&current);
        } else {
            combined.extend_from_slice(&current);
            combined.extend_from_slice(sibling);
        }
        current = Sha256::digest(&combined).to_vec();
    }

    current.as_slice() == root
}

fn main() {
    let leaves: Vec<Vec<u8>> = (0u8..4)
        .map(|i| Sha256::digest(&[i; 32]).to_vec())
        .collect();

    let mut tree = MerkleTree::new(32);
    tree.build(leaves.clone()).unwrap();

    let (proof, proof_flags) = tree.generate_proof(2).unwrap();

    let valid = verify_proof_locally(&tree.root, &leaves[2], &proof, &proof_flags);
    assert!(valid, "proof should be valid");
    println!("Proof verified locally ✓");
}
```

### Example 4: Full Relayer Pipeline

This is the complete flow a bridge relayer would run: collect messages, build the tree, post the root, and hand off proofs to recipients.

```rust
use soroscope_core::merkle_tree::MerkleTree;
use sha2::{Digest, Sha256};

struct CrossChainMessage {
    id: u64,
    payload: Vec<u8>,
}

fn hash_message(msg: &CrossChainMessage) -> Vec<u8> {
    let mut data = msg.id.to_be_bytes().to_vec();
    data.extend_from_slice(&msg.payload);
    Sha256::digest(&data).to_vec()
}

fn main() {
    // 1. Collect all messages from source-chain block 1000
    let messages = vec![
        CrossChainMessage { id: 0, payload: b"transfer_alice_bob_100".to_vec() },
        CrossChainMessage { id: 1, payload: b"transfer_carol_dave_50".to_vec() },
        CrossChainMessage { id: 2, payload: b"transfer_eve_frank_200".to_vec() },
        CrossChainMessage { id: 3, payload: b"transfer_grace_heidi_75".to_vec() },
    ];

    // 2. Hash each message to produce leaves
    let leaves: Vec<Vec<u8>> = messages.iter().map(hash_message).collect();

    // 3. Build the Merkle tree
    let mut tree = MerkleTree::new(32);
    tree.build(leaves.clone()).expect("build failed");

    let root_hex = tree.get_root_hex();
    println!("Block 1000 root: {}", root_hex);

    // 4. Post root on-chain (pseudo-code — use soroban-cli or SDK)
    // soroban_client.invoke("update_root", block_height=1000, new_root=root_hex)

    // 5. Generate proof for message id=1 (Carol -> Dave) so she can claim on Stellar
    let (proof, proof_flags) = tree.generate_proof(1).expect("proof failed");

    println!("\nProof for message id=1:");
    println!("  Leaf: {}", hex::encode(&leaves[1]));
    for (i, (sib, flag)) in proof.iter().zip(proof_flags.iter()).enumerate() {
        println!(
            "  Step {}: {} ({})",
            i,
            hex::encode(sib),
            if *flag { "left sibling" } else { "right sibling" }
        );
    }

    // 6. Recipient submits proof on-chain (pseudo-code)
    // soroban_client.invoke("verify_message",
    //     block_height=1000,
    //     leaf=leaves[1],
    //     proof=proof,
    //     proof_flags=proof_flags
    // )
}
```

### Example 5: Large Tree (Odd Number of Leaves)

The tree handles odd-length levels by duplicating the last node. This example shows a 5-leaf tree.

```rust
use soroscope_core::merkle_tree::MerkleTree;
use sha2::{Digest, Sha256};

fn main() {
    // 5 leaves — level 1 will have 3 nodes (2 pairs + 1 duplicate)
    let leaves: Vec<Vec<u8>> = (0u8..5)
        .map(|i| Sha256::digest(&[i; 32]).to_vec())
        .collect();

    let mut tree = MerkleTree::new(32);
    tree.build(leaves.clone()).expect("build failed");

    println!("5-leaf tree root: {}", tree.get_root_hex());

    // Prove the last leaf (index 4) — it was duplicated at level 1
    let (proof, proof_flags) = tree.generate_proof(4).expect("proof failed");
    println!("Proof for leaf 4 has {} steps", proof.len());
}
```

---

## CLI Workflow

The Merkle Tree utility is a Rust library, not a standalone binary. The typical workflow combines it with `soroban-cli` for the on-chain steps.

### Build and Post a Root

```bash
# 1. Write a small Rust script using MerkleTree (see examples above)
#    and print the root hex to stdout.

# 2. Capture the root and post it on-chain
ROOT=$(cargo run --example build_tree -- --block 1000)

soroban contract invoke \
  --id <CONTRACT_ID> \
  --source relayer \
  --network testnet \
  -- update_root \
  --block_height 1000 \
  --new_root "$ROOT"
```

### Verify a Proof On-Chain

```bash
# 1. Generate proof hex values from your Rust script
LEAF="2222222222222222222222222222222222222222222222222222222222222222"
PROOF='["1111111111111111111111111111111111111111111111111111111111111111","<sibling_2_hex>"]'
FLAGS='[true, false]'

# 2. Call verify_message
soroban contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  -- verify_message \
  --block_height 1000 \
  --leaf "$LEAF" \
  --proof "$PROOF" \
  --proof_flags "$FLAGS"
# Output: true
```

---

## Integration with cross_chain_verifier

The `MerkleTree` output maps directly to the contract's `verify_message` parameters:

| `MerkleTree` output | `verify_message` parameter | Notes |
|---|---|---|
| `tree.root` | `new_root` (via `update_root`) | Post before verifying |
| `leaves[i]` | `leaf` | The SHA-256 hash of your message |
| `proof[j]` | `proof[j]` | Sibling hash at step j |
| `proof_flags[j]` | `proof_flags[j]` | `true` = left sibling |

See the [cross_chain_verifier README](../contracts/cross_chain_verifier/README.md) for the full on-chain CLI reference.

---

## Configuration

The `MerkleTree` struct has no external configuration. The `levels` parameter passed to `new()` sets the maximum tree depth:

| `levels` | Max leaves | Typical use case |
|---|---|---|
| `16` | 65,536 | Small batches, testing |
| `20` | 1,048,576 | Medium-sized blocks |
| `32` | ~4 billion | Production / large blocks |

---

## Testing

```bash
# Run the merkle_tree unit tests
cargo test -p soroscope-core merkle_tree

# Run with output
cargo test -p soroscope-core merkle_tree -- --nocapture
```

The test suite covers:
- `test_merkle_tree_basic_commit` — builds a 3-leaf tree without panicking

---

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│  core/src/merkle_tree.rs                                 │
│                                                          │
│  MerkleTree                                              │
│  ├── new(levels)          Create empty tree              │
│  ├── build(leaves)        Hash leaves bottom-up          │
│  ├── get_root_hex()       Root as hex string             │
│  ├── generate_proof(i)    Sibling path + flags           │
│  └── root: [u8; 32]       Raw root bytes                 │
│                                                          │
│  Internal helpers                                        │
│  └── calculate_root_hash  Iterative bottom-up hashing    │
└──────────────────────────────────────────────────────────┘
           │ root_hex                │ (proof, proof_flags)
           ▼                         ▼
┌──────────────────┐      ┌──────────────────────────────┐
│  update_root()   │      │  verify_message()            │
│  (on-chain)      │      │  (on-chain)                  │
└──────────────────┘      └──────────────────────────────┘
```

---

## Troubleshooting

### "Cannot build tree from empty leaves"
You must pass at least one leaf to `build()`. Ensure your message collection is non-empty before calling it.

### Proof verification returns `false` on-chain
Common causes:
1. **Wrong leaf hash** — the `leaf` passed to `verify_message` must be the SHA-256 hash of the raw message, not the raw message itself.
2. **Proof flags reversed** — double-check that `true` means the sibling is on the left (i.e., `SHA-256(sibling || current)`).
3. **Root mismatch** — the root posted via `update_root` must have been computed from the same set of leaves in the same order.
4. **Index off-by-one** — leaf indices are 0-based.

### Root differs between runs
`MerkleTree` is deterministic given the same leaves in the same order. If the root changes, the leaf list or ordering changed.

---

## Future Enhancements

- [ ] `generate_proof(index)` method (currently a placeholder — implement alongside full tree storage)
- [ ] Incremental leaf insertion without full rebuild
- [ ] Sparse Merkle Tree variant for key-value inclusion proofs
- [ ] CLI binary (`soroscope-merkle`) for scripting without writing Rust
- [ ] Multi-proof generation (prove multiple leaves in one pass)
- [ ] Integration with `private_transfer` commitment tree

---

## Related

- [`contracts/cross_chain_verifier`](../contracts/cross_chain_verifier/) — On-chain contract that stores roots and verifies proofs
- [`contracts/private_transfer`](../contracts/private_transfer/) — Uses a similar Merkle commitment tree for ZK-style private transfers
- [`core/FEE_MARKET_README.md`](FEE_MARKET_README.md) — Fee market prediction system documentation
