# Cross Chain Verifier Standard

## Overview

The **Cross Chain Verifier Standard** defines how external bridges must format and submit cross-chain messages for verification on Soroban. This standard ensures interoperability between different bridge implementations and provides a consistent security model for cross-chain message verification.

### Purpose

Cross-chain bridges need a standardized way to prove that a message or transaction occurred on a source chain. The Cross Chain Verifier contract uses **Binary Merkle Tree proofs** to cryptographically verify that a specific message was included in a block on the source chain, without requiring the verifier to process the entire source chain state.

### Why a Standard Format?

- **Interoperability**: Multiple bridge implementations can work with the same verifier contract
- **Security**: Consistent validation rules prevent malformed or malicious messages
- **Auditability**: Clear message structure enables security reviews and monitoring
- **Upgradability**: Version fields allow the standard to evolve without breaking existing bridges

---

## Message Envelope

External bridges must construct messages that can be verified using Binary Merkle Tree proofs. The verification process requires the following components:

### Required Components

#### 1. Block Height (`u32`)

The block height on the source chain where the message was included.

- **Type**: Unsigned 32-bit integer
- **Purpose**: Identifies which state root to verify against
- **Validation**: Must correspond to a state root previously submitted by the relayer network
- **Example**: `100`, `1234567`

#### 2. Leaf Hash (`BytesN<32>`)

The SHA-256 hash of the cross-chain message payload.

- **Type**: 32-byte fixed-length array
- **Purpose**: Represents the message in the Merkle tree
- **Encoding**: Raw bytes (not hex-encoded)
- **Computation**: `SHA256(canonical_message_bytes)`
- **Example**: `[0x2a, 0x3b, 0x4c, ..., 0xff]` (32 bytes)

#### 3. Merkle Proof (`Vec<BytesN<32>>`)

An ordered list of sibling hashes forming the Merkle proof path from leaf to root.

- **Type**: Vector of 32-byte arrays
- **Purpose**: Provides the cryptographic proof of inclusion
- **Ordering**: Must match the tree traversal from leaf to root
- **Length**: Equals the tree depth (typically log₂(transaction_count))
- **Example**: `[[0x3a, ...], [0x4b, ...], [0x5c, ...]]`

#### 4. Proof Flags (`Vec<bool>`)

Boolean flags indicating sibling position for each proof element.

- **Type**: Vector of booleans
- **Purpose**: Specifies whether each sibling is on the left (`true`) or right (`false`)
- **Length**: Must equal the length of the Merkle proof
- **Validation**: Length mismatch causes verification to fail
- **Example**: `[true, false, true]` means:
  - First sibling is on the left
  - Second sibling is on the right
  - Third sibling is on the left

### Message Structure Summary

```rust
struct CrossChainMessage {
    block_height: u32,           // Source chain block height
    leaf: BytesN<32>,            // SHA-256 hash of message payload
    proof: Vec<BytesN<32>>,      // Merkle proof siblings
    proof_flags: Vec<bool>,      // Sibling positions (left=true, right=false)
}
```

---

## Formatting Rules

### 1. Canonical Message Serialization

Before computing the leaf hash, bridges must serialize the message payload in a canonical format:

#### Serialization Requirements

- **Deterministic**: Same message must always produce the same byte sequence
- **No padding**: Remove unnecessary padding or alignment bytes
- **Fixed field order**: Fields must appear in a consistent order
- **Explicit lengths**: Variable-length fields must be prefixed with their length

#### Recommended Serialization Format

```
[version: 1 byte]
[source_chain_id: 4 bytes, big-endian]
[destination_chain_id: 4 bytes, big-endian]
[nonce: 8 bytes, big-endian]
[timestamp: 8 bytes, big-endian, Unix seconds]
[sender_address_length: 4 bytes, big-endian]
[sender_address: variable bytes]
[recipient_address_length: 4 bytes, big-endian]
[recipient_address: variable bytes]
[payload_length: 4 bytes, big-endian]
[payload: variable bytes]
```

#### Example Canonical Message

```
Version: 0x01
Source Chain: 0x00000001 (Ethereum)
Destination Chain: 0x00000002 (Stellar)
Nonce: 0x0000000000000042
Timestamp: 0x0000000065a1b2c3
Sender: 0x0000001a + "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb"
Recipient: 0x00000038 + "GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H"
Payload: 0x00000010 + [16 bytes of token transfer data]
```

### 2. Leaf Hash Computation

```rust
// Pseudocode
canonical_bytes = serialize_message(message);
leaf_hash = SHA256(canonical_bytes);
```

**Critical**: The leaf hash must be computed over the **exact canonical bytes** that were included in the source chain's Merkle tree.

### 3. Merkle Proof Construction

The Merkle proof must follow the Binary Merkle Tree structure used by the source chain:

#### Proof Construction Algorithm

```
current_hash = leaf_hash
proof = []
proof_flags = []

for each level from leaf to root:
    sibling = get_sibling_at_level(current_index, level)
    is_left_sibling = (current_index % 2 == 1)
    
    proof.append(sibling)
    proof_flags.append(is_left_sibling)
    
    if is_left_sibling:
        current_hash = SHA256(sibling || current_hash)
    else:
        current_hash = SHA256(current_hash || sibling)
    
    current_index = current_index / 2
```

#### Hash Combination Rules

When combining hashes during proof verification:

```rust
// If sibling is on the left (proof_flag = true)
combined = [sibling_bytes (32 bytes)] + [current_hash (32 bytes)]
next_hash = SHA256(combined)  // 64 bytes input

// If sibling is on the right (proof_flag = false)
combined = [current_hash (32 bytes)] + [sibling_bytes (32 bytes)]
next_hash = SHA256(combined)  // 64 bytes input
```

**Important**: The concatenation order matters. Incorrect ordering will produce an invalid root hash.

### 4. Encoding Rules

#### Binary Representation

All components must be provided in raw binary format:

- **Hashes**: 32-byte arrays (not hex strings)
- **Integers**: Native integer types (u32 for block height)
- **Booleans**: Native boolean type
- **Vectors**: Soroban SDK `Vec` type

#### No Base64 or Hex Encoding

The verifier contract expects raw bytes. Do not encode hashes as:
- ❌ Hex strings: `"0x1234abcd..."`
- ❌ Base64 strings: `"EjQ6vN..."`
- ✅ Raw bytes: `BytesN<32>`

---

## Validation Rules

The Cross Chain Verifier performs the following validation checks:

### 1. Structural Validation

**Check**: Proof and proof_flags vectors have equal length

```rust
if proof.len() != proof_flags.len() {
    panic!("Invalid proof format");
}
```

**Reason**: Each proof element requires a corresponding position flag.

### 2. State Root Availability

**Check**: State root exists for the specified block height

```rust
let expected_root = storage.get(StateRoot(block_height))
    .unwrap_or_else(|| panic!("State root not found"));
```

**Reason**: Cannot verify messages against unknown state roots.

**Bridge Requirement**: Bridges must wait for the relayer network to submit the state root before attempting verification.

### 3. Merkle Proof Verification

**Check**: Recomputed root matches the stored state root

```rust
let computed_root = compute_merkle_root(leaf, proof, proof_flags);
if computed_root != expected_root {
    return false;  // Verification failed
}
```

**Reason**: Proves the message was included in the source chain block.

### 4. Replay Protection

**Responsibility**: Bridge contracts (not the verifier)

The verifier only checks cryptographic proof validity. Bridge contracts must implement their own replay protection using:

- **Nonce tracking**: Store processed message nonces
- **Message ID tracking**: Store processed message hashes
- **Sequence numbers**: Enforce monotonically increasing sequences

**Example**:

```rust
pub fn process_cross_chain_message(
    env: Env,
    block_height: u32,
    leaf: BytesN<32>,
    proof: Vec<BytesN<32>>,
    proof_flags: Vec<bool>,
) {
    // 1. Verify the message proof
    let verified = CrossChainVerifierClient::new(&env, &verifier_id)
        .verify_message(&block_height, &leaf, &proof, &proof_flags);
    
    if !verified {
        panic!("Invalid cross-chain proof");
    }
    
    // 2. Check for replay (bridge's responsibility)
    if env.storage().instance().has(&MessageProcessed(leaf)) {
        panic!("Message already processed");
    }
    
    // 3. Mark as processed
    env.storage().instance().set(&MessageProcessed(leaf), &true);
    
    // 4. Execute message logic
    execute_message_payload(...);
}
```

### 5. Payload Integrity

**Responsibility**: Bridge contracts (not the verifier)

The verifier only validates that the leaf hash was included in the Merkle tree. Bridge contracts must:

- Parse the message payload
- Validate payload structure
- Check payload signatures if required
- Verify sender authorization
- Validate amounts and addresses

---

## Versioning and Compatibility

### Current Version: 1.0

The initial version of the Cross Chain Verifier Standard uses:

- **Binary Merkle Trees** with SHA-256 hashing
- **32-byte hash outputs** (SHA-256 standard)
- **u32 block heights** (supports up to 4.2 billion blocks)
- **Boolean proof flags** for sibling positioning

### Version Field

Bridges should include a version byte in their canonical message format:

```rust
const STANDARD_VERSION: u8 = 1;
```

### Future Compatibility

#### Adding New Fields

Future versions may add optional fields to the message envelope. Bridges should:

- Include version in the canonical message
- Ignore unknown fields when parsing older versions
- Validate required fields based on version

#### Changing Hash Algorithms

If the standard migrates to a different hash algorithm (e.g., SHA-3, BLAKE3):

- New version number will be assigned
- Old verifier contracts will continue supporting v1
- New verifier contracts will support both versions
- Bridges can upgrade gradually

#### Backward Compatibility Policy

- **Minor versions** (1.x): Backward compatible, add optional fields only
- **Major versions** (2.0): May break compatibility, require bridge upgrades

### Deprecation Process

When deprecating a version:

1. **Announcement**: 6 months advance notice
2. **Dual support**: Both versions supported for 12 months
3. **Deprecation**: Old version marked deprecated but still functional
4. **Removal**: Old version removed after 18 months total

---

## Security Considerations

### What the Verifier Guarantees

✅ **Cryptographic Proof**: The message was included in the source chain block  
✅ **State Root Integrity**: The state root was submitted by authorized relayers  
✅ **Proof Validity**: The Merkle proof correctly links leaf to root  

### What the Verifier Does NOT Guarantee

❌ **Message Authenticity**: Does not verify who sent the message  
❌ **Replay Protection**: Does not prevent processing the same message twice  
❌ **Payload Validity**: Does not validate message content or structure  
❌ **Authorization**: Does not check if sender is authorized  
❌ **Economic Security**: Does not validate amounts or balances  

### Bridge Operator Responsibilities

Bridge operators **MUST** ensure:

1. **State Root Accuracy**: Only submit correct state roots from the source chain
2. **Timely Updates**: Submit state roots promptly to avoid verification delays
3. **Admin Security**: Protect admin keys with multi-signature or HSM
4. **Monitoring**: Detect and respond to invalid state root submissions
5. **Liveness**: Maintain continuous operation to prevent message delays

### Security Assumptions

The verifier assumes:

1. **Trusted Relayers**: The admin/relayer network submits honest state roots
2. **Source Chain Security**: The source chain's Merkle tree construction is correct
3. **Hash Function Security**: SHA-256 is collision-resistant
4. **No State Root Manipulation**: Admins cannot be coerced to submit false roots

### Attack Vectors and Mitigations

#### 1. Invalid State Root Submission

**Attack**: Malicious admin submits incorrect state root  
**Impact**: Could verify fraudulent messages  
**Mitigation**: Use multi-signature admin control, monitor state root submissions

#### 2. Replay Attacks

**Attack**: Resubmit valid proof multiple times  
**Impact**: Process same message multiple times  
**Mitigation**: Bridge contracts must implement nonce/message ID tracking

#### 3. Proof Manipulation

**Attack**: Modify proof or proof_flags  
**Impact**: Verification will fail (computed root won't match)  
**Mitigation**: Cryptographic security of Merkle proofs prevents this

#### 4. Leaf Hash Collision

**Attack**: Find two messages with same SHA-256 hash  
**Impact**: Could substitute message content  
**Mitigation**: SHA-256 collision resistance (computationally infeasible)

#### 5. State Root Delay

**Attack**: Delay state root submission to prevent message verification  
**Impact**: Denial of service for cross-chain messages  
**Mitigation**: Monitor relayer liveness, implement fallback relayers

### Recommended Security Practices

1. **Multi-Signature Admin**: Require M-of-N signatures for state root updates
2. **State Root Monitoring**: Compare submitted roots against multiple source chain nodes
3. **Rate Limiting**: Limit state root update frequency to prevent spam
4. **Event Logging**: Log all state root updates for audit trails
5. **Emergency Pause**: Implement pause mechanism for bridge contracts
6. **Proof Validation**: Verify proof length is reasonable (< 64 elements)
7. **Message Expiry**: Include timestamps and reject old messages

---

## Examples

### Example 1: Simple Token Transfer Message

#### Message Payload

```json
{
  "version": 1,
  "source_chain": "ethereum",
  "destination_chain": "stellar",
  "nonce": 42,
  "timestamp": 1704067200,
  "sender": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
  "recipient": "GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H",
  "payload": {
    "type": "token_transfer",
    "token": "USDC",
    "amount": "1000000000"
  }
}
```

#### Canonical Serialization

```
01                                          // version
00000001                                    // source_chain_id (Ethereum)
00000002                                    // destination_chain_id (Stellar)
000000000000002a                            // nonce (42)
0000000065a1b2c3                            // timestamp
0000002a                                    // sender length (42 bytes)
307837343264333543633636333443303533323932356133623834344263396537353935663062456220  // sender
00000038                                    // recipient length (56 bytes)
4742525059484...                            // recipient
00000020                                    // payload length (32 bytes)
...                                         // payload bytes
```

#### Leaf Hash Computation

```rust
let canonical_bytes = serialize_message(&message);
let leaf_hash = env.crypto().sha256(&canonical_bytes);
// Result: BytesN<32> = [0x7a, 0x3f, 0x9c, ..., 0x2d]
```

#### Merkle Proof

Assuming the message is at index 5 in a tree with 8 transactions:

```
Tree structure:
                    Root
                   /    \
                  /      \
                 /        \
              H01          H23
             /  \         /  \
           H0    H1     H2    H3
          / \   / \    / \   / \
         T0 T1 T2 T3  T4 T5 T6 T7
                         ^
                      (our message)

Proof path: T5 -> H2 -> H01 -> Root
Siblings needed: T4, H3, H01
```

```rust
let proof = vec![
    BytesN::from_array(&env, &[0x4a, 0x2b, ..., 0x1c]),  // T4 (sibling of T5)
    BytesN::from_array(&env, &[0x8f, 0x7e, ..., 0x3d]),  // H3 (sibling of H2)
    BytesN::from_array(&env, &[0x2c, 0x9a, ..., 0x5f]),  // H01 (sibling of H23)
];

let proof_flags = vec![
    true,   // T4 is on the left of T5
    false,  // H3 is on the right of H2
    true,   // H01 is on the left of H23
];
```

#### Verification Call

```rust
let verified = verifier_client.verify_message(
    &block_height,  // e.g., 100
    &leaf_hash,     // SHA-256 of canonical message
    &proof,         // [T4, H3, H01]
    &proof_flags,   // [true, false, true]
);

assert!(verified);
```

### Example 2: Verification Computation

Step-by-step verification of the proof from Example 1:

```rust
// Initial state
let mut current_hash = leaf_hash;  // T5

// Level 1: Combine with T4
let sibling_1 = proof[0];  // T4
let is_left_1 = proof_flags[0];  // true (T4 is left)

let mut combined = [0u8; 64];
combined[0..32].copy_from_slice(&sibling_1.to_array());  // T4 on left
combined[32..64].copy_from_slice(&current_hash);         // T5 on right
current_hash = SHA256(combined);  // Result: H2

// Level 2: Combine with H3
let sibling_2 = proof[1];  // H3
let is_left_2 = proof_flags[1];  // false (H3 is right)

combined[0..32].copy_from_slice(&current_hash);          // H2 on left
combined[32..64].copy_from_slice(&sibling_2.to_array()); // H3 on right
current_hash = SHA256(combined);  // Result: H23

// Level 3: Combine with H01
let sibling_3 = proof[2];  // H01
let is_left_3 = proof_flags[2];  // true (H01 is left)

combined[0..32].copy_from_slice(&sibling_3.to_array());  // H01 on left
combined[32..64].copy_from_slice(&current_hash);         // H23 on right
current_hash = SHA256(combined);  // Result: Root

// Final check
assert_eq!(current_hash, expected_root);
```

### Example 3: Bridge Contract Integration

```rust
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Vec};

#[contract]
pub struct TokenBridge;

#[contractimpl]
impl TokenBridge {
    /// Process an incoming cross-chain token transfer
    pub fn receive_tokens(
        env: Env,
        block_height: u32,
        message_hash: BytesN<32>,
        proof: Vec<BytesN<32>>,
        proof_flags: Vec<bool>,
        recipient: Address,
        amount: i128,
    ) {
        // 1. Verify the Merkle proof
        let verifier = CrossChainVerifierClient::new(&env, &get_verifier_id(&env));
        let verified = verifier.verify_message(
            &block_height,
            &message_hash,
            &proof,
            &proof_flags,
        );
        
        if !verified {
            panic!("Invalid cross-chain proof");
        }
        
        // 2. Check for replay
        let processed_key = DataKey::ProcessedMessage(message_hash);
        if env.storage().instance().has(&processed_key) {
            panic!("Message already processed");
        }
        
        // 3. Mark as processed
        env.storage().instance().set(&processed_key, &true);
        
        // 4. Execute the transfer
        let token = get_token_client(&env);
        token.mint(&recipient, &amount);
        
        // 5. Emit event
        env.events().publish(
            (symbol_short!("received"),),
            (recipient, amount, block_height),
        );
    }
}
```

---

## Implementation Checklist

### For Bridge Developers

- [ ] Implement canonical message serialization
- [ ] Compute leaf hash using SHA-256
- [ ] Construct Binary Merkle Tree proofs correctly
- [ ] Generate proof_flags matching sibling positions
- [ ] Wait for state root submission before verification
- [ ] Implement replay protection in bridge contract
- [ ] Validate message payload structure
- [ ] Handle verification failures gracefully
- [ ] Add event logging for cross-chain messages
- [ ] Test with various tree sizes and positions
- [ ] Document message format for users
- [ ] Implement message expiry mechanism

### For Relayer Operators

- [ ] Monitor source chain for new blocks
- [ ] Extract state roots from source chain
- [ ] Submit state roots to verifier contract
- [ ] Secure admin keys with multi-signature
- [ ] Implement monitoring and alerting
- [ ] Maintain high availability
- [ ] Log all state root submissions
- [ ] Verify state roots against multiple nodes
- [ ] Implement rate limiting
- [ ] Document operational procedures

### For Auditors

- [ ] Verify canonical serialization is deterministic
- [ ] Check leaf hash computation matches standard
- [ ] Validate Merkle proof construction algorithm
- [ ] Confirm proof_flags logic is correct
- [ ] Review replay protection implementation
- [ ] Check admin key security
- [ ] Verify state root submission process
- [ ] Test edge cases (empty proofs, invalid flags)
- [ ] Review error handling
- [ ] Validate event logging

---

## Reference Implementation

The reference implementation is available in the SoroScope repository:

- **Contract**: `contracts/cross_chain_verifier/src/lib.rs`
- **Tests**: `contracts/cross_chain_verifier/src/test.rs`
- **Repository**: https://github.com/SoroLabs/soroscope

### Key Functions

```rust
/// Initialize the verifier with an admin
pub fn initialize(env: Env, admin: Address)

/// Update state root for a block height (admin only)
pub fn update_root(env: Env, block_height: u32, new_root: BytesN<32>)

/// Retrieve stored state root
pub fn get_root(env: Env, block_height: u32) -> Option<BytesN<32>>

/// Verify a cross-chain message
pub fn verify_message(
    env: Env,
    block_height: u32,
    leaf: BytesN<32>,
    proof: Vec<BytesN<32>>,
    proof_flags: Vec<bool>,
) -> bool
```

---

## Glossary

- **Binary Merkle Tree**: A tree structure where each non-leaf node is the hash of its two children
- **Block Height**: The sequential number of a block in a blockchain
- **Canonical Serialization**: A deterministic byte representation of structured data
- **Leaf Hash**: The hash of a message that appears as a leaf in the Merkle tree
- **Merkle Proof**: A list of sibling hashes proving a leaf is in the tree
- **Proof Flags**: Boolean indicators of sibling position (left or right)
- **Relayer**: A service that submits state roots from source chain to verifier
- **Replay Attack**: Resubmitting a valid message to execute it multiple times
- **State Root**: The root hash of a Merkle tree representing blockchain state
- **Sibling**: The adjacent node at the same level in a Merkle tree

---

## Support and Feedback

For questions, issues, or suggestions regarding this standard:

- **GitHub Issues**: https://github.com/SoroLabs/soroscope/issues
- **Documentation**: https://github.com/SoroLabs/soroscope/tree/main/docs
- **Contributing**: See [CONTRIBUTING.md](../CONTRIBUTING.md)

---

**Version**: 1.0  
**Last Updated**: May 30, 2026  
**Status**: ✅ Active Standard  
**Maintainer**: SoroLabs
