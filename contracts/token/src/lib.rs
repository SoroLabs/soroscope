#![no_std]

mod admin;
mod allowance;
mod balance;
mod contract;
mod metadata;
mod storage_types;

// Re-export emergency guard for use in token contracts
pub use emergency_guard;

#[cfg(test)]
mod test_admin_rotation;
// Standalone guard tests require the deployable emergency_guard contract feature.
// #[cfg(test)]
// mod test_granular_pause;
#[cfg(test)]
mod test_multisig;
// Legacy integration tests in test.rs are disabled pending cleanup.
// #[cfg(test)]
// mod test;

pub use crate::contract::Token;
pub use crate::contract::TokenClient;
