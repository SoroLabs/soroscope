#![no_std]

#[cfg(test)]
extern crate std;

mod contract;
mod storage_types;

#[cfg(test)]
mod test;

pub use crate::contract::{DIDRegistry, DIDRegistryClient, DIDRegistryTrait};
