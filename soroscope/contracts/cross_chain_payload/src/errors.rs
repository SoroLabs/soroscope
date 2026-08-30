use soroban_sdk::contracterror;

/// Errors that can occur during cross-chain payload verification
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum CrossChainError {
    /// Payload hash does not match
    InvalidPayloadHash = 1,
    /// One or more signatures are invalid
    InvalidSignature = 2,
    /// Not enough signatures to reach consensus
    InsufficientSignatures = 3,
    /// Signature verification failed with unknown error
    SignatureVerificationFailed = 4,
    /// Payload has expired
    PayloadExpired = 5,
    /// Payload hash already verified (replay attack detected)
    ReplayAttack = 6,
    /// Validator set is invalid or missing
    InvalidValidatorSet = 7,
    /// Validator is not in the active set
    ValidatorNotInSet = 8,
    /// Sender is not authorized to execute payload
    UnauthorizedSender = 9,
    /// Recipient chain or address is invalid
    InvalidRecipient = 10,
    /// Source chain is not recognized
    UnknownSourceChain = 11,
    /// Destination chain is not accessible
    InaccessibleDestinationChain = 12,
    /// Bridge between chains is disabled or inactive
    BridgeInactive = 13,
    /// Payload data is malformed
    MalformedPayload = 14,
    /// Encoding/decoding of payload failed
    EncodingError = 15,
    /// Operation is not supported
    UnsupportedOperation = 16,
    /// Gas limit is too low for execution
    InsufficientGas = 17,
    /// Verification context is missing required data
    IncompleteVerificationContext = 18,
    /// Nonce has already been used (replay protection)
    NonceAlreadyUsed = 19,
    /// Timestamp is too far in the past or future
    InvalidTimestamp = 20,
    /// Sequence number is out of order
    SequenceOutOfOrder = 21,
    /// Cross-chain contract is in maintenance mode
    MaintenanceMode = 22,
    /// Generic verification failure
    VerificationFailed = 23,
    /// Too many payloads pending verification
    BacklogExceeded = 24,
    /// Bridge fee validation failed
    FeeValidationFailed = 25,
    /// Liquidity pool error
    LiquidityError = 26,
    /// Storage operation failed
    StorageError = 27,
    /// Unauthorized operation
    Unauthorized = 28,
    /// Generic error
    Unknown = 255,
}

impl CrossChainError {
    /// Convert error to a numeric code for external representation
    pub fn as_u32(&self) -> u32 {
        match self {
            Self::InvalidPayloadHash => 1,
            Self::InvalidSignature => 2,
            Self::InsufficientSignatures => 3,
            Self::SignatureVerificationFailed => 4,
            Self::PayloadExpired => 5,
            Self::ReplayAttack => 6,
            Self::InvalidValidatorSet => 7,
            Self::ValidatorNotInSet => 8,
            Self::UnauthorizedSender => 9,
            Self::InvalidRecipient => 10,
            Self::UnknownSourceChain => 11,
            Self::InaccessibleDestinationChain => 12,
            Self::BridgeInactive => 13,
            Self::MalformedPayload => 14,
            Self::EncodingError => 15,
            Self::UnsupportedOperation => 16,
            Self::InsufficientGas => 17,
            Self::IncompleteVerificationContext => 18,
            Self::NonceAlreadyUsed => 19,
            Self::InvalidTimestamp => 20,
            Self::SequenceOutOfOrder => 21,
            Self::MaintenanceMode => 22,
            Self::VerificationFailed => 23,
            Self::BacklogExceeded => 24,
            Self::FeeValidationFailed => 25,
            Self::LiquidityError => 26,
            Self::StorageError => 27,
            Self::Unauthorized => 28,
            Self::Unknown => 255,
        }
    }
}
