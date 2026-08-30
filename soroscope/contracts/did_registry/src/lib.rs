#![no_std]

mod contract;
mod storage_types;

#[cfg(test)]
mod test;

pub use crate::contract::DIDRegistry;
pub use crate::contract::DIDRegistryClient;