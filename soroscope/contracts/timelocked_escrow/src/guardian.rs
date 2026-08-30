use crate::storage_types::{DataKey, Error};
use soroban_sdk::{Address, Env, Vec};

pub const REQUIRED_GUARDIANS: u32 = 5;
pub const REQUIRED_APPROVALS: u32 = 3;

// ── Guardian List ─────────────────────────────────────────────

pub fn read_guardians(e: &Env) -> Vec<Address> {
    e.storage()
        .instance()
        .get(&DataKey::Guardians)
        .unwrap_or(Vec::new(e))
}

pub fn write_guardians(e: &Env, guardians: &Vec<Address>) {
    e.storage().instance().set(&DataKey::Guardians, guardians);
}

/// Returns the index (0..4) of `addr` in the guardian list.
pub fn guardian_index(guardians: &Vec<Address>, addr: &Address) -> Result<u32, Error> {
    for i in 0..guardians.len() {
        if guardians.get(i).unwrap() == *addr {
            return Ok(i);
        }
    }
    Err(Error::NotGuardian)
}

/// Validates exactly 5 unique addresses. O(n^2) with n=5 is trivial.
pub fn validate_guardians(guardians: &Vec<Address>) -> Result<(), Error> {
    if guardians.len() != REQUIRED_GUARDIANS {
        return Err(Error::InvalidGuardianCount);
    }
    for i in 0..guardians.len() {
        for j in (i + 1)..guardians.len() {
            if guardians.get(i).unwrap() == guardians.get(j).unwrap() {
                return Err(Error::DuplicateGuardian);
            }
        }
    }
    Ok(())
}

// ── Approval Bitmap ───────────────────────────────────────────

pub fn read_approvals(e: &Env) -> u32 {
    e.storage()
        .instance()
        .get(&DataKey::Approvals)
        .unwrap_or(0u32)
}

pub fn write_approvals(e: &Env, bitmap: u32) {
    e.storage().instance().set(&DataKey::Approvals, &bitmap);
}

pub fn read_epoch(e: &Env) -> u32 {
    e.storage()
        .instance()
        .get(&DataKey::ApprovalEpoch)
        .unwrap_or(0u32)
}

pub fn write_epoch(e: &Env, epoch: u32) {
    e.storage().instance().set(&DataKey::ApprovalEpoch, &epoch);
}

/// Sets bit `index` in bitmap. Returns error if already set.
pub fn set_approval(bitmap: u32, index: u32) -> Result<u32, Error> {
    let mask = 1u32 << index;
    if bitmap & mask != 0 {
        return Err(Error::AlreadyApproved);
    }
    Ok(bitmap | mask)
}

/// Counts set bits. O(1) for a 5-bit range.
pub fn approval_count(bitmap: u32) -> u32 {
    let mut n = bitmap;
    let mut count = 0u32;
    while n != 0 {
        count += n & 1;
        n >>= 1;
    }
    count
}

pub fn has_quorum(bitmap: u32) -> bool {
    approval_count(bitmap) >= REQUIRED_APPROVALS
}
