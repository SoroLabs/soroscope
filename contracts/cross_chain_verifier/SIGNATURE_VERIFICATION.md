# Cross-Chain Message Signature Verification

## Overview

The Cross-Chain Verifier contract implements comprehensive signature verification for incoming cross-chain messages using industry-standard cryptographic algorithms. This document describes the signature verification implementation and security considerations.

## Supported Signature Algorithms

### 1. Ed25519

**Characteristics:**
- Modern elliptic curve signature scheme
- 128-bit security level
- Deterministic signatures (no randomness needed)
- Fast verification
- Resistance to side-channel attacks
- Public key size: 32 bytes
- Signature size: 64 bytes

**Use Cases:**
- High-performance applications
- Systems requiring deterministic signatures
- Modern blockchain protocols

**Implementation:**
```rust
env.crypto().ed25519_verify(&public_key, &message_hash, &signature)
```

### 2. Secp256k1 (ECDSA)

**Characteristics:**
- ECDSA curve used by Bitcoin and Ethereum
- 128-bit security level
- Widely adopted and battle-tested
- Support for key recovery from signatures
- Public key size: 33 bytes (compressed) or 65 bytes (uncompressed)
- Signature size: 64 bytes

**Use Cases:**
- Interoperability with Bitcoin/Ethereum ecosystems
- Legacy system integration
- Cross-chain bridges with established networks

**Implementation:**
```rust
env.crypto().secp256k1_verify(&public_key, &message_hash, &signature)
```

## Message Verification Pipeline

The `verify_signed_message` function implements a four-step verification process:

### Step 1: Signature Verification

1. Retrieve the list of authorized signers from contract storage
2. Check if the signer's public key is in the authorized list
3. Retrieve the signature algorithm associated with the signer
4. Hash the message with domain separation
5. Verify the signature using the appropriate algorithm

**Security Properties:**
- Only authorized signers can verify messages
- Each signer is associated with a specific algorithm
- Unauthorized signers are rejected immediately

### Step 2: Replay Protection

1. Hash the message to create a unique identifier
2. Check if the message hash exists in the `ProcessedMessages` storage
3. Reject if the message has already been processed

**Security Properties:**
- Prevents duplicate message execution
- Each message can only be processed once
- Persistent storage ensures protection across contract invocations

### Step 3: Merkle Proof Verification

1. Retrieve the expected state root for the specified block height
2. Validate that the proof structure is well-formed
3. Iteratively hash proof siblings with the current hash
4. Compare the computed root with the stored state root

**Security Properties:**
- Confirms message inclusion in a specific block
- Prevents messages from being verified against wrong blocks
- Merkle tree structure ensures efficient verification

### Step 4: State Update and Event Emission

1. Mark the message as processed in persistent storage
2. Emit a `message_verified` event with chain IDs and nonce

**Security Properties:**
- Ensures replay protection is enforced
- Provides audit trail of verified messages
- Enables off-chain monitoring and alerting

## Domain Separation

The contract implements domain separation to prevent cross-protocol attacks:

```
CROSS_CHAIN_MESSAGE_V1 || source_chain || destination_chain || nonce || timestamp || sha256(payload)
```

**Benefits:**
- Messages intended for one protocol cannot be replayed in another
- Different protocol versions have different domain separators
- Prevents accidental or malicious cross-protocol attacks

## Authorized Signer Management

### Adding a Signer

```rust
pub fn add_authorized_signer(env: Env, public_key: Bytes, algorithm: SignatureAlgorithm)
```

**Requirements:**
- Caller must be the contract admin
- Public key must not already be authorized
- Algorithm must be specified (Ed25519 or Secp256k1)

**Events:**
- Emits `signer_added` event

### Removing a Signer

```rust
pub fn remove_authorized_signer(env: Env, public_key: Bytes)
```

**Requirements:**
- Caller must be the contract admin
- Public key must exist in authorized signers list

**Events:**
- Emits `signer_removed` event

### Querying Signers

```rust
pub fn get_authorized_signers(env: Env) -> Vec<(Bytes, SignatureAlgorithm)>
```

Returns all currently authorized signers with their associated algorithms.

## Security Considerations

### 1. Public Key Validation

- Ed25519 public keys must be exactly 32 bytes
- Secp256k1 public keys can be 33 bytes (compressed) or 65 bytes (uncompressed)
- Invalid key sizes should be rejected at the application level

### 2. Signature Validation

- Signatures must be exactly 64 bytes for both algorithms
- Invalid signatures are rejected by the crypto module
- Malformed signatures cannot be exploited

### 3. Message Hashing

- SHA256 is used for all hashing operations
- Domain separation prevents cross-protocol attacks
- Message fields are encoded in big-endian format for consistency

### 4. Replay Protection

- Each message is uniquely identified by its hash
- Processed messages are stored in persistent storage
- Cannot be bypassed by changing message fields

### 5. Merkle Proof Verification

- Proof structure must match the number of proof flags
- Sibling hashes are combined in the correct order
- Computed root must exactly match the stored root

### 6. Admin Authorization

- Only the contract admin can manage signers
- Admin authorization is enforced by Soroban's `require_auth()` mechanism
- Admin cannot be changed after initialization

## Testing

The contract includes comprehensive tests for:

1. **Initialization Tests**
   - Single initialization succeeds
   - Double initialization fails

2. **Signer Management Tests**
   - Adding Ed25519 signers
   - Adding Secp256k1 signers
   - Preventing duplicate signers
   - Removing signers
   - Querying signers

3. **Message Verification Tests**
   - Verification with invalid signer fails
   - Replay protection prevents duplicate messages
   - Multiple signers can be authorized

4. **Merkle Proof Tests**
   - Valid proofs are accepted
   - Missing state roots are rejected
   - Mismatched proof structures are rejected

## Integration Guide

### 1. Initialize the Contract

```rust
let admin = Address::generate(&env);
client.initialize(&admin);
```

### 2. Add Authorized Signers

```rust
// Add Ed25519 signer
let ed25519_key = Bytes::from_slice(&env, &[...]);
client.add_authorized_signer(&ed25519_key, &SignatureAlgorithm::Ed25519);

// Add Secp256k1 signer
let secp256k1_key = Bytes::from_slice(&env, &[...]);
client.add_authorized_signer(&secp256k1_key, &SignatureAlgorithm::Secp256k1);
```

### 3. Update State Roots

```rust
let block_height = 100;
let state_root = BytesN::from_array(&env, &[...]);
client.update_root(&block_height, &state_root);
```

### 4. Verify Cross-Chain Messages

```rust
let message = CrossChainMessage {
    source_chain: 1,
    destination_chain: 2,
    nonce: 1,
    payload: Bytes::from_slice(&env, b"data"),
    timestamp: 1000,
};

let signed_message = SignedMessage {
    message,
    signature: BytesN::from_array(&env, &[...]),
    signer_public_key: Bytes::from_slice(&env, &[...]),
    algorithm: SignatureAlgorithm::Ed25519,
};

let result = client.verify_signed_message(
    &signed_message,
    &block_height,
    &proof,
    &proof_flags,
);
```

## Performance Characteristics

- **Signature Verification**: O(1) - constant time
- **Signer Lookup**: O(n) - linear in number of signers
- **Merkle Proof Verification**: O(log n) - logarithmic in tree depth
- **Replay Protection Check**: O(1) - constant time hash lookup

## Future Enhancements

1. **Multi-Signature Support**: Require multiple signatures for critical messages
2. **Signature Aggregation**: Combine multiple signatures into one
3. **Threshold Schemes**: Support m-of-n signature schemes
4. **Key Rotation**: Implement time-based key rotation
5. **Signature Batching**: Verify multiple messages in one transaction

## References

- [Ed25519 Specification](https://tools.ietf.org/html/rfc8032)
- [Secp256k1 Specification](https://en.wikipedia.org/wiki/Secp256k1)
- [Soroban Crypto Module](https://docs.rs/soroban-sdk/latest/soroban_sdk/crypto/index.html)
- [Domain Separation Best Practices](https://crypto.stackexchange.com/questions/41740/what-is-domain-separation)
