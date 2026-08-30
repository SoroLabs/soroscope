#![cfg(test)]
extern crate std;

use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, Events, Ledger, MockAuth, MockAuthInvoke},
    token, Address, Env, IntoVal, Symbol, TryFromVal,
};

use crate::{make_meta_tx, DataKey, Gasless, GaslessClient, MetaTx, MAX_NONCE_ADVANCE};

const AMOUNT: i128 = 1_000_000;
const BASE_TIME: u64 = 1_000_000;
const DEADLINE: u64 = BASE_TIME + 3_600; // 1 hour from now

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

struct Ctx {
    env: Env,
    id: Address,
    relayer: Address,
    alice: Address,
    bob: Address,
    token: Address,
}

impl Ctx {
    fn setup() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set_timestamp(BASE_TIME);

        let relayer = Address::generate(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);

        let issuer = Address::generate(&env);
        let sac = env.register_stellar_asset_contract_v2(issuer);
        let token = sac.address();
        token::StellarAssetClient::new(&env, &token).mint(&alice, &(AMOUNT * 5));

        let id = env.register(Gasless, ());
        GaslessClient::new(&env, &id).initialize(&relayer);

        Ctx {
            env,
            id,
            relayer,
            alice,
            bob,
            token,
        }
    }

    fn client(&self) -> GaslessClient<'_> {
        GaslessClient::new(&self.env, &self.id)
    }

    fn balance(&self, who: &Address) -> i128 {
        token::Client::new(&self.env, &self.token).balance(who)
    }

    fn meta_tx(&self, nonce: u64) -> MetaTx {
        make_meta_tx(
            &self.env,
            self.alice.clone(),
            self.bob.clone(),
            self.token.clone(),
            AMOUNT,
            nonce,
            DEADLINE,
        )
    }

    /// True if `user`'s nonce is held in persistent storage.
    fn nonce_in_persistent(&self, user: &Address) -> bool {
        self.env.as_contract(&self.id, || {
            self.env
                .storage()
                .persistent()
                .has(&DataKey::Nonce(user.clone()))
        })
    }

    /// True if `user`'s nonce is held in instance storage.
    fn nonce_in_instance(&self, user: &Address) -> bool {
        self.env.as_contract(&self.id, || {
            self.env
                .storage()
                .instance()
                .has(&DataKey::Nonce(user.clone()))
        })
    }

    fn emitted(&self, name: &str) -> bool {
        let want = Symbol::new(&self.env, name);
        self.env.events().all().iter().any(|(_, topics, _)| {
            topics.iter().any(|t| {
                Symbol::try_from_val(&self.env, &t)
                    .map(|s: Symbol| s == want)
                    .unwrap_or(false)
            })
        })
    }
}

// ---------------------------------------------------------------------------
// Existing behaviour
// ---------------------------------------------------------------------------

#[test]
fn test_execute_transfers_tokens() {
    let ctx = Ctx::setup();
    ctx.client().execute(&ctx.relayer, &ctx.meta_tx(0));

    assert_eq!(ctx.balance(&ctx.alice), AMOUNT * 4);
    assert_eq!(ctx.balance(&ctx.bob), AMOUNT);
}

#[test]
fn test_nonce_increments_after_execute() {
    let ctx = Ctx::setup();
    assert_eq!(ctx.client().nonce(&ctx.alice), 0);
    ctx.client().execute(&ctx.relayer, &ctx.meta_tx(0));
    assert_eq!(ctx.client().nonce(&ctx.alice), 1);
}

#[test]
#[should_panic(expected = "invalid nonce")]
fn test_replay_attack_reverts() {
    let ctx = Ctx::setup();
    ctx.client().execute(&ctx.relayer, &ctx.meta_tx(0));
    // Replay with same nonce.
    ctx.client().execute(&ctx.relayer, &ctx.meta_tx(0));
}

#[test]
fn test_sequential_nonces_succeed() {
    let ctx = Ctx::setup();
    ctx.client().execute(&ctx.relayer, &ctx.meta_tx(0));
    ctx.client().execute(&ctx.relayer, &ctx.meta_tx(1));
    assert_eq!(ctx.balance(&ctx.bob), AMOUNT * 2);
}

#[test]
#[should_panic(expected = "meta-tx expired")]
fn test_expired_meta_tx_reverts() {
    let ctx = Ctx::setup();
    ctx.env.ledger().set_timestamp(DEADLINE + 1);
    ctx.client().execute(&ctx.relayer, &ctx.meta_tx(0));
}

#[test]
fn test_execute_at_exact_deadline_succeeds() {
    let ctx = Ctx::setup();
    // The check is `now > deadline`, so the deadline ledger itself is valid.
    ctx.env.ledger().set_timestamp(DEADLINE);
    ctx.client().execute(&ctx.relayer, &ctx.meta_tx(0));
    assert_eq!(ctx.balance(&ctx.bob), AMOUNT);
}

#[test]
#[should_panic(expected = "unauthorized relayer")]
fn test_unauthorized_relayer_reverts() {
    let ctx = Ctx::setup();
    // alice tries to act as relayer.
    ctx.client().execute(&ctx.alice, &ctx.meta_tx(0));
}

#[test]
#[should_panic(expected = "invalid nonce")]
fn test_wrong_nonce_reverts() {
    let ctx = Ctx::setup();
    // Nonce 1 is wrong when 0 is expected.
    ctx.client().execute(&ctx.relayer, &ctx.meta_tx(1));
}

#[test]
fn test_relayer_returns_correct_address() {
    let ctx = Ctx::setup();
    assert_eq!(ctx.client().relayer(), ctx.relayer);
}

#[test]
fn test_nonce_starts_at_zero() {
    let ctx = Ctx::setup();
    assert_eq!(ctx.client().nonce(&ctx.alice), 0);
    assert_eq!(ctx.client().nonce(&ctx.bob), 0);
}

#[test]
fn test_execute_emits_event() {
    let ctx = Ctx::setup();
    ctx.client().execute(&ctx.relayer, &ctx.meta_tx(0));
    assert!(ctx.emitted("executed"), "expected 'executed' event");
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_double_initialize_reverts() {
    let ctx = Ctx::setup();
    ctx.client().initialize(&ctx.relayer);
}

// ---------------------------------------------------------------------------
// Per-account nonce storage
// ---------------------------------------------------------------------------

/// Nonces are per-account ledger entries in *persistent* storage.
///
/// Instance storage would pack every account's nonce into one shared entry, and
/// temporary storage would let a consumed nonce be deleted and read back as 0 —
/// silently rewinding the counter and making spent meta-txs replayable.
#[test]
fn test_nonce_is_stored_per_account_in_persistent_storage() {
    let ctx = Ctx::setup();
    ctx.client().execute(&ctx.relayer, &ctx.meta_tx(0));

    assert!(
        ctx.nonce_in_persistent(&ctx.alice),
        "alice's nonce must live in persistent storage"
    );
    assert!(
        !ctx.nonce_in_instance(&ctx.alice),
        "nonce must not be packed into the shared instance entry"
    );
    // An account that has never transacted has no entry at all.
    assert!(!ctx.nonce_in_persistent(&ctx.bob));
    assert_eq!(ctx.client().nonce(&ctx.bob), 0);
}

#[test]
fn test_multiple_users_independent_nonces() {
    let ctx = Ctx::setup();
    token::StellarAssetClient::new(&ctx.env, &ctx.token).mint(&ctx.bob, &(AMOUNT * 5));

    // alice executes nonce 0.
    ctx.client().execute(&ctx.relayer, &ctx.meta_tx(0));

    // bob's nonce is still 0 independently.
    let bob_tx = make_meta_tx(
        &ctx.env,
        ctx.bob.clone(),
        ctx.alice.clone(),
        ctx.token.clone(),
        AMOUNT,
        0,
        DEADLINE,
    );
    ctx.client().execute(&ctx.relayer, &bob_tx);

    assert_eq!(ctx.client().nonce(&ctx.alice), 1);
    assert_eq!(ctx.client().nonce(&ctx.bob), 1);
    assert!(ctx.nonce_in_persistent(&ctx.alice));
    assert!(ctx.nonce_in_persistent(&ctx.bob));
}

// ---------------------------------------------------------------------------
// Batch invalidation
// ---------------------------------------------------------------------------

#[test]
fn test_invalidate_nonces_retires_a_batch_in_one_call() {
    let ctx = Ctx::setup();
    assert_eq!(ctx.client().invalidate_nonces(&ctx.alice, &5), 5);
    assert_eq!(ctx.client().nonce(&ctx.alice), 5);
}

#[test]
#[should_panic(expected = "invalid nonce")]
fn test_invalidated_nonce_cannot_be_executed() {
    let ctx = Ctx::setup();
    ctx.client().invalidate_nonces(&ctx.alice, &5);
    // Nonce 3 was signed before the invalidation and is now dead.
    ctx.client().execute(&ctx.relayer, &ctx.meta_tx(3));
}

#[test]
fn test_execute_resumes_at_the_invalidated_nonce() {
    let ctx = Ctx::setup();
    ctx.client().invalidate_nonces(&ctx.alice, &5);

    ctx.client().execute(&ctx.relayer, &ctx.meta_tx(5));
    assert_eq!(ctx.client().nonce(&ctx.alice), 6);
    assert_eq!(ctx.balance(&ctx.bob), AMOUNT);
}

#[test]
fn test_invalidate_after_execute_advances_from_current() {
    let ctx = Ctx::setup();
    ctx.client().execute(&ctx.relayer, &ctx.meta_tx(0));
    assert_eq!(ctx.client().nonce(&ctx.alice), 1);

    ctx.client().invalidate_nonces(&ctx.alice, &10);
    assert_eq!(ctx.client().nonce(&ctx.alice), 10);
}

#[test]
#[should_panic(expected = "invalid nonce")]
fn test_invalidate_cannot_rewind() {
    let ctx = Ctx::setup();
    ctx.client().invalidate_nonces(&ctx.alice, &5);
    // Rewinding would resurrect already-dead nonces.
    ctx.client().invalidate_nonces(&ctx.alice, &2);
}

#[test]
#[should_panic(expected = "invalid nonce")]
fn test_invalidate_to_current_nonce_reverts() {
    let ctx = Ctx::setup();
    // A no-op invalidation is a caller mistake, not a silent success.
    ctx.client().invalidate_nonces(&ctx.alice, &0);
}

#[test]
fn test_invalidate_accepts_the_maximum_advance() {
    let ctx = Ctx::setup();
    ctx.client()
        .invalidate_nonces(&ctx.alice, &MAX_NONCE_ADVANCE);
    assert_eq!(ctx.client().nonce(&ctx.alice), MAX_NONCE_ADVANCE);
}

#[test]
#[should_panic(expected = "advance too large")]
fn test_invalidate_rejects_oversized_advance() {
    let ctx = Ctx::setup();
    ctx.client()
        .invalidate_nonces(&ctx.alice, &(MAX_NONCE_ADVANCE + 1));
}

/// The cap is on the *jump*, not the absolute value, so it is measured from the
/// current counter rather than from zero.
#[test]
fn test_invalidate_cap_is_relative_to_current_nonce() {
    let ctx = Ctx::setup();
    ctx.client()
        .invalidate_nonces(&ctx.alice, &MAX_NONCE_ADVANCE);
    ctx.client()
        .invalidate_nonces(&ctx.alice, &(MAX_NONCE_ADVANCE * 2));
    assert_eq!(ctx.client().nonce(&ctx.alice), MAX_NONCE_ADVANCE * 2);
}

#[test]
fn test_invalidate_is_per_account() {
    let ctx = Ctx::setup();
    ctx.client().invalidate_nonces(&ctx.alice, &7);

    assert_eq!(ctx.client().nonce(&ctx.alice), 7);
    assert_eq!(ctx.client().nonce(&ctx.bob), 0, "bob is unaffected");
    assert!(!ctx.nonce_in_persistent(&ctx.bob));
}

#[test]
fn test_invalidate_emits_event() {
    let ctx = Ctx::setup();
    ctx.client().invalidate_nonces(&ctx.alice, &5);
    assert!(ctx.emitted("invalidated"), "expected 'invalidated' event");
}

#[test]
fn test_invalidate_stores_nonce_persistently() {
    let ctx = Ctx::setup();
    ctx.client().invalidate_nonces(&ctx.alice, &5);
    assert!(ctx.nonce_in_persistent(&ctx.alice));
    assert!(!ctx.nonce_in_instance(&ctx.alice));
}

/// Invalidation carries the user's own authorization, so a relayer can submit
/// it on their behalf — cancelling is gasless too.
#[test]
fn test_invalidate_requires_user_auth() {
    let ctx = Ctx::setup();
    ctx.client().invalidate_nonces(&ctx.alice, &5);

    let auths = ctx.env.auths();
    assert!(
        auths.iter().any(|(addr, _)| addr == &ctx.alice),
        "invalidate_nonces must require the user's authorization"
    );
}

// ---------------------------------------------------------------------------
// Authorization is genuinely enforced
//
// Every test above runs under `mock_all_auths()`, which makes any
// `require_auth` succeed. These two clear the mocked auths so the calls are
// exercised with no signature at all — otherwise a missing `require_auth` in
// the contract would go unnoticed by the whole suite.
// ---------------------------------------------------------------------------

/// A relayer cannot forge a meta-tx: the registered relayer authorizes the
/// submission, but without the *user's* authorization the execution still
/// fails.
///
/// The relayer's auth is mocked explicitly rather than cleared, because
/// `relayer.require_auth()` runs first — clearing everything would abort on the
/// relayer and never reach the check this test is about.
#[test]
#[should_panic]
fn test_execute_without_user_auth_reverts() {
    let ctx = Ctx::setup();
    let meta_tx = ctx.meta_tx(0);
    let args = (ctx.relayer.clone(), meta_tx.clone()).into_val(&ctx.env);

    ctx.env.mock_auths(&[MockAuth {
        address: &ctx.relayer,
        invoke: &MockAuthInvoke {
            contract: &ctx.id,
            fn_name: "execute",
            args,
            sub_invokes: &[],
        },
    }]);
    ctx.client().execute(&ctx.relayer, &meta_tx);
}

/// Guards the test above: with *both* authorizations mocked the same call
/// succeeds, so its failure is attributable to the missing user auth and not to
/// the narrower mock setup.
#[test]
fn test_execute_succeeds_when_both_parties_authorize() {
    let ctx = Ctx::setup();
    let meta_tx = ctx.meta_tx(0);
    let args = (ctx.relayer.clone(), meta_tx.clone()).into_val(&ctx.env);

    // alice's authorization also has to cover the token transfer that
    // `execute` performs on her behalf, as a sub-invocation.
    let transfer = [MockAuthInvoke {
        contract: &ctx.token,
        fn_name: "transfer",
        args: (ctx.alice.clone(), ctx.bob.clone(), AMOUNT).into_val(&ctx.env),
        sub_invokes: &[],
    }];
    let relayer_invoke = MockAuthInvoke {
        contract: &ctx.id,
        fn_name: "execute",
        args,
        sub_invokes: &[],
    };
    let alice_invoke = MockAuthInvoke {
        contract: &ctx.id,
        fn_name: "execute",
        args: (ctx.relayer.clone(), meta_tx.clone()).into_val(&ctx.env),
        sub_invokes: &transfer,
    };

    ctx.env.mock_auths(&[
        MockAuth {
            address: &ctx.relayer,
            invoke: &relayer_invoke,
        },
        MockAuth {
            address: &ctx.alice,
            invoke: &alice_invoke,
        },
    ]);
    ctx.client().execute(&ctx.relayer, &meta_tx);
    assert_eq!(ctx.client().nonce(&ctx.alice), 1);
}

/// Nobody can burn another account's nonces.
#[test]
#[should_panic]
fn test_invalidate_without_user_auth_reverts() {
    let ctx = Ctx::setup();
    ctx.env.mock_auths(&[]);
    ctx.client().invalidate_nonces(&ctx.alice, &5);
}

// ---------------------------------------------------------------------------
// Re-entrancy
// ---------------------------------------------------------------------------

/// A "token" whose `transfer` calls straight back into `Gasless::execute` with
/// the meta-tx that is currently mid-flight.
#[contract]
pub struct ReentrantToken;

#[contractimpl]
impl ReentrantToken {
    pub fn setup(env: Env, gasless: Address, relayer: Address, meta_tx: MetaTx) {
        env.storage()
            .instance()
            .set(&symbol_short_key(&env), &(gasless, relayer, meta_tx));
    }

    pub fn transfer(env: Env, _from: Address, _to: Address, _amount: i128) {
        let (gasless, relayer, meta_tx): (Address, Address, MetaTx) = env
            .storage()
            .instance()
            .get(&symbol_short_key(&env))
            .unwrap();
        GaslessClient::new(&env, &gasless).execute(&relayer, &meta_tx);
    }
}

fn symbol_short_key(env: &Env) -> Symbol {
    Symbol::new(env, "cfg")
}

/// A hostile token cannot re-enter `execute` to replay the transfer.
///
/// Soroban rejects this in the host with `Contract re-entry is not allowed`
/// before the contract's own nonce check is reached, so this pins the platform
/// guarantee rather than contract logic. The contract still consumes the nonce
/// before the token call, so replay protection does not depend on it.
#[test]
#[should_panic(expected = "InvalidAction")]
fn test_reentrant_token_cannot_replay() {
    let ctx = Ctx::setup();

    let evil = ctx.env.register(ReentrantToken, ());
    let meta_tx = make_meta_tx(
        &ctx.env,
        ctx.alice.clone(),
        ctx.bob.clone(),
        evil.clone(),
        AMOUNT,
        0,
        DEADLINE,
    );
    ReentrantTokenClient::new(&ctx.env, &evil).setup(&ctx.id, &ctx.relayer, &meta_tx);

    ctx.client().execute(&ctx.relayer, &meta_tx);
}
