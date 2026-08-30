#![cfg(test)]
extern crate std;

use soroban_sdk::{testutils::Address as _, Address, Env};
use crate::{ConcentratedAmm, ConcentratedAmmClient};

// 1.0 in Q64 fixed-point
const Q64: u128 = 1u128 << 64;

/// Registers two stellar asset contracts and returns (token_a_addr, token_b_addr,
/// token_a_admin_client, token_b_admin_client).
fn setup_tokens(e: &Env) -> (
    Address,
    Address,
    soroban_sdk::token::StellarAssetClient,
    soroban_sdk::token::StellarAssetClient,
) {
    let admin = Address::generate(e);
    let token_a_addr = e.register_stellar_asset_contract_v2(admin.clone()).address();
    let token_b_addr = e.register_stellar_asset_contract_v2(admin.clone()).address();
    let admin_a = soroban_sdk::token::StellarAssetClient::new(e, &token_a_addr);
    let admin_b = soroban_sdk::token::StellarAssetClient::new(e, &token_b_addr);
    (token_a_addr, token_b_addr, admin_a, admin_b)
}

/// Deploys the AMM and initialises it at tick=0 (sqrt_price=Q64), tick_spacing=10, fee=30bps.
fn setup_amm<'a>(e: &'a Env, token_a: &Address, token_b: &Address) -> ConcentratedAmmClient<'a> {
    let contract_id = e.register(ConcentratedAmm, ());
    let client = ConcentratedAmmClient::new(e, &contract_id);
    client.initialize(token_a, token_b, &30u32, &10i32, &Q64, &0i32);
    client
}

#[test]
fn test_initialization() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(ConcentratedAmm, ());
    let client = ConcentratedAmmClient::new(&e, &contract_id);

    let token_a = Address::generate(&e);
    let token_b = Address::generate(&e);

    client.initialize(&token_a, &token_b, &30, &10, &Q64, &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_double_initialization() {
    let e = Env::default();
    e.mock_all_auths();

    let (token_a, token_b, _, _) = setup_tokens(&e);
    let client = setup_amm(&e, &token_a, &token_b);

    // Second call must fail with AlreadyInitialized.
    client.initialize(&token_a, &token_b, &30, &10, &Q64, &0);
}

#[test]
fn test_mint_in_range() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let (token_a, token_b, admin_a, admin_b) = setup_tokens(&e);
    let client = setup_amm(&e, &token_a, &token_b);

    let user = Address::generate(&e);
    admin_a.mint(&user, &1_000_000);
    admin_b.mint(&user, &1_000_000);

    // Tick range [-100, 100] straddles the current tick (0), so both tokens are deposited.
    let (liq, amt_a, amt_b) = client.mint(&user, &-100i32, &100i32, &100_000u128, &100_000u128);

    assert!(liq > 0, "should mint positive liquidity");
    assert!(amt_a > 0, "in-range mint should consume token A");
    assert!(amt_b > 0, "in-range mint should consume token B");
    assert!(amt_a <= 100_000, "should not over-consume token A");
    assert!(amt_b <= 100_000, "should not over-consume token B");
}

#[test]
fn test_mint_below_range() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let (token_a, token_b, admin_a, admin_b) = setup_tokens(&e);
    let client = setup_amm(&e, &token_a, &token_b);

    let user = Address::generate(&e);
    admin_a.mint(&user, &1_000_000);
    admin_b.mint(&user, &1_000_000);

    // Range [200, 400] is fully above the current price (tick 0), so only token A is consumed.
    let (liq, amt_a, amt_b) = client.mint(&user, &200i32, &400i32, &100_000u128, &0u128);

    assert!(liq > 0);
    assert!(amt_a > 0, "below-range mint consumes only token A");
    assert_eq!(amt_b, 0, "below-range mint uses no token B");
}

#[test]
fn test_mint_above_range() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let (token_a, token_b, admin_a, admin_b) = setup_tokens(&e);
    let client = setup_amm(&e, &token_a, &token_b);

    let user = Address::generate(&e);
    admin_a.mint(&user, &1_000_000);
    admin_b.mint(&user, &1_000_000);

    // Range [-400, -200] is fully below the current price (tick 0), so only token B is consumed.
    let (liq, amt_a, amt_b) = client.mint(&user, &-400i32, &-200i32, &0u128, &100_000u128);

    assert!(liq > 0);
    assert_eq!(amt_a, 0, "above-range mint uses no token A");
    assert!(amt_b > 0, "above-range mint consumes only token B");
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_mint_invalid_tick_range() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let (token_a, token_b, admin_a, _) = setup_tokens(&e);
    let client = setup_amm(&e, &token_a, &token_b);

    let user = Address::generate(&e);
    admin_a.mint(&user, &1_000_000);

    // tick_lower >= tick_upper — must fail with InvalidTickRange.
    client.mint(&user, &100i32, &-100i32, &100_000u128, &100_000u128);
}

#[test]
fn test_burn_recovers_tokens() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let (token_a, token_b, admin_a, admin_b) = setup_tokens(&e);
    let client = setup_amm(&e, &token_a, &token_b);

    let user = Address::generate(&e);
    admin_a.mint(&user, &1_000_000);
    admin_b.mint(&user, &1_000_000);

    let (liq, amt_a_in, amt_b_in) = client.mint(&user, &-100i32, &100i32, &100_000u128, &100_000u128);

    let client_a = soroban_sdk::token::Client::new(&e, &token_a);
    let client_b = soroban_sdk::token::Client::new(&e, &token_b);

    let bal_a_before = client_a.balance(&user);
    let bal_b_before = client_b.balance(&user);

    let (recovered_a, recovered_b) = client.burn(&user, &-100i32, &100i32, &liq);

    let bal_a_after = client_a.balance(&user);
    let bal_b_after = client_b.balance(&user);

    assert_eq!(recovered_a, amt_a_in, "should recover all of token A");
    assert_eq!(recovered_b, amt_b_in, "should recover all of token B");
    assert_eq!(bal_a_after - bal_a_before, recovered_a as i128);
    assert_eq!(bal_b_after - bal_b_before, recovered_b as i128);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_burn_more_than_owned() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let (token_a, token_b, admin_a, admin_b) = setup_tokens(&e);
    let client = setup_amm(&e, &token_a, &token_b);

    let user = Address::generate(&e);
    admin_a.mint(&user, &1_000_000);
    admin_b.mint(&user, &1_000_000);

    let (liq, _, _) = client.mint(&user, &-100i32, &100i32, &100_000u128, &100_000u128);

    // Attempt to burn more liquidity than minted.
    client.burn(&user, &-100i32, &100i32, &(liq + 1));
}

#[test]
fn test_swap_zero_for_one_within_range() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let (token_a, token_b, admin_a, admin_b) = setup_tokens(&e);
    let client = setup_amm(&e, &token_a, &token_b);

    let lp = Address::generate(&e);
    let trader = Address::generate(&e);

    admin_a.mint(&lp, &10_000_000);
    admin_b.mint(&lp, &10_000_000);
    admin_a.mint(&trader, &1_000_000);
    admin_b.mint(&trader, &1_000_000);

    // Provide wide in-range liquidity.
    client.mint(&lp, &-500i32, &500i32, &5_000_000u128, &5_000_000u128);

    let client_a = soroban_sdk::token::Client::new(&e, &token_a);
    let client_b = soroban_sdk::token::Client::new(&e, &token_b);

    let bal_a_before = client_a.balance(&trader);
    let bal_b_before = client_b.balance(&trader);

    // Sell token A, receive token B (zero_for_one = true).
    let (amt_in, amt_out) = client.swap(&trader, &true, &10_000u128);

    assert!(amt_in > 0);
    assert!(amt_out > 0);
    assert_eq!(client_a.balance(&trader), bal_a_before - amt_in as i128);
    assert_eq!(client_b.balance(&trader), bal_b_before + amt_out as i128);
}

#[test]
fn test_swap_one_for_zero_within_range() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let (token_a, token_b, admin_a, admin_b) = setup_tokens(&e);
    let client = setup_amm(&e, &token_a, &token_b);

    let lp = Address::generate(&e);
    let trader = Address::generate(&e);

    admin_a.mint(&lp, &10_000_000);
    admin_b.mint(&lp, &10_000_000);
    admin_a.mint(&trader, &1_000_000);
    admin_b.mint(&trader, &1_000_000);

    client.mint(&lp, &-500i32, &500i32, &5_000_000u128, &5_000_000u128);

    let client_a = soroban_sdk::token::Client::new(&e, &token_a);
    let client_b = soroban_sdk::token::Client::new(&e, &token_b);

    let bal_a_before = client_a.balance(&trader);
    let bal_b_before = client_b.balance(&trader);

    // Sell token B, receive token A (zero_for_one = false).
    let (amt_in, amt_out) = client.swap(&trader, &false, &10_000u128);

    assert!(amt_in > 0);
    assert!(amt_out > 0);
    assert_eq!(client_b.balance(&trader), bal_b_before - amt_in as i128);
    assert_eq!(client_a.balance(&trader), bal_a_before + amt_out as i128);
}

#[test]
fn test_swap_crosses_tick() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let (token_a, token_b, admin_a, admin_b) = setup_tokens(&e);
    let client = setup_amm(&e, &token_a, &token_b);

    let lp1 = Address::generate(&e);
    let lp2 = Address::generate(&e);
    let trader = Address::generate(&e);

    admin_a.mint(&lp1, &10_000_000);
    admin_b.mint(&lp1, &10_000_000);
    admin_a.mint(&lp2, &10_000_000);
    admin_b.mint(&lp2, &10_000_000);
    admin_a.mint(&trader, &10_000_000);
    admin_b.mint(&trader, &10_000_000);

    // Two adjacent liquidity bands: [-500, 0] and [0, 500].
    client.mint(&lp1, &-500i32, &0i32, &2_000_000u128, &2_000_000u128);
    client.mint(&lp2, &0i32, &500i32, &2_000_000u128, &2_000_000u128);

    let client_a = soroban_sdk::token::Client::new(&e, &token_a);
    let client_b = soroban_sdk::token::Client::new(&e, &token_b);

    let bal_b_before = client_b.balance(&trader);

    // Large swap that should cross tick 0 and continue into the [-500, 0] band.
    let (amt_in, amt_out) = client.swap(&trader, &true, &3_000_000u128);

    assert!(amt_in > 0);
    assert!(amt_out > 0);
    // Trader paid token A and received token B.
    assert!(client_b.balance(&trader) > bal_b_before, "trader should have received token B");
    let _ = client_a.balance(&trader); // just access to ensure no panic
}

#[test]
fn test_collect_fees_after_swap() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let (token_a, token_b, admin_a, admin_b) = setup_tokens(&e);
    let client = setup_amm(&e, &token_a, &token_b);

    let lp = Address::generate(&e);
    let trader = Address::generate(&e);

    admin_a.mint(&lp, &10_000_000);
    admin_b.mint(&lp, &10_000_000);
    admin_a.mint(&trader, &1_000_000);
    admin_b.mint(&trader, &1_000_000);

    client.mint(&lp, &-500i32, &500i32, &5_000_000u128, &5_000_000u128);

    // Do several swaps to accumulate fees.
    client.swap(&trader, &true, &50_000u128);
    client.swap(&trader, &false, &50_000u128);
    client.swap(&trader, &true, &50_000u128);

    // LP collects fees; should get some positive amount of at least one token.
    let (fee0, fee1) = client.collect_fees(&lp, &-500i32, &500i32);
    assert!(fee0 > 0 || fee1 > 0, "LP should have earned fees from swaps");
}

#[test]
fn test_multiple_lps_independent_positions() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let (token_a, token_b, admin_a, admin_b) = setup_tokens(&e);
    let client = setup_amm(&e, &token_a, &token_b);

    let lp1 = Address::generate(&e);
    let lp2 = Address::generate(&e);

    admin_a.mint(&lp1, &1_000_000);
    admin_b.mint(&lp1, &1_000_000);
    admin_a.mint(&lp2, &1_000_000);
    admin_b.mint(&lp2, &1_000_000);

    // Same range for both LPs.
    let (liq1, _, _) = client.mint(&lp1, &-100i32, &100i32, &100_000u128, &100_000u128);
    let (liq2, _, _) = client.mint(&lp2, &-100i32, &100i32, &100_000u128, &100_000u128);

    assert!(liq1 > 0);
    assert!(liq2 > 0);

    // Burn lp1's position independently.
    let (a1, b1) = client.burn(&lp1, &-100i32, &100i32, &liq1);
    assert!(a1 > 0 || b1 > 0);

    // lp2's position is unaffected.
    let (a2, b2) = client.burn(&lp2, &-100i32, &100i32, &liq2);
    assert!(a2 > 0 || b2 > 0);
}

#[test]
fn test_partial_burn() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let (token_a, token_b, admin_a, admin_b) = setup_tokens(&e);
    let client = setup_amm(&e, &token_a, &token_b);

    let lp = Address::generate(&e);
    admin_a.mint(&lp, &1_000_000);
    admin_b.mint(&lp, &1_000_000);

    let (liq, _, _) = client.mint(&lp, &-100i32, &100i32, &100_000u128, &100_000u128);

    // Burn half the liquidity.
    let half = liq / 2;
    let (a1, b1) = client.burn(&lp, &-100i32, &100i32, &half);
    assert!(a1 > 0 || b1 > 0, "partial burn should recover tokens");

    // Burn the remainder.
    let (a2, b2) = client.burn(&lp, &-100i32, &100i32, &(liq - half));
    assert!(a2 > 0 || b2 > 0, "remaining burn should also recover tokens");
}
