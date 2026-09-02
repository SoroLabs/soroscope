#![cfg(test)]

extern crate std;

use super::*;
use soroban_sdk::{
    contract, contractimpl, contracttype,
    testutils::{Address as _, Events},
    Address, Bytes, BytesN, Env, TryIntoVal,
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
    pub fn configure(
        env: Env,
        accept: bool,
        verification_key_hash: BytesN<32>,
        public_input_hash: BytesN<32>,
    ) {
        env.storage()
            .instance()
            .set(&VerifierDataKey::Accept, &accept);
        env.storage()
            .instance()
            .set(&VerifierDataKey::ExpectedVkHash, &verification_key_hash);
        env.storage()
            .instance()
            .set(&VerifierDataKey::ExpectedInputHash, &public_input_hash);
    }

    pub fn verify(
        env: Env,
        verification_key_hash: BytesN<32>,
        public_input_hash: BytesN<32>,
        _proof: Bytes,
    ) -> bool {
        let accept: bool = env
            .storage()
            .instance()
            .get(&VerifierDataKey::Accept)
            .unwrap_or(false);
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

fn append32(data: &mut Bytes, value: &BytesN<32>) {
    data.extend_from_slice(&value.to_array());
}

fn statement_for(env: &Env, transfer: &PrivateTransfer, next_root: &BytesN<32>) -> BytesN<32> {
    let sender_hash: BytesN<32> = env
        .crypto()
        .sha256(&transfer.sender_update.ciphertext)
        .into();
    let recipient_hash: BytesN<32> = env
        .crypto()
        .sha256(&transfer.recipient_update.ciphertext)
        .into();
    let mut data = Bytes::new(env);
    append32(&mut data, &transfer.old_root);
    append32(&mut data, &transfer.nullifier);
    append32(&mut data, &transfer.sender_update.commitment);
    append32(&mut data, &sender_hash);
    append32(&mut data, &transfer.recipient_update.commitment);
    append32(&mut data, &recipient_hash);
    append32(&mut data, next_root);
    env.crypto().sha256(&data).into()
}

fn setup() -> (Env, Address, Address, Address, BytesN<32>) {
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

fn configure_accepting_verifier(
    env: &Env,
    contract_id: &Address,
    verifier_id: &Address,
    vk_hash: &BytesN<32>,
    transfer: &PrivateTransfer,
) {
    let client = PrivateTransferContractClient::new(env, contract_id);
    let verifier = MockGroth16VerifierClient::new(env, verifier_id);
    let expected_next_root = client.preview_next_root(transfer);
    verifier.configure(
        &true,
        vk_hash,
        &statement_for(env, transfer, &expected_next_root),
    );
}

#[test]
fn applies_verified_private_transfer_and_updates_root_history() {
    let (env, contract_id, verifier_id, relayer, vk_hash) = setup();
    let client = PrivateTransferContractClient::new(&env, &contract_id);

    let current_root = client.current_root();
    let transfer = make_transfer(&env, current_root.clone());
    let expected_next_root = client.preview_next_root(&transfer);
    configure_accepting_verifier(&env, &contract_id, &verifier_id, &vk_hash, &transfer);

    let receipt =
        client.apply_private_transfer(&relayer, &transfer, &Bytes::from_slice(&env, &[42]));

    assert_eq!(receipt.new_root, expected_next_root);
    assert!(client.contains_root(&expected_next_root));
    assert!(client.is_nullifier_used(&transfer.nullifier));
    assert_eq!(
        client
            .encrypted_note(&transfer.sender_update.commitment)
            .unwrap(),
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

    let err =
        client.try_apply_private_transfer(&relayer, &transfer, &Bytes::from_slice(&env, &[1]));
    assert_eq!(err, Err(Ok(Error::ProofVerificationFailed)));
}

#[test]
fn rejects_unknown_roots_and_nullifier_reuse() {
    let (env, contract_id, verifier_id, relayer, vk_hash) = setup();
    let client = PrivateTransferContractClient::new(&env, &contract_id);

    let unknown_transfer = make_transfer(&env, bytes32(&env, 88));
    let err = client.try_apply_private_transfer(
        &relayer,
        &unknown_transfer,
        &Bytes::from_slice(&env, &[1]),
    );
    assert_eq!(err, Err(Ok(Error::InvalidRoot)));

    let current_root = client.current_root();
    let transfer = make_transfer(&env, current_root.clone());
    configure_accepting_verifier(&env, &contract_id, &verifier_id, &vk_hash, &transfer);

    client.apply_private_transfer(&relayer, &transfer, &Bytes::from_slice(&env, &[5]));
    let err =
        client.try_apply_private_transfer(&relayer, &transfer, &Bytes::from_slice(&env, &[5]));
    assert_eq!(err, Err(Ok(Error::NullifierAlreadyUsed)));
}

#[test]
fn verifies_stealth_address_generation_commitment() {
    let (env, contract_id, _, _, _) = setup();
    let client = PrivateTransferContractClient::new(&env, &contract_id);

    let proof = StealthAddressProof {
        ephemeral_pubkey: bytes32(&env, 11),
        view_tag: bytes32(&env, 12),
        stealth_pubkey: bytes32(&env, 13),
    };
    let commitment = client.compute_stealth_commitment(&proof);
    assert!(client.verify_stealth_commitment(&proof, &commitment));

    let mismatched = bytes32(&env, 99);
    assert!(!client.verify_stealth_commitment(&proof, &mismatched));

    let zero = bytes32(&env, 0);
    let zero_proof = StealthAddressProof {
        ephemeral_pubkey: zero.clone(),
        view_tag: bytes32(&env, 12),
        stealth_pubkey: bytes32(&env, 13),
    };
    assert_eq!(
        client.try_verify_stealth_commitment(&zero_proof, &commitment),
        Err(Ok(Error::InvalidCommitment))
    );
    assert_eq!(
        client.try_verify_stealth_commitment(&proof, &zero),
        Err(Ok(Error::InvalidCommitment))
    );
}

#[test]
fn registers_stealth_meta_and_one_time_deposit_keys() {
    let (env, contract_id, verifier_id, relayer, vk_hash) = setup();
    let client = PrivateTransferContractClient::new(&env, &contract_id);
    let receiver = Address::generate(&env);

    client.register_stealth_meta(&receiver, &bytes32(&env, 21), &bytes32(&env, 22));
    let meta = client.stealth_meta(&receiver).unwrap();
    assert_eq!(meta.spend_pubkey, bytes32(&env, 21));
    assert_eq!(meta.view_pubkey, bytes32(&env, 22));

    let deposit_commitment = bytes32(&env, 7);
    client.register_deposit_key(&receiver, &deposit_commitment);
    assert_eq!(
        client.deposit_key_owner(&deposit_commitment),
        Some(receiver.clone())
    );
    assert!(!client.is_deposit_key_used(&deposit_commitment));

    let duplicate = client.try_register_deposit_key(&receiver, &deposit_commitment);
    assert_eq!(duplicate, Err(Ok(Error::DepositKeyExists)));

    let current_root = client.current_root();
    let transfer = make_transfer(&env, current_root);
    configure_accepting_verifier(&env, &contract_id, &verifier_id, &vk_hash, &transfer);
    client.apply_private_transfer(&relayer, &transfer, &Bytes::from_slice(&env, &[3]));
    assert!(client.is_deposit_key_used(&deposit_commitment));
}

#[test]
fn rejects_reuse_of_disposable_deposit_key() {
    let (env, contract_id, verifier_id, relayer, vk_hash) = setup();
    let client = PrivateTransferContractClient::new(&env, &contract_id);
    let receiver = Address::generate(&env);
    let deposit_commitment = bytes32(&env, 7);
    client.register_deposit_key(&receiver, &deposit_commitment);

    let first = make_transfer(&env, client.current_root());
    configure_accepting_verifier(&env, &contract_id, &verifier_id, &vk_hash, &first);
    client.apply_private_transfer(&relayer, &first, &Bytes::from_slice(&env, &[1]));

    let mut second = make_transfer(&env, client.current_root());
    second.nullifier = bytes32(&env, 10);
    second.sender_update.commitment = bytes32(&env, 5);
    configure_accepting_verifier(&env, &contract_id, &verifier_id, &vk_hash, &second);
    let err = client.try_apply_private_transfer(&relayer, &second, &Bytes::from_slice(&env, &[2]));
    assert_eq!(err, Err(Ok(Error::DepositKeyAlreadyUsed)));
}

#[test]
fn rejects_zero_commitments() {
    let (env, contract_id, _, relayer, _) = setup();
    let client = PrivateTransferContractClient::new(&env, &contract_id);
    let mut transfer = make_transfer(&env, client.current_root());
    transfer.recipient_update.commitment = bytes32(&env, 0);
    let err =
        client.try_apply_private_transfer(&relayer, &transfer, &Bytes::from_slice(&env, &[1]));
    assert_eq!(err, Err(Ok(Error::InvalidCommitment)));
}

#[test]
fn emits_obfuscated_transfer_event_without_addresses() {
    let (env, contract_id, verifier_id, relayer, vk_hash) = setup();
    let client = PrivateTransferContractClient::new(&env, &contract_id);

    let transfer = make_transfer(&env, client.current_root());
    configure_accepting_verifier(&env, &contract_id, &verifier_id, &vk_hash, &transfer);
    let receipt =
        client.apply_private_transfer(&relayer, &transfer, &Bytes::from_slice(&env, &[9]));

    let mut expected_id_bytes = Bytes::new(&env);
    append32(&mut expected_id_bytes, &transfer.nullifier);
    append32(&mut expected_id_bytes, &transfer.sender_update.commitment);
    append32(
        &mut expected_id_bytes,
        &transfer.recipient_update.commitment,
    );
    let expected_id: BytesN<32> = env.crypto().sha256(&expected_id_bytes).into();

    let events = env.events().all();
    let matching: std::vec::Vec<_> = events
        .iter()
        .filter(|(id, topics, _)| {
            if id != &contract_id || topics.len() != 2 {
                return false;
            }
            let name: Result<soroban_sdk::Symbol, _> = topics.get(0).unwrap().try_into_val(&env);
            let hid: Result<BytesN<32>, _> = topics.get(1).unwrap().try_into_val(&env);
            name.ok() == Some(symbol_short!("xfer")) && hid.ok() == Some(expected_id.clone())
        })
        .collect();
    assert_eq!(matching.len(), 1);

    let (_, _, data) = &matching[0];
    let payload: (BytesN<32>, u32) = data.clone().try_into_val(&env).unwrap();
    assert_eq!(payload.0, receipt.new_root);
    assert_eq!(payload.1, receipt.leaf_index_start);
}
