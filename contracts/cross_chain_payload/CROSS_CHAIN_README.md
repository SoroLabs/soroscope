# Cross-Chain Payload Verification Data Structures

## Overview

This module provides comprehensive, production-ready data structures for cross-chain payload verification in Soroban smart contracts. It enables secure validation and management of payloads being transferred between different blockchain networks.

## Module Structure

### 1. **chain_info.rs** - Chain and Bridge Management
Defines structures for identifying and managing blockchain networks in a cross-chain ecosystem.

#### Key Data Structures:
- **ChainInfo**: Core blockchain identification
  - `chain_id`: Unique identifier for the blockchain
  - `chain_name`: Human-readable chain name
  - `network_version`: Fork/version tracking
  - `bridge_contract`: Registry contract identifier
  - `consensus_round`: Current consensus epoch
  - `is_active`: Network availability status

- **BridgeEndpoint**: Inter-chain connection configuration
  - Source and destination chain information
  - Fee structure (percentage-based)
  - Minimum liquidity requirements
  - Bridge operational status

- **ValidatorSet**: Consensus validator tracking
  - Chain association
  - Quorum threshold configuration
  - Validator list hash for verification
  - Version control for validator changes

### 2. **payload.rs** - Cross-Chain Data Transfer
Structures representing payloads and payload collections being transmitted across chains.

#### Key Data Structures:
- **PayloadMetadata**: Payload versioning and lifecycle
  - Version number for format compatibility
  - Timestamps for ordering and expiration
  - Sequence numbers for replay prevention
  - TTL (time-to-live) configuration
  - Cryptographic nonce

- **CrossChainPayload**: Main payload structure
  - Unique payload identifier
  - Source and destination chain IDs
  - Sender and recipient addresses
  - Operation type (transfer, swap, etc.)
  - Gas limit for execution
  - Payload hash for integrity verification

- **PayloadBatch**: Batch processing support
  - Batch grouping for efficiency
  - Merkle root for collective verification
  - TTL at batch level
  - Payload count tracking

- **PayloadRoute**: Path and priority management
  - Multi-hop route support
  - Priority levels for queuing
  - Critical payload flagging

- **EncodedPayload**: Transmission format handling
  - Encoding scheme support (RLP, Borsh, Protobuf)
  - Compression optimization (gzip, zstd)
  - Size tracking for fees and limits

### 3. **verification.rs** - Verification State Management
Structures for tracking payload verification status and consensus.

#### Key Data Structures:
- **VerificationStatus**: Enum for verification state
  - `Pending`: Awaiting verification
  - `Verified`: Successfully verified
  - `Failed`: Verification failed
  - `Expired`: Payload TTL exceeded
  - `Cancelled`: Explicitly cancelled

- **VerificationResult**: Detailed verification report
  - Overall status
  - Signature count tracking
  - Error messages and diagnostics
  - Block height and rejection information

- **VerificationContext**: Verification parameters
  - Current block height and timestamp
  - Validator set specification
  - Signature requirements
  - Replay protection flags
  - Ordering enforcement options

- **ValidationRecord**: Individual validator action log
  - Per-validator validation result
  - Timestamp and block height
  - Notes for audit trail

- **ConsensusState**: Multi-validator consensus tracking
  - Vote counting (for, against, abstain)
  - Consensus finality determination
  - Final result recording

### 4. **signatures.rs** - Cryptographic Signature Management
Structures for signature handling and verification.

#### Key Data Structures:
- **SignatureScheme**: Enum of supported algorithms
  - Ed25519
  - Secp256k1
  - BLS12-381 (threshold signatures)
  - ECDSA
  - Multi-signature composite

- **PayloadSignature**: Individual signature structure
  - Signature bytes
  - Signer's public key
  - Scheme used
  - Signer index in validator set
  - Temporal information (height, timestamp)

- **SignatureCollection**: Multiple signature aggregation
  - Payload being signed
  - Signature list
  - Threshold tracking
  - Validity status

- **RecoveryKey**: Key registration and lifecycle
  - Compressed public key format
  - Activation and deactivation heights
  - Chain association
  - Active status flag

- **AggregatedSignature**: Threshold signature support
  - Combined signature bytes
  - Signer bitmap for participation tracking
  - Verification key
  - Aggregation scheme

- **SignatureRequirement**: Signature policy definition
  - Minimum signature count
  - Specific required signers
  - Approved schemes
  - Scheme homogeneity requirement
  - Timeout configuration

### 5. **errors.rs** - Comprehensive Error Handling
Enumeration and error code definitions for all failure scenarios.

#### Key Error Types:
- **Validation Errors**: 
  - `InvalidPayloadHash`
  - `MalformedPayload`
  - `EncodingError`

- **Signature Errors**:
  - `InvalidSignature`
  - `InsufficientSignatures`
  - `SignatureVerificationFailed`

- **Security Errors**:
  - `ReplayAttack`
  - `NonceAlreadyUsed`
  - `UnauthorizedSender`

- **Chain/Bridge Errors**:
  - `UnknownSourceChain`
  - `InaccessibleDestinationChain`
  - `BridgeInactive`

- **Operational Errors**:
  - `PayloadExpired`
  - `InsufficientGas`
  - `MaintenanceMode`
  - `BacklogExceeded`

Each error has:
- Unique error code (1-255)
- Clear semantic meaning
- Traceable through `as_u32()` method

## Design Principles

### 1. **Type Safety**
All structures use Soroban's `#[contracttype]` attribute for:
- Serialization consistency
- Host environment compatibility
- Type checking at compile time

### 2. **Security**
- Cryptographic hash fields for integrity
- Nonce support for replay protection
- Multi-validator consensus mechanism
- Signature scheme flexibility
- Audit trail through ValidationRecords

### 3. **Extensibility**
- Version fields for format compatibility
- Enum-based signature schemes for new algorithms
- Metadata structure for additional data
- Vector support for dynamic collections

### 4. **Scalability**
- Batch processing support
- Merkle root aggregation
- Signature threshold configuration
- Priority-based queue support
- TTL mechanisms for cleanup

### 5. **Interoperability**
- Multiple encoding schemes
- Compression support
- Chain-agnostic design
- Standard cryptographic algorithms
- Bridge endpoint flexibility

## Usage Patterns

### Creating a Cross-Chain Payload
```rust
let payload = CrossChainPayload {
    payload_id: generate_id(),
    source_chain_id: 1,
    destination_chain_id: 2,
    sender: sender_address,
    recipient: recipient_address,
    data: encoded_data,
    operation: Symbol::new(&env, "transfer"),
    metadata: PayloadMetadata {
        version: 1,
        timestamp: current_timestamp,
        sequence: sequence_num,
        expiration_height: current_height + 10000,
        nonce: generate_nonce(),
    },
    payload_hash: compute_hash(&payload_data),
    gas_limit: 1000000,
};
```

### Verifying Payload Signatures
```rust
let verification = VerificationResult {
    status: VerificationStatus::Verified,
    signatures_verified: 5,
    signatures_required: 5,
    error_message: String::from_small_str(""),
    verified_at_height: current_height,
    has_rejections: false,
    rejection_count: 0,
};
```

### Managing Cross-Chain State
```rust
let batch = PayloadBatch {
    batch_id: generate_batch_id(),
    source_chain_id: source,
    payload_count: payloads.len() as u32,
    merkle_root: compute_merkle_root(&payloads),
    batch_timestamp: current_timestamp,
    batch_ttl_seconds: 3600,
};
```

## Integration Points

These data structures integrate with:
- **Soroban SDK**: For contract types and cryptographic operations
- **Host Environment**: For timestamp, block height, and crypto functions
- **External Validators**: For signature collection and verification
- **Bridge Infrastructure**: For inter-chain communication
- **Storage Layer**: For payload persistence and state management

## Security Considerations

1. **Payload Integrity**: Always verify `payload_hash` matches computed hash
2. **Replay Protection**: Check `nonce` and `sequence` against stored values
3. **Signature Validation**: Verify minimum `signatures_required` met
4. **Chain Validation**: Confirm source and destination chains are valid
5. **Temporal Checks**: Ensure payload hasn't expired
6. **Authorization**: Validate sender is authorized for operation

## Testing

The `test.rs` module includes:
- Structure initialization tests
- Error code mapping tests
- Status variant verification
- Data consistency tests
- Integration scenario tests

Run tests with:
```bash
cargo test -p cross-chain-payload
```

## Future Enhancements

Potential additions:
- Zero-knowledge proof support
- Sharded validator sets
- Dynamic fee markets
- Multi-signature threshold optimization
- Cross-chain atomic swaps
- Advanced meta-transaction support
