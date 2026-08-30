#![cfg(test)]

extern crate std;

use super::*;
use soroban_sdk::{
    contract, contractimpl, contracttype,
    testutils::Address as _,
    Address, Bytes, BytesN, Env,
};

#[contract]
struct MockGroth16Verifier;

#[contracttype]
#[derive(Clone)]
enum VerifierDataKey {
    Accept,
    ExpectedVkHash,
    ExpectedInputHash,
}

#[contractimpl]
impl MockGroth16Verifier {
    pub fn configure(env: Env, accept: bool, verification_key_hash: BytesN<32>, public_input_hash: BytesN<32>) {
        env.storage().instance().set(&VerifierDataKey::Accept, &accept);
        env.storage()
            .instance()
            .set(&VerifierDataKey::ExpectedVkHash, &verification_key_hash);
        env.storage()
            .instance()
            .set(&VerifierDataKey::ExpectedInputHash, &public_input_hash);
    }

    pub fn verify(env: Env, verification_key_hash: BytesN<32>, public_input_hash: BytesN<32>, _proof: Bytes) -> bool {
        let accept: bool = env.storage().instance().get(&VerifierDataKey::Accept).unwrap_or(false);
        let expected_vk: BytesN<32> = env
            .storage()
            .instance()
            .get(&VerifierDataKey::ExpectedVkHash)
            .unwrap();
        let expected_input: BytesN<32> = env
            .storage()
            .instance()
            .get(&VerifierDataKey::ExpectedInputHash)
            .unwrap();

        accept && expected_vk == verification_key_hash && expected_input == public_input_hash
    }
}

fn bytes32(env: &Env, seed: u8) -> BytesN<32> {
    BytesN::from_array(env, &[seed; 32])
}

fn make_transfer(env: &Env, root: BytesN<32>) -> PrivateTransfer {
    PrivateTransfer {
        old_root: root,
        nullifier: bytes32(env, 9),
        sender_update: EncryptedBalanceUpdate {
            commitment: bytes32(env, 4),
            ciphertext: Bytes::from_slice(env, &[1, 2, 3]),
        },
        recipient_update: EncryptedBalanceUpdate {
            commitment: bytes32(env, 7),
            ciphertext: Bytes::from_slice(env, &[7, 8, 9]),
        },
    }
}

fn setup() -> (
    Env,
    Address,
    Address,
    Address,
    BytesN<32>,
) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let relayer = Address::generate(&env);
    let verifier_id = env.register(MockGroth16Verifier, ());
    let contract_id = env.register(PrivateTransferContract, ());
    let client = PrivateTransferContractClient::new(&env, &contract_id);
    let initial_root = bytes32(&env, 1);
    let vk_hash = bytes32(&env, 2);

    client.initialize(&admin, &verifier_id, &vk_hash, &initial_root);
    (env, contract_id, verifier_id, relayer, vk_hash)
}

#[test]
fn applies_verified_private_transfer_and_updates_root_history() {
    let (env, contract_id, verifier_id, relayer, vk_hash) = setup();
    let client = PrivateTransferContractClient::new(&env, &contract_id);
    let verifier = MockGroth16VerifierClient::new(&env, &verifier_id);

    let current_root = client.current_root();
    let transfer = make_transfer(&env, current_root.clone());
    let expected_next_root = client.preview_next_root(&transfer);
    let statement_hash = env.crypto().sha256(&{
        let mut data = Bytes::new(&env);
        let sender_hash = env.crypto().sha256(&transfer.sender_update.ciphertext);
        let recipient_hash = env.crypto().sha256(&transfer.recipient_update.ciphertext);
        data.extend_from_slice(&transfer.old_root);
        data.extend_from_slice(&transfer.nullifier);
        data.extend_from_slice(&transfer.sender_update.commitment);
        data.extend_from_slice(&sender_hash);
        data.extend_from_slice(&transfer.recipient_update.commitment);
        data.extend_from_slice(&recipient_hash);
        data.extend_from_slice(&expected_next_root);
        data
    });
    verifier.configure(&true, &vk_hash, &statement_hash);

    let receipt = client.apply_private_transfer(&relayer, &transfer, &Bytes::from_slice(&env, &[42]));

    assert_eq!(receipt.new_root, expected_next_root);
    assert!(client.contains_root(&expected_next_root));
    assert!(client.is_nullifier_used(&transfer.nullifier));
    assert_eq!(
        client.encrypted_note(&transfer.sender_update.commitment).unwrap(),
        transfer.sender_update.ciphertext
    );
    assert_eq!(client.next_leaf_index(), 2);
}

#[test]
fn rejects_transfer_when_verifier_fails() {
    let (env, contract_id, verifier_id, relayer, vk_hash) = setup();
    let client = PrivateTransferContractClient::new(&env, &contract_id);
    let verifier = MockGroth16VerifierClient::new(&env, &verifier_id);

    let current_root = client.current_root();
    let transfer = make_transfer(&env, current_root);
    verifier.configure(&false, &vk_hash, &bytes32(&env, 33));

    let err = client.try_apply_private_transfer(&relayer, &transfer, &Bytes::from_slice(&env, &[1]));
    assert_eq!(err, Err(Ok(Error::ProofVerificationFailed)));
}

#[test]
fn rejects_unknown_roots_and_nullifier_reuse() {
    let (env, contract_id, verifier_id, relayer, vk_hash) = setup();
    let client = PrivateTransferContractClient::new(&env, &contract_id);
    let verifier = MockGroth16VerifierClient::new(&env, &verifier_id);

    let unknown_transfer = make_transfer(&env, bytes32(&env, 88));
    let err = client.try_apply_private_transfer(&relayer, &unknown_transfer, &Bytes::from_slice(&env, &[1]));
    assert_eq!(err, Err(Ok(Error::InvalidRoot)));

    let current_root = client.current_root();
    let transfer = make_transfer(&env, current_root.clone());
    let expected_next_root = client.preview_next_root(&transfer);
    let statement_hash = env.crypto().sha256(&{
        let mut data = Bytes::new(&env);
        let sender_hash = env.crypto().sha256(&transfer.sender_update.ciphertext);
        let recipient_hash = env.crypto().sha256(&transfer.recipient_update.ciphertext);
        data.extend_from_slice(&transfer.old_root);
        data.extend_from_slice(&transfer.nullifier);
        data.extend_from_slice(&transfer.sender_update.commitment);
        data.extend_from_slice(&sender_hash);
        data.extend_from_slice(&transfer.recipient_update.commitment);
        data.extend_from_slice(&recipient_hash);
        data.extend_from_slice(&expected_next_root);
        data
    });
    verifier.configure(&true, &vk_hash, &statement_hash);

    client.apply_private_transfer(&relayer, &transfer, &Bytes::from_slice(&env, &[5]));
    let err = client.try_apply_private_transfer(&relayer, &transfer, &Bytes::from_slice(&env, &[5]));
    assert_eq!(err, Err(Ok(Error::NullifierAlreadyUsed)));
}
