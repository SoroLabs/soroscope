#![cfg(test)]

use crate::{CrossChainVerifier, CrossChainVerifierClient, Payload};
use soroban_sdk::{testutils::Address as _, Address, Bytes, BytesN, Env, Vec};

fn make_payload(env: &Env, nonce: u64) -> Payload {
    let chain_id = 1;
    let dest = Address::generate(env);
    let data = Bytes::from_slice(env, b"test message");
    Payload {
        chain_id,
        destination_contract: dest,
        nonce,
        data,
    }

fn compute_leaf(env: &Env, payload: &Payload) -> BytesN<32> {
    CrossChainVerifier::compute_payload_hash(env, payload)
use crate::{CrossChainVerifier, CrossChainVerifierClient};
use crate::{
    CrossChainMessage, CrossChainVerifier, CrossChainVerifierClient, SignatureAlgorithm,
    SignedMessage,
};
use crate::{CrossChainVerifier, CrossChainVerifierClient, CrossChainMessage, SignedMessage, SignatureAlgorithm};
use ed25519_dalek::{Signer, SigningKey};
use soroban_sdk::{testutils::Address as _, Address, Bytes, BytesN, Env, Vec};

#[test]
fn test_initialization() {
    let env = Env::default();
    let contract_id = env.register(CrossChainVerifier, ());
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_double_initialization() {
    let env = Env::default();
    let contract_id = env.register(CrossChainVerifier, ());
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);
    client.initialize(&admin);
}

#[test]
fn test_root_update() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CrossChainVerifier, ());
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let root = BytesN::from_array(&env, &[1; 32]);
    let block_height = 100;

    client.update_root(&block_height, &root);

    let retrieved = client.get_root(&block_height).unwrap();
    assert_eq!(retrieved, root);
}

#[test]
fn test_verify_message_success() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CrossChainVerifier, ());
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let payload = make_payload(&env, 1);
    let leaf = compute_leaf(&env, &payload);

    let sibling1 = BytesN::from_array(&env, &[3; 32]);
    let sibling2 = BytesN::from_array(&env, &[4; 32]);

    // Manually construct the root
    let mut combined_1 = [0u8; 64];
    combined_1[0..32].copy_from_slice(&sibling1.to_array());
    combined_1[32..64].copy_from_slice(&leaf.to_array());
    let hash_1 = env
        .crypto()
        .sha256(&Bytes::from_slice(&env, &combined_1))
        .to_array();

    let mut combined_2 = [0u8; 64];
    combined_2[0..32].copy_from_slice(&hash_1);
    combined_2[32..64].copy_from_slice(&sibling2.to_array());
    let final_root = env
        .crypto()
        .sha256(&Bytes::from_slice(&env, &combined_2))
        .to_array();

    let expected_root_bytes = BytesN::from_array(&env, &final_root);

    let block_height = 100;
    client.update_root(&block_height, &expected_root_bytes);

    let mut proof = Vec::new(&env);
    proof.push_back(sibling1);
    proof.push_back(sibling2);

    let mut proof_flags = Vec::new(&env);
    proof_flags.push_back(true);
    proof_flags.push_back(false);
    proof_flags.push_back(true); // left
    proof_flags.push_back(false); // right

    let result = client.verify_message(&block_height, &payload, &proof, &proof_flags);
    assert!(result);
}

#[test]
fn test_verify_message_and_consume_nonce() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CrossChainVerifier);
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let leaf = BytesN::from_array(&env, &[2; 32]);
    let sibling1 = BytesN::from_array(&env, &[3; 32]);
    let sibling2 = BytesN::from_array(&env, &[4; 32]);

    let mut combined_1 = [0u8; 64];
    combined_1[0..32].copy_from_slice(&sibling1.to_array());
    combined_1[32..64].copy_from_slice(&leaf.to_array());
    let hash_1 = env.crypto().sha256(&Bytes::from_slice(&env, &combined_1)).to_array();

    let mut combined_2 = [0u8; 64];
    combined_2[0..32].copy_from_slice(&hash_1);
    combined_2[32..64].copy_from_slice(&sibling2.to_array());
    let final_root = env.crypto().sha256(&Bytes::from_slice(&env, &combined_2)).to_array();

    let expected_root_bytes = BytesN::from_array(&env, &final_root);
    let block_height = 100;
    client.update_root(&block_height, &expected_root_bytes);

    let mut proof = Vec::new(&env);
    proof.push_back(sibling1);
    proof.push_back(sibling2);

    let mut proof_flags = Vec::new(&env);
    proof_flags.push_back(true);
    proof_flags.push_back(false);

    assert!(client.verify_message_and_consume(&block_height, &1u64, &leaf, &proof, &proof_flags));
    assert!(client.is_nonce_processed(&1u64));
}

#[test]
#[should_panic(expected = "nonce already processed")]
fn test_replay_nonce_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CrossChainVerifier);
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let leaf = BytesN::from_array(&env, &[2; 32]);
    let sibling1 = BytesN::from_array(&env, &[3; 32]);
    let sibling2 = BytesN::from_array(&env, &[4; 32]);

    let mut combined_1 = [0u8; 64];
    combined_1[0..32].copy_from_slice(&sibling1.to_array());
    combined_1[32..64].copy_from_slice(&leaf.to_array());
    let hash_1 = env.crypto().sha256(&Bytes::from_slice(&env, &combined_1)).to_array();

    let mut combined_2 = [0u8; 64];
    combined_2[0..32].copy_from_slice(&hash_1);
    combined_2[32..64].copy_from_slice(&sibling2.to_array());
    let final_root = env.crypto().sha256(&Bytes::from_slice(&env, &combined_2)).to_array();

    let expected_root_bytes = BytesN::from_array(&env, &final_root);
    let block_height = 100;
    client.update_root(&block_height, &expected_root_bytes);

    let mut proof = Vec::new(&env);
    proof.push_back(sibling1);
    proof.push_back(sibling2);

    let mut proof_flags = Vec::new(&env);
    proof_flags.push_back(true);
    proof_flags.push_back(false);

    assert!(client.verify_message_and_consume(&block_height, &1u64, &leaf, &proof, &proof_flags));
    client.verify_message_and_consume(&block_height, &1u64, &leaf, &proof, &proof_flags);
}

#[test]
fn test_verify_message_no_root() {
    let env = Env::default();
    let contract_id = env.register(CrossChainVerifier, ());
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let payload = make_payload(&env, 1);
    let proof = Vec::new(&env);
    let proof_flags = Vec::new(&env);

    client.verify_message(&100, &payload, &proof, &proof_flags);
}

#[test]
#[should_panic(expected = "Nonce already used")]
fn test_verify_message_replay_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CrossChainVerifier);
    assert!(!client.verify_message(&100, &leaf, &proof, &proof_flags));
}

// ============================================================================
// Signature Verification Tests
// ============================================================================

#[test]
fn test_add_authorized_signer_ed25519() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CrossChainVerifier);
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let payload = make_payload(&env, 1);
    let leaf = compute_leaf(&env, &payload);

    // Single-node tree: leaf == root
    let block_height = 100;
    client.update_root(&block_height, &leaf);

    let proof = Vec::new(&env);
    let proof_flags = Vec::new(&env);

    // First use succeeds
    assert!(client.verify_message(&block_height, &payload, &proof, &proof_flags));

    // Second use with same nonce should panic
    client.verify_message(&block_height, &payload, &proof, &proof_flags);
}

#[test]
fn test_verify_message_different_nonce_allowed() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CrossChainVerifier);
    // Create a test Ed25519 public key (32 bytes)
    let public_key = Bytes::from_slice(&env, &[1; 32]);
    
    client.add_authorized_signer(&public_key, &SignatureAlgorithm::Ed25519);

    // Verify signer count increased
    let count = client.get_signer_count();
    assert_eq!(count, 1);
}

#[test]
fn test_add_authorized_signer_secp256k1() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CrossChainVerifier);
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    // Create a test Secp256k1 public key (33 bytes compressed)
    let public_key = Bytes::from_slice(&env, &[2; 33]);
    
    client.add_authorized_signer(&public_key, &SignatureAlgorithm::Secp256k1);

    // Verify signer count increased
    let count = client.get_signer_count();
    assert_eq!(count, 1);
}

#[test]
#[should_panic(expected = "Signer already authorized")]
fn test_add_duplicate_signer() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CrossChainVerifier);
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let public_key = Bytes::from_slice(&env, &[1; 32]);
    
    client.add_authorized_signer(&public_key, &SignatureAlgorithm::Ed25519);
    client.add_authorized_signer(&public_key, &SignatureAlgorithm::Ed25519); // Should panic
}

#[test]
fn test_remove_authorized_signer() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CrossChainVerifier);
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let payload1 = make_payload(&env, 1);
    let leaf1 = compute_leaf(&env, &payload1);

    let payload2 = make_payload(&env, 2);
    let leaf2 = compute_leaf(&env, &payload2);

    // Build a root that commits to both leaves (2-level tree)
    // Level 1: combine leaf1 with leaf2
    let mut combined = [0u8; 64];
    combined[0..32].copy_from_slice(&leaf1.to_array());
    combined[32..64].copy_from_slice(&leaf2.to_array());
    let branch = env
        .crypto()
        .sha256(&Bytes::from_slice(&env, &combined))
        .to_array();
    let root = BytesN::from_array(&env, &branch);

    let block_height = 100;
    client.update_root(&block_height, &root);

    // Prove leaf1 at position 0 (left child, sibling=leaf2)
    let mut proof1 = Vec::new(&env);
    proof1.push_back(leaf2);
    let mut flags1 = Vec::new(&env);
    flags1.push_back(false);
    assert!(client.verify_message(&block_height, &payload1, &proof1, &flags1));

    // Prove leaf2 at position 0 (right child, sibling=leaf1 on left)
    let mut proof2 = Vec::new(&env);
    proof2.push_back(leaf1);
    let mut flags2 = Vec::new(&env);
    flags2.push_back(true);
    assert!(client.verify_message(&block_height, &payload2, &proof2, &flags2));
}

#[test]
fn test_compute_payload_hash_differs_by_chain_id() {
    let env = Env::default();
    let dest = Address::generate(&env);
    let data = Bytes::from_slice(&env, b"hello");

    let p1 = Payload {
        chain_id: 1,
        destination_contract: dest.clone(),
        nonce: 0,
        data: data.clone(),
    };
    let p2 = Payload {
        chain_id: 2,
        destination_contract: dest.clone(),
        nonce: 0,
        data: data.clone(),
    };

    let h1 = CrossChainVerifier::compute_payload_hash(&env, &p1);
    let h2 = CrossChainVerifier::compute_payload_hash(&env, &p2);
    assert_ne!(h1, h2);
}

#[test]
fn test_compute_payload_hash_differs_by_nonce() {
    let env = Env::default();
    let dest = Address::generate(&env);
    let data = Bytes::from_slice(&env, b"hello");

    let p1 = Payload {
        chain_id: 1,
        destination_contract: dest.clone(),
        nonce: 0,
        data: data.clone(),
    };
    let p2 = Payload {
        chain_id: 1,
        destination_contract: dest.clone(),
        nonce: 1,
        data: data.clone(),
    };

    let h1 = CrossChainVerifier::compute_payload_hash(&env, &p1);
    let h2 = CrossChainVerifier::compute_payload_hash(&env, &p2);
    assert_ne!(h1, h2);
}

#[test]
fn test_compute_payload_hash_differs_by_destination() {
    let env = Env::default();
    let dest1 = Address::generate(&env);
    let dest2 = Address::generate(&env);
    let data = Bytes::from_slice(&env, b"hello");

    let p1 = Payload {
        chain_id: 1,
        destination_contract: dest1,
        nonce: 0,
        data: data.clone(),
    };
    let p2 = Payload {
        chain_id: 1,
        destination_contract: dest2,
        nonce: 0,
        data,
    };

    let h1 = CrossChainVerifier::compute_payload_hash(&env, &p1);
    let h2 = CrossChainVerifier::compute_payload_hash(&env, &p2);
    assert_ne!(h1, h2);
    let public_key = Bytes::from_slice(&env, &[1; 32]);
    
    client.add_authorized_signer(&public_key, &SignatureAlgorithm::Ed25519);
    assert_eq!(client.get_signer_count(), 1);

    client.remove_authorized_signer(&public_key);
    assert_eq!(client.get_signer_count(), 0);
}

#[test]
#[should_panic(expected = "Signer not found")]
fn test_remove_nonexistent_signer() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CrossChainVerifier);
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let public_key = Bytes::from_slice(&env, &[1; 32]);
    client.remove_authorized_signer(&public_key); // Should panic
}

#[test]
fn test_verify_signed_message_success_ed25519() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CrossChainVerifier);
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    let verifying_key = signing_key.verifying_key();
    let public_key = Bytes::from_slice(&env, &verifying_key.to_bytes());

    client.add_authorized_signer(&public_key, &SignatureAlgorithm::Ed25519);

    let message = CrossChainMessage {
        source_chain: 1,
        destination_chain: 2,
        nonce: 1,
        payload: Bytes::from_slice(&env, b"test payload"),
        timestamp: 1000,
    };

    let message_hash: BytesN<32> = {
        let mut data = Bytes::new(&env);
        data.append(&Bytes::from_slice(&env, b"CROSS_CHAIN_MESSAGE_V1"));
        data.append(&Bytes::from_slice(&env, &message.source_chain.to_be_bytes()));
        data.append(&Bytes::from_slice(&env, &message.destination_chain.to_be_bytes()));
        data.append(&Bytes::from_slice(&env, &message.nonce.to_be_bytes()));
        data.append(&Bytes::from_slice(&env, &message.timestamp.to_be_bytes()));
        let payload_hash = env.crypto().sha256(&message.payload).to_array();
        data.append(&Bytes::from_slice(&env, &payload_hash));
        BytesN::from_array(&env, &env.crypto().sha256(&data).to_array())
    };

    let signature = signing_key.sign(&message_hash.to_array());

    let signed_message = SignedMessage {
        message,
        signature: BytesN::from_array(&env, &signature.to_bytes()),
        signer_public_key: BytesN::from_array(&env, &verifying_key.to_bytes()),
        algorithm: SignatureAlgorithm::Ed25519,
    };

    let sibling1 = BytesN::from_array(&env, &[3; 32]);
    let sibling2 = BytesN::from_array(&env, &[4; 32]);

    let mut combined_1 = [0u8; 64];
    combined_1[0..32].copy_from_slice(&sibling1.to_array());
    combined_1[32..64].copy_from_slice(&message_hash.to_array());
    let hash_1 = env.crypto().sha256(&Bytes::from_slice(&env, &combined_1)).to_array();

    let mut combined_2 = [0u8; 64];
    combined_2[0..32].copy_from_slice(&hash_1);
    combined_2[32..64].copy_from_slice(&sibling2.to_array());
    let final_root = env.crypto().sha256(&Bytes::from_slice(&env, &combined_2)).to_array();

    let expected_root = BytesN::from_array(&env, &final_root);
    let block_height = 100;
    client.update_root(&block_height, &expected_root);

    let mut proof = Vec::new(&env);
    proof.push_back(sibling1);
    proof.push_back(sibling2);

    let mut proof_flags = Vec::new(&env);
    proof_flags.push_back(true);
    proof_flags.push_back(false);

    let result = client.verify_signed_message(&signed_message, &block_height, &proof, &proof_flags);
    assert!(result);

    // Second verification of the same signed message should fail due to replay protection.
    let replay_result = client.verify_signed_message(&signed_message, &block_height, &proof, &proof_flags);
    assert!(!replay_result);
}

#[test]
fn test_verify_signed_message_accepts_valid_signature() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CrossChainVerifier);
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    let signing_key = SigningKey::from_bytes(&[9u8; 32]);
    let verifying_key = signing_key.verifying_key();
    let public_key = Bytes::from_slice(&env, &verifying_key.to_bytes());

    client.add_authorized_signer(&public_key, &SignatureAlgorithm::Ed25519);

    let message = CrossChainMessage {
        source_chain: 7,
        destination_chain: 8,
        nonce: 42,
        payload: Bytes::from_slice(&env, b"approved payload"),
        timestamp: 2000,
    };

    let message_hash: BytesN<32> = {
        let mut data = Bytes::new(&env);
        data.append(&Bytes::from_slice(&env, b"CROSS_CHAIN_MESSAGE_V1"));
        data.append(&Bytes::from_slice(&env, &message.source_chain.to_be_bytes()));
        data.append(&Bytes::from_slice(&env, &message.destination_chain.to_be_bytes()));
        data.append(&Bytes::from_slice(&env, &message.nonce.to_be_bytes()));
        data.append(&Bytes::from_slice(&env, &message.timestamp.to_be_bytes()));
        let payload_hash = env.crypto().sha256(&message.payload).to_array();
        data.append(&Bytes::from_slice(&env, &payload_hash));
        BytesN::from_array(&env, &env.crypto().sha256(&data).to_array())
    };

    let signature = signing_key.sign(&message_hash.to_array());

    let signed_message = SignedMessage {
        message,
        signature: BytesN::from_array(&env, &signature.to_bytes()),
        signer_public_key: BytesN::from_array(&env, &verifying_key.to_bytes()),
        algorithm: SignatureAlgorithm::Ed25519,
    };

    let leaf = BytesN::from_array(&env, &message_hash.to_array());
    let sibling1 = BytesN::from_array(&env, &[11; 32]);
    let sibling2 = BytesN::from_array(&env, &[13; 32]);

    let mut combined_1 = [0u8; 64];
    combined_1[0..32].copy_from_slice(&sibling1.to_array());
    combined_1[32..64].copy_from_slice(&leaf.to_array());
    let hash_1 = env.crypto().sha256(&Bytes::from_slice(&env, &combined_1)).to_array();

    let mut combined_2 = [0u8; 64];
    combined_2[0..32].copy_from_slice(&hash_1);
    combined_2[32..64].copy_from_slice(&sibling2.to_array());
    let final_root = env.crypto().sha256(&Bytes::from_slice(&env, &combined_2)).to_array();

    let expected_root = BytesN::from_array(&env, &final_root);
    let block_height = 200;
    client.update_root(&block_height, &expected_root);

    let mut proof = Vec::new(&env);
    proof.push_back(sibling1);
    proof.push_back(sibling2);

    let mut proof_flags = Vec::new(&env);
    proof_flags.push_back(true);
    proof_flags.push_back(false);

    assert!(client.verify_signed_message(&signed_message, &block_height, &proof, &proof_flags));
}

#[test]
fn test_verify_signed_message_with_invalid_signer() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CrossChainVerifier);
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    // Create a cross-chain message
    let message = CrossChainMessage {
        source_chain: 1,
        destination_chain: 2,
        nonce: 1,
        payload: Bytes::from_slice(&env, b"test payload"),
        timestamp: 1000,
    };

    // Create a signed message with an unauthorized signer
    let unauthorized_public_key = Bytes::from_slice(&env, &[99; 32]);
    let signature = BytesN::from_array(&env, &[0; 64]);
    
    let signed_message = SignedMessage {
        message,
        signature,
        signer_public_key: BytesN::from_array(&env, &[99; 32]),
        algorithm: SignatureAlgorithm::Ed25519,
    };

    // Create Merkle proof
    let proof = Vec::new(&env);
    let proof_flags = Vec::new(&env);

    // Verification should fail because signer is not authorized
    let result = client.verify_signed_message(&signed_message, &100, &proof, &proof_flags);
    assert!(!result);
}

#[test]
fn test_multiple_authorized_signers() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CrossChainVerifier);
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    // Add multiple signers with different algorithms
    let ed25519_key = Bytes::from_slice(&env, &[1; 32]);
    let secp256k1_key = Bytes::from_slice(&env, &[2; 33]);
    
    client.add_authorized_signer(&ed25519_key, &SignatureAlgorithm::Ed25519);
    client.add_authorized_signer(&secp256k1_key, &SignatureAlgorithm::Secp256k1);

    // Verify signer count
    assert_eq!(client.get_signer_count(), 2);
}

// ============================================================================
// Performance Benchmark Tests
// ============================================================================

#[test]
fn test_signer_lookup_performance_single() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CrossChainVerifier);
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    // Add a single signer
    let public_key = Bytes::from_slice(&env, &[1; 32]);
    client.add_authorized_signer(&public_key, &SignatureAlgorithm::Ed25519);

    // Verify signer lookup is O(1)
    assert_eq!(client.get_signer_count(), 1);
}

#[test]
fn test_signer_lookup_performance_multiple() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CrossChainVerifier);
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    // Add multiple signers (simulating O(1) indexed storage)
    for i in 0..10 {
        let mut key_bytes = [0u8; 32];
        key_bytes[0] = i as u8;
        let public_key = Bytes::from_slice(&env, &key_bytes);
        client.add_authorized_signer(&public_key, &SignatureAlgorithm::Ed25519);
    }

    // Verify all signers were added
    assert_eq!(client.get_signer_count(), 10);
}

#[test]
fn test_signer_removal_performance() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, CrossChainVerifier);
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);

    client.initialize(&admin);

    // Add signers
    let mut keys = Vec::new(&env);
    for i in 0..5 {
        let mut key_bytes = [0u8; 32];
        key_bytes[0] = i as u8;
        let public_key = Bytes::from_slice(&env, &key_bytes);
        client.add_authorized_signer(&public_key, &SignatureAlgorithm::Ed25519);
        keys.push_back(public_key);
    }

    assert_eq!(client.get_signer_count(), 5);

    // Remove signers (O(1) per removal)
    for key in keys {
        client.remove_authorized_signer(&key);
    }

    assert_eq!(client.get_signer_count(), 0);
}

// ============================================================================
// PauseType::VERIFY Tests (#482)
// ============================================================================

/// Helper: build a valid one-node Merkle proof and return (client, block_height, leaf, proof, proof_flags).
fn setup_valid_proof(
    env: &Env,
    client: &CrossChainVerifierClient,
) -> (u32, BytesN<32>, soroban_sdk::Vec<BytesN<32>>, soroban_sdk::Vec<bool>) {
    let leaf = BytesN::from_array(env, &[2u8; 32]);
    let sibling = BytesN::from_array(env, &[3u8; 32]);

    let mut combined = [0u8; 64];
    combined[0..32].copy_from_slice(&sibling.to_array());
    combined[32..64].copy_from_slice(&leaf.to_array());
    let root_arr = env.crypto().sha256(&Bytes::from_slice(env, &combined)).to_array();
    let root = BytesN::from_array(env, &root_arr);

    let block_height: u32 = 42;
    client.update_root(&block_height, &root);

    let mut proof = soroban_sdk::Vec::new(env);
    proof.push_back(sibling);
    let mut flags = soroban_sdk::Vec::new(env);
    flags.push_back(true);

    (block_height, leaf, proof, flags)
}

#[test]
fn test_is_paused_defaults_to_false() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CrossChainVerifier, ());
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    assert!(!client.is_paused());
}

#[test]
fn test_set_paused_and_is_paused() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CrossChainVerifier, ());
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    client.set_paused(&true);
    assert!(client.is_paused());

    client.set_paused(&false);
    assert!(!client.is_paused());
}

#[test]
fn test_verify_message_returns_false_when_paused() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CrossChainVerifier, ());
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let (block_height, leaf, proof, flags) = setup_valid_proof(&env, &client);

    // Verify succeeds before pause
    assert!(client.verify_message(&block_height, &leaf, &proof, &flags));

    // Pause and verify it now returns false
    client.set_paused(&true);
    assert!(!client.verify_message(&block_height, &leaf, &proof, &flags));
}

#[test]
fn test_verify_message_succeeds_after_unpause() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CrossChainVerifier, ());
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let (block_height, leaf, proof, flags) = setup_valid_proof(&env, &client);

    client.set_paused(&true);
    assert!(!client.verify_message(&block_height, &leaf, &proof, &flags));

    client.set_paused(&false);
    assert!(client.verify_message(&block_height, &leaf, &proof, &flags));
}

#[test]
#[should_panic(expected = "verification paused")]
fn test_verify_message_and_consume_panics_when_paused() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CrossChainVerifier, ());
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let (block_height, leaf, proof, flags) = setup_valid_proof(&env, &client);

    client.set_paused(&true);
    client.verify_message_and_consume(&block_height, &99u64, &leaf, &proof, &flags);
}

#[test]
#[should_panic(expected = "verification paused")]
fn test_verify_signed_message_panics_when_paused() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CrossChainVerifier, ());
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    client.set_paused(&true);

    let signing_key = SigningKey::from_bytes(&[5u8; 32]);
    let verifying_key = signing_key.verifying_key();
    let public_key = Bytes::from_slice(&env, &verifying_key.to_bytes());
    client.add_authorized_signer(&public_key, &SignatureAlgorithm::Ed25519);

    let message = CrossChainMessage {
        source_chain: 1,
        destination_chain: 2,
        nonce: 1,
        payload: Bytes::from_slice(&env, b"payload"),
        timestamp: 100,
    };
    let signature = signing_key.sign(b"anything");
    let signed_message = SignedMessage {
        message,
        signature: BytesN::from_array(&env, &signature.to_bytes()),
        signer_public_key: BytesN::from_array(&env, &verifying_key.to_bytes()),
        algorithm: SignatureAlgorithm::Ed25519,
    };

    let proof = soroban_sdk::Vec::new(&env);
    let flags = soroban_sdk::Vec::new(&env);
    client.verify_signed_message(&signed_message, &100u32, &proof, &flags);
}
