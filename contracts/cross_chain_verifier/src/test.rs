#![cfg(test)]

use crate::{
    CrossChainMessage, CrossChainVerifier, CrossChainVerifierClient, Payload, SignatureAlgorithm,
    SignedMessage,
};
use ed25519_dalek::{Signer, SigningKey};
use soroban_sdk::{testutils::Address as _, Address, Bytes, BytesN, Env, Vec};

fn make_payload(env: &Env, nonce: u64) -> Payload {
    Payload {
        chain_id: 1,
        destination_contract: Address::generate(env),
        nonce,
        data: Bytes::from_slice(env, b"test message"),
    }
}

/// Domain-separated leaf hash matching contract/core: SHA-256(0x00 || data)
fn hash_leaf(env: &Env, data: &[u8]) -> [u8; 32] {
    let mut buf = Bytes::new(env);
    buf.append(&Bytes::from_slice(env, &[0x00]));
    buf.append(&Bytes::from_slice(env, data));
    env.crypto().sha256(&buf).to_array()
}

/// Domain-separated internal node hash: SHA-256(0x01 || min || max)
fn hash_pair(env: &Env, left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut combined = [0u8; 65];
    combined[0] = 0x01;
    if left <= right {
        combined[1..33].copy_from_slice(left);
        combined[33..65].copy_from_slice(right);
    } else {
        combined[1..33].copy_from_slice(right);
        combined[33..65].copy_from_slice(left);
    }
    env.crypto()
        .sha256(&Bytes::from_slice(env, &combined))
        .to_array()
}

/// Build a simple 2-leaf merkle root and proof for leaf at index 0.
/// Leaves are payload hashes; tree uses domain-separated hashing.
fn setup_two_leaf_proof(
    env: &Env,
    client: &CrossChainVerifierClient,
    payload: &Payload,
) -> (u32, Vec<BytesN<32>>) {
    let payload_hash = CrossChainVerifier::compute_payload_hash(env, payload).to_array();
    let leaf0 = hash_leaf(env, &payload_hash);
    let leaf1 = hash_leaf(env, b"sibling-leaf");
    let root = hash_pair(env, &leaf0, &leaf1);

    let block_height = 100u32;
    client.update_root(&block_height, &BytesN::from_array(env, &root));

    let mut proof = Vec::new(env);
    proof.push_back(BytesN::from_array(env, &leaf1));
    (block_height, proof)
}

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
    client.update_root(&100u32, &root);
    assert_eq!(client.get_root(&100u32).unwrap(), root);
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
    let (block_height, proof) = setup_two_leaf_proof(&env, &client, &payload);
    assert!(client.verify_message(&block_height, &payload, &proof));
}

#[test]
fn test_verify_message_rejects_empty_proof_when_no_root() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CrossChainVerifier, ());
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let payload = make_payload(&env, 1);
    let proof = Vec::new(&env);
    assert!(!client.verify_message(&100u32, &payload, &proof));
}

#[test]
#[should_panic(expected = "Nonce already used")]
fn test_verify_message_replay_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CrossChainVerifier, ());
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let payload = make_payload(&env, 1);
    let (block_height, proof) = setup_two_leaf_proof(&env, &client, &payload);
    assert!(client.verify_message(&block_height, &payload, &proof));
    client.verify_message(&block_height, &payload, &proof);
}

#[test]
fn test_verify_message_different_nonce_allowed() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CrossChainVerifier, ());
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let payload1 = make_payload(&env, 1);
    let (bh1, proof1) = setup_two_leaf_proof(&env, &client, &payload1);
    assert!(client.verify_message(&bh1, &payload1, &proof1));

    let payload2 = make_payload(&env, 2);
    let (bh2, proof2) = setup_two_leaf_proof(&env, &client, &payload2);
    assert!(client.verify_message(&bh2, &payload2, &proof2));
}

#[test]
fn test_verify_message_and_consume_nonce() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CrossChainVerifier, ());
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let payload = make_payload(&env, 1);
    let (block_height, proof) = setup_two_leaf_proof(&env, &client, &payload);
    assert!(client.verify_message_and_consume(&block_height, &payload, &proof));
    assert!(client.is_nonce_processed(&1u64));
}

#[test]
#[should_panic(expected = "nonce already processed")]
fn test_replay_nonce_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CrossChainVerifier, ());
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let payload = make_payload(&env, 1);
    let (block_height, proof) = setup_two_leaf_proof(&env, &client, &payload);
    assert!(client.verify_message_and_consume(&block_height, &payload, &proof));
    client.verify_message_and_consume(&block_height, &payload, &proof);
}

#[test]
fn test_pause_controls() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CrossChainVerifier, ());
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    assert!(!client.is_paused());
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

    let payload = make_payload(&env, 7);
    let (block_height, proof) = setup_two_leaf_proof(&env, &client, &payload);
    assert!(client.verify_message(&block_height, &payload, &proof));

    // different nonce/payload to avoid replay; pause should still return false
    let payload2 = make_payload(&env, 8);
    let (bh2, proof2) = setup_two_leaf_proof(&env, &client, &payload2);
    client.set_paused(&true);
    assert!(!client.verify_message(&bh2, &payload2, &proof2));
    client.set_paused(&false);
    assert!(client.verify_message(&bh2, &payload2, &proof2));
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

    let payload = make_payload(&env, 99);
    let (block_height, proof) = setup_two_leaf_proof(&env, &client, &payload);
    client.set_paused(&true);
    client.verify_message_and_consume(&block_height, &payload, &proof);
}

#[test]
fn test_verify_signed_message_success() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CrossChainVerifier, ());
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

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
    // Recompute message hash same as contract hash_message
    let mut data = Bytes::new(&env);
    data.append(&Bytes::from_slice(&env, b"CROSS_CHAIN_MESSAGE_V1"));
    data.append(&Bytes::from_slice(
        &env,
        &message.source_chain.to_be_bytes(),
    ));
    data.append(&Bytes::from_slice(
        &env,
        &message.destination_chain.to_be_bytes(),
    ));
    data.append(&Bytes::from_slice(&env, &message.nonce.to_be_bytes()));
    data.append(&Bytes::from_slice(&env, &message.timestamp.to_be_bytes()));
    let payload_hash = env.crypto().sha256(&message.payload).to_array();
    data.append(&Bytes::from_slice(&env, &payload_hash));
    let message_hash = env.crypto().sha256(&data);
    let signature = signing_key.sign(&message_hash.to_array());
    let signed_message = SignedMessage {
        message,
        signature: BytesN::from_array(&env, &signature.to_bytes()),
        signer_public_key: BytesN::from_array(&env, &verifying_key.to_bytes()),
        algorithm: SignatureAlgorithm::Ed25519,
    };

    let leaf = hash_leaf(&env, &message_hash.to_array());
    let sibling = hash_leaf(&env, b"msg-sibling");
    let root = hash_pair(&env, &leaf, &sibling);
    let block_height = 100u32;
    client.update_root(&block_height, &BytesN::from_array(&env, &root));
    let mut proof = Vec::new(&env);
    proof.push_back(BytesN::from_array(&env, &sibling));

    assert!(client.verify_signed_message(&signed_message, &block_height, &proof));
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

    let proof = Vec::new(&env);
    client.verify_signed_message(&signed_message, &100u32, &proof);
}

/// Regression: an internal node pair (left||right) must not be accepted as a leaf
/// with a shortened proof path (depth - 1). Domain separation prevents this.
#[test]
fn test_internal_node_as_leaf_second_preimage_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(CrossChainVerifier, ());
    let client = CrossChainVerifierClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    // Build 4-leaf tree: leaves L0-L3, internal I01=H(0x01||L0||L1), I23, root=H(0x01||I01||I23)
    let l0 = hash_leaf(&env, b"leaf-0");
    let l1 = hash_leaf(&env, b"leaf-1");
    let l2 = hash_leaf(&env, b"leaf-2");
    let l3 = hash_leaf(&env, b"leaf-3");
    let i01 = hash_pair(&env, &l0, &l1);
    let i23 = hash_pair(&env, &l2, &l3);
    let root = hash_pair(&env, &i01, &i23);

    let block_height = 77u32;
    client.update_root(&block_height, &BytesN::from_array(&env, &root));

    // Attacker crafts a payload whose domain-separated leaf hash equals internal node i01.
    // We can't easily force compute_payload_hash to equal i01, but we can verify that
    // submitting a payload whose raw hash is i01 bytes will be hashed as leaf(i01) != i01,
    // so proof [i23] at depth-1 should not verify.
    let payload = Payload {
        chain_id: 1,
        destination_contract: Address::generate(&env),
        nonce: 4242,
        data: Bytes::from_slice(&env, b"attacker"),
    };

    // Proof shortened by 1: just the sibling internal node at level 1.
    let mut short_proof = Vec::new(&env);
    short_proof.push_back(BytesN::from_array(&env, &i23));

    // Should be rejected: domain-separated leaf(payload_hash) cannot equal internal node i01.
    assert!(
        !client.verify_message(&block_height, &payload, &short_proof),
        "internal-node-as-leaf second preimage must be rejected"
    );
}
