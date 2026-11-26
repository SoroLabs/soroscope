#![no_std]

mod contract_a;
mod contract_b;

#[cfg(test)]
mod test;

use soroban_sdk::contracterror;

/// Maximum number of nested cross-contract hops accepted by the benchmark
/// entry points. Anything above this is rejected before a call is made.
pub const MAX_DEPTH: u32 = 10;

/// Errors returned by the cross-contract benchmark entry points.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// The contract was re-entered while its lock was held.
    Reentrancy = 1,
    /// The requested hop depth exceeded [`MAX_DEPTH`].
    DepthExceeded = 2,
    /// A benchmark was pointed back at its own contract address.
    SelfInvocation = 3,
}

pub use crate::contract_a::{ContractA, ContractAClient};
pub use crate::contract_b::{ContractB, ContractBClient};
