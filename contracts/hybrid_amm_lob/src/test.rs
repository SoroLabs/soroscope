use crate::{
    Guards, HybridAmmLob, HybridAmmLobClient, DEFAULT_MAX_MATCH_DEPTH,
    DEFAULT_MAX_PRICE_DEVIATION_BPS, MAX_MATCH_DEPTH_LIMIT, PRICE_SCALE,
};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env,
};

/// Set up a pool with the default safeguards (depth 8, 5% deviation).
fn setup(e: &Env) -> (HybridAmmLobClient<'_>, Address, Address, Address, Address) {
    setup_with(e, DEFAULT_MAX_MATCH_DEPTH, DEFAULT_MAX_PRICE_DEVIATION_BPS)
}

fn setup_with(
    e: &Env,
    max_match_depth: u32,
    max_price_deviation_bps: i128,
) -> (HybridAmmLobClient<'_>, Address, Address, Address, Address) {
    let admin = Address::generate(e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let contract_id = e.register(HybridAmmLob, ());
    let client = HybridAmmLobClient::new(e, &contract_id);
    client.initialize(
        &admin,
        &token_a,
        &token_b,
        &30,
        &10,
        &Guards {
            max_match_depth,
            max_price_deviation_bps,
        },
    );
    (client, admin, token_a, token_b, contract_id)
}

fn mint(e: &Env, _admin: &Address, token: &Address, to: &Address, amount: i128) {
    soroban_sdk::token::StellarAssetClient::new(e, token).mint(to, &amount);
}

fn guards(max_match_depth: u32, max_price_deviation_bps: i128) -> Guards {
    Guards {
        max_match_depth,
        max_price_deviation_bps,
    }
}

fn balance(e: &Env, token: &Address, who: &Address) -> i128 {
    soroban_sdk::token::Client::new(e, token).balance(who)
}

// ── Liquidity ─────────────────────────────────────────────────────────────────

#[test]
fn test_deposit_and_withdraw() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, token_a, token_b, _) = setup(&e);

    let lp = Address::generate(&e);
    mint(&e, &admin, &token_a, &lp, 10_000);
    mint(&e, &admin, &token_b, &lp, 10_000);

    let shares = client.deposit(&lp, &1_000, &1_000);
    assert_eq!(shares, 1_000); // sqrt(1000*1000)
    assert_eq!(client.lp_balance(&lp), 1_000);

    let (out_a, out_b) = client.withdraw(&lp, &500);
    assert_eq!(out_a, 500);
    assert_eq!(out_b, 500);
    assert_eq!(client.lp_balance(&lp), 500);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_withdraw_too_many_shares() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, token_a, token_b, _) = setup(&e);

    let lp = Address::generate(&e);
    mint(&e, &admin, &token_a, &lp, 1_000);
    mint(&e, &admin, &token_b, &lp, 1_000);
    client.deposit(&lp, &1_000, &1_000);
    client.withdraw(&lp, &2_000); // more than owned
}

// ── Order placement & priority sorting ───────────────────────────────────────

#[test]
fn test_ask_priority_sorting() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, token_a, _token_b, _) = setup(&e);

    let maker1 = Address::generate(&e);
    let maker2 = Address::generate(&e);
    let maker3 = Address::generate(&e);
    mint(&e, &admin, &token_a, &maker1, 1_000);
    mint(&e, &admin, &token_a, &maker2, 1_000);
    mint(&e, &admin, &token_a, &maker3, 1_000);

    // Place asks at prices 1.2, 1.0, 1.1 (should sort ascending: 1.0, 1.1, 1.2)
    client.place_order(&maker1, &false, &(12 * PRICE_SCALE / 10), &100);
    client.place_order(&maker2, &false, &(10 * PRICE_SCALE / 10), &100);
    client.place_order(&maker3, &false, &(11 * PRICE_SCALE / 10), &100);

    let asks = client.get_asks();
    assert_eq!(asks.len(), 3);
    assert!(asks.get(0).unwrap().price <= asks.get(1).unwrap().price);
    assert!(asks.get(1).unwrap().price <= asks.get(2).unwrap().price);
}

#[test]
fn test_bid_priority_sorting() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, _token_a, token_b, _) = setup(&e);

    let maker1 = Address::generate(&e);
    let maker2 = Address::generate(&e);
    mint(&e, &admin, &token_b, &maker1, 10_000);
    mint(&e, &admin, &token_b, &maker2, 10_000);

    // Place bids at prices 0.9 and 1.1 (should sort descending: 1.1, 0.9)
    client.place_order(&maker1, &true, &(9 * PRICE_SCALE / 10), &100);
    client.place_order(&maker2, &true, &(11 * PRICE_SCALE / 10), &100);

    let bids = client.get_bids();
    assert_eq!(bids.len(), 2);
    assert!(bids.get(0).unwrap().price >= bids.get(1).unwrap().price);
}

// ── Cancel order ──────────────────────────────────────────────────────────────

#[test]
fn test_cancel_ask() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, token_a, _, _) = setup(&e);

    let maker = Address::generate(&e);
    mint(&e, &admin, &token_a, &maker, 1_000);

    let id = client.place_order(&maker, &false, &PRICE_SCALE, &500);
    assert_eq!(client.get_asks().len(), 1);

    client.cancel_order(&maker, &id);
    assert_eq!(client.get_asks().len(), 0);

    // Token refunded
    let bal = soroban_sdk::token::Client::new(&e, &token_a).balance(&maker);
    assert_eq!(bal, 1_000);
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_cancel_unauthorized() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, token_a, _, _) = setup(&e);

    let maker = Address::generate(&e);
    let attacker = Address::generate(&e);
    mint(&e, &admin, &token_a, &maker, 1_000);

    let id = client.place_order(&maker, &false, &PRICE_SCALE, &500);
    client.cancel_order(&attacker, &id); // should panic
}

// ── LOB fill ──────────────────────────────────────────────────────────────────

#[test]
fn test_swap_fully_filled_by_lob() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, token_a, token_b, _) = setup(&e);

    // Maker places ask: sell 200 token_a at price 1.0 (1 token_b per token_a).
    let maker = Address::generate(&e);
    mint(&e, &admin, &token_a, &maker, 200);
    client.place_order(&maker, &false, &PRICE_SCALE, &200);

    // Taker buys 100 token_a.
    let taker = Address::generate(&e);
    mint(&e, &admin, &token_b, &taker, 10_000);

    let result = client.swap(&taker, &true, &100, &200);

    assert_eq!(result.lob_filled, 100);
    assert_eq!(result.amm_filled, 0);
    // Taker received 100 token_a
    let ta_bal = soroban_sdk::token::Client::new(&e, &token_a).balance(&taker);
    assert_eq!(ta_bal, 100);
    // 1 ask still has 100 remaining
    assert_eq!(client.get_asks().get(0).unwrap().amount, 100);
}

// ── AMM fallback ──────────────────────────────────────────────────────────────

#[test]
fn test_swap_amm_fallback_when_no_orders() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, token_a, token_b, _) = setup(&e);

    // Seed AMM liquidity.
    let lp = Address::generate(&e);
    mint(&e, &admin, &token_a, &lp, 10_000);
    mint(&e, &admin, &token_b, &lp, 10_000);
    client.deposit(&lp, &10_000, &10_000);

    let taker = Address::generate(&e);
    mint(&e, &admin, &token_b, &taker, 5_000);

    let result = client.swap(&taker, &true, &100, &200);

    assert_eq!(result.lob_filled, 0);
    assert_eq!(result.amm_filled, 100);
    assert!(result.amount_in > 0 && result.amount_in <= 200);
}

// ── Hybrid fill (LOB + AMM) ───────────────────────────────────────────────────

#[test]
fn test_swap_hybrid_lob_then_amm() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, token_a, token_b, _) = setup(&e);

    // Seed AMM.
    let lp = Address::generate(&e);
    mint(&e, &admin, &token_a, &lp, 10_000);
    mint(&e, &admin, &token_b, &lp, 10_000);
    client.deposit(&lp, &10_000, &10_000);

    // Maker places ask for only 50 token_a.
    let maker = Address::generate(&e);
    mint(&e, &admin, &token_a, &maker, 50);
    client.place_order(&maker, &false, &PRICE_SCALE, &50);

    // Taker wants 150 token_a: 50 from LOB, 100 from AMM.
    let taker = Address::generate(&e);
    mint(&e, &admin, &token_b, &taker, 5_000);

    let result = client.swap(&taker, &true, &150, &500);

    assert_eq!(result.lob_filled, 50);
    assert_eq!(result.amm_filled, 100);
    assert_eq!(result.amount_out, 150);
    // LOB order fully consumed.
    assert_eq!(client.get_asks().len(), 0);
}

// ── Slippage guard ────────────────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_swap_slippage_exceeded() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, token_a, token_b, _) = setup(&e);

    let lp = Address::generate(&e);
    mint(&e, &admin, &token_a, &lp, 10_000);
    mint(&e, &admin, &token_b, &lp, 10_000);
    client.deposit(&lp, &10_000, &10_000);

    let taker = Address::generate(&e);
    mint(&e, &admin, &token_b, &taker, 5_000);

    // in_max = 1 is impossibly tight.
    client.swap(&taker, &true, &100, &1);
}

// ── Fee distribution ──────────────────────────────────────────────────────────

#[test]
fn test_lp_fee_accrues_in_reserves() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, token_a, token_b, _) = setup(&e);

    let lp = Address::generate(&e);
    mint(&e, &admin, &token_a, &lp, 10_000);
    mint(&e, &admin, &token_b, &lp, 10_000);
    client.deposit(&lp, &10_000, &10_000);

    let pool_before = client.get_pool();

    let taker = Address::generate(&e);
    mint(&e, &admin, &token_b, &taker, 5_000);
    client.swap(&taker, &true, &100, &500);

    let pool_after = client.get_pool();
    // reserve_b increased by more than the spot price (fee stayed in pool).
    assert!(pool_after.reserve_b > pool_before.reserve_b);
    // reserve_a decreased by exactly the output.
    assert_eq!(pool_before.reserve_a - pool_after.reserve_a, 100);
}

#[test]
fn test_maker_fee_is_a_premium_on_taker_input() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, token_a, token_b, _) = setup(&e);

    // No AMM liquidity, so the price band is inactive and only the LOB fills.
    let maker = Address::generate(&e);
    mint(&e, &admin, &token_a, &maker, 10_000);
    client.place_order(&maker, &false, &PRICE_SCALE, &10_000);

    let taker = Address::generate(&e);
    mint(&e, &admin, &token_b, &taker, 100_000);

    // 10_000 token_a at 1.0 costs 10_000 token_b, plus 10 bps = 10.
    let result = client.swap(&taker, &true, &10_000, &20_000);

    assert_eq!(result.amount_in, 10_010);
    assert_eq!(result.amount_out, 10_000);
    // The taker still receives exactly the requested output.
    assert_eq!(balance(&e, &token_a, &taker), 10_000);
    // The maker keeps the premium; nothing is stranded in the contract.
    assert_eq!(balance(&e, &token_b, &maker), 10_010);
}

#[test]
fn test_bid_side_lob_fill() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, token_a, token_b, _) = setup(&e);

    // Maker bids for 10_000 token_a at 1.0, escrowing 10_000 token_b.
    let maker = Address::generate(&e);
    mint(&e, &admin, &token_b, &maker, 10_000);
    client.place_order(&maker, &true, &PRICE_SCALE, &10_000);

    // Taker sells token_a to receive 10_000 token_b.
    let taker = Address::generate(&e);
    mint(&e, &admin, &token_a, &taker, 100_000);

    let result = client.swap(&taker, &false, &10_000, &20_000);

    assert_eq!(result.lob_filled, 10_000);
    assert_eq!(result.amm_filled, 0);
    assert_eq!(result.amount_in, 10_010); // 10_000 + 10 bps premium
    assert_eq!(balance(&e, &token_b, &taker), 10_000);
    assert_eq!(balance(&e, &token_a, &maker), 10_010);
    assert_eq!(client.get_bids().len(), 0);
}

// ── Safeguard: execution depth limit ─────────────────────────────────────────

/// Place `n` asks of `size` each at the pool's spot price.
fn seed_asks(
    e: &Env,
    client: &HybridAmmLobClient,
    admin: &Address,
    token_a: &Address,
    n: u32,
    size: i128,
) {
    for _ in 0..n {
        let maker = Address::generate(e);
        mint(e, admin, token_a, &maker, size);
        client.place_order(&maker, &false, &PRICE_SCALE, &size);
    }
}

#[test]
fn test_match_depth_caps_book_consumption() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, token_a, token_b, _) = setup_with(&e, 2, DEFAULT_MAX_PRICE_DEVIATION_BPS);

    // Deep AMM liquidity so the remainder can be priced without tripping the
    // deviation guard, isolating the depth cap.
    let lp = Address::generate(&e);
    mint(&e, &admin, &token_a, &lp, 1_000_000);
    mint(&e, &admin, &token_b, &lp, 1_000_000);
    client.deposit(&lp, &1_000_000, &1_000_000);

    seed_asks(&e, &client, &admin, &token_a, 3, 50);

    let taker = Address::generate(&e);
    mint(&e, &admin, &token_b, &taker, 100_000);

    // 150 output would need all three orders; only two may be consumed, so the
    // last 50 comes from the AMM instead of cascading deeper into the book.
    let result = client.swap(&taker, &true, &150, &100_000);

    assert_eq!(result.lob_filled, 100);
    assert_eq!(result.amm_filled, 50);
    assert_eq!(client.get_match_depth_used(), 2);
    // The third order is untouched and still resting.
    assert_eq!(client.get_asks().len(), 1);
}

#[test]
fn test_match_depth_survives_an_exhausted_budget() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, token_a, token_b, _) = setup_with(&e, 2, DEFAULT_MAX_PRICE_DEVIATION_BPS);

    let lp = Address::generate(&e);
    mint(&e, &admin, &token_a, &lp, 1_000_000);
    mint(&e, &admin, &token_b, &lp, 1_000_000);
    client.deposit(&lp, &1_000_000, &1_000_000);

    seed_asks(&e, &client, &admin, &token_a, 3, 50);

    let taker = Address::generate(&e);
    mint(&e, &admin, &token_b, &taker, 100_000);

    client.swap(&taker, &true, &100, &100_000);
    assert_eq!(client.get_match_depth_remaining(), 0);

    // An exhausted budget must not make the pool unswappable: a later swap in
    // the same ledger still clears via the AMM even though the book has a
    // matchable order resting.
    assert_eq!(client.get_asks().len(), 1);
    let result = client.swap(&taker, &true, &50, &100_000);
    assert_eq!(result.lob_filled, 0);
    assert_eq!(result.amm_filled, 50);
    assert_eq!(client.get_asks().len(), 1);
}

#[test]
fn test_match_depth_allows_swap_within_budget() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, token_a, token_b, _) = setup_with(&e, 3, DEFAULT_MAX_PRICE_DEVIATION_BPS);

    seed_asks(&e, &client, &admin, &token_a, 3, 50);

    let taker = Address::generate(&e);
    mint(&e, &admin, &token_b, &taker, 10_000);

    let result = client.swap(&taker, &true, &150, &10_000);

    assert_eq!(result.lob_filled, 150);
    assert_eq!(result.amm_filled, 0);
    assert_eq!(client.get_match_depth_used(), 3);
    assert_eq!(client.get_match_depth_remaining(), 0);
}

#[test]
fn test_match_depth_accumulates_across_swaps_in_one_ledger() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, token_a, token_b, _) = setup_with(&e, 2, DEFAULT_MAX_PRICE_DEVIATION_BPS);

    let lp = Address::generate(&e);
    mint(&e, &admin, &token_a, &lp, 1_000_000);
    mint(&e, &admin, &token_b, &lp, 1_000_000);
    client.deposit(&lp, &1_000_000, &1_000_000);

    seed_asks(&e, &client, &admin, &token_a, 3, 50);

    let taker = Address::generate(&e);
    mint(&e, &admin, &token_b, &taker, 100_000);

    // First swap consumes the full budget for this ledger.
    client.swap(&taker, &true, &100, &100_000);
    assert_eq!(client.get_match_depth_used(), 2);

    // Splitting into a second call in the same ledger must not buy more depth,
    // so the third resting order stays untouched.
    let result = client.swap(&taker, &true, &50, &100_000);
    assert_eq!(result.lob_filled, 0);
    assert_eq!(client.get_match_depth_used(), 2);
    assert_eq!(client.get_asks().len(), 1);
}

#[test]
fn test_match_depth_resets_on_new_ledger() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, token_a, token_b, _) = setup_with(&e, 2, DEFAULT_MAX_PRICE_DEVIATION_BPS);

    seed_asks(&e, &client, &admin, &token_a, 3, 50);

    let taker = Address::generate(&e);
    mint(&e, &admin, &token_b, &taker, 10_000);

    client.swap(&taker, &true, &100, &10_000);
    assert_eq!(client.get_match_depth_used(), 2);

    // Next ledger: the budget is replenished.
    let seq = e.ledger().sequence();
    e.ledger().set_sequence_number(seq + 1);
    assert_eq!(client.get_match_depth_used(), 0);

    let result = client.swap(&taker, &true, &50, &10_000);
    assert_eq!(result.lob_filled, 50);
    assert_eq!(client.get_match_depth_used(), 1);
}

#[test]
fn test_match_depth_does_not_block_pure_amm_swap() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, token_a, token_b, _) = setup_with(&e, 1, DEFAULT_MAX_PRICE_DEVIATION_BPS);

    let lp = Address::generate(&e);
    mint(&e, &admin, &token_a, &lp, 100_000);
    mint(&e, &admin, &token_b, &lp, 100_000);
    client.deposit(&lp, &100_000, &100_000);

    seed_asks(&e, &client, &admin, &token_a, 1, 50);

    let taker = Address::generate(&e);
    mint(&e, &admin, &token_b, &taker, 10_000);

    // Spends the whole depth budget on the single resting order.
    client.swap(&taker, &true, &50, &10_000);
    assert_eq!(client.get_match_depth_remaining(), 0);

    // The book is empty now, so this swap never touches the depth guard.
    let result = client.swap(&taker, &true, &50, &10_000);
    assert_eq!(result.lob_filled, 0);
    assert_eq!(result.amm_filled, 50);
}

#[test]
fn test_depth_cap_without_amm_liquidity_reverts_cleanly() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, token_a, token_b, _) = setup_with(&e, 2, DEFAULT_MAX_PRICE_DEVIATION_BPS);

    // No AMM to absorb the remainder, so the capped swap cannot complete.
    seed_asks(&e, &client, &admin, &token_a, 3, 50);

    let taker = Address::generate(&e);
    mint(&e, &admin, &token_b, &taker, 10_000);

    let res = client.try_swap(&taker, &true, &150, &10_000);
    assert!(res.is_err());

    // No partial fill survived: the taker paid nothing and the book is intact.
    assert_eq!(balance(&e, &token_b, &taker), 10_000);
    assert_eq!(balance(&e, &token_a, &taker), 0);
    assert_eq!(client.get_asks().len(), 3);
    assert_eq!(client.get_match_depth_used(), 0);
}

// ── Safeguard: limit-order price band ────────────────────────────────────────

#[test]
fn test_lob_order_outside_price_band_is_skipped() {
    let e = Env::default();
    e.mock_all_auths();
    // 5% tolerance against a 1.0 spot price.
    let (client, admin, token_a, token_b, _) = setup_with(&e, 8, 500);

    let lp = Address::generate(&e);
    mint(&e, &admin, &token_a, &lp, 100_000);
    mint(&e, &admin, &token_b, &lp, 100_000);
    client.deposit(&lp, &100_000, &100_000);
    assert_eq!(client.get_spot_price(), PRICE_SCALE);

    // Ask priced at 2.0 — 100% above spot, far outside the band.
    let maker = Address::generate(&e);
    mint(&e, &admin, &token_a, &maker, 100);
    client.place_order(&maker, &false, &(2 * PRICE_SCALE), &100);

    let taker = Address::generate(&e);
    mint(&e, &admin, &token_b, &taker, 10_000);

    let result = client.swap(&taker, &true, &100, &10_000);

    // The out-of-band order was ignored; the AMM priced the whole swap.
    assert_eq!(result.lob_filled, 0);
    assert_eq!(result.amm_filled, 100);
    assert_eq!(client.get_asks().len(), 1);
    assert_eq!(client.get_match_depth_used(), 0);
}

#[test]
fn test_lob_order_inside_price_band_is_matched() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, token_a, token_b, _) = setup_with(&e, 8, 500);

    let lp = Address::generate(&e);
    mint(&e, &admin, &token_a, &lp, 100_000);
    mint(&e, &admin, &token_b, &lp, 100_000);
    client.deposit(&lp, &100_000, &100_000);

    // Ask at 1.02 — 200 bps above spot, inside the 500 bps band.
    let maker = Address::generate(&e);
    mint(&e, &admin, &token_a, &maker, 100);
    client.place_order(&maker, &false, &(102 * PRICE_SCALE / 100), &100);

    let taker = Address::generate(&e);
    mint(&e, &admin, &token_b, &taker, 10_000);

    let result = client.swap(&taker, &true, &100, &10_000);

    assert_eq!(result.lob_filled, 100);
    assert_eq!(result.amm_filled, 0);
    assert_eq!(client.get_asks().len(), 0);
}

#[test]
fn test_bid_outside_price_band_is_skipped() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, token_a, token_b, _) = setup_with(&e, 8, 500);

    let lp = Address::generate(&e);
    mint(&e, &admin, &token_a, &lp, 100_000);
    mint(&e, &admin, &token_b, &lp, 100_000);
    client.deposit(&lp, &100_000, &100_000);

    // Bid at 0.5 — 50% below spot, far outside the band.
    let maker = Address::generate(&e);
    mint(&e, &admin, &token_b, &maker, 10_000);
    client.place_order(&maker, &true, &(PRICE_SCALE / 2), &100);

    let taker = Address::generate(&e);
    mint(&e, &admin, &token_a, &taker, 10_000);

    let result = client.swap(&taker, &false, &100, &10_000);

    assert_eq!(result.lob_filled, 0);
    assert_eq!(result.amm_filled, 100);
    assert_eq!(client.get_bids().len(), 1);
}

#[test]
fn test_price_band_inactive_without_pool_reference() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, token_a, token_b, _) = setup_with(&e, 8, 500);

    // No AMM liquidity at all, so there is no reference price to band against
    // and the book remains fully usable.
    let maker = Address::generate(&e);
    mint(&e, &admin, &token_a, &maker, 100);
    client.place_order(&maker, &false, &(5 * PRICE_SCALE), &100);

    let taker = Address::generate(&e);
    mint(&e, &admin, &token_b, &taker, 10_000);

    let result = client.swap(&taker, &true, &100, &10_000);
    assert_eq!(result.lob_filled, 100);
}

// ── Safeguard: pool price deviation ──────────────────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #10)")]
fn test_amm_swap_beyond_deviation_tolerance_reverts() {
    let e = Env::default();
    e.mock_all_auths();
    // 1% tolerance on a deliberately thin pool.
    let (client, admin, token_a, token_b, _) = setup_with(&e, 8, 100);

    let lp = Address::generate(&e);
    mint(&e, &admin, &token_a, &lp, 1_000);
    mint(&e, &admin, &token_b, &lp, 1_000);
    client.deposit(&lp, &1_000, &1_000);

    let taker = Address::generate(&e);
    mint(&e, &admin, &token_b, &taker, 100_000);

    // Taking 10% of the reserve moves spot ~23%, well past the 1% tolerance.
    client.swap(&taker, &true, &100, &100_000);
}

#[test]
fn test_amm_swap_within_deviation_tolerance_succeeds() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, token_a, token_b, _) = setup_with(&e, 8, 500);

    let lp = Address::generate(&e);
    mint(&e, &admin, &token_a, &lp, 100_000);
    mint(&e, &admin, &token_b, &lp, 100_000);
    client.deposit(&lp, &100_000, &100_000);

    let taker = Address::generate(&e);
    mint(&e, &admin, &token_b, &taker, 100_000);

    // 0.1% of the reserve barely moves the price.
    let result = client.swap(&taker, &true, &100, &100_000);
    assert_eq!(result.amm_filled, 100);
}

#[test]
fn test_deviation_rejection_leaves_reserves_untouched() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, token_a, token_b, _) = setup_with(&e, 8, 100);

    let lp = Address::generate(&e);
    mint(&e, &admin, &token_a, &lp, 1_000);
    mint(&e, &admin, &token_b, &lp, 1_000);
    client.deposit(&lp, &1_000, &1_000);

    let before = client.get_pool();

    let taker = Address::generate(&e);
    mint(&e, &admin, &token_b, &taker, 100_000);
    let res = client.try_swap(&taker, &true, &100, &100_000);
    assert!(res.is_err());

    let after = client.get_pool();
    assert_eq!(before.reserve_a, after.reserve_a);
    assert_eq!(before.reserve_b, after.reserve_b);
    assert_eq!(balance(&e, &token_b, &taker), 100_000);
    assert_eq!(client.get_spot_price(), PRICE_SCALE);
}

#[test]
fn test_raising_tolerance_admits_the_same_swap() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, token_a, token_b, _) = setup_with(&e, 8, 100);

    let lp = Address::generate(&e);
    mint(&e, &admin, &token_a, &lp, 1_000);
    mint(&e, &admin, &token_b, &lp, 1_000);
    client.deposit(&lp, &1_000, &1_000);

    let taker = Address::generate(&e);
    mint(&e, &admin, &token_b, &taker, 100_000);
    assert!(client.try_swap(&taker, &true, &100, &100_000).is_err());

    // Admin widens the band; the previously rejected swap now clears.
    client.set_guards(&guards(8, 5_000));
    let result = client.swap(&taker, &true, &100, &100_000);
    assert_eq!(result.amm_filled, 100);
}

// ── Guard configuration ──────────────────────────────────────────────────────

#[test]
fn test_initialize_stores_guards() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, _, _, _, _) = setup_with(&e, 5, 250);

    let stored = client.get_guards();
    assert_eq!(stored.max_match_depth, 5);
    assert_eq!(stored.max_price_deviation_bps, 250);
    assert_eq!(client.get_match_depth_remaining(), 5);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_initialize_rejects_zero_depth() {
    let e = Env::default();
    e.mock_all_auths();
    setup_with(&e, 0, 500);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_initialize_rejects_depth_above_hard_limit() {
    let e = Env::default();
    e.mock_all_auths();
    setup_with(&e, MAX_MATCH_DEPTH_LIMIT + 1, 500);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_initialize_rejects_zero_deviation() {
    let e = Env::default();
    e.mock_all_auths();
    setup_with(&e, 8, 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_initialize_rejects_deviation_above_100_percent() {
    let e = Env::default();
    e.mock_all_auths();
    setup_with(&e, 8, 10_001);
}

#[test]
fn test_set_guards_updates_pool() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, _, _, _, _) = setup(&e);

    client.set_guards(&guards(16, 1_000));

    let stored = client.get_guards();
    assert_eq!(stored.max_match_depth, 16);
    assert_eq!(stored.max_price_deviation_bps, 1_000);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_set_guards_rejects_invalid_depth() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, _, _, _, _) = setup(&e);
    client.set_guards(&guards(MAX_MATCH_DEPTH_LIMIT + 1, 500));
}

#[test]
fn test_set_guards_requires_admin_auth() {
    use soroban_sdk::testutils::{MockAuth, MockAuthInvoke};
    use soroban_sdk::IntoVal;

    let e = Env::default();
    e.mock_all_auths();
    let (client, _, _, _, contract_id) = setup(&e);

    // Only a non-admin address authorizes the call, so the admin's
    // require_auth() finds no matching authorization.
    let attacker = Address::generate(&e);
    let res = client
        .mock_auths(&[MockAuth {
            address: &attacker,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "set_guards",
                args: (guards(16, 1_000),).into_val(&e),
                sub_invokes: &[],
            },
        }])
        .try_set_guards(&guards(16, 1_000));

    assert!(res.is_err());
    // Guards are unchanged.
    assert_eq!(client.get_guards().max_match_depth, DEFAULT_MAX_MATCH_DEPTH);
}
