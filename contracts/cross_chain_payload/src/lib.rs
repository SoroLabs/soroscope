#![no_std]

pub mod chain_info;
pub mod errors;
pub mod payload;
pub mod signatures;
pub mod verification;

#[cfg(test)]
mod test;

pub use chain_info::{BridgeEndpoint, ChainInfo};
pub use errors::CrossChainError;
pub use payload::{CrossChainPayload, EncodedPayload, PayloadBatch, PayloadMetadata, PayloadRoute};
pub use signatures::{PayloadSignature, RecoveryKey, SignatureScheme};
pub use chain_info::ChainInfo;
pub use errors::CrossChainError;
pub use payload::{CrossChainPayload, PayloadMetadata};
pub use signatures::{PayloadSignature, SignatureScheme};
pub use verification::{VerificationContext, VerificationResult, VerificationStatus};
