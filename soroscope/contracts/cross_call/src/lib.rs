#![no_std]

mod contract_a;
mod contract_b;

#[cfg(test)]
mod test;

pub use crate::contract_a::{ContractA, ContractAClient};
pub use crate::contract_b::{ContractB, ContractBClient};
