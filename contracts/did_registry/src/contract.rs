use crate::storage_types::{
    Attestation, Claim, DIDCreated, DIDDeactivated, DIDDocument, DIDMetadata, DIDUpdated, Service,
    ServiceEndpoint, ServiceEndpointUpdated, VerificationMethod, VerificationMethodAdded,
    ATTESTATIONS, CLAIMS, DID_CID, DID_DOCUMENT, DID_INDEX, DID_METADATA, OWNER,
};
use crate::Error;
use soroban_sdk::{contract, contractimpl, Address, Bytes, Env, String, Symbol, Vec};

pub trait DIDRegistryTrait {
    fn initialize(e: Env, owner: Address);

    fn register_did(e: Env, did: String, document: DIDDocument, expiration_timestamp: Option<u64>);

    fn revoke_did(e: Env, did: String) -> Result<(), Error>;

    fn set_expiration(e: Env, did: String, expiration_timestamp: Option<u64>) -> Result<(), Error>;

    fn is_did_valid(e: Env, did: String) -> bool;

    fn update_did_document(e: Env, did: String, document: DIDDocument) -> Result<(), Error>;

    fn update_document(e: Env, did_id: String, new_cid: String) -> Result<(), Error>;

    fn get_document_cid(e: Env, did_id: String) -> String;

    fn add_verification_method(
        e: Env,
        did: String,
        method: VerificationMethod,
    ) -> Result<(), Error>;

    fn remove_verification_method(e: Env, did: String, method_id: String) -> Result<(), Error>;

    fn rotate_verification_method(
        e: Env,
        did: String,
        method_id: String,
        new_public_key_multibase: Bytes,
    ) -> Result<(), Error>;

    fn add_service(e: Env, did: String, service: Service) -> Result<(), Error>;

    fn remove_service(e: Env, did: String, service_id: String) -> Result<(), Error>;

    fn deactivate_did(e: Env, controller: Address, did: String) -> Result<(), Error>;

    fn add_controller(
        e: Env,
        controller: Address,
        did: String,
        new_controller: Address,
    ) -> Result<(), Error>;

    fn remove_controller(
        e: Env,
        controller: Address,
        did: String,
        removed_controller: Address,
    ) -> Result<(), Error>;

    fn add_service_endpoint(
        e: Env,
        controller: Address,
        did: String,
        id: Symbol,
        service_type: Symbol,
        uri: String,
    ) -> Result<(), Error>;

    fn update_service_endpoint(
        e: Env,
        controller: Address,
        did: String,
        id: Symbol,
        service_type: Symbol,
        uri: String,
    ) -> Result<(), Error>;

    fn remove_service_endpoint(
        e: Env,
        controller: Address,
        did: String,
        id: Symbol,
    ) -> Result<(), Error>;

    fn get_service_endpoint(e: Env, did: String, id: Symbol) -> Option<ServiceEndpoint>;

    fn deactivate_verification_method(
        e: Env,
        controller: Address,
        did: String,
        method_id: String,
    ) -> Result<(), Error>;

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
    pub fn owner(e: &Env) -> Address {
        e.storage().instance().get(&OWNER).unwrap()
    }

    pub fn require_owner_auth(e: &Env) {
        let owner = Self::owner(e);
        owner.require_auth();
    }

    pub fn validate_did_uri(_e: &Env, did: &String) {
        let len = did.len() as usize;
        if !(4..=256).contains(&len) {
            panic!("invalid DID URI format");
        }
        let mut buf = [0u8; 256];
        did.copy_into_slice(&mut buf[..len]);
        if &buf[..4] != b"did:" {
            panic!("invalid DID URI format");
        }
    }

    pub fn emit_did_updated(e: &Env, did: &String, action: &str) {
        e.events().publish(
            (Symbol::new(e, "did_updated"), did.clone()),
            DIDUpdated {
                did: did.clone(),
                action: String::from_str(e, action),
                timestamp: e.ledger().timestamp(),
            },
        );
    }

    pub fn append_did_index(e: &Env, did: &String) {
        let mut dids: Vec<String> = e
            .storage()
            .persistent()
            .get(&DID_INDEX)
            .unwrap_or(Vec::new(e));
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

    pub fn attester_is_authorized(e: &Env, attester: &Address) -> bool {
        let owner = Self::owner(e);
        if attester == &owner {
            return true;
        }

        let dids: Vec<String> = e
            .storage()
            .persistent()
            .get(&DID_INDEX)
            .unwrap_or(Vec::new(e));
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

    pub fn get_document(e: &Env, did: &String) -> Result<DIDDocument, Error> {
        e.storage()
            .persistent()
            .get(&(DID_DOCUMENT, did.clone()))
            .ok_or(Error::DIDNotFound)
    }

    pub fn get_metadata(e: &Env, did: &String) -> Result<DIDMetadata, Error> {
        e.storage()
            .persistent()
            .get(&(DID_METADATA, did.clone()))
            .ok_or(Error::DIDNotFound)
    }

    pub fn ensure_not_deactivated(e: &Env, did: &String) -> Result<(), Error> {
        if let Ok(metadata) = Self::get_metadata(e, did) {
            if metadata.deactivated {
                return Err(Error::DIDDeactivated);
            }
        }
        Ok(())
    }

    pub fn require_active_document(e: &Env, did: &String) -> Result<DIDDocument, Error> {
        Self::ensure_not_deactivated(e, did)?;
        Self::get_document(e, did)
    }

    pub fn require_controller_auth(
        e: &Env,
        did: &String,
        controller: &Address,
    ) -> Result<DIDDocument, Error> {
        let document = Self::require_active_document(e, did)?;
        controller.require_auth();
        if !document.controllers.contains(controller) {
            return Err(Error::Unauthorized);
        }
        Ok(document)
    }

    pub fn _is_did_valid(e: &Env, did: String) -> bool {
        let key = (DID_DOCUMENT, did.clone());
        if !e.storage().persistent().has(&key) {
            return false;
        }

        let metadata_key = (DID_METADATA, did.clone());
        let metadata: DIDMetadata = e.storage().persistent().get(&metadata_key).unwrap();

        if metadata.revocation_bitmap != 0 || metadata.deactivated {
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
impl DIDRegistry {
    pub fn initialize(e: Env, owner: Address) {
        if e.storage().instance().has(&OWNER) {
            panic!("already initialized");
        }
        e.storage().instance().set(&OWNER, &owner);
        e.storage()
            .persistent()
            .set(&DID_INDEX, &Vec::<String>::new(&e));
    }

    pub fn register_did(
        e: Env,
        did: String,
        document: DIDDocument,
        expiration_timestamp: Option<u64>,
    ) {
        let owner = Self::owner(&e);
        owner.require_auth();
        Self::validate_did_uri(&e, &did);

        let key = (DID_DOCUMENT, did.clone());
        if e.storage().persistent().has(&key) {
            panic!("DID already registered");
        }

        let mut document = document;
        if document.controllers.is_empty() {
            document.controllers.push_back(owner.clone());
            for method in document.verification_method.iter() {
                if !document.controllers.contains(&method.controller) {
                    document.controllers.push_back(method.controller);
                }
            }
        }

        let metadata = DIDMetadata {
            expiration_timestamp,
            revocation_bitmap: 0,
            deactivated: false,
            deactivated_at: None,
        };
        let metadata_key = (DID_METADATA, did.clone());

        e.storage().persistent().set(&key, &document);
        e.storage().persistent().set(&metadata_key, &metadata);
        Self::append_did_index(&e, &did);
        e.events().publish(
            (Symbol::new(&e, "did_created"), did.clone()),
            DIDCreated {
                did: did.clone(),
                controllers: document.controllers.clone(),
                timestamp: e.ledger().timestamp(),
            },
        );
        Self::emit_did_updated(&e, &did, "register");
    }

    pub fn revoke_did(e: Env, did: String) -> Result<(), Error> {
        Self::require_owner_auth(&e);
        Self::ensure_not_deactivated(&e, &did)?;

        let key = (DID_DOCUMENT, did.clone());
        if !e.storage().persistent().has(&key) {
            panic!("DID not found");
        }

        let metadata_key = (DID_METADATA, did.clone());
        let mut metadata: DIDMetadata = e.storage().persistent().get(&metadata_key).unwrap();
        metadata.revocation_bitmap = 1;
        e.storage().persistent().set(&metadata_key, &metadata);
        Self::emit_did_updated(&e, &did, "revoke");
        Ok(())
    }

    pub fn set_expiration(
        e: Env,
        did: String,
        expiration_timestamp: Option<u64>,
    ) -> Result<(), Error> {
        Self::require_owner_auth(&e);
        Self::ensure_not_deactivated(&e, &did)?;

        let key = (DID_DOCUMENT, did.clone());
        if !e.storage().persistent().has(&key) {
            panic!("DID not found");
        }

        let metadata_key = (DID_METADATA, did.clone());
        let mut metadata: DIDMetadata = e.storage().persistent().get(&metadata_key).unwrap();
        metadata.expiration_timestamp = expiration_timestamp;
        e.storage().persistent().set(&metadata_key, &metadata);
        Self::emit_did_updated(&e, &did, "set_expiration");
        Ok(())
    }

    pub fn is_did_valid(e: Env, did: String) -> bool {
        Self::_is_did_valid(&e, did)
    }

    pub fn update_did_document(e: Env, did: String, document: DIDDocument) -> Result<(), Error> {
        Self::require_owner_auth(&e);
        Self::ensure_not_deactivated(&e, &did)?;

        let key = (DID_DOCUMENT, did.clone());
        let previous: DIDDocument = match e.storage().persistent().get(&key) {
            Some(previous) => previous,
            None => panic!("DID not found"),
        };
        let mut document = document;
        if document.controllers.is_empty() {
            document.controllers = previous.controllers;
        }

        e.storage().persistent().set(&key, &document);
        Self::emit_did_updated(&e, &did, "update");
        Ok(())
    }

    pub fn update_document(e: Env, did_id: String, new_cid: String) -> Result<(), Error> {
        Self::require_owner_auth(&e);
        Self::validate_did_uri(&e, &did_id);
        Self::ensure_not_deactivated(&e, &did_id)?;

        let did_key = (DID_DOCUMENT, did_id.clone());
        let cid_key = (DID_CID, did_id.clone());

        if !e.storage().persistent().has(&did_key) && !e.storage().persistent().has(&cid_key) {
            panic!("DID not found");
        }

        let old_cid: String = e
            .storage()
            .persistent()
            .get(&cid_key)
            .unwrap_or_else(|| String::from_str(&e, ""));

        e.storage().persistent().set(&cid_key, &new_cid);
        Self::append_did_index(&e, &did_id);

        e.events().publish(
            (Symbol::new(&e, "did_updated"), did_id.clone()),
            (old_cid.clone(), new_cid.clone()),
        );
        Ok(())
    }

    pub fn get_document_cid(e: Env, did_id: String) -> String {
        let cid_key = (DID_CID, did_id.clone());
        if let Some(cid) = e.storage().persistent().get(&cid_key) {
            cid
        } else {
            panic!("CID not found");
        }
    }

    pub fn add_verification_method(
        e: Env,
        did: String,
        method: VerificationMethod,
    ) -> Result<(), Error> {
        let document = Self::require_controller_auth(&e, &did, &method.controller)?;

        let key = (DID_DOCUMENT, did.clone());
        let mut document = document;
        let mut method = method;
        method.deactivated = false;
        document.verification_method.push_back(method.clone());
        e.storage().persistent().set(&key, &document);
        e.events().publish(
            (Symbol::new(&e, "verification_method_added"), did.clone()),
            VerificationMethodAdded {
                did: did.clone(),
                method_id: method.id,
                controller: method.controller,
                timestamp: e.ledger().timestamp(),
            },
        );
        Self::emit_did_updated(&e, &did, "add_verification_method");
        Ok(())
    }

    pub fn remove_verification_method(e: Env, did: String, method_id: String) -> Result<(), Error> {
        Self::require_owner_auth(&e);
        Self::ensure_not_deactivated(&e, &did)?;

        let key = (DID_DOCUMENT, did.clone());
        let mut document: DIDDocument = e.storage().persistent().get(&key).unwrap();
        let mut removed = false;
        let mut i = 0;
        while i < document.verification_method.len() {
            if document.verification_method.get(i).unwrap().id == method_id {
                document.verification_method.remove(i);
                removed = true;
            } else {
                i += 1;
            }
        }

        if !removed {
            panic!("verification method not found");
        }

        e.storage().persistent().set(&key, &document);
        Self::emit_did_updated(&e, &did, "remove_verification_method");
        Ok(())
    }

    pub fn rotate_verification_method(
        e: Env,
        did: String,
        method_id: String,
        new_public_key_multibase: Bytes,
    ) -> Result<(), Error> {
        Self::require_owner_auth(&e);
        Self::ensure_not_deactivated(&e, &did)?;

        let key = (DID_DOCUMENT, did.clone());
        let mut document: DIDDocument = e.storage().persistent().get(&key).unwrap();
        let mut rotated = false;
        let mut i = 0;
        while i < document.verification_method.len() {
            let mut method = document.verification_method.get(i).unwrap();
            if method.id == method_id && !method.deactivated {
                let replacement = VerificationMethod {
                    id: method.id.clone(),
                    type_: method.type_.clone(),
                    controller: method.controller.clone(),
                    public_key_multibase: new_public_key_multibase.clone(),
                    deactivated: false,
                };
                method.deactivated = true;
                document.verification_method.set(i, method);
                document.verification_method.push_back(replacement);
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
        Ok(())
    }

    pub fn add_service(e: Env, did: String, service: Service) -> Result<(), Error> {
        Self::require_owner_auth(&e);
        Self::ensure_not_deactivated(&e, &did)?;

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
        Ok(())
    }

    pub fn remove_service(e: Env, did: String, service_id: String) -> Result<(), Error> {
        Self::require_owner_auth(&e);
        Self::ensure_not_deactivated(&e, &did)?;

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
        Ok(())
    }

    pub fn deactivate_did(e: Env, controller: Address, did: String) -> Result<(), Error> {
        Self::require_controller_auth(&e, &did, &controller)?;
        let metadata_key = (DID_METADATA, did.clone());
        let mut metadata = Self::get_metadata(&e, &did)?;
        if metadata.deactivated {
            return Err(Error::DIDDeactivated);
        }
        metadata.deactivated = true;
        metadata.deactivated_at = Some(e.ledger().timestamp());
        e.storage().persistent().set(&metadata_key, &metadata);
        e.events().publish(
            (Symbol::new(&e, "did_deactivated"), did.clone()),
            DIDDeactivated {
                did: did.clone(),
                controller,
                timestamp: e.ledger().timestamp(),
            },
        );
        Ok(())
    }

    pub fn add_controller(
        e: Env,
        controller: Address,
        did: String,
        new_controller: Address,
    ) -> Result<(), Error> {
        let mut document = Self::require_controller_auth(&e, &did, &controller)?;
        if document.controllers.contains(&new_controller) {
            return Err(Error::ControllerExists);
        }
        document.controllers.push_back(new_controller);
        e.storage()
            .persistent()
            .set(&(DID_DOCUMENT, did.clone()), &document);
        Self::emit_did_updated(&e, &did, "add_controller");
        Ok(())
    }

    pub fn remove_controller(
        e: Env,
        controller: Address,
        did: String,
        removed_controller: Address,
    ) -> Result<(), Error> {
        let mut document = Self::require_controller_auth(&e, &did, &controller)?;
        if document.controllers.len() == 1 {
            return Err(Error::Unauthorized);
        }
        let mut index = None;
        let mut i = 0;
        while i < document.controllers.len() {
            if document.controllers.get(i).unwrap() == removed_controller {
                index = Some(i);
                break;
            }
            i += 1;
        }
        match index {
            Some(index) => document.controllers.remove(index),
            None => return Err(Error::ControllerNotFound),
        };
        e.storage()
            .persistent()
            .set(&(DID_DOCUMENT, did.clone()), &document);
        Self::emit_did_updated(&e, &did, "remove_controller");
        Ok(())
    }

    pub fn add_service_endpoint(
        e: Env,
        controller: Address,
        did: String,
        id: Symbol,
        service_type: Symbol,
        uri: String,
    ) -> Result<(), Error> {
        let mut document = Self::require_controller_auth(&e, &did, &controller)?;
        if document
            .service_endpoints
            .iter()
            .any(|endpoint| endpoint.id == id)
        {
            return Err(Error::ServiceEndpointExists);
        }
        document.service_endpoints.push_back(ServiceEndpoint {
            id: id.clone(),
            service_type: service_type.clone(),
            uri: uri.clone(),
        });
        e.storage()
            .persistent()
            .set(&(DID_DOCUMENT, did.clone()), &document);
        e.events().publish(
            (Symbol::new(&e, "service_endpoint_updated"), did.clone()),
            ServiceEndpointUpdated {
                did: did.clone(),
                service_id: id,
                service_type,
                uri,
                timestamp: e.ledger().timestamp(),
            },
        );
        Ok(())
    }

    pub fn update_service_endpoint(
        e: Env,
        controller: Address,
        did: String,
        id: Symbol,
        service_type: Symbol,
        uri: String,
    ) -> Result<(), Error> {
        let mut document = Self::require_controller_auth(&e, &did, &controller)?;
        let mut found = false;
        let mut i = 0;
        while i < document.service_endpoints.len() {
            if document.service_endpoints.get(i).unwrap().id == id {
                document.service_endpoints.set(
                    i,
                    ServiceEndpoint {
                        id: id.clone(),
                        service_type: service_type.clone(),
                        uri: uri.clone(),
                    },
                );
                found = true;
                break;
            }
            i += 1;
        }
        if !found {
            return Err(Error::ServiceEndpointNotFound);
        }
        e.storage()
            .persistent()
            .set(&(DID_DOCUMENT, did.clone()), &document);
        e.events().publish(
            (Symbol::new(&e, "service_endpoint_updated"), did.clone()),
            ServiceEndpointUpdated {
                did: did.clone(),
                service_id: id,
                service_type,
                uri,
                timestamp: e.ledger().timestamp(),
            },
        );
        Ok(())
    }

    pub fn remove_service_endpoint(
        e: Env,
        controller: Address,
        did: String,
        id: Symbol,
    ) -> Result<(), Error> {
        let mut document = Self::require_controller_auth(&e, &did, &controller)?;
        let mut index = None;
        let mut endpoint = None;
        let mut i = 0;
        while i < document.service_endpoints.len() {
            let candidate = document.service_endpoints.get(i).unwrap();
            if candidate.id == id {
                index = Some(i);
                endpoint = Some(candidate);
                break;
            }
            i += 1;
        }
        match (index, endpoint) {
            (Some(index), Some(endpoint)) => {
                document.service_endpoints.remove(index);
                e.storage()
                    .persistent()
                    .set(&(DID_DOCUMENT, did.clone()), &document);
                e.events().publish(
                    (Symbol::new(&e, "service_endpoint_updated"), did.clone()),
                    ServiceEndpointUpdated {
                        did: did.clone(),
                        service_id: endpoint.id,
                        service_type: endpoint.service_type,
                        uri: endpoint.uri,
                        timestamp: e.ledger().timestamp(),
                    },
                );
            }
            _ => return Err(Error::ServiceEndpointNotFound),
        }
        Ok(())
    }

    pub fn get_service_endpoint(e: Env, did: String, id: Symbol) -> Option<ServiceEndpoint> {
        let document = Self::get_document(&e, &did).ok()?;
        document
            .service_endpoints
            .iter()
            .find(|endpoint| endpoint.id == id)
    }

    pub fn deactivate_verification_method(
        e: Env,
        controller: Address,
        did: String,
        method_id: String,
    ) -> Result<(), Error> {
        let mut document = Self::require_controller_auth(&e, &did, &controller)?;
        let mut found = false;
        let mut i = 0;
        while i < document.verification_method.len() {
            let mut method = document.verification_method.get(i).unwrap();
            if method.id == method_id && !method.deactivated {
                method.deactivated = true;
                document.verification_method.set(i, method);
                found = true;
                break;
            }
            i += 1;
        }
        if !found {
            return Err(Error::VerificationMethodNotFound);
        }
        e.storage()
            .persistent()
            .set(&(DID_DOCUMENT, did.clone()), &document);
        Self::emit_did_updated(&e, &did, "deactivate_verification_method");
        Ok(())
    }

    pub fn add_claim(e: Env, claim: Claim) {
        Self::require_owner_auth(&e);

        let key = (CLAIMS, claim.subject.clone());
        let mut claims: Vec<Claim> = e.storage().persistent().get(&key).unwrap_or(Vec::new(&e));
        claims.push_back(claim);
        e.storage().persistent().set(&key, &claims);
    }

    pub fn attest_claim(e: Env, attestation: Attestation) {
        Self::require_owner_auth(&e);

        let key = (ATTESTATIONS, attestation.claim_hash.clone());
        let mut attestations: Vec<Attestation> =
            e.storage().persistent().get(&key).unwrap_or(Vec::new(&e));
        attestations.push_back(attestation);
        e.storage().persistent().set(&key, &attestations);
    }

    pub fn get_did_document(e: Env, did: String) -> DIDDocument {
        let deactivated = Self::get_metadata(&e, &did)
            .map(|metadata| metadata.deactivated)
            .unwrap_or(false);
        if !deactivated && !Self::_is_did_valid(&e, did.clone()) {
            panic!("DID is invalid (expired or revoked)");
        }

        let key = (DID_DOCUMENT, did.clone());
        e.storage().persistent().get(&key).unwrap()
    }

    pub fn get_claims(e: Env, subject: Address) -> Vec<Claim> {
        let key = (CLAIMS, subject.clone());
        e.storage().persistent().get(&key).unwrap_or(Vec::new(&e))
    }

    pub fn get_attestations(e: Env, claim_hash: Bytes) -> Vec<Attestation> {
        let key = (ATTESTATIONS, claim_hash.clone());
        e.storage().persistent().get(&key).unwrap_or(Vec::new(&e))
    }

    pub fn verify_attestation(e: Env, attestation: Attestation) -> bool {
        if attestation.timestamp == 0 || attestation.signature.is_empty() {
            return false;
        }

        let key = (ATTESTATIONS, attestation.claim_hash.clone());
        let attestations: Vec<Attestation> =
            e.storage().persistent().get(&key).unwrap_or(Vec::new(&e));
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

impl DIDRegistryTrait for DIDRegistry {
    fn initialize(e: Env, owner: Address) {
        Self::initialize(e, owner)
    }

    fn register_did(e: Env, did: String, document: DIDDocument, expiration_timestamp: Option<u64>) {
        Self::register_did(e, did, document, expiration_timestamp)
    }

    fn revoke_did(e: Env, did: String) -> Result<(), Error> {
        Self::revoke_did(e, did)
    }

    fn set_expiration(e: Env, did: String, expiration_timestamp: Option<u64>) -> Result<(), Error> {
        Self::set_expiration(e, did, expiration_timestamp)
    }

    fn is_did_valid(e: Env, did: String) -> bool {
        Self::is_did_valid(e, did)
    }

    fn update_did_document(e: Env, did: String, document: DIDDocument) -> Result<(), Error> {
        Self::update_did_document(e, did, document)
    }

    fn update_document(e: Env, did_id: String, new_cid: String) -> Result<(), Error> {
        Self::update_document(e, did_id, new_cid)
    }

    fn get_document_cid(e: Env, did_id: String) -> String {
        Self::get_document_cid(e, did_id)
    }

    fn add_verification_method(
        e: Env,
        did: String,
        method: VerificationMethod,
    ) -> Result<(), Error> {
        Self::add_verification_method(e, did, method)
    }

    fn remove_verification_method(e: Env, did: String, method_id: String) -> Result<(), Error> {
        Self::remove_verification_method(e, did, method_id)
    }

    fn rotate_verification_method(
        e: Env,
        did: String,
        method_id: String,
        new_public_key_multibase: Bytes,
    ) -> Result<(), Error> {
        Self::rotate_verification_method(e, did, method_id, new_public_key_multibase)
    }

    fn add_service(e: Env, did: String, service: Service) -> Result<(), Error> {
        Self::add_service(e, did, service)
    }

    fn remove_service(e: Env, did: String, service_id: String) -> Result<(), Error> {
        Self::remove_service(e, did, service_id)
    }

    fn deactivate_did(e: Env, controller: Address, did: String) -> Result<(), Error> {
        Self::deactivate_did(e, controller, did)
    }

    fn add_controller(
        e: Env,
        controller: Address,
        did: String,
        new_controller: Address,
    ) -> Result<(), Error> {
        Self::add_controller(e, controller, did, new_controller)
    }

    fn remove_controller(
        e: Env,
        controller: Address,
        did: String,
        removed_controller: Address,
    ) -> Result<(), Error> {
        Self::remove_controller(e, controller, did, removed_controller)
    }

    fn add_service_endpoint(
        e: Env,
        controller: Address,
        did: String,
        id: Symbol,
        service_type: Symbol,
        uri: String,
    ) -> Result<(), Error> {
        Self::add_service_endpoint(e, controller, did, id, service_type, uri)
    }

    fn update_service_endpoint(
        e: Env,
        controller: Address,
        did: String,
        id: Symbol,
        service_type: Symbol,
        uri: String,
    ) -> Result<(), Error> {
        Self::update_service_endpoint(e, controller, did, id, service_type, uri)
    }

    fn remove_service_endpoint(
        e: Env,
        controller: Address,
        did: String,
        id: Symbol,
    ) -> Result<(), Error> {
        Self::remove_service_endpoint(e, controller, did, id)
    }

    fn get_service_endpoint(e: Env, did: String, id: Symbol) -> Option<ServiceEndpoint> {
        Self::get_service_endpoint(e, did, id)
    }

    fn deactivate_verification_method(
        e: Env,
        controller: Address,
        did: String,
        method_id: String,
    ) -> Result<(), Error> {
        Self::deactivate_verification_method(e, controller, did, method_id)
    }

    fn add_claim(e: Env, claim: Claim) {
        Self::add_claim(e, claim)
    }

    fn attest_claim(e: Env, attestation: Attestation) {
        Self::attest_claim(e, attestation)
    }

    fn get_did_document(e: Env, did: String) -> DIDDocument {
        Self::get_did_document(e, did)
    }

    fn get_claims(e: Env, subject: Address) -> Vec<Claim> {
        Self::get_claims(e, subject)
    }

    fn get_attestations(e: Env, claim_hash: Bytes) -> Vec<Attestation> {
        Self::get_attestations(e, claim_hash)
    }

    fn verify_attestation(e: Env, attestation: Attestation) -> bool {
        Self::verify_attestation(e, attestation)
    }
}
