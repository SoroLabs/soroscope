#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, xdr::ToXdr, Address, Bytes, BytesN, Env, Vec,
};
use soroban_sdk::{contract, contractimpl, contracttype, Address, Bytes, BytesN, Env, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignatureAlgorithm {
    Ed25519,
    Secp256k1,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossChainMessage {
    pub source_chain: u32,
    pub destination_chain: u32,
    pub nonce: u64,
    pub payload: Bytes,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedMessage {
    pub message: CrossChainMessage,
    pub signature: BytesN<64>,
    pub signer_public_key: BytesN<32>,
    pub algorithm: SignatureAlgorithm,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    StateRoot(u32),
    NonceUsed(u64),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Payload {
    pub chain_id: u32,
    pub destination_contract: Address,
    pub nonce: u64,
    pub data: Bytes,
    AuthorizedSigners,
    SignerAlgorithm(Bytes),
    SignerCount,
    ProcessedMessages(BytesN<32>),
    ProcessedNonce(u64),
    /// Whether verification is paused (PauseType::VERIFY)
    VerifyPaused,
}

#[contract]
pub struct CrossChainVerifier;

#[contractimpl]
impl CrossChainVerifier {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::SignerCount, &0u32);
    }

    pub fn update_root(env: Env, block_height: u32, new_root: BytesN<32>) {
    /// Admin-only: pause or unpause all verification operations.
    pub fn set_paused(env: Env, paused: bool) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        env.storage().instance().set(&DataKey::VerifyPaused, &paused);
    }

    /// Returns true if verification is currently paused.
    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::VerifyPaused)
            .unwrap_or(false)
    }

    pub fn update_root(env: Env, block_height: u32, new_root: BytesN<32>) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        env.storage().persistent().set(&DataKey::StateRoot(block_height), &new_root);
    }

    pub fn get_root(env: Env, block_height: u32) -> Option<BytesN<32>> {
        env.storage().persistent().get(&DataKey::StateRoot(block_height))
    }

    /// Verifies a cross-chain payload using a Merkle proof.
    /// Includes domain separation via chain_id and destination_contract,
    /// plus sequential nonce tracking to prevent replay attacks.
    }

    /// Add an authorized signer for cross-chain message verification.
    /// Only the admin can add signers.
    /// 
    /// This function allows the admin to register new signers that are authorized to sign
    /// cross-chain messages. Each signer is associated with a specific signature algorithm
    /// (Ed25519 or Secp256k1).
    /// 
    /// **Performance:** O(1) - Constant time indexed storage lookup
    /// 
    /// # Parameters
    /// * `public_key`: The public key of the signer (32 bytes for Ed25519, 33-65 bytes for Secp256k1)
    /// * `algorithm`: The signature algorithm used by this signer (Ed25519 or Secp256k1)
    /// 
    /// # Panics
    /// - If the caller is not the admin
    /// - If the signer is already authorized
    /// 
    /// # Events
    /// Emits a "signer_added" event on successful addition
    pub fn add_authorized_signer(env: Env, public_key: Bytes, algorithm: SignatureAlgorithm) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();

        if env.storage().persistent().has(&DataKey::SignerAlgorithm(public_key.clone())) {
            panic!("Signer already authorized");
        }

        env.storage().persistent().set(&DataKey::SignerAlgorithm(public_key.clone()), &algorithm);

        let count: u32 = env.storage().instance().get(&DataKey::SignerCount).unwrap_or(0);
        env.storage().instance().set(&DataKey::SignerCount, &(count + 1));
    }

    pub fn remove_authorized_signer(env: Env, public_key: Bytes) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();

        if !env.storage().persistent().has(&DataKey::SignerAlgorithm(public_key.clone())) {
            panic!("Signer not found");
        }

        env.storage().persistent().remove(&DataKey::SignerAlgorithm(public_key));

        let count: u32 = env.storage().instance().get(&DataKey::SignerCount).unwrap_or(0);
        if count > 0 {
            env.storage().instance().set(&DataKey::SignerCount, &(count - 1));
        }
    }

    pub fn get_authorized_signers(_env: Env) -> Vec<(Bytes, SignatureAlgorithm)> {
        Vec::new(&_env)
    }

    pub fn get_signer_count(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::SignerCount).unwrap_or(0)
    }

    pub fn verify_signed_message(
        env: Env,
        signed_message: SignedMessage,
        block_height: u32,
        proof: Vec<BytesN<32>>,
        proof_flags: Vec<bool>,
    ) -> bool {
        if Self::is_paused(env.clone()) {
            panic!("verification paused");
        }

        if !Self::verify_signature(&env, &signed_message) {
            return false;
        }

        let message_hash = Self::hash_message(&env, &signed_message.message);
        let processed_key = DataKey::ProcessedMessages(message_hash.clone());
        if env.storage().persistent().has(&processed_key) {
            return false;
        }

        if !Self::verify_merkle_proof(&env, &message_hash, &block_height, &proof, &proof_flags) {
            return false;
        }

        env.storage().persistent().set(&processed_key, &true);
        true
    }

    pub fn verify_message(
        env: Env,
        block_height: u32,
        payload: Payload,
        proof: Vec<BytesN<32>>,
        proof_flags: Vec<bool>,
    ) -> bool {
        if Self::is_paused(env.clone()) {
            return false;
        }
        Self::verify_merkle_proof(&env, &leaf, &block_height, &proof, &proof_flags)
    }

    pub fn verify_message_and_consume(
        env: Env,
        block_height: u32,
        nonce: u64,
        leaf: BytesN<32>,
        proof: Vec<BytesN<32>>,
        proof_flags: Vec<bool>,
    ) -> bool {
        if Self::is_paused(env.clone()) {
            panic!("verification paused");
        }

        if Self::is_nonce_processed(env.clone(), nonce) {
            panic!("nonce already processed");
        }

        let valid = Self::verify_message(env.clone(), block_height, leaf, proof, proof_flags);
        if !valid {
            return false;
        }

        env.storage().persistent().set(&DataKey::ProcessedNonce(nonce), &true);
        true
    }

    pub fn is_nonce_processed(env: Env, nonce: u64) -> bool {
        env.storage().persistent().get(&DataKey::ProcessedNonce(nonce)).unwrap_or(false)
    }

    fn verify_signature(env: &Env, signed_message: &SignedMessage) -> bool {
        let signer_key_bytes = Bytes::from_array(&env, &signed_message.signer_public_key.to_array());
        let signer_algorithm: Option<SignatureAlgorithm> = env
            .storage()
            .persistent()
            .get(&DataKey::SignerAlgorithm(signer_key_bytes));

        let signer_algorithm = match signer_algorithm {
            Some(algo) => algo,
            None => return false,
        };

        let message_hash = Self::hash_message(env, &signed_message.message);

        match signer_algorithm {
            SignatureAlgorithm::Ed25519 => {
                let message_bytes = Bytes::from_array(env, &message_hash.to_array());
                let _ = env.crypto().ed25519_verify(&signed_message.signer_public_key, &message_bytes, &signed_message.signature);
                true
            }
            SignatureAlgorithm::Secp256k1 => false,
        }
    }

    fn hash_message(env: &Env, message: &CrossChainMessage) -> BytesN<32> {
        let mut data = Bytes::new(env);
        data.append(&Bytes::from_slice(env, b"CROSS_CHAIN_MESSAGE_V1"));
        data.append(&Bytes::from_slice(env, &message.source_chain.to_be_bytes()));
        data.append(&Bytes::from_slice(env, &message.destination_chain.to_be_bytes()));
        data.append(&Bytes::from_slice(env, &message.nonce.to_be_bytes()));
        data.append(&Bytes::from_slice(env, &message.timestamp.to_be_bytes()));

        let payload_hash = env.crypto().sha256(&message.payload).to_array();
        data.append(&Bytes::from_slice(env, &payload_hash));

        let digest = env.crypto().sha256(&data).to_array();
        BytesN::from_array(env, &digest)
    }

    fn verify_merkle_proof(
        env: &Env,
        leaf: &BytesN<32>,
        block_height: &u32,
        proof: &Vec<BytesN<32>>,
        proof_flags: &Vec<bool>,
    ) -> bool {
        let expected_root: BytesN<32> = match env.storage().persistent().get(&DataKey::StateRoot(*block_height)) {
            Some(root) => root,
            None => return false,
        };

        if proof.len() != proof_flags.len() {
            return false;
        }

        // Replay protection: nonce must not have been used before
        if env
            .storage()
            .persistent()
            .has(&DataKey::NonceUsed(payload.nonce))
        {
            panic!("Nonce already used");
        }
        env.storage()
            .persistent()
            .set(&DataKey::NonceUsed(payload.nonce), &true);

        // Compute domain-separated leaf hash
        let leaf = Self::compute_payload_hash(&env, &payload);
        let mut current_hash = leaf.to_array();
        let mut i = 0;
        while i < proof.len() {
            let sibling = proof.get(i).unwrap().to_array();
            let is_left_sibling = proof_flags.get(i).unwrap();

            let mut combined = [0u8; 64];
            if is_left_sibling {
                combined[0..32].copy_from_slice(&sibling);
                combined[32..64].copy_from_slice(&current_hash);
            } else {
                combined[0..32].copy_from_slice(&current_hash);
                combined[32..64].copy_from_slice(&sibling);
            }

            let combined_bytes = Bytes::from_slice(env, &combined);
            current_hash = env.crypto().sha256(&combined_bytes).to_array();
            i += 1;
        }

        let computed_root = BytesN::from_array(env, &current_hash);
        computed_root == expected_root
    }
}

/// Helper methods outside #[contractimpl] so they can accept reference parameters.
impl CrossChainVerifier {
    /// Computes a domain-separated payload hash:
    ///   sha256(chain_id || destination_contract || nonce || data)
    /// This binds every message to a specific source chain, destination contract,
    /// and unique nonce, preventing cross-chain replay attacks.
    pub fn compute_payload_hash(env: &Env, payload: &Payload) -> BytesN<32> {
        let mut buf = Bytes::new(env);
        buf.append(&Bytes::from_slice(env, &payload.chain_id.to_be_bytes()));
        buf.append(&payload.destination_contract.clone().to_xdr(env));
        buf.append(&Bytes::from_slice(env, &payload.nonce.to_be_bytes()));
        buf.append(&payload.data.clone());
        env.crypto().sha256(&buf).into()
    }
}

mod test;
