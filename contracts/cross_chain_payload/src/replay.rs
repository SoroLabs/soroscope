use crate::errors::CrossChainError;
use crate::payload::CrossChainPayload;
use soroban_sdk::{contract, contractimpl, BytesN, Env};

/// 24 hours on Stellar (approx. 5 seconds per ledger -> 17,280 ledgers).
pub const REPLAY_PROTECTION_TTL_LEDGERS: u32 = 17_280;
pub const MIN_REPLAY_TTL_LEDGERS: u32 = 17_280;

/// Check whether a payload ID has already been recorded in temporary storage.
pub fn is_payload_recorded(env: &Env, payload_id: &BytesN<32>) -> bool {
    env.storage().temporary().has(payload_id)
}

/// Records a payload ID in temporary storage with replay protection window of at least 17,280 ledgers (~24 hours).
/// Rejects duplicate payload IDs with `CrossChainError::ReplayAttack`.
pub fn record_payload_id(env: &Env, payload_id: &BytesN<32>) -> Result<(), CrossChainError> {
    if is_payload_recorded(env, payload_id) {
        return Err(CrossChainError::ReplayAttack);
    }
    env.storage().temporary().set(payload_id, &true);
    env.storage()
        .temporary()
        .extend_ttl(payload_id, MIN_REPLAY_TTL_LEDGERS, MIN_REPLAY_TTL_LEDGERS);
    Ok(())
}

/// Contract interface for cross-chain payload verification and replay protection.
#[contract]
pub struct CrossChainPayloadContract;

#[contractimpl]
impl CrossChainPayloadContract {
    pub const REPLAY_TTL_LEDGERS: u32 = REPLAY_PROTECTION_TTL_LEDGERS;

    /// Records payload_id with 24-hour replay protection TTL (17,280 ledgers).
    pub fn record_payload(env: Env, payload_id: BytesN<32>) -> Result<(), CrossChainError> {
        record_payload_id(&env, &payload_id)
    }

    /// Check if payload ID is already recorded / replayed.
    pub fn is_payload_replayed(env: Env, payload_id: BytesN<32>) -> bool {
        is_payload_recorded(&env, &payload_id)
    }

    /// Process a cross-chain payload, enforcing replay protection with the 17,280 ledger TTL.
    pub fn process_payload(env: Env, payload: CrossChainPayload) -> Result<bool, CrossChainError> {
        record_payload_id(&env, &payload.payload_id)?;
        Ok(true)
    }

    /// Returns the active replay protection TTL in ledgers.
    pub fn get_replay_ttl(_env: Env) -> u32 {
        Self::REPLAY_TTL_LEDGERS
    }
}
