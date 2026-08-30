use soroban_sdk::{contracttype, Bytes, BytesN, String, Symbol, Vec};

/// Metadata about a cross-chain payload
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PayloadMetadata {
    /// Version of the payload format
    pub version: u32,
    /// Timestamp when the payload was created (Unix seconds)
    pub timestamp: u64,
    /// Sequence number for ordering payloads from the same source
    pub sequence: u64,
    /// TTL or expiration block height
    pub expiration_height: u64,
    /// Nonce for replay attack prevention
    pub nonce: BytesN<32>,
}

/// Main cross-chain payload structure
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossChainPayload {
    /// Unique payload identifier
    pub payload_id: BytesN<32>,
    /// Source chain ID
    pub source_chain_id: u64,
    /// Destination chain ID
    pub destination_chain_id: u64,
    /// Address of the sender on the source chain
    pub sender: Bytes,
    /// Address of the receiver on the destination chain
    pub recipient: Bytes,
    /// Main payload data
    pub data: Bytes,
    /// Function or operation to execute (e.g., "transfer", "swap")
    pub operation: Symbol,
    /// Metadata about the payload
    pub metadata: PayloadMetadata,
    /// Hash of the payload for verification
    pub payload_hash: BytesN<32>,
    /// Gas limit for execution
    pub gas_limit: u64,
}

/// Represents a collection of payloads to be verified together
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PayloadBatch {
    /// Unique identifier for this batch
    pub batch_id: BytesN<32>,
    /// Chain ID where batch originated
    pub source_chain_id: u64,
    /// Number of payloads in this batch
    pub payload_count: u32,
    /// Root hash of all payloads in the batch (Merkle root)
    pub merkle_root: BytesN<32>,
    /// Timestamp when batch was created
    pub batch_timestamp: u64,
    /// TTL for the batch in seconds
    pub batch_ttl_seconds: u32,
}

/// Represents routing information for a payload
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PayloadRoute {
    /// Source chain identifier
    pub from_chain: u64,
    /// Destination chain identifier
    pub to_chain: u64,
    /// Optional intermediate chain hops
    pub route_path: Vec<u64>,
    /// Priority level for execution (0-255, higher = more priority)
    pub priority: u32,
    /// Whether this is a critical payload requiring immediate processing
    pub is_critical: bool,
}

/// Represents encoded payload data for transmission
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedPayload {
    /// The encoded payload bytes
    pub encoded_data: Bytes,
    /// Encoding scheme used (e.g., "rlp", "borsh", "protobuf")
    pub encoding_scheme: String,
    /// Compression applied (e.g., "none", "gzip", "zstd")
    pub compression_type: String,
    /// Size of the original uncompressed payload
    pub original_size: u32,
    /// Size after compression
    pub compressed_size: u32,
}
