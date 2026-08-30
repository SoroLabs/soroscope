use crate::storage_types::{
    Attestation, Claim, DIDDocument, DIDMetadata, DIDUpdated, Service, VerificationMethod,
    ATTESTATIONS, CLAIMS, DID_DOCUMENT, DID_INDEX, DID_METADATA, OWNER,
};
use soroban_sdk::{contract, contractimpl, Address, Bytes, Env, String, Symbol, Vec};

pub trait DIDRegistryTrait {
    fn initialize(e: Env, owner: Address);

    fn register_did(e: Env, did: String, document: DIDDocument, expiration_timestamp: Option<u64>);

    fn revoke_did(e: Env, did: String);

    fn set_expiration(e: Env, did: String, expiration_timestamp: Option<u64>);

    fn is_did_valid(e: Env, did: String) -> bool;

    fn update_did_document(e: Env, did: String, document: DIDDocument);

    fn add_verification_method(e: Env, did: String, method: VerificationMethod);

    fn remove_verification_method(e: Env, did: String, method_id: String);

    fn rotate_verification_method(
        e: Env,
        did: String,
        method_id: String,
        new_public_key_multibase: Bytes,
    );

    fn add_service(e: Env, did: String, service: Service);

    fn remove_service(e: Env, did: String, service_id: String);

    fn add_claim(e: Env, claim: Claim);

    fn attest_claim(e: Env, attestation: Attestation);

    fn get_did_document(e: Env, did: String) -> DIDDocument;

    fn get_claims(e: Env, subject: Address) -> Vec<Claim>;

    fn get_attestations(e: Env, claim_hash: Bytes) -> Vec<Attestation>;

    fn verify_attestation(e: Env, attestation: Attestation) -> bool;
}

#[contract]
pub struct DIDRegistry;

impl DIDRegistry {
    fn owner(e: &Env) -> Address {
        e.storage().instance().get(&OWNER).unwrap()
    }

    fn require_owner_auth(e: &Env) {
        let owner = Self::owner(e);
        owner.require_auth();
    }

    fn validate_did_uri(e: &Env, did: &String) {
        if did.len() < 4 {
            panic!("invalid DID URI format");
        }
        let mut buf = [0u8; 4];
        did.copy_into_slice(&mut buf[..4]);
        if &buf != b"did:" {
            panic!("invalid DID URI format");
        }
    }

    fn emit_did_updated(e: &Env, did: &String, action: &str) {
        e.events().publish(
            (Symbol::new(e, "did_updated"), did.clone()),
            DIDUpdated {
                did: did.clone(),
                action: String::from_str(e, action),
                timestamp: e.ledger().timestamp(),
            },
        );
    }

    fn append_did_index(e: &Env, did: &String) {
        let mut dids: Vec<String> = e.storage().persistent().get(&DID_INDEX).unwrap_or(Vec::new(&e));
        let mut i = 0;
        while i < dids.len() {
            if dids.get(i).unwrap() == *did {
                return;
            }
            i += 1;
        }
        dids.push_back(did.clone());
        e.storage().persistent().set(&DID_INDEX, &dids);
    }

    fn attester_is_authorized(e: &Env, attester: &Address) -> bool {
        let owner = Self::owner(e);
        if attester == &owner {
            return true;
        }

        let dids: Vec<String> = e.storage().persistent().get(&DID_INDEX).unwrap_or(Vec::new(&e));
        let mut i = 0;
        while i < dids.len() {
            let did = dids.get(i).unwrap();
            let key = (DID_DOCUMENT, did.clone());
            let document: DIDDocument = e.storage().persistent().get(&key).unwrap();
            let mut j = 0;
            while j < document.verification_method.len() {
                if document.verification_method.get(j).unwrap().controller == *attester {
                    return true;
                }
                j += 1;
            }
            i += 1;
        }
        false
    }

    fn _is_did_valid(e: &Env, did: String) -> bool {
        let key = (DID_DOCUMENT, did.clone());
        if !e.storage().persistent().has(&key) {
            return false;
        }

        let metadata_key = (DID_METADATA, did.clone());
        let metadata: DIDMetadata = e.storage().persistent().get(&metadata_key).unwrap();

        if metadata.revocation_bitmap != 0 {
            return false;
        }

        if let Some(expiration) = metadata.expiration_timestamp {
            let current_ledger_time = e.ledger().timestamp();
            if current_ledger_time >= expiration {
                return false;
            }
        }

        true
    }
}

#[contractimpl]
impl DIDRegistryTrait for DIDRegistry {
    fn initialize(e: Env, owner: Address) {
        if e.storage().instance().has(&OWNER) {
            panic!("already initialized");
        }
        e.storage().instance().set(&OWNER, &owner);
        e.storage().persistent().set(&DID_INDEX, &Vec::new(&e));
    }

    fn register_did(e: Env, did: String, document: DIDDocument, expiration_timestamp: Option<u64>) {
        Self::require_owner_auth(&e);
        Self::validate_did_uri(&e, &did);

        let key = (DID_DOCUMENT, did.clone());
        if e.storage().persistent().has(&key) {
            panic!("DID already registered");
        }

        let metadata = DIDMetadata {
            expiration_timestamp,
            revocation_bitmap: 0,
        };
        let metadata_key = (DID_METADATA, did.clone());

        e.storage().persistent().set(&key, &document);
        e.storage().persistent().set(&metadata_key, &metadata);
        Self::append_did_index(&e, &did);
        Self::emit_did_updated(&e, &did, "register");
    }

    fn revoke_did(e: Env, did: String) {
        Self::require_owner_auth(&e);

        let key = (DID_DOCUMENT, did.clone());
        if !e.storage().persistent().has(&key) {
            panic!("DID not found");
        }

        let metadata_key = (DID_METADATA, did.clone());
        let mut metadata: DIDMetadata = e.storage().persistent().get(&metadata_key).unwrap();
        metadata.revocation_bitmap = 1;
        e.storage().persistent().set(&metadata_key, &metadata);
        Self::emit_did_updated(&e, &did, "revoke");
    }

    fn set_expiration(e: Env, did: String, expiration_timestamp: Option<u64>) {
        Self::require_owner_auth(&e);

        let key = (DID_DOCUMENT, did.clone());
        if !e.storage().persistent().has(&key) {
            panic!("DID not found");
        }

        let metadata_key = (DID_METADATA, did.clone());
        let mut metadata: DIDMetadata = e.storage().persistent().get(&metadata_key).unwrap();
        metadata.expiration_timestamp = expiration_timestamp;
        e.storage().persistent().set(&metadata_key, &metadata);
        Self::emit_did_updated(&e, &did, "set_expiration");
    }

    fn is_did_valid(e: Env, did: String) -> bool {
        Self::_is_did_valid(&e, did)
    }

    fn update_did_document(e: Env, did: String, document: DIDDocument) {
        Self::require_owner_auth(&e);

        let key = (DID_DOCUMENT, did.clone());
        if !e.storage().persistent().has(&key) {
            panic!("DID not found");
        }

        e.storage().persistent().set(&key, &document);
        Self::emit_did_updated(&e, &did, "update");
    }

    fn add_verification_method(e: Env, did: String, method: VerificationMethod) {
        Self::require_owner_auth(&e);

        let key = (DID_DOCUMENT, did.clone());
        let mut document: DIDDocument = e.storage().persistent().get(&key).unwrap();
        document.verification_method.push_back(method);
        e.storage().persistent().set(&key, &document);
        Self::emit_did_updated(&e, &did, "add_verification_method");
    }

    fn remove_verification_method(e: Env, did: String, method_id: String) {
        Self::require_owner_auth(&e);

        let key = (DID_DOCUMENT, did.clone());
        let mut document: DIDDocument = e.storage().persistent().get(&key).unwrap();
        let mut removed = false;
        let mut i = 0;
        while i < document.verification_method.len() {
            if document.verification_method.get(i).unwrap().id == method_id {
                document.verification_method.remove(i);
                removed = true;
                break;
            }
            i += 1;
        }

        if !removed {
            panic!("verification method not found");
        }

        e.storage().persistent().set(&key, &document);
        Self::emit_did_updated(&e, &did, "remove_verification_method");
    }

    fn rotate_verification_method(
        e: Env,
        did: String,
        method_id: String,
        new_public_key_multibase: Bytes,
    ) {
        Self::require_owner_auth(&e);

        let key = (DID_DOCUMENT, did.clone());
        let mut document: DIDDocument = e.storage().persistent().get(&key).unwrap();
        let mut rotated = false;
        let mut i = 0;
        while i < document.verification_method.len() {
            let mut method = document.verification_method.get(i).unwrap().clone();
            if method.id == method_id {
                method.public_key_multibase = new_public_key_multibase.clone();
                document.verification_method.remove(i);
                document.verification_method.insert(i, method);
                rotated = true;
                break;
            }
            i += 1;
        }

        if !rotated {
            panic!("verification method not found");
        }

        e.storage().persistent().set(&key, &document);
        Self::emit_did_updated(&e, &did, "rotate_verification_method");
    }

    fn add_service(e: Env, did: String, service: Service) {
        Self::require_owner_auth(&e);

        let key = (DID_DOCUMENT, did.clone());
        let mut document: DIDDocument = e.storage().persistent().get(&key).unwrap();
        let mut i = 0;
        while i < document.service.len() {
            if document.service.get(i).unwrap().id == service.id {
                panic!("service already exists");
            }
            i += 1;
        }
        document.service.push_back(service);
        e.storage().persistent().set(&key, &document);
    }

    fn remove_service(e: Env, did: String, service_id: String) {
        Self::require_owner_auth(&e);

        let key = (DID_DOCUMENT, did.clone());
        let mut document: DIDDocument = e.storage().persistent().get(&key).unwrap();
        let mut removed = false;
        let mut i = 0;
        while i < document.service.len() {
            if document.service.get(i).unwrap().id == service_id {
                document.service.remove(i);
                removed = true;
                break;
            }
            i += 1;
        }

        if !removed {
            panic!("service not found");
        }

        e.storage().persistent().set(&key, &document);
    }

    fn add_claim(e: Env, claim: Claim) {
        Self::require_owner_auth(&e);

        let key = (CLAIMS, claim.subject.clone());
        let mut claims: Vec<Claim> = e.storage().persistent().get(&key).unwrap_or(Vec::new(&e));
        claims.push_back(claim);
        e.storage().persistent().set(&key, &claims);
    }

    fn attest_claim(e: Env, attestation: Attestation) {
        Self::require_owner_auth(&e);

        let key = (ATTESTATIONS, attestation.claim_hash.clone());
        let mut attestations: Vec<Attestation> = e.storage().persistent().get(&key).unwrap_or(Vec::new(&e));
        attestations.push_back(attestation);
        e.storage().persistent().set(&key, &attestations);
    }

    fn get_did_document(e: Env, did: String) -> DIDDocument {
        if !Self::_is_did_valid(&e, did.clone()) {
            panic!("DID is invalid (expired or revoked)");
        }

        let key = (DID_DOCUMENT, did.clone());
        e.storage().persistent().get(&key).unwrap()
    }

    fn get_claims(e: Env, subject: Address) -> Vec<Claim> {
        let key = (CLAIMS, subject.clone());
        e.storage().persistent().get(&key).unwrap_or(Vec::new(&e))
    }

    fn get_attestations(e: Env, claim_hash: Bytes) -> Vec<Attestation> {
        let key = (ATTESTATIONS, claim_hash.clone());
        e.storage().persistent().get(&key).unwrap_or(Vec::new(&e))
    }

    fn verify_attestation(e: Env, attestation: Attestation) -> bool {
        if attestation.timestamp == 0 || attestation.signature.len() == 0 {
            return false;
        }

        let key = (ATTESTATIONS, attestation.claim_hash.clone());
        let attestations: Vec<Attestation> = e.storage().persistent().get(&key).unwrap_or(Vec::new(&e));
        let mut found = false;
        let mut i = 0;
        while i < attestations.len() {
            if attestations.get(i).unwrap() == attestation {
                found = true;
                break;
            }
            i += 1;
        }

        if !found {
            return false;
        }

        Self::attester_is_authorized(&e, &attestation.attester)
    }
}