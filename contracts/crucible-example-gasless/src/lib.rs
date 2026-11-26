#![no_std]
#![allow(deprecated)]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, Env, Symbol,
};

/// A meta-transaction (gasless) request signed by the user.
#[contracttype]
#[derive(Clone)]
pub struct MetaTx {
    /// The user whose tokens will be transferred.
    pub from: Address,
    /// Recipient of the transfer.
    pub to: Address,
    /// Token contract address.
    pub token: Address,
    /// Amount to transfer.
    pub amount: i128,
    /// Nonce to prevent replay attacks.
    pub nonce: u64,
    /// Deadline (unix timestamp) after which this meta-tx is invalid.
    pub deadline: u64,
}

#[contracttype]
enum DataKey {
    /// Admin / relayer address.
    Admin,
    /// Per-user nonce counter. Lives in *persistent* storage, one ledger entry
    /// per account — see `read_nonce` for why this must not be instance or
    /// temporary storage.
    Nonce(Address),
}

/// Extend a nonce entry's TTL to this many ledgers on every write
/// (~60 days at a 5s close time).
pub const NONCE_TTL_EXTEND_TO: u32 = 1_036_800;
/// Extend a nonce entry's TTL when it drops below this many ledgers
/// (~30 days at a 5s close time).
pub const NONCE_TTL_THRESHOLD: u32 = 518_400;
/// Largest number of nonces a single `invalidate_nonces` call may burn.
///
/// Invalidation is a single O(1) write regardless of how far it jumps, so this
/// cap is not about cost: it stops one mistaken call from advancing the counter
/// so far that the account can never issue a usable meta-tx again.
pub const MAX_NONCE_ADVANCE: u64 = 10_000;

/// A gasless transaction (meta-transaction) contract.
///
/// A user signs a `MetaTx` off-chain. A trusted relayer submits it on-chain,
/// paying the network fee. The contract verifies the nonce and deadline, then
/// executes the token transfer on behalf of the user.
///
/// In Soroban, "signing" is handled by `require_auth` — the user's auth entry
/// is attached to the transaction by the relayer. This contract enforces:
/// - Nonce uniqueness (replay protection).
/// - Deadline enforcement (expiry protection).
/// - Relayer-only submission.
/// - User-driven batch invalidation of pending nonces.
#[contract]
#[derive(Default)]
pub struct Gasless;

// Internal helpers. Kept out of the `#[contractimpl]` block below so they do
// not become part of the contract's public interface.
impl Gasless {
    /// Read `user`'s next expected nonce.
    ///
    /// The counter lives in persistent storage, which matters for replay
    /// protection in two separate ways:
    ///
    /// - It must not be *temporary* storage. A temporary entry is deleted once
    ///   it expires, and a deleted entry reads back as `0` — which would reset
    ///   the counter and make every previously consumed nonce replayable. A
    ///   persistent entry is archived rather than deleted, and archived state
    ///   must be restored (with its value intact) before the contract can be
    ///   invoked against it, so the counter can never silently rewind.
    /// - It must not be *instance* storage. Instance storage is a single ledger
    ///   entry shared by the whole contract, so every account's nonce would be
    ///   packed into one value that is read and rewritten on every execution,
    ///   and the contract would stop working once enough accounts had been seen
    ///   to exceed the entry size limit.
    fn read_nonce(env: &Env, user: &Address) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::Nonce(user.clone()))
            .unwrap_or(0u64)
    }

    /// Write `user`'s next expected nonce and refresh the entry's TTL.
    ///
    /// The TTL bump happens on the same write that consumes a nonce, so an
    /// account stays alive for as long as it keeps transacting.
    fn write_nonce(env: &Env, user: &Address, value: u64) {
        let key = DataKey::Nonce(user.clone());
        env.storage().persistent().set(&key, &value);
        env.storage()
            .persistent()
            .extend_ttl(&key, NONCE_TTL_THRESHOLD, NONCE_TTL_EXTEND_TO);
    }
}

#[contractimpl]
impl Gasless {
    /// Initialize the contract with a trusted relayer address.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .extend_ttl(NONCE_TTL_THRESHOLD, NONCE_TTL_EXTEND_TO);
    }

    /// Execute a meta-transaction on behalf of `meta_tx.from`.
    ///
    /// Must be called by the registered relayer (admin).
    /// The user's authorization is verified via `meta_tx.from.require_auth()`.
    pub fn execute(env: Env, relayer: Address, meta_tx: MetaTx) {
        // Only the registered relayer may submit.
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");
        if relayer != admin {
            panic!("unauthorized relayer");
        }
        relayer.require_auth();

        // Deadline check.
        let now = env.ledger().timestamp();
        if now > meta_tx.deadline {
            panic!("meta-tx expired");
        }

        // Nonce check — must match the stored next-nonce for this user. An
        // invalidated nonce fails here too, because invalidation moves the same
        // counter forward.
        let expected_nonce = Self::read_nonce(&env, &meta_tx.from);
        if meta_tx.nonce != expected_nonce {
            panic!("invalid nonce");
        }

        // Require the user's authorization (attached by the relayer).
        meta_tx.from.require_auth();

        // Consume the nonce *before* handing control to the token contract.
        // Soroban already forbids contract re-entry at the host level, so a
        // hostile token cannot call back into `execute` at all; ordering the
        // write first is defence in depth (checks-effects-interactions) that
        // keeps replay protection correct without relying on that guarantee.
        // `checked_add` keeps a saturated counter from wrapping back to 0.
        let next_nonce = expected_nonce.checked_add(1).expect("nonce overflow");
        Self::write_nonce(&env, &meta_tx.from, next_nonce);

        env.storage()
            .instance()
            .extend_ttl(NONCE_TTL_THRESHOLD, NONCE_TTL_EXTEND_TO);

        // Execute the transfer.
        token::Client::new(&env, &meta_tx.token).transfer(
            &meta_tx.from,
            &meta_tx.to,
            &meta_tx.amount,
        );

        env.events().publish(
            (symbol_short!("executed"),),
            (meta_tx.from, meta_tx.to, meta_tx.amount, meta_tx.nonce),
        );
    }

    /// Invalidate every nonce below `new_nonce` for `user`, in one write.
    ///
    /// This is how a user cancels meta-transactions they have already signed
    /// but that have not been submitted yet — for example after handing a
    /// batch to a relayer that never broadcast them. Advancing the counter to
    /// `new_nonce` retires the whole contiguous range `[current, new_nonce)` at
    /// once, so cancelling a hundred pending meta-txs costs the same single
    /// ledger write as cancelling one.
    ///
    /// Requires `user`'s authorization, so a relayer can submit this for the
    /// user exactly like an `execute` — invalidation is itself gasless. Returns
    /// the new next-expected nonce.
    ///
    /// Panics if `new_nonce` does not move the counter forward, or if it would
    /// advance it by more than [`MAX_NONCE_ADVANCE`] at once.
    pub fn invalidate_nonces(env: Env, user: Address, new_nonce: u64) -> u64 {
        user.require_auth();

        let current = Self::read_nonce(&env, &user);
        if new_nonce <= current {
            panic!("invalid nonce");
        }
        if new_nonce - current > MAX_NONCE_ADVANCE {
            panic!("advance too large");
        }

        Self::write_nonce(&env, &user, new_nonce);
        env.storage()
            .instance()
            .extend_ttl(NONCE_TTL_THRESHOLD, NONCE_TTL_EXTEND_TO);

        env.events().publish(
            (Symbol::new(&env, "invalidated"), user),
            (current, new_nonce),
        );

        new_nonce
    }

    /// Return the current nonce for `user` (the next expected nonce).
    pub fn nonce(env: Env, user: Address) -> u64 {
        Self::read_nonce(&env, &user)
    }

    /// Return the relayer address.
    pub fn relayer(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized")
    }
}

/// Helper to build a `MetaTx` value (used in tests).
pub fn make_meta_tx(
    _env: &Env,
    from: Address,
    to: Address,
    token: Address,
    amount: i128,
    nonce: u64,
    deadline: u64,
) -> MetaTx {
    MetaTx {
        from,
        to,
        token,
        amount,
        nonce,
        deadline,
    }
}

#[cfg(test)]
mod test;
