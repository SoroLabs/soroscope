use crate::error::Error;
use crate::storage_types::DataKey;
use soroban_sdk::{panic_with_error, Address, Env};

// ── Authorization (freeze) flags ──────────────────────────────────────────
//
// Accounts are authorized by default, so only deauthorized accounts occupy
// storage. A frozen account can neither send nor receive tokens until the
// admin re-authorizes it.

pub fn read_authorized(e: &Env, id: &Address) -> bool {
    let key = DataKey::State(id.clone());
    e.storage().persistent().get(&key).unwrap_or(true)
}

pub fn write_authorized(e: &Env, id: &Address, authorize: bool) {
    let key = DataKey::State(id.clone());
    if authorize {
        e.storage().persistent().remove(&key);
    } else {
        e.storage().persistent().set(&key, &false);
    }
}

/// Reverts with `Error::AccountFrozen` when `id` has been deauthorized.
pub fn require_authorized(e: &Env, id: &Address) {
    if !read_authorized(e, id) {
        panic_with_error!(e, Error::AccountFrozen);
    }
}
