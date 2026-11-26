//! Issue #1308: a malicious token must not be able to re-enter `withdraw`
//! during its `transfer` and pull out the same LP shares twice.

use super::*;
use soroban_sdk::{contract, contractimpl, contracttype, testutils::Address as _, Address, Env};

#[contracttype]
#[derive(Clone)]
enum EvilKey {
    Balance(Address),
    Target,
    ReentryAttempted,
    ReentrySucceeded,
}

/// Token that calls back into the pool's `withdraw` when the pool pays it out.
#[contract]
struct EvilToken;

#[contractimpl]
impl EvilToken {
    pub fn mint(e: Env, to: Address, amount: i128) {
        let bal = Self::balance(e.clone(), to.clone());
        e.storage()
            .persistent()
            .set(&EvilKey::Balance(to), &(bal + amount));
    }

    pub fn balance(e: Env, id: Address) -> i128 {
        e.storage()
            .persistent()
            .get(&EvilKey::Balance(id))
            .unwrap_or(0)
    }

    /// Arm the re-entry: on the next payout from `pool`, call
    /// `pool.withdraw(attacker, shares)` again.
    pub fn arm(e: Env, pool: Address, attacker: Address, shares: i128) {
        e.storage()
            .instance()
            .set(&EvilKey::Target, &(pool, attacker, shares));
    }

    pub fn reentry_attempted(e: Env) -> bool {
        e.storage()
            .instance()
            .get(&EvilKey::ReentryAttempted)
            .unwrap_or(false)
    }

    pub fn reentry_succeeded(e: Env) -> bool {
        e.storage()
            .instance()
            .get(&EvilKey::ReentrySucceeded)
            .unwrap_or(false)
    }

    pub fn transfer(e: Env, from: Address, to: Address, amount: i128) {
        let from_bal = Self::balance(e.clone(), from.clone());
        let to_bal = Self::balance(e.clone(), to.clone());
        e.storage()
            .persistent()
            .set(&EvilKey::Balance(from.clone()), &(from_bal - amount));
        e.storage()
            .persistent()
            .set(&EvilKey::Balance(to), &(to_bal + amount));

        let target: Option<(Address, Address, i128)> = e.storage().instance().get(&EvilKey::Target);
        if let Some((pool, attacker, shares)) = target {
            if from == pool {
                // Disarm first so the attack is attempted only once.
                e.storage().instance().remove(&EvilKey::Target);
                e.storage().instance().set(&EvilKey::ReentryAttempted, &true);
                let result = LiquidityPoolClient::new(&e, &pool).try_withdraw(&attacker, &shares);
                let succeeded = matches!(result, Ok(Ok(_)));
                e.storage()
                    .instance()
                    .set(&EvilKey::ReentrySucceeded, &succeeded);
            }
        }
    }
}

#[test]
fn reentrant_withdraw_cannot_double_spend_shares() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&e);
    let honest_lp = Address::generate(&e);
    let attacker = Address::generate(&e);

    let evil_id = e.register(EvilToken, ());
    let evil = EvilTokenClient::new(&e, &evil_id);
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let pool_id = e.register(LiquidityPool, ());
    let pool = LiquidityPoolClient::new(&e, &pool_id);
    pool.initialize(&admin, &evil_id, &token_b);

    for lp in [&honest_lp, &attacker] {
        evil.mint(lp, &10_000);
        token_b_admin.mint(lp, &10_000);
    }

    // An honest LP funds the pool so there is something to steal.
    pool.deposit(&honest_lp, &5_000, &5_000);
    let attacker_shares = pool.deposit(&attacker, &1_000, &1_000);
    let (reserve_a_before, reserve_b_before) = pool.get_reserves();

    // Withdraw everything; the token tries to withdraw the same shares again.
    evil.arm(&pool_id, &attacker, &attacker_shares);
    let outer = pool.try_withdraw(&attacker, &attacker_shares);

    assert!(evil.reentry_attempted(), "malicious token never re-entered");
    assert!(!evil.reentry_succeeded(), "re-entrant withdraw must fail");

    // Whether the host aborts the whole call or the outer withdraw completes,
    // the attacker must not get more than one withdrawal's worth.
    let attacker_gain_a = evil.balance(&attacker) - 9_000;
    assert!(attacker_gain_a <= 1_000, "attacker drained {attacker_gain_a}");

    let (reserve_a_after, reserve_b_after) = pool.get_reserves();
    match outer {
        Ok(Ok((out_a, out_b))) => {
            assert_eq!(pool.balance(&attacker), 0);
            assert_eq!(reserve_a_after, reserve_a_before - out_a);
            assert_eq!(reserve_b_after, reserve_b_before - out_b);
            assert_eq!(attacker_gain_a, out_a);
        }
        _ => {
            // Whole call rolled back: nothing changed.
            assert_eq!(pool.balance(&attacker), attacker_shares);
            assert_eq!((reserve_a_after, reserve_b_after), (reserve_a_before, reserve_b_before));
        }
    }
    // The pool still holds exactly what its reserves say for the honest LP.
    assert_eq!(evil.balance(&pool_id), reserve_a_after);
}
