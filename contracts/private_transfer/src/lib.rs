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

/// Point on the fixed elliptic curve y^2 = x^3 + 7x + 11 (mod p) where p = 2^64 - 59.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurvePoint {
    pub x: u64,
    pub y: u64,
    pub is_infinity: bool,
}

/// Opening of a Pedersen commitment (private amount and blinding factor).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitmentOpening {
    pub amount: i128,
    pub blinding_factor: BytesN<32>,
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
    StoredCommitment(BytesN<32>),
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

/// Fixed elliptic curve arithmetic and Pedersen commitment implementation.
///
/// Curve equation: y^2 = x^3 + 7x + 11 (mod p)
/// Field prime: p = 2^64 - 59 = 18446744073709551557
/// Generator G: (3, 2^32) = (3, 4294967296)
/// Generator H: (102, 10432324945265058214)
pub mod pedersen {
    use super::*;

    pub const FIELD_P: u64 = 18446744073709551557;
    pub const CURVE_A: u64 = 7;
    pub const CURVE_B: u64 = 11;

    pub const GENERATOR_G: CurvePoint = CurvePoint {
        x: 3,
        y: 4294967296,
        is_infinity: false,
    };

    pub const GENERATOR_H: CurvePoint = CurvePoint {
        x: 102,
        y: 10432324945265058214,
        is_infinity: false,
    };

    #[inline(always)]
    pub fn mod_add(a: u64, b: u64) -> u64 {
        let sum = (a as u128) + (b as u128);
        (sum % (FIELD_P as u128)) as u64
    }

    #[inline(always)]
    pub fn mod_sub(a: u64, b: u64) -> u64 {
        if a >= b {
            a - b
        } else {
            FIELD_P - (b - a)
        }
    }

    #[inline(always)]
    pub fn mod_mul(a: u64, b: u64) -> u64 {
        (((a as u128) * (b as u128)) % (FIELD_P as u128)) as u64
    }

    pub fn mod_pow(mut base: u64, mut exp: u64) -> u64 {
        let mut res = 1u64;
        base = mod_mul(base, 1);
        while exp > 0 {
            if exp & 1 == 1 {
                res = mod_mul(res, base);
            }
            base = mod_mul(base, base);
            exp >>= 1;
        }
        res
    }

    pub fn mod_inv(val: u64) -> u64 {
        if val == 0 {
            0
        } else {
            mod_pow(val, FIELD_P - 2)
        }
    }

    pub fn point_is_on_curve(pt: &CurvePoint) -> bool {
        if pt.is_infinity {
            return true;
        }
        let lhs = mod_mul(pt.y, pt.y);
        let x2 = mod_mul(pt.x, pt.x);
        let x3 = mod_mul(x2, pt.x);
        let ax = mod_mul(CURVE_A, pt.x);
        let rhs = mod_add(mod_add(x3, ax), CURVE_B);
        lhs == rhs
    }

    pub fn point_add(p1: &CurvePoint, p2: &CurvePoint) -> CurvePoint {
        if p1.is_infinity {
            return p2.clone();
        }
        if p2.is_infinity {
            return p1.clone();
        }
        if p1.x == p2.x {
            if mod_add(p1.y, p2.y) == 0 || p1.y == 0 {
                return CurvePoint {
                    x: 0,
                    y: 0,
                    is_infinity: true,
                };
            }
            let x1_sq = mod_mul(p1.x, p1.x);
            let num = mod_add(mod_mul(3, x1_sq), CURVE_A);
            let den = mod_mul(2, p1.y);
            let m = mod_mul(num, mod_inv(den));

            let m_sq = mod_mul(m, m);
            let x3 = mod_sub(mod_sub(m_sq, p1.x), p1.x);
            let y3 = mod_sub(mod_mul(m, mod_sub(p1.x, x3)), p1.y);
            CurvePoint {
                x: x3,
                y: y3,
                is_infinity: false,
            }
        } else {
            let num = mod_sub(p2.y, p1.y);
            let den = mod_sub(p2.x, p1.x);
            let m = mod_mul(num, mod_inv(den));

            let m_sq = mod_mul(m, m);
            let x3 = mod_sub(mod_sub(m_sq, p1.x), p2.x);
            let y3 = mod_sub(mod_mul(m, mod_sub(p1.x, x3)), p1.y);
            CurvePoint {
                x: x3,
                y: y3,
                is_infinity: false,
            }
        }
    }

    pub fn scalar_mul(mut k: u64, pt: &CurvePoint) -> CurvePoint {
        let mut res = CurvePoint {
            x: 0,
            y: 0,
            is_infinity: true,
        };
        let mut curr = pt.clone();
        while k > 0 {
            if k & 1 == 1 {
                res = point_add(&res, &curr);
            }
            curr = point_add(&curr, &curr);
            k >>= 1;
        }
        res
    }

    pub fn point_to_bytes(env: &Env, pt: &CurvePoint) -> BytesN<32> {
        let mut out = [0u8; 32];
        if !pt.is_infinity {
            out[0..8].copy_from_slice(&pt.x.to_be_bytes());
            out[8..16].copy_from_slice(&pt.y.to_be_bytes());
            out[16] = 1;
        }
        BytesN::from_array(env, &out)
    }

    pub fn bytes_to_point(bytes: &BytesN<32>) -> CurvePoint {
        let arr = bytes.to_array();
        if arr[16] == 0 {
            CurvePoint {
                x: 0,
                y: 0,
                is_infinity: true,
            }
        } else {
            let mut x_bytes = [0u8; 8];
            let mut y_bytes = [0u8; 8];
            x_bytes.copy_from_slice(&arr[0..8]);
            y_bytes.copy_from_slice(&arr[8..16]);
            CurvePoint {
                x: u64::from_be_bytes(x_bytes),
                y: u64::from_be_bytes(y_bytes),
                is_infinity: false,
            }
        }
    }

    pub fn commit(env: &Env, amount: i128, blinding_factor: &BytesN<32>) -> Result<BytesN<32>, Error> {
        if amount < 0 {
            return Err(Error::InvalidCommitment);
        }
        let scalar_amount = ((amount as u128) % (FIELD_P as u128)) as u64;

        let b_arr = blinding_factor.to_array();
        let mut b_first8 = [0u8; 8];
        b_first8.copy_from_slice(&b_arr[0..8]);
        let raw_blinding = u64::from_be_bytes(b_first8);
        let scalar_blinding = raw_blinding % FIELD_P;

        let pt_g = scalar_mul(scalar_amount, &GENERATOR_G);
        let pt_h = scalar_mul(scalar_blinding, &GENERATOR_H);
        let commitment_pt = point_add(&pt_g, &pt_h);

        Ok(point_to_bytes(env, &commitment_pt))
    }
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

    /// Store a Pedersen commitment on-chain.
    pub fn store_commitment(env: Env, commitment: BytesN<32>) -> Result<(), Error> {
        require_nonzero_commitment(&env, &commitment)?;
        env.storage()
            .persistent()
            .set(&DataKey::StoredCommitment(commitment), &true);
        Ok(())
    }

    /// Check if a commitment is stored on-chain.
    pub fn has_commitment(env: Env, commitment: BytesN<32>) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::StoredCommitment(commitment))
            .unwrap_or(false)
    }

    /// Compute a Pedersen commitment: commitment = amount*G + blinding_factor*H.
    pub fn compute_pedersen_commitment(
        env: Env,
        amount: i128,
        blinding_factor: BytesN<32>,
    ) -> Result<BytesN<32>, Error> {
        pedersen::commit(&env, amount, &blinding_factor)
    }

    /// Verifier function: accepts (commitment, opening) and verifies opening against stored commitment.
    pub fn verify_opening(
        env: Env,
        commitment: BytesN<32>,
        opening: CommitmentOpening,
    ) -> Result<bool, Error> {
        require_nonzero_commitment(&env, &commitment)?;
        if !Self::has_commitment(env.clone(), commitment.clone()) {
            return Ok(false);
        }
        let expected = pedersen::commit(&env, opening.amount, &opening.blinding_factor)?;
        Ok(expected == commitment)
    }

    /// Alias for verify_opening.
    pub fn verify_commitment(
        env: Env,
        commitment: BytesN<32>,
        opening: CommitmentOpening,
    ) -> Result<bool, Error> {
        Self::verify_opening(env, commitment, opening)
    }

    /// Directly verify that an opening matches a given commitment value.
    pub fn verify_commitment_opening(
        env: Env,
        commitment: BytesN<32>,
        opening: CommitmentOpening,
    ) -> Result<bool, Error> {
        require_nonzero_commitment(&env, &commitment)?;
        let expected = pedersen::commit(&env, opening.amount, &opening.blinding_factor)?;
        Ok(expected == commitment)
    }

    /// Returns the fixed curve generator G.
    pub fn curve_generator_g(_env: Env) -> CurvePoint {
        pedersen::GENERATOR_G
    }

    /// Returns the fixed curve generator H.
    pub fn curve_generator_h(_env: Env) -> CurvePoint {
        pedersen::GENERATOR_H
    }
}

