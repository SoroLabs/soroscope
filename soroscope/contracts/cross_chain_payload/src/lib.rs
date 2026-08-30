#![no_std]

pub mod chain_info;
pub mod payload;
pub mod verification;
pub mod signatures;
pub mod errors;

#[cfg(test)]
mod test;

pub use chain_info::ChainInfo;
pub use payload::{CrossChainPayload, PayloadMetadata};
pub use verification::{VerificationStatus, VerificationContext, VerificationResult};
pub use signatures::{PayloadSignature, SignatureScheme};
pub use errors::CrossChainError;
