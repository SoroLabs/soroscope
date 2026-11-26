use soroban_sdk::{contracttype, Bytes, BytesN, String, Symbol, Vec};

/// Maximum canonical wire payload size accepted by the parser (10 KiB).
pub const MAX_PAYLOAD_SIZE: u32 = 10 * 1024;

/// The fields encoded by the canonical cross-chain message format.
///
/// Integers use big-endian encoding. Variable-length values are prefixed with
/// a four-byte big-endian length, as specified in `CROSS_CHAIN_VERIFIER_STANDARD.md`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedPayload {
    pub version: u8,
    pub source_chain_id: u32,
    pub destination_chain_id: u32,
    pub nonce: u64,
    pub timestamp: u64,
    pub sender: Bytes,
    pub recipient: Bytes,
    pub data: Bytes,
}

/// Parse and validate a canonical cross-chain message.
///
/// Rejects oversized or truncated input, invalid zero nonces, empty chain IDs
/// and addresses, trailing bytes, and lengths that overflow or exceed the input.
pub fn parse_payload(encoded: &Bytes) -> Result<ParsedPayload, crate::errors::CrossChainError> {
    use crate::errors::CrossChainError;

    if encoded.len() > MAX_PAYLOAD_SIZE {
        return Err(CrossChainError::MalformedPayload);
    }

    let mut cursor = 0u32;
    let version = read_u8(encoded, &mut cursor)?;
    let source_chain_id = read_u32(encoded, &mut cursor)?;
    let destination_chain_id = read_u32(encoded, &mut cursor)?;
    let nonce = read_u64(encoded, &mut cursor)?;
    let timestamp = read_u64(encoded, &mut cursor)?;
    if version != 1 {
        return Err(CrossChainError::MalformedPayload);
    }
    if nonce == 0 {
        return Err(CrossChainError::InvalidNonce);
    }
    if source_chain_id == 0 || destination_chain_id == 0 {
        return Err(CrossChainError::MalformedPayload);
    }

    let sender = read_bytes(encoded, &mut cursor)?;
    let recipient = read_bytes(encoded, &mut cursor)?;
    let data = read_bytes(encoded, &mut cursor)?;
    if sender.is_empty() || recipient.is_empty() || cursor != encoded.len() {
        return Err(CrossChainError::MalformedPayload);
    }

    Ok(ParsedPayload {
        version,
        source_chain_id,
        destination_chain_id,
        nonce,
        timestamp,
        sender,
        recipient,
        data,
    })
}

fn read_u8(bytes: &Bytes, cursor: &mut u32) -> Result<u8, crate::errors::CrossChainError> {
    let value = bytes
        .get(*cursor)
        .ok_or(crate::errors::CrossChainError::MalformedPayload)?;
    *cursor += 1;
    Ok(value)
}

fn read_u32(bytes: &Bytes, cursor: &mut u32) -> Result<u32, crate::errors::CrossChainError> {
    let mut value = 0u32;
    for _ in 0..4 {
        value = (value << 8) | read_u8(bytes, cursor)? as u32;
    }
    Ok(value)
}

fn read_u64(bytes: &Bytes, cursor: &mut u32) -> Result<u64, crate::errors::CrossChainError> {
    let mut value = 0u64;
    for _ in 0..8 {
        value = (value << 8) | read_u8(bytes, cursor)? as u64;
    }
    Ok(value)
}

fn read_bytes(bytes: &Bytes, cursor: &mut u32) -> Result<Bytes, crate::errors::CrossChainError> {
    let length = read_u32(bytes, cursor)?;
    let end = (*cursor)
        .checked_add(length)
        .filter(|end| *end <= bytes.len())
        .ok_or(crate::errors::CrossChainError::MalformedPayload)?;
    let value = bytes.slice(*cursor..end);
    *cursor = end;
    Ok(value)
}

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
