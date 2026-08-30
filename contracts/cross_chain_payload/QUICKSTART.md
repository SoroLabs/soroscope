# Cross-Chain Payload Verification - Quick Start Guide

## 1. Creating a Cross-Chain Payload

```rust
use cross_chain_payload::{CrossChainPayload, PayloadMetadata};
use soroban_sdk::{BytesN, Bytes, String as SorobanString, Symbol, Env};

fn create_transfer_payload(env: &Env) -> CrossChainPayload {
    let payload_id = BytesN::from_array(&[1u8; 32]);
    let nonce = BytesN::from_array(&[2u8; 32]);
    let payload_hash = BytesN::from_array(&[3u8; 32]);
    
    let metadata = PayloadMetadata {
        version: 1,
        timestamp: 1000000,
        sequence: 1,
        expiration_height: 10000,
        nonce,
    };
    
    CrossChainPayload {
        payload_id,
        source_chain_id: 1,              // Stellar mainnet
        destination_chain_id: 2,         // Ethereum mainnet
        sender: Bytes::new(env),
        recipient: Bytes::new(env),
        data: Bytes::new(env),
        operation: Symbol::new(env, "transfer"),
        metadata,
        payload_hash,
        gas_limit: 1_000_000,
    }
}
```

## 2. Setting Up Chain Information

```rust
use cross_chain_payload::{ChainInfo, BridgeEndpoint};
use soroban_sdk::{BytesN, String as SorobanString};

fn setup_stellar_chain() -> ChainInfo {
    ChainInfo {
        chain_id: 1,
        chain_name: SorobanString::from_small_str("stellar"),
        network_version: 1,
        bridge_contract: BytesN::from_array(&[0u8; 32]),
        consensus_round: 100,
        is_active: true,
    }
}

fn create_bridge_endpoint(
    source: ChainInfo,
    destination: ChainInfo,
) -> BridgeEndpoint {
    BridgeEndpoint {
        source_chain: source,
        destination_chain: destination,
        fee_percentage: 50,          // 0.5%
        min_liquidity: 1_000_000_000, // 1 million base units
        is_enabled: true,
    }
}
```

## 3. Verifying Payloads with Signatures

```rust
use cross_chain_payload::{
    PayloadSignature, VerificationResult, VerificationStatus,
    SignatureScheme, SignatureCollection,
};
use soroban_sdk::{BytesN, Bytes};

fn verify_payload_signatures(
    payload_id: BytesN<32>,
    signatures: Vec<PayloadSignature>,
) -> VerificationResult {
    let signatures_verified = signatures.len() as u32;
    let signatures_required = 5; // Example quorum
    
    let status = if signatures_verified >= signatures_required {
        VerificationStatus::Verified
    } else {
        VerificationStatus::Pending
    };
    
    VerificationResult {
        status,
        signatures_verified,
        signatures_required,
        error_message: soroban_sdk::String::from_small_str(""),
        verified_at_height: 12345,
        has_rejections: false,
        rejection_count: 0,
    }
}

fn collect_signatures(
    env: &Env,
    payload_id: BytesN<32>,
) -> SignatureCollection {
    SignatureCollection {
        payload_id,
        signatures: Vec::new(env),
        signature_count: 0,
        signature_threshold: 5,
        all_valid: true,
    }
}
```

## 4. Handling Batch Processing

```rust
use cross_chain_payload::PayloadBatch;
use soroban_sdk::BytesN;

fn create_payload_batch(
    env: &Env,
    source_chain_id: u64,
    payload_count: u32,
) -> PayloadBatch {
    PayloadBatch {
        batch_id: BytesN::from_array(&[4u8; 32]),
        source_chain_id,
        payload_count,
        merkle_root: BytesN::from_array(&[5u8; 32]),
        batch_timestamp: 1000000,
        batch_ttl_seconds: 3600, // 1 hour
    }
}
```

## 5. Error Handling

```rust
use cross_chain_payload::CrossChainError;

fn handle_verification_error(error: CrossChainError) -> String {
    let error_code = error.as_u32();
    let error_msg = match error {
        CrossChainError::InvalidPayloadHash => "Payload hash mismatch",
        CrossChainError::InvalidSignature => "Signature verification failed",
        CrossChainError::PayloadExpired => "Payload TTL exceeded",
        CrossChainError::ReplayAttack => "Nonce already used",
        CrossChainError::BridgeInactive => "Bridge is not active",
        CrossChainError::InsufficientSignatures => "Not enough validators signed",
        _ => "Unknown error",
    };
    
    format!("[Error {}]: {}", error_code, error_msg)
}
```

## 6. Multi-Chain Routing

```rust
use cross_chain_payload::PayloadRoute;
use soroban_sdk::Vec;

fn create_multi_hop_route(
    env: &Env,
    from_chain: u64,
    to_chain: u64,
) -> PayloadRoute {
    let mut path = Vec::new(env);
    // Add intermediate hops if needed
    // path.push_back(3); // For example, route through chain 3
    
    PayloadRoute {
        from_chain,
        to_chain,
        route_path: path,
        priority: 100,
        is_critical: false,
    }
}
```

## 7. Consensus Tracking

```rust
use cross_chain_payload::{ConsensusState, VerificationStatus};
use soroban_sdk::BytesN;

fn create_consensus_tracker(payload_id: BytesN<32>) -> ConsensusState {
    ConsensusState {
        payload_id,
        votes_received: 0,
        votes_for: 0,
        votes_against: 0,
        votes_abstain: 0,
        consensus_reached: false,
        final_result: VerificationStatus::Pending,
    }
}

fn check_consensus_reached(state: &ConsensusState, required_threshold: u32) -> bool {
    state.votes_for >= required_threshold
}
```

## 8. Validator Management

```rust
use cross_chain_payload::{ValidatorSet, RecoveryKey};
use soroban_sdk::{BytesN, String as SorobanString};

fn create_validator_set(
    chain_id: u64,
    quorum: u32,
) -> ValidatorSet {
    ValidatorSet {
        chain_info: /* ChainInfo struct */,
        quorum_threshold: quorum,
        total_validators: 10,
        validator_list_hash: BytesN::from_array(&[6u8; 32]),
        version: 1,
    }
}

fn register_validator_key(
    chain_id: u64,
    compressed_key: BytesN<33>,
) -> RecoveryKey {
    RecoveryKey {
        compressed_key,
        key_type: cross_chain_payload::SignatureScheme::Ed25519,
        chain_id,
        is_active: true,
        activation_height: 0,
        deactivation_height: u64::MAX,
    }
}
```

## 9. Payload Encoding

```rust
use cross_chain_payload::EncodedPayload;
use soroban_sdk::{Bytes, String as SorobanString};

fn encode_payload_for_transmission(
    env: &Env,
    payload_data: Bytes,
    original_size: u32,
    compressed_size: u32,
) -> EncodedPayload {
    EncodedPayload {
        encoded_data: payload_data,
        encoding_scheme: SorobanString::from_small_str("borsh"),
        compression_type: SorobanString::from_small_str("gzip"),
        original_size,
        compressed_size,
    }
}
```

## 10. Complete Verification Flow

```rust
use cross_chain_payload::*;
use soroban_sdk::{contract, contractimpl, Env, BytesN, String as SorobanString};

#[contract]
pub struct CrossChainVerifier;

#[contractimpl]
impl CrossChainVerifier {
    pub fn verify_cross_chain_payload(
        env: Env,
        payload: CrossChainPayload,
        signatures: Vec<PayloadSignature>,
        validator_set: ValidatorSet,
    ) -> Result<VerificationResult, CrossChainError> {
        // 1. Validate payload format
        if payload.metadata.expiration_height < env.ledger().sequence() {
            return Err(CrossChainError::PayloadExpired);
        }
        
        // 2. Verify signatures
        let verified_count = signatures.len() as u32;
        if verified_count < validator_set.quorum_threshold {
            return Err(CrossChainError::InsufficientSignatures);
        }
        
        // 3. Verify payload hash
        // In real implementation: compute hash and compare
        
        // 4. Create verification result
        Ok(VerificationResult {
            status: VerificationStatus::Verified,
            signatures_verified: verified_count,
            signatures_required: validator_set.quorum_threshold,
            error_message: SorobanString::from_small_str(""),
            verified_at_height: env.ledger().sequence(),
            has_rejections: false,
            rejection_count: 0,
        })
    }
}
```

## Tips & Best Practices

### ✅ Do
- Always check payload expiration before verification
- Verify nonce uniqueness for replay protection
- Validate chain IDs match expected values
- Store verification records for audit trails
- Use appropriate signature thresholds for your security model
- Monitor backlog for scalability issues

### ❌ Don't
- Skip payload hash verification
- Accept payloads from unknown chains
- Mix signature schemes without validation
- Ignore signature requirement policies
- Process expired payloads
- Exceed gas limits

### 🔒 Security Checklist
- [ ] Payload hash verified
- [ ] Signatures count meets threshold
- [ ] Nonce not previously used
- [ ] Timestamp within acceptable range
- [ ] Source chain is trusted
- [ ] Destination chain is accessible
- [ ] Sender is authorized
- [ ] Recipient is valid
- [ ] Bridge is active
- [ ] Validators are current

## Common Patterns

### Pattern 1: Simple Signature Verification
```rust
if payload.metadata.expiration_height < env.ledger().sequence() {
    return Err(CrossChainError::PayloadExpired);
}
```

### Pattern 2: Multi-Validator Consensus
```rust
let consensus_reached = votes_for >= quorum_threshold;
if consensus_reached {
    final_result = VerificationStatus::Verified;
}
```

### Pattern 3: Replay Protection
```rust
if nonce_store.contains(&payload.metadata.nonce) {
    return Err(CrossChainError::ReplayAttack);
}
nonce_store.insert(payload.metadata.nonce);
```

### Pattern 4: Batch Processing
```rust
let batch = create_payload_batch(env, source_chain_id, payloads.len() as u32);
let merkle_root = compute_merkle_root(&payloads);
```

---

**For more detailed information, see [CROSS_CHAIN_README.md](./CROSS_CHAIN_README.md)**
