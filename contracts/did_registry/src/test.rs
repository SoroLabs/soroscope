extern crate std;

use crate::contract::{DIDRegistry, DIDRegistryClient};
use crate::storage_types::{Claim, DIDDocument, VerificationMethod};
use crate::Error;
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _},
    Address, Bytes, Env, String, Symbol, TryIntoVal, Vec,
};

#[test]
fn test_register_and_update_did() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DIDRegistry, ());
    let client = DIDRegistryClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    client.initialize(&owner);

    let did = String::from_str(&env, "did:example:123");
    let mut document = DIDDocument {
        context: Vec::from_array(
            &env,
            [String::from_str(&env, "https://www.w3.org/ns/did/v1")],
        ),
        id: did.clone(),
        verification_method: Vec::new(&env),
        authentication: Vec::new(&env),
        assertion_method: Vec::new(&env),
        key_agreement: Vec::new(&env),
        capability_invocation: Vec::new(&env),
        capability_delegation: Vec::new(&env),
        service: Vec::new(&env),
        controllers: Vec::new(&env),
        service_endpoints: Vec::new(&env),
    };

    client.register_did(&did, &document, &None);

    let retrieved = client.get_did_document(&did);
    assert_eq!(retrieved.id, did);

    // Update document
    document
        .context
        .push_back(String::from_str(&env, "https://example.com/context"));
    client.update_did_document(&did, &document);

    let updated = client.get_did_document(&did);
    assert_eq!(updated.context.len(), 2);
}

#[test]
fn test_add_verification_method() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DIDRegistry, ());
    let client = DIDRegistryClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    client.initialize(&owner);

    let did = String::from_str(&env, "did:example:123");
    let document = DIDDocument {
        context: Vec::from_array(
            &env,
            [String::from_str(&env, "https://www.w3.org/ns/did/v1")],
        ),
        id: did.clone(),
        verification_method: Vec::new(&env),
        authentication: Vec::new(&env),
        assertion_method: Vec::new(&env),
        key_agreement: Vec::new(&env),
        capability_invocation: Vec::new(&env),
        capability_delegation: Vec::new(&env),
        service: Vec::new(&env),
        controllers: Vec::new(&env),
        service_endpoints: Vec::new(&env),
    };

    client.register_did(&did, &document, &None);

    let method = VerificationMethod {
        id: String::from_str(&env, "key-1"),
        type_: String::from_str(&env, "Ed25519VerificationKey2020"),
        controller: owner.clone(),
        public_key_multibase: Bytes::from_array(&env, &[1, 2, 3]),
        deactivated: false,
    };

    client.add_verification_method(&did, &method);

    let updated_doc = client.get_did_document(&did);
    assert_eq!(updated_doc.verification_method.len(), 1);
    assert_eq!(
        updated_doc.verification_method.get(0).unwrap().id,
        method.id
    );
}

#[test]
fn test_add_claim() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DIDRegistry, ());
    let client = DIDRegistryClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    client.initialize(&owner);

    let subject = Address::generate(&env);
    let claim = Claim {
        key: String::from_str(&env, "name"),
        value: String::from_str(&env, "Alice"),
        issuer: owner.clone(),
        subject: subject.clone(),
    };

    client.add_claim(&claim);

    let claims = client.get_claims(&subject);
    assert_eq!(claims.len(), 1);
    assert_eq!(claims.get(0).unwrap().value, claim.value);
}

#[test]
fn test_did_with_expiration() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DIDRegistry, ());
    let client = DIDRegistryClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    client.initialize(&owner);

    let did = String::from_str(&env, "did:example:456");
    let document = DIDDocument {
        context: Vec::from_array(
            &env,
            [String::from_str(&env, "https://www.w3.org/ns/did/v1")],
        ),
        id: did.clone(),
        verification_method: Vec::new(&env),
        authentication: Vec::new(&env),
        assertion_method: Vec::new(&env),
        key_agreement: Vec::new(&env),
        capability_invocation: Vec::new(&env),
        capability_delegation: Vec::new(&env),
        service: Vec::new(&env),
        controllers: Vec::new(&env),
        service_endpoints: Vec::new(&env),
    };

    let expiration = Some(1000u64);
    client.register_did(&did, &document, &expiration);

    let is_valid = client.is_did_valid(&did);
    assert!(is_valid);

    let retrieved = client.get_did_document(&did);
    assert_eq!(retrieved.id, did);
}

#[test]
fn test_did_expiration_passed() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DIDRegistry, ());
    let client = DIDRegistryClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    client.initialize(&owner);

    let did = String::from_str(&env, "did:example:789");
    let document = DIDDocument {
        context: Vec::from_array(
            &env,
            [String::from_str(&env, "https://www.w3.org/ns/did/v1")],
        ),
        id: did.clone(),
        verification_method: Vec::new(&env),
        authentication: Vec::new(&env),
        assertion_method: Vec::new(&env),
        key_agreement: Vec::new(&env),
        capability_invocation: Vec::new(&env),
        capability_delegation: Vec::new(&env),
        service: Vec::new(&env),
        controllers: Vec::new(&env),
        service_endpoints: Vec::new(&env),
    };

    let expiration = Some(100u64);
    client.register_did(&did, &document, &expiration);

    env.ledger().set_timestamp(200);

    let is_valid = client.is_did_valid(&did);
    assert!(!is_valid);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.get_did_document(&did);
    }));
    assert!(result.is_err());
}

#[test]
fn test_revoke_did() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DIDRegistry, ());
    let client = DIDRegistryClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    client.initialize(&owner);

    let did = String::from_str(&env, "did:example:999");
    let document = DIDDocument {
        context: Vec::from_array(
            &env,
            [String::from_str(&env, "https://www.w3.org/ns/did/v1")],
        ),
        id: did.clone(),
        verification_method: Vec::new(&env),
        authentication: Vec::new(&env),
        assertion_method: Vec::new(&env),
        key_agreement: Vec::new(&env),
        capability_invocation: Vec::new(&env),
        capability_delegation: Vec::new(&env),
        service: Vec::new(&env),
        controllers: Vec::new(&env),
        service_endpoints: Vec::new(&env),
    };

    client.register_did(&did, &document, &None);

    let is_valid = client.is_did_valid(&did);
    assert!(is_valid);

    client.revoke_did(&did);

    let is_valid_after = client.is_did_valid(&did);
    assert!(!is_valid_after);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.get_did_document(&did);
    }));
    assert!(result.is_err());
}

#[test]
fn test_set_expiration() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DIDRegistry, ());
    let client = DIDRegistryClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    client.initialize(&owner);

    let did = String::from_str(&env, "did:example:111");
    let document = DIDDocument {
        context: Vec::from_array(
            &env,
            [String::from_str(&env, "https://www.w3.org/ns/did/v1")],
        ),
        id: did.clone(),
        verification_method: Vec::new(&env),
        authentication: Vec::new(&env),
        assertion_method: Vec::new(&env),
        key_agreement: Vec::new(&env),
        capability_invocation: Vec::new(&env),
        capability_delegation: Vec::new(&env),
        service: Vec::new(&env),
        controllers: Vec::new(&env),
        service_endpoints: Vec::new(&env),
    };

    client.register_did(&did, &document, &None);

    let new_expiration = Some(5000u64);
    client.set_expiration(&did, &new_expiration);

    let is_valid = client.is_did_valid(&did);
    assert!(is_valid);

    env.ledger().set_timestamp(6000);

    let is_valid_after = client.is_did_valid(&did);
    assert!(!is_valid_after);
}

#[test]
fn test_did_without_expiration_remains_valid() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DIDRegistry, ());
    let client = DIDRegistryClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    client.initialize(&owner);

    let did = String::from_str(&env, "did:example:222");
    let document = DIDDocument {
        context: Vec::from_array(
            &env,
            [String::from_str(&env, "https://www.w3.org/ns/did/v1")],
        ),
        id: did.clone(),
        verification_method: Vec::new(&env),
        authentication: Vec::new(&env),
        assertion_method: Vec::new(&env),
        key_agreement: Vec::new(&env),
        capability_invocation: Vec::new(&env),
        capability_delegation: Vec::new(&env),
        service: Vec::new(&env),
        controllers: Vec::new(&env),
        service_endpoints: Vec::new(&env),
    };

    client.register_did(&did, &document, &None);

    env.ledger().set_timestamp(999999);

    let is_valid = client.is_did_valid(&did);
    assert!(is_valid);

    let retrieved = client.get_did_document(&did);
    assert_eq!(retrieved.id, did);
}

#[test]
fn test_rotate_and_remove_verification_method() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DIDRegistry, ());
    let client = DIDRegistryClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    client.initialize(&owner);

    let did = String::from_str(&env, "did:soroban:test1234");
    let document = DIDDocument {
        context: Vec::from_array(
            &env,
            [String::from_str(&env, "https://www.w3.org/ns/did/v1")],
        ),
        id: did.clone(),
        verification_method: Vec::new(&env),
        authentication: Vec::new(&env),
        assertion_method: Vec::new(&env),
        key_agreement: Vec::new(&env),
        capability_invocation: Vec::new(&env),
        capability_delegation: Vec::new(&env),
        service: Vec::new(&env),
        controllers: Vec::new(&env),
        service_endpoints: Vec::new(&env),
    };

    client.register_did(&did, &document, &None);

    let method_id = String::from_str(&env, "key-1");
    let method = VerificationMethod {
        id: method_id.clone(),
        type_: String::from_str(&env, "Ed25519VerificationKey2020"),
        controller: owner.clone(),
        public_key_multibase: Bytes::from_array(&env, &[1, 2, 3]),
        deactivated: false,
    };

    client.add_verification_method(&did, &method);

    // Rotate key
    let new_key = Bytes::from_array(&env, &[9, 8, 7]);
    client.rotate_verification_method(&did, &method_id, &new_key);

    let doc_after_rotation = client.get_did_document(&did);
    assert_eq!(
        doc_after_rotation
            .verification_method
            .get(1)
            .unwrap()
            .public_key_multibase,
        new_key
    );
    assert!(
        doc_after_rotation
            .verification_method
            .get(0)
            .unwrap()
            .deactivated
    );

    // Remove key
    client.remove_verification_method(&did, &method_id);
    let doc_after_removal = client.get_did_document(&did);
    assert_eq!(doc_after_removal.verification_method.len(), 0);
}

#[test]
#[should_panic(expected = "invalid DID URI format")]
fn test_invalid_did_uri() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DIDRegistry, ());
    let client = DIDRegistryClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    client.initialize(&owner);

    let invalid_did = String::from_str(&env, "http://invalid-uri");
    let document = DIDDocument {
        context: Vec::new(&env),
        id: invalid_did.clone(),
        verification_method: Vec::new(&env),
        authentication: Vec::new(&env),
        assertion_method: Vec::new(&env),
        key_agreement: Vec::new(&env),
        capability_invocation: Vec::new(&env),
        capability_delegation: Vec::new(&env),
        service: Vec::new(&env),
        controllers: Vec::new(&env),
        service_endpoints: Vec::new(&env),
    };

    client.register_did(&invalid_did, &document, &None);
}

#[test]
fn test_update_document_emits_event_and_chains_cid() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DIDRegistry, ());
    let client = DIDRegistryClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    client.initialize(&owner);

    let did_id = String::from_str(&env, "did:soroban:test_cid_tracking");
    let document = DIDDocument {
        context: Vec::from_array(
            &env,
            [String::from_str(&env, "https://www.w3.org/ns/did/v1")],
        ),
        id: did_id.clone(),
        verification_method: Vec::new(&env),
        authentication: Vec::new(&env),
        assertion_method: Vec::new(&env),
        key_agreement: Vec::new(&env),
        capability_invocation: Vec::new(&env),
        capability_delegation: Vec::new(&env),
        service: Vec::new(&env),
        controllers: Vec::new(&env),
        service_endpoints: Vec::new(&env),
    };

    client.register_did(&did_id, &document, &None);

    let cid_v1 = String::from_str(
        &env,
        "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi",
    );
    client.update_document(&did_id, &cid_v1);

    // Verify first event immediately after update_document
    let events = env.events().all();
    let event_name = Symbol::new(&env, "did_updated");
    let matching: std::vec::Vec<_> = events
        .iter()
        .filter(|(c_id, topics, _)| {
            if c_id != &contract_id || topics.len() != 2 {
                return false;
            }
            let sym: Result<Symbol, _> = topics.get(0).unwrap().try_into_val(&env);
            let did_topic: Result<String, _> = topics.get(1).unwrap().try_into_val(&env);
            sym.ok() == Some(event_name.clone()) && did_topic.ok() == Some(did_id.clone())
        })
        .collect();

    assert_eq!(matching.len(), 1);
    let (_, _, data) = matching.last().unwrap();
    let payload: (String, String) = data.clone().try_into_val(&env).unwrap();
    assert_eq!(payload.0, String::from_str(&env, ""));
    assert_eq!(payload.1, cid_v1);

    assert_eq!(client.get_document_cid(&did_id), cid_v1);

    // Second update to confirm old CID chains correctly
    let cid_v2 = String::from_str(
        &env,
        "bafybeihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku",
    );
    client.update_document(&did_id, &cid_v2);

    let events2 = env.events().all();
    let matching2: std::vec::Vec<_> = events2
        .iter()
        .filter(|(c_id, topics, _)| {
            if c_id != &contract_id || topics.len() != 2 {
                return false;
            }
            let sym: Result<Symbol, _> = topics.get(0).unwrap().try_into_val(&env);
            let did_topic: Result<String, _> = topics.get(1).unwrap().try_into_val(&env);
            sym.ok() == Some(event_name.clone()) && did_topic.ok() == Some(did_id.clone())
        })
        .collect();

    assert_eq!(matching2.len(), 1);
    let (_, _, data2) = matching2.last().unwrap();
    let payload2: (String, String) = data2.clone().try_into_val(&env).unwrap();
    assert_eq!(payload2.0, cid_v1);
    assert_eq!(payload2.1, cid_v2);

    assert_eq!(client.get_document_cid(&did_id), cid_v2);
}

#[test]
#[should_panic(expected = "DID not found")]
fn test_update_document_did_not_found() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DIDRegistry, ());
    let client = DIDRegistryClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    client.initialize(&owner);

    let unregistered_did = String::from_str(&env, "did:soroban:unregistered");
    let cid = String::from_str(
        &env,
        "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi",
    );
    client.update_document(&unregistered_did, &cid);
}

#[test]
#[should_panic(expected = "invalid DID URI format")]
fn test_update_document_invalid_did_uri() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DIDRegistry, ());
    let client = DIDRegistryClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    client.initialize(&owner);

    let invalid_did = String::from_str(&env, "http://invalid-uri");
    let cid = String::from_str(
        &env,
        "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi",
    );
    client.update_document(&invalid_did, &cid);
}

fn document_with_controllers(env: &Env, did: &String, controllers: Vec<Address>) -> DIDDocument {
    DIDDocument {
        context: Vec::from_array(env, [String::from_str(env, "https://www.w3.org/ns/did/v1")]),
        id: did.clone(),
        verification_method: Vec::new(env),
        authentication: Vec::new(env),
        assertion_method: Vec::new(env),
        key_agreement: Vec::new(env),
        capability_invocation: Vec::new(env),
        capability_delegation: Vec::new(env),
        service: Vec::new(env),
        controllers,
        service_endpoints: Vec::new(env),
    }
}

fn event_names(env: &Env) -> std::vec::Vec<Symbol> {
    env.events()
        .all()
        .iter()
        .map(|(_, topics, _)| topics.get(0).unwrap().try_into_val(env).unwrap())
        .collect()
}

#[test]
fn secondary_controller_can_add_methods_and_service_endpoints() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DIDRegistry, ());
    let client = DIDRegistryClient::new(&env, &contract_id);
    let owner = Address::generate(&env);
    let secondary = Address::generate(&env);
    client.initialize(&owner);

    let did = String::from_str(&env, "did:example:controllers");
    let controllers = Vec::from_array(&env, [owner.clone(), secondary.clone()]);
    let document = document_with_controllers(&env, &did, controllers);
    client.register_did(&did, &document, &None);

    let method = VerificationMethod {
        id: String::from_str(&env, "secondary-key"),
        type_: String::from_str(&env, "Ed25519VerificationKey2020"),
        controller: secondary.clone(),
        public_key_multibase: Bytes::from_array(&env, &[4, 5, 6]),
        deactivated: false,
    };
    client.add_verification_method(&did, &method);

    let service_id = Symbol::new(&env, "linked_domains");
    let service_type = Symbol::new(&env, "LinkedDomains");
    let uri = String::from_str(&env, "https://example.com");
    client.add_service_endpoint(&secondary, &did, &service_id, &service_type, &uri);

    let updated_uri = String::from_str(&env, "https://example.org");
    client.update_service_endpoint(&secondary, &did, &service_id, &service_type, &updated_uri);

    let updated = client.get_did_document(&did);
    assert_eq!(updated.controllers.len(), 2);
    assert_eq!(updated.verification_method.len(), 1);
    let endpoint = client
        .get_service_endpoint(&did, &service_id)
        .expect("service endpoint should exist");
    assert_eq!(endpoint.service_type, service_type);
    assert_eq!(endpoint.uri, updated_uri);
}

#[test]
fn secondary_controller_can_be_delegated_and_removed() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DIDRegistry, ());
    let client = DIDRegistryClient::new(&env, &contract_id);
    let owner = Address::generate(&env);
    let secondary = Address::generate(&env);
    client.initialize(&owner);

    let did = String::from_str(&env, "did:example:delegation");
    let document = document_with_controllers(&env, &did, Vec::from_array(&env, [owner.clone()]));
    client.register_did(&did, &document, &None);

    client.add_controller(&owner, &did, &secondary);
    let after_add = client.get_did_document(&did);
    assert_eq!(after_add.controllers.len(), 2);

    let service_id = Symbol::new(&env, "didcomm");
    client.add_service_endpoint(
        &secondary,
        &did,
        &service_id,
        &Symbol::new(&env, "DIDCommMessaging"),
        &String::from_str(&env, "https://didcomm.example.com"),
    );

    client.remove_controller(&owner, &did, &secondary);
    assert_eq!(client.get_did_document(&did).controllers.len(), 1);
    assert_eq!(
        client.try_add_service_endpoint(
            &secondary,
            &did,
            &service_id,
            &Symbol::new(&env, "DIDCommMessaging"),
            &String::from_str(&env, "https://didcomm.example.com"),
        ),
        Err(Ok(Error::Unauthorized))
    );
}

#[test]
fn did_lifecycle_events_are_emitted() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DIDRegistry, ());
    let client = DIDRegistryClient::new(&env, &contract_id);
    let owner = Address::generate(&env);
    client.initialize(&owner);

    let did = String::from_str(&env, "did:example:events");
    let document = document_with_controllers(&env, &did, Vec::from_array(&env, [owner.clone()]));
    client.register_did(&did, &document, &None);
    let mut emitted = std::vec::Vec::new();
    emitted.extend(event_names(&env));

    client.add_verification_method(
        &did,
        &VerificationMethod {
            id: String::from_str(&env, "key-1"),
            type_: String::from_str(&env, "Ed25519VerificationKey2020"),
            controller: owner.clone(),
            public_key_multibase: Bytes::from_array(&env, &[1]),
            deactivated: false,
        },
    );
    emitted.extend(event_names(&env));

    client.add_service_endpoint(
        &owner,
        &did,
        &Symbol::new(&env, "linked_domains"),
        &Symbol::new(&env, "LinkedDomains"),
        &String::from_str(&env, "https://example.com"),
    );
    emitted.extend(event_names(&env));

    client.deactivate_did(&owner, &did);
    emitted.extend(event_names(&env));

    let expected = [
        Symbol::new(&env, "did_created"),
        Symbol::new(&env, "verification_method_added"),
        Symbol::new(&env, "service_endpoint_updated"),
        Symbol::new(&env, "did_deactivated"),
    ];
    for event_name in expected {
        assert!(
            emitted.contains(&event_name),
            "missing event: {:?}",
            event_name
        );
    }
}

#[test]
fn deactivation_is_permanent_and_blocks_mutations() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DIDRegistry, ());
    let client = DIDRegistryClient::new(&env, &contract_id);
    let owner = Address::generate(&env);
    let secondary = Address::generate(&env);
    client.initialize(&owner);

    let did = String::from_str(&env, "did:example:deactivated");
    let controllers = Vec::from_array(&env, [owner.clone(), secondary.clone()]);
    let document = document_with_controllers(&env, &did, controllers);
    client.register_did(&did, &document, &None);
    client.deactivate_did(&secondary, &did);

    assert!(!client.is_did_valid(&did));
    assert_eq!(client.get_did_document(&did).id, did);
    assert_eq!(
        client.try_deactivate_did(&owner, &did),
        Err(Ok(Error::DIDDeactivated))
    );
    assert_eq!(
        client.try_add_verification_method(
            &did,
            &VerificationMethod {
                id: String::from_str(&env, "late-key"),
                type_: String::from_str(&env, "Ed25519VerificationKey2020"),
                controller: owner.clone(),
                public_key_multibase: Bytes::from_array(&env, &[7]),
                deactivated: false,
            },
        ),
        Err(Ok(Error::DIDDeactivated))
    );
    assert_eq!(
        client.try_add_service_endpoint(
            &owner,
            &did,
            &Symbol::new(&env, "late_service"),
            &Symbol::new(&env, "LinkedDomains"),
            &String::from_str(&env, "https://example.com"),
        ),
        Err(Ok(Error::DIDDeactivated))
    );
    assert_eq!(
        client.try_update_did_document(&did, &document),
        Err(Ok(Error::DIDDeactivated))
    );
    assert_eq!(client.try_revoke_did(&did), Err(Ok(Error::DIDDeactivated)));
}

#[test]
fn verification_method_deactivation_keeps_history() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(DIDRegistry, ());
    let client = DIDRegistryClient::new(&env, &contract_id);
    let owner = Address::generate(&env);
    client.initialize(&owner);

    let did = String::from_str(&env, "did:example:key-history");
    let document = document_with_controllers(&env, &did, Vec::from_array(&env, [owner.clone()]));
    client.register_did(&did, &document, &None);
    let method_id = String::from_str(&env, "key-1");
    client.add_verification_method(
        &did,
        &VerificationMethod {
            id: method_id.clone(),
            type_: String::from_str(&env, "Ed25519VerificationKey2020"),
            controller: owner.clone(),
            public_key_multibase: Bytes::from_array(&env, &[1, 2]),
            deactivated: false,
        },
    );
    client.deactivate_verification_method(&owner, &did, &method_id);

    let methods = client.get_did_document(&did).verification_method;
    assert_eq!(methods.len(), 1);
    assert!(methods.get(0).unwrap().deactivated);
}
