use crate::chain_info::ValidatorSet;
use soroban_sdk::{contracttype, BytesN, String};

/// Status of a cross-chain payload verification
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerificationStatus {
    /// Payload not yet verified
    Pending,
    /// Payload has been verified successfully
    Verified,
    /// Payload verification failed
    Failed,
    /// Payload expired before verification could complete
    Expired,
    /// Payload verification was cancelled
    Cancelled,
}

/// Detailed verification result
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationResult {
    /// Overall verification status
    pub status: VerificationStatus,
    /// Signatures verified count
    pub signatures_verified: u32,
    /// Total signatures required
    pub signatures_required: u32,
    /// Error message if verification failed
    pub error_message: String,
    /// Block height at which verification occurred
    pub verified_at_height: u64,
    /// Whether payload was rejected by any validator
    pub has_rejections: bool,
    /// Number of validators who rejected the payload
    pub rejection_count: u32,
}

/// Context needed for payload verification
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationContext {
    /// Payload ID being verified
    pub payload_id: BytesN<32>,
    /// Current block height
    pub current_height: u64,
    /// Current block timestamp
    pub current_timestamp: u64,
    /// Validator set to use for verification
    pub validator_set: ValidatorSet,
    /// Hash of the payload to verify
    pub payload_hash: BytesN<32>,
    /// Minimum signatures required for consensus
    pub min_signatures_required: u32,
    /// Whether to enforce strict ordering
    pub enforce_ordering: bool,
    /// Whether replay protection is enabled
    pub replay_protection_enabled: bool,
}

/// Record of a single validation attempt
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationRecord {
    /// Payload ID that was validated
    pub payload_id: BytesN<32>,
    /// Validator public key that performed validation
    pub validator_public_key: BytesN<32>,
    /// Result of the validation
    pub validation_result: VerificationStatus,
    /// Block height when validation occurred
    pub validation_height: u64,
    /// Timestamp of the validation
    pub validation_timestamp: u64,
    /// Any notes or comments about the validation
    pub notes: String,
}

/// Cross-chain consensus state
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsensusState {
    /// Payload ID for this consensus
    pub payload_id: BytesN<32>,
    /// Total validators that have voted
    pub votes_received: u32,
    /// Validators who voted to accept
    pub votes_for: u32,
    /// Validators who voted to reject
    pub votes_against: u32,
    /// Validators who abstained/didn't vote
    pub votes_abstain: u32,
    /// Whether consensus has been reached
    pub consensus_reached: bool,
    /// Final consensus result (if reached)
    pub final_result: VerificationStatus,
}
