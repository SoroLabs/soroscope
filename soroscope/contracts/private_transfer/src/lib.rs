#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Bytes, BytesN, Env};

#[cfg(test)]
mod test;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    InvalidRoot = 4,
    ProofVerificationFailed = 5,
    NullifierAlreadyUsed = 6,
    InvalidUpdate = 7,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncryptedBalanceUpdate {
    pub commitment: BytesN<32>,
    pub ciphertext: Bytes,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivateTransfer {
    pub old_root: BytesN<32>,
    pub nullifier: BytesN<32>,
    pub sender_update: EncryptedBalanceUpdate,
    pub recipient_update: EncryptedBalanceUpdate,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferReceipt {
    pub previous_root: BytesN<32>,
    pub new_root: BytesN<32>,
    pub nullifier: BytesN<32>,
    pub leaf_index_start: u32,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Verifier,
    VerificationKeyHash,
    CurrentRoot,
    RootHistory(BytesN<32>),
    RootByIndex(u32),
    NextLeafIndex,
    Nullifier(BytesN<32>),
    Ciphertext(BytesN<32>),
}

pub trait Groth16Verifier {
    fn verify(env: Env, verification_key_hash: BytesN<32>, public_input_hash: BytesN<32>, proof: Bytes) -> bool;
}

soroban_sdk::contractclient!(name = "Groth16VerifierClient", trait = Groth16Verifier);

fn zero_root(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[0; 32])
}

fn read_current_root(env: &Env) -> Result<BytesN<32>, Error> {
    env.storage()
        .instance()
        .get(&DataKey::CurrentRoot)
        .ok_or(Error::NotInitialized)
}

fn write_root(env: &Env, root: &BytesN<32>, index: u32) {
    env.storage().instance().set(&DataKey::CurrentRoot, root);
    env.storage().persistent().set(&DataKey::RootHistory(root.clone()), &true);
    env.storage().persistent().set(&DataKey::RootByIndex(index), root);
}

fn root_exists(env: &Env, root: &BytesN<32>) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::RootHistory(root.clone()))
        .unwrap_or(false)
}

fn hash_concat(env: &Env, left: &BytesN<32>, right: &BytesN<32>) -> BytesN<32> {
    let mut data = Bytes::new(env);
    data.extend_from_slice(left);
    data.extend_from_slice(right);
    env.crypto().sha256(&data)
}

fn statement_hash(env: &Env, transfer: &PrivateTransfer, next_root: &BytesN<32>) -> BytesN<32> {
    let sender_cipher_hash = env.crypto().sha256(&transfer.sender_update.ciphertext);
    let recipient_cipher_hash = env.crypto().sha256(&transfer.recipient_update.ciphertext);

    let mut data = Bytes::new(env);
    data.extend_from_slice(&transfer.old_root);
    data.extend_from_slice(&transfer.nullifier);
    data.extend_from_slice(&transfer.sender_update.commitment);
    data.extend_from_slice(&sender_cipher_hash);
    data.extend_from_slice(&transfer.recipient_update.commitment);
    data.extend_from_slice(&recipient_cipher_hash);
    data.extend_from_slice(next_root);
    env.crypto().sha256(&data)
}

fn next_root_for_transfer(env: &Env, current_root: &BytesN<32>, transfer: &PrivateTransfer) -> BytesN<32> {
    let first = hash_concat(env, current_root, &transfer.sender_update.commitment);
    hash_concat(env, &first, &transfer.recipient_update.commitment)
}

fn require_admin(env: &Env) -> Result<Address, Error> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(Error::NotInitialized)?;
    admin.require_auth();
    Ok(admin)
}

#[contract]
pub struct PrivateTransferContract;

#[contractimpl]
impl PrivateTransferContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        verifier: Address,
        verification_key_hash: BytesN<32>,
        initial_root: BytesN<32>,
    ) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Verifier, &verifier);
        env.storage()
            .instance()
            .set(&DataKey::VerificationKeyHash, &verification_key_hash);
        env.storage().instance().set(&DataKey::NextLeafIndex, &0u32);

        let root = if initial_root == zero_root(&env) {
            zero_root(&env)
        } else {
            initial_root
        };
        write_root(&env, &root, 0);
        Ok(())
    }

    pub fn set_verifier(
        env: Env,
        verifier: Address,
        verification_key_hash: BytesN<32>,
    ) -> Result<(), Error> {
        require_admin(&env)?;
        env.storage().instance().set(&DataKey::Verifier, &verifier);
        env.storage()
            .instance()
            .set(&DataKey::VerificationKeyHash, &verification_key_hash);
        Ok(())
    }

    pub fn current_root(env: Env) -> Result<BytesN<32>, Error> {
        read_current_root(&env)
    }

    pub fn contains_root(env: Env, root: BytesN<32>) -> bool {
        root_exists(&env, &root)
    }

    pub fn next_leaf_index(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::NextLeafIndex).unwrap_or(0)
    }

    pub fn is_nullifier_used(env: Env, nullifier: BytesN<32>) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Nullifier(nullifier))
            .unwrap_or(false)
    }

    pub fn encrypted_note(env: Env, commitment: BytesN<32>) -> Option<Bytes> {
        env.storage().persistent().get(&DataKey::Ciphertext(commitment))
    }

    pub fn preview_next_root(env: Env, transfer: PrivateTransfer) -> Result<BytesN<32>, Error> {
        let current = read_current_root(&env)?;
        if transfer.old_root != current && !root_exists(&env, &transfer.old_root) {
            return Err(Error::InvalidRoot);
        }
        Ok(next_root_for_transfer(&env, &transfer.old_root, &transfer))
    }

    pub fn apply_private_transfer(
        env: Env,
        relayer: Address,
        transfer: PrivateTransfer,
        proof: Bytes,
    ) -> Result<TransferReceipt, Error> {
        relayer.require_auth();

        if transfer.sender_update.commitment == transfer.recipient_update.commitment {
            return Err(Error::InvalidUpdate);
        }
        if Self::is_nullifier_used(env.clone(), transfer.nullifier.clone()) {
            return Err(Error::NullifierAlreadyUsed);
        }
        if !root_exists(&env, &transfer.old_root) {
            return Err(Error::InvalidRoot);
        }

        let current_root = read_current_root(&env)?;
        let next_root = next_root_for_transfer(&env, &transfer.old_root, &transfer);
        let public_input_hash = statement_hash(&env, &transfer, &next_root);

        let verifier: Address = env
            .storage()
            .instance()
            .get(&DataKey::Verifier)
            .ok_or(Error::NotInitialized)?;
        let verification_key_hash: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::VerificationKeyHash)
            .ok_or(Error::NotInitialized)?;
        let valid = Groth16VerifierClient::new(&env, &verifier)
            .verify(&verification_key_hash, &public_input_hash, &proof);
        if !valid {
            return Err(Error::ProofVerificationFailed);
        }

        let leaf_index_start = Self::next_leaf_index(env.clone());
        env.storage()
            .persistent()
            .set(&DataKey::Nullifier(transfer.nullifier.clone()), &true);
        env.storage().persistent().set(
            &DataKey::Ciphertext(transfer.sender_update.commitment.clone()),
            &transfer.sender_update.ciphertext,
        );
        env.storage().persistent().set(
            &DataKey::Ciphertext(transfer.recipient_update.commitment.clone()),
            &transfer.recipient_update.ciphertext,
        );

        env.storage()
            .instance()
            .set(&DataKey::NextLeafIndex, &leaf_index_start.saturating_add(2));
        write_root(&env, &next_root, leaf_index_start.saturating_add(2));

        Ok(TransferReceipt {
            previous_root: current_root,
            new_root: next_root,
            nullifier: transfer.nullifier,
            leaf_index_start,
        })
    }
}
