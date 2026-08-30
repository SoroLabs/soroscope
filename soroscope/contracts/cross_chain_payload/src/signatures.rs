use soroban_sdk::{contracttype, Bytes, BytesN, String, Vec};

/// Supported signature schemes for cross-chain verification
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignatureScheme {
    /// Ed25519 elliptic curve signature
    Ed25519,
    /// Secp256k1 elliptic curve signature
    Secp256k1,
    /// BLS12-381 signature for threshold schemes
    BLS12381,
    /// ECDSA with SHA-256
    ECDSA,
    /// Multi-signature composite
    MultiSig,
}

/// Represents a single signature in the verification process
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PayloadSignature {
    /// The signature bytes
    pub signature_bytes: Bytes,
    /// Public key of the signer
    pub public_key: BytesN<32>,
    /// Signature scheme used
    pub scheme: SignatureScheme,
    /// Index of the signer in the validator set
    pub signer_index: u32,
    /// Block height at which signature was created
    pub signed_at_height: u64,
    /// Timestamp of the signature
    pub signed_timestamp: u64,
}

/// Collection of signatures for multi-validator verification
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureCollection {
    /// Payload ID these signatures are for
    pub payload_id: BytesN<32>,
    /// List of signatures
    pub signatures: Vec<PayloadSignature>,
    /// Total signatures collected
    pub signature_count: u32,
    /// Threshold needed for consensus
    pub signature_threshold: u32,
    /// Whether all collected signatures are valid
    pub all_valid: bool,
}

/// Recovery key information for signature verification
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryKey {
    /// Public key in compressed format
    pub compressed_key: BytesN<33>,
    /// Key type/scheme
    pub key_type: SignatureScheme,
    /// Chain where this key is registered
    pub chain_id: u64,
    /// Whether this key is currently active
    pub is_active: bool,
    /// Block height at which key was activated
    pub activation_height: u64,
    /// Block height at which key becomes inactive (if set)
    pub deactivation_height: u64,
}

/// Signature aggregation for threshold signatures
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AggregatedSignature {
    /// Combined/aggregated signature bytes
    pub aggregate_signature: Bytes,
    /// Bitmap indicating which validators signed (for BLS)
    pub signer_bitmap: Bytes,
    /// Number of signers combined
    pub signer_count: u32,
    /// Verification key for this aggregated signature
    pub verification_key: BytesN<32>,
    /// Scheme used for aggregation
    pub scheme: SignatureScheme,
}

/// Defines acceptable signature requirements
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureRequirement {
    /// Minimum number of signatures needed
    pub min_signatures: u32,
    /// Specific validator indices that must sign (if empty, any can sign)
    pub required_signers: Vec<u32>,
    /// Acceptable signature schemes
    pub approved_schemes: Vec<SignatureScheme>,
    /// Whether all signatures must use the same scheme
    pub scheme_homogeneity_required: bool,
    /// Timeout in blocks for collecting signatures
    pub signature_collection_timeout: u64,
}
