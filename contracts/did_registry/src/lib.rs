#![no_std]

#[cfg(test)]
extern crate std;

use soroban_sdk::contracterror;

mod contract;
pub mod storage_types;

#[cfg(test)]
mod test;

pub use crate::contract::{DIDRegistry, DIDRegistryClient, DIDRegistryTrait};
pub use crate::storage_types::{
    DIDCreated, DIDDeactivated, DIDDocument, ServiceEndpoint, ServiceEndpointUpdated,
    VerificationMethod, VerificationMethodAdded,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    DIDDeactivated = 1,
    Unauthorized = 2,
    DIDNotFound = 3,
    AlreadyInitialized = 4,
    ControllerNotFound = 5,
    VerificationMethodNotFound = 6,
    ServiceEndpointNotFound = 7,
    ServiceEndpointExists = 8,
    ControllerExists = 9,
}
