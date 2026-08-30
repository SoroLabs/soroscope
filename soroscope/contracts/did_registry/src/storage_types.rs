use soroban_sdk::{contracttype, Address, Bytes, Map, String, Symbol, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationMethod {
    pub id: String,
    pub type_: String, // e.g., "Ed25519VerificationKey2020"
    pub controller: Address,
    pub public_key_multibase: Bytes, // or whatever format
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Service {
    pub id: String,
    pub type_: String,
    pub service_endpoint: String,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DIDDocument {
    pub context: Vec<String>, // @context
    pub id: String,
    pub verification_method: Vec<VerificationMethod>,
    pub authentication: Vec<String>, // references to verification methods
    pub assertion_method: Vec<String>,
    pub key_agreement: Vec<String>,
    pub capability_invocation: Vec<String>,
    pub capability_delegation: Vec<String>,
    pub service: Vec<Service>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Claim {
    pub key: String,
    pub value: String,
    pub issuer: Address,
    pub subject: Address,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attestation {
    pub claim_hash: Bytes, // hash of the claim
    pub attester: Address,
    pub signature: Bytes,
    pub timestamp: u64,
}

// Storage keys
pub const DID_DOCUMENT: Symbol = Symbol::short("DID_DOC");
pub const DID_INDEX: Symbol = Symbol::short("DID_IDX");
pub const CLAIMS: Symbol = Symbol::short("CLAIMS");
pub const ATTESTATIONS: Symbol = Symbol::short("ATTEST");
pub const OWNER: Symbol = Symbol::short("OWNER");