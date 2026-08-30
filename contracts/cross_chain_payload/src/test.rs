#![cfg(test)]

use crate::*;
use crate::chain_info::BridgeEndpoint;
use crate::payload::{EncodedPayload, PayloadBatch, PayloadRoute};
use crate::signatures::{RecoveryKey, SignatureScheme};
use crate::verification::{VerificationResult, VerificationStatus};
use crate::{ChainInfo, CrossChainError, CrossChainPayload, PayloadMetadata};
use soroban_sdk::{Bytes, BytesN, Env, String, Symbol, Vec};

#[test]
fn test_chain_info_creation() {
    let bridge_contract = BytesN::from_array(&[0u8; 32]);
    let e = Env::default();

    let bridge_contract = BytesN::from_array(&e, &[0u8; 32]);

    let chain_info = ChainInfo {
        chain_id: 1,
        chain_name: String::from_str(&e, "stellar"),
        network_version: 1,
        bridge_contract,
        consensus_round: 100,
        is_active: true,
    };

    assert_eq!(chain_info.chain_id, 1);
    assert_eq!(chain_info.consensus_round, 100);
    assert!(chain_info.is_active);
}

#[test]
fn test_bridge_endpoint_creation() {
    let bridge_contract = BytesN::from_array(&[0u8; 32]);
    let e = Env::default();
    let bridge_contract = BytesN::from_array(&e, &[0u8; 32]);

    let source_chain = ChainInfo {
        chain_id: 1,
        chain_name: String::from_str(&e, "chain-1"),
        network_version: 1,
        bridge_contract: bridge_contract.clone(),
        consensus_round: 100,
        is_active: true,
    };

    let dest_chain = ChainInfo {
        chain_id: 2,
        chain_name: String::from_str(&e, "chain-2"),
        network_version: 1,
        bridge_contract,
        consensus_round: 100,
        is_active: true,
    };

    let endpoint = BridgeEndpoint {
        source_chain,
        destination_chain: dest_chain,
        fee_percentage: 100, // 1%
        min_liquidity: 1000000,
        is_enabled: true,
    };

    assert_eq!(endpoint.fee_percentage, 100);
    assert!(endpoint.is_enabled);
}

#[test]
fn test_cross_chain_payload_creation() {
    let payload_id = BytesN::from_array(&[1u8; 32]);
    let nonce = BytesN::from_array(&[2u8; 32]);
    let payload_hash = BytesN::from_array(&[3u8; 32]);
    let e = Env::default();
    let payload_id = BytesN::from_array(&e, &[1u8; 32]);
    let nonce = BytesN::from_array(&e, &[2u8; 32]);
    let payload_hash = BytesN::from_array(&e, &[3u8; 32]);

    let metadata = PayloadMetadata {
        version: 1,
        timestamp: 1000000,
        sequence: 1,
        expiration_height: 10000,
        nonce,
    };

    let sender = Bytes::new(&soroban_sdk::Env::default());
    let recipient = Bytes::new(&soroban_sdk::Env::default());
    let data = Bytes::new(&soroban_sdk::Env::default());
    let sender = Bytes::new(&e);
    let recipient = Bytes::new(&e);
    let data = Bytes::new(&e);

    let payload = CrossChainPayload {
        payload_id,
        source_chain_id: 1,
        destination_chain_id: 2,
        sender,
        recipient,
        data,
        operation: Symbol::new(&e, "transfer"),
        metadata,
        payload_hash,
        gas_limit: 1000000,
    };

    assert_eq!(payload.source_chain_id, 1);
    assert_eq!(payload.destination_chain_id, 2);
    assert_eq!(payload.gas_limit, 1000000);
}

#[test]
fn test_payload_batch_creation() {
    let batch_id = BytesN::from_array(&[4u8; 32]);
    let merkle_root = BytesN::from_array(&[5u8; 32]);
    let e = Env::default();
    let batch_id = BytesN::from_array(&e, &[4u8; 32]);
    let merkle_root = BytesN::from_array(&e, &[5u8; 32]);

    let batch = PayloadBatch {
        batch_id,
        source_chain_id: 1,
        payload_count: 10,
        merkle_root,
        batch_timestamp: 1000000,
        batch_ttl_seconds: 3600,
    };

    assert_eq!(batch.payload_count, 10);
    assert_eq!(batch.batch_ttl_seconds, 3600);
}

#[test]
fn test_verification_result_creation() {
    let error_msg = String::from_small_str("test error");
    let e = Env::default();
    let error_msg = String::from_str(&e, "test error");

    let result = VerificationResult {
        status: VerificationStatus::Verified,
        signatures_verified: 5,
        signatures_required: 5,
        error_message: error_msg,
        verified_at_height: 12345,
        has_rejections: false,
        rejection_count: 0,
    };

    assert_eq!(result.status, VerificationStatus::Verified);
    assert_eq!(result.signatures_verified, 5);
    assert!(!result.has_rejections);
}

#[test]
fn test_signature_schemes() {
    let schemes = vec![
        SignatureScheme::Ed25519,
        SignatureScheme::Secp256k1,
        SignatureScheme::BLS12381,
        SignatureScheme::ECDSA,
    ];

    assert_eq!(schemes.len(), 4);
}

#[test]
fn test_error_codes() {
    assert_eq!(CrossChainError::InvalidPayloadHash.as_u32(), 1);
    assert_eq!(CrossChainError::InvalidSignature.as_u32(), 2);
    assert_eq!(CrossChainError::PayloadExpired.as_u32(), 5);
    assert_eq!(CrossChainError::ReplayAttack.as_u32(), 6);
    assert_eq!(CrossChainError::Unknown.as_u32(), 255);
}

#[test]
fn test_verification_status_variants() {
    let statuses = vec![
        VerificationStatus::Pending,
        VerificationStatus::Verified,
        VerificationStatus::Failed,
        VerificationStatus::Expired,
        VerificationStatus::Cancelled,
    ];

    assert_eq!(statuses.len(), 5);
    assert_eq!(statuses[1], VerificationStatus::Verified);
}

#[test]
fn test_payload_route_creation() {
    let e = Env::default();
    let route = PayloadRoute {
        from_chain: 1,
        to_chain: 3,
        route_path: Vec::new(&e),
        priority: 100,
        is_critical: true,
    };

    assert_eq!(route.from_chain, 1);
    assert_eq!(route.to_chain, 3);
    assert!(route.is_critical);
}

#[test]
fn test_encoded_payload_creation() {
    let encoded_data = Bytes::new(&soroban_sdk::Env::default());
    let encoding_scheme = String::from_small_str("borsh");
    let compression_type = String::from_small_str("gzip");
    let e = Env::default();
    let encoded_data = Bytes::new(&e);
    let encoding_scheme = String::from_str(&e, "borsh");
    let compression_type = String::from_str(&e, "gzip");

    let encoded = EncodedPayload {
        encoded_data,
        encoding_scheme,
        compression_type,
        original_size: 1000,
        compressed_size: 600,
    };

    assert_eq!(encoded.original_size, 1000);
    assert_eq!(encoded.compressed_size, 600);
}

#[test]
fn test_recovery_key_creation() {
    let compressed_key = BytesN::from_array(&[6u8; 33]);
    let e = Env::default();
    let compressed_key = BytesN::from_array(&e, &[6u8; 33]);

    let key = RecoveryKey {
        compressed_key,
        key_type: SignatureScheme::Secp256k1,
        chain_id: 1,
        is_active: true,
        activation_height: 1000,
        deactivation_height: u64::MAX,
    };

    assert!(key.is_active);
    assert_eq!(key.chain_id, 1);
}
