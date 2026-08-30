use crate::{HybridAmmLob, HybridAmmLobClient, PRICE_SCALE};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup(e: &Env) -> (HybridAmmLobClient, Address, Address, Address, Address) {
    let admin = Address::generate(e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let contract_id = e.register(HybridAmmLob, ());
    let client = HybridAmmLobClient::new(e, &contract_id);
    client.initialize(&admin, &token_a, &token_b, &30, &10);
    (client, admin, token_a, token_b, contract_id)
}

fn mint(e: &Env, admin: &Address, token: &Address, to: &Address, amount: i128) {
    soroban_sdk::token::StellarAssetClient::new(e, token).mint(to, &amount);
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
    let (client, admin, token_a, token_b, _) = setup(&e);

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
    let (client, admin, token_a, token_b, _) = setup(&e);

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
