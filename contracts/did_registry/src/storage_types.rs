use soroban_sdk::{contracttype, symbol_short, Address, Bytes, String, Symbol, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationMethod {
    pub id: String,
    pub type_: String, // e.g., "Ed25519VerificationKey2020"
    pub controller: Address,
    pub public_key_multibase: Bytes, // or whatever format
    pub deactivated: bool,
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
pub struct ServiceEndpoint {
    pub id: Symbol,
    pub service_type: Symbol,
    pub uri: String,
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
    pub controllers: Vec<Address>,
    pub service_endpoints: Vec<ServiceEndpoint>,
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

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DIDMetadata {
    pub expiration_timestamp: Option<u64>,
    pub revocation_bitmap: u64,
    pub deactivated: bool,
    pub deactivated_at: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DIDUpdated {
    pub did: String,
    pub action: String,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DIDCreated {
    pub did: String,
    pub controllers: Vec<Address>,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationMethodAdded {
    pub did: String,
    pub method_id: String,
    pub controller: Address,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceEndpointUpdated {
    pub did: String,
    pub service_id: Symbol,
    pub service_type: Symbol,
    pub uri: String,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DIDDeactivated {
    pub did: String,
    pub controller: Address,
    pub timestamp: u64,
}

// Storage keys
pub const DID_DOCUMENT: Symbol = symbol_short!("DID_DOC");
pub const DID_METADATA: Symbol = symbol_short!("DID_META");
pub const DID_INDEX: Symbol = symbol_short!("DID_IDX");
pub const CLAIMS: Symbol = symbol_short!("CLAIMS");
pub const ATTESTATIONS: Symbol = symbol_short!("ATTEST");
pub const OWNER: Symbol = symbol_short!("OWNER");
pub const DID_CID: Symbol = symbol_short!("DID_CID");
