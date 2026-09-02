#![no_std]

use soroban_sdk::{
    contract, contractclient, contracterror, contractimpl, contracttype, symbol_short, Address,
    Bytes, BytesN, Env,
};

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
    InvalidCommitment = 8,
    DepositKeyAlreadyUsed = 9,
    DepositKeyExists = 10,
}

/// Stealth meta-address published by a receiver (spend + view public keys).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StealthMetaAddress {
    pub spend_pubkey: BytesN<32>,
    pub view_pubkey: BytesN<32>,
}

/// Inputs used to derive and verify a stealth address commitment.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StealthAddressProof {
    pub ephemeral_pubkey: BytesN<32>,
    pub view_tag: BytesN<32>,
    pub stealth_pubkey: BytesN<32>,
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
    DepositKeyUsed(BytesN<32>),
    DepositKeyOwner(BytesN<32>),
    StealthMeta(Address),
}

#[contractclient(name = "Groth16VerifierClient")]
pub trait Groth16Verifier {
    fn verify(
        env: Env,
        verification_key_hash: BytesN<32>,
        public_input_hash: BytesN<32>,
        proof: Bytes,
    ) -> bool;
}

fn zero_bytes(env: &Env) -> BytesN<32> {
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
    env.storage()
        .persistent()
        .set(&DataKey::RootHistory(root.clone()), &true);
    env.storage()
        .persistent()
        .set(&DataKey::RootByIndex(index), root);
}

fn root_exists(env: &Env, root: &BytesN<32>) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::RootHistory(root.clone()))
        .unwrap_or(false)
}

fn append32(data: &mut Bytes, value: &BytesN<32>) {
    data.extend_from_slice(&value.to_array());
}

fn sha256_32(env: &Env, data: &Bytes) -> BytesN<32> {
    env.crypto().sha256(data).into()
}

fn hash_concat(env: &Env, left: &BytesN<32>, right: &BytesN<32>) -> BytesN<32> {
    let mut data = Bytes::new(env);
    append32(&mut data, left);
    append32(&mut data, right);
    sha256_32(env, &data)
}

fn statement_hash(env: &Env, transfer: &PrivateTransfer, next_root: &BytesN<32>) -> BytesN<32> {
    let sender_cipher_hash: BytesN<32> = env
        .crypto()
        .sha256(&transfer.sender_update.ciphertext)
        .into();
    let recipient_cipher_hash: BytesN<32> = env
        .crypto()
        .sha256(&transfer.recipient_update.ciphertext)
        .into();

    let mut data = Bytes::new(env);
    append32(&mut data, &transfer.old_root);
    append32(&mut data, &transfer.nullifier);
    append32(&mut data, &transfer.sender_update.commitment);
    append32(&mut data, &sender_cipher_hash);
    append32(&mut data, &transfer.recipient_update.commitment);
    append32(&mut data, &recipient_cipher_hash);
    append32(&mut data, next_root);
    sha256_32(env, &data)
}

fn next_root_for_transfer(
    env: &Env,
    current_root: &BytesN<32>,
    transfer: &PrivateTransfer,
) -> BytesN<32> {
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

fn require_nonzero_commitment(env: &Env, commitment: &BytesN<32>) -> Result<(), Error> {
    if *commitment == zero_bytes(env) {
        return Err(Error::InvalidCommitment);
    }
    Ok(())
}

/// Stealth address commitment: H(ephemeral_pubkey || view_tag || stealth_pubkey).
///
/// The view tag lets the intended receiver scan cheaply without revealing
/// the mapping from meta-address to one-time stealth pubkey on-chain.
fn stealth_commitment_hash(env: &Env, proof: &StealthAddressProof) -> BytesN<32> {
    let mut data = Bytes::new(env);
    append32(&mut data, &proof.ephemeral_pubkey);
    append32(&mut data, &proof.view_tag);
    append32(&mut data, &proof.stealth_pubkey);
    sha256_32(env, &data)
}

fn obfuscated_transfer_id(env: &Env, transfer: &PrivateTransfer) -> BytesN<32> {
    let mut data = Bytes::new(env);
    append32(&mut data, &transfer.nullifier);
    append32(&mut data, &transfer.sender_update.commitment);
    append32(&mut data, &transfer.recipient_update.commitment);
    sha256_32(env, &data)
}

fn is_registered_deposit_key(env: &Env, commitment: &BytesN<32>) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::DepositKeyOwner(commitment.clone()))
}

fn is_deposit_key_spent(env: &Env, commitment: &BytesN<32>) -> bool {
    env.storage()
        .persistent()
        .get(&DataKey::DepositKeyUsed(commitment.clone()))
        .unwrap_or(false)
}

fn consume_deposit_key_if_registered(env: &Env, commitment: &BytesN<32>) -> Result<(), Error> {
    if !is_registered_deposit_key(env, commitment) {
        return Ok(());
    }
    if is_deposit_key_spent(env, commitment) {
        return Err(Error::DepositKeyAlreadyUsed);
    }
    env.storage()
        .persistent()
        .set(&DataKey::DepositKeyUsed(commitment.clone()), &true);
    Ok(())
}

fn emit_obfuscated_transfer(env: &Env, transfer: &PrivateTransfer, receipt: &TransferReceipt) {
    let obfuscated_id = obfuscated_transfer_id(env, transfer);
    env.events().publish(
        (symbol_short!("xfer"), obfuscated_id),
        (receipt.new_root.clone(), receipt.leaf_index_start),
    );
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

        let root = if initial_root == zero_bytes(&env) {
            zero_bytes(&env)
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
        env.storage()
            .instance()
            .get(&DataKey::NextLeafIndex)
            .unwrap_or(0)
    }

    pub fn is_nullifier_used(env: Env, nullifier: BytesN<32>) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Nullifier(nullifier))
            .unwrap_or(false)
    }

    pub fn encrypted_note(env: Env, commitment: BytesN<32>) -> Option<Bytes> {
        env.storage()
            .persistent()
            .get(&DataKey::Ciphertext(commitment))
    }

    pub fn preview_next_root(env: Env, transfer: PrivateTransfer) -> Result<BytesN<32>, Error> {
        let current = read_current_root(&env)?;
        if transfer.old_root != current && !root_exists(&env, &transfer.old_root) {
            return Err(Error::InvalidRoot);
        }
        Ok(next_root_for_transfer(&env, &transfer.old_root, &transfer))
    }

    /// Hash of stealth-address generation inputs used as an on-chain commitment.
    pub fn compute_stealth_commitment(env: Env, proof: StealthAddressProof) -> BytesN<32> {
        stealth_commitment_hash(&env, &proof)
    }

    /// Returns true when `expected` matches H(ephemeral || view_tag || stealth).
    pub fn verify_stealth_commitment(
        env: Env,
        proof: StealthAddressProof,
        expected: BytesN<32>,
    ) -> Result<bool, Error> {
        require_nonzero_commitment(&env, &proof.ephemeral_pubkey)?;
        require_nonzero_commitment(&env, &proof.stealth_pubkey)?;
        require_nonzero_commitment(&env, &expected)?;
        Ok(stealth_commitment_hash(&env, &proof) == expected)
    }

    /// Publish spend/view keys so senders can derive one-time stealth addresses.
    pub fn register_stealth_meta(
        env: Env,
        owner: Address,
        spend_pubkey: BytesN<32>,
        view_pubkey: BytesN<32>,
    ) -> Result<(), Error> {
        owner.require_auth();
        require_nonzero_commitment(&env, &spend_pubkey)?;
        require_nonzero_commitment(&env, &view_pubkey)?;
        env.storage().persistent().set(
            &DataKey::StealthMeta(owner),
            &StealthMetaAddress {
                spend_pubkey,
                view_pubkey,
            },
        );
        Ok(())
    }

    pub fn stealth_meta(env: Env, owner: Address) -> Option<StealthMetaAddress> {
        env.storage().persistent().get(&DataKey::StealthMeta(owner))
    }

    /// Register a one-time disposable deposit key (stealth commitment).
    ///
    /// The key can receive exactly one private transfer; a second credit fails.
    pub fn register_deposit_key(
        env: Env,
        owner: Address,
        commitment: BytesN<32>,
    ) -> Result<(), Error> {
        owner.require_auth();
        require_nonzero_commitment(&env, &commitment)?;
        if is_registered_deposit_key(&env, &commitment) {
            return Err(Error::DepositKeyExists);
        }
        env.storage()
            .persistent()
            .set(&DataKey::DepositKeyOwner(commitment.clone()), &owner);
        env.storage()
            .persistent()
            .set(&DataKey::DepositKeyUsed(commitment), &false);
        Ok(())
    }

    pub fn is_deposit_key_used(env: Env, commitment: BytesN<32>) -> bool {
        is_deposit_key_spent(&env, &commitment)
    }

    pub fn deposit_key_owner(env: Env, commitment: BytesN<32>) -> Option<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::DepositKeyOwner(commitment))
    }

    pub fn apply_private_transfer(
        env: Env,
        relayer: Address,
        transfer: PrivateTransfer,
        proof: Bytes,
    ) -> Result<TransferReceipt, Error> {
        relayer.require_auth();

        require_nonzero_commitment(&env, &transfer.sender_update.commitment)?;
        require_nonzero_commitment(&env, &transfer.recipient_update.commitment)?;
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
        let valid = Groth16VerifierClient::new(&env, &verifier).verify(
            &verification_key_hash,
            &public_input_hash,
            &proof,
        );
        if !valid {
            return Err(Error::ProofVerificationFailed);
        }

        consume_deposit_key_if_registered(&env, &transfer.recipient_update.commitment)?;

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

        let receipt = TransferReceipt {
            previous_root: current_root,
            new_root: next_root,
            nullifier: transfer.nullifier.clone(),
            leaf_index_start,
        };
        emit_obfuscated_transfer(&env, &transfer, &receipt);
        Ok(receipt)
    }
}
