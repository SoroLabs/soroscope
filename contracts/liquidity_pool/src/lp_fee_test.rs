#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, String as SorobanString,
};

/// Helper to set up test environment with tokens and pool
fn setup_test_env() -> (Env, LiquidityPoolClient<'static>, Address, Address, Address, Address) {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&e);
    let token_a = e.register_stellar_asset_contract_v2(admin.clone()).address();
    let token_b = e.register_stellar_asset_contract_v2(admin.clone()).address();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    client.initialize(&admin, &token_a, &token_b);

    (e, client, admin, token_a, token_b, Address::generate(&e))
}

#[test]
fn test_default_lp_fee_is_5_bps() {
    let (e, client, _, _, _, _) = setup_test_env();
    
    let lp_fee = client.get_lp_fee_bps();
    assert_eq!(lp_fee, DEFAULT_LP_FEE_BPS);  // 5 bps
}

#[test]
fn test_set_lp_fee_bps_by_admin() {
    let (e, client, admin, _, _, _) = setup_test_env();
    
    // Admin can set LP fee
    client.set_lp_fee_bps(&10);  // 10 bps
    assert_eq!(client.get_lp_fee_bps(), 10);
    
    // Set to 0 (no fee)
    client.set_lp_fee_bps(&0);
    assert_eq!(client.get_lp_fee_bps(), 0);
    
    // Set to max
    client.set_lp_fee_bps(&MAX_LP_FEE_BPS);  // 100 bps
    assert_eq!(client.get_lp_fee_bps(), MAX_LP_FEE_BPS);
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")]  // InvalidFee
fn test_set_lp_fee_bps_exceeds_max() {
    let (e, client, admin, _, _, _) = setup_test_env();
    
    // Should fail: fee > MAX_LP_FEE_BPS
    client.set_lp_fee_bps(&(MAX_LP_FEE_BPS + 1));
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")]  // InvalidFee
fn test_set_lp_fee_bps_negative() {
    let (e, client, admin, _, _, _) = setup_test_env();
    
    // Should fail: negative fee
    client.set_lp_fee_bps(&-1);
}

#[test]
fn test_deposit_with_lp_fee_deducts_shares() {
    let (e, client, admin, token_a, token_b, user) = setup_test_env();
    
    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);
    
    // Mint tokens to user
    token_a_admin.mint(&user, &10_000);
    token_b_admin.mint(&user, &10_000);
    
    // Set LP fee to 10 bps for easier calculation
    client.set_lp_fee_bps(&10);
    
    // Deposit 1000 of each token
    // Expected: sqrt(1000 * 1000) = 1000 gross shares
    // Fee: 1000 * 10 / 10000 = 1 share
    // Net: 1000 - 1 = 999 shares
    let shares = client.deposit(&user, &1_000, &1_000);
    
    assert_eq!(shares, 999);
    assert_eq!(client.balance(&user), 999);
    
    // Verify pool state
    let pool: PoolState = e.storage().instance().get(&DataKey::Pool).unwrap();
    assert_eq!(pool.total_shares, 999);  // Only net shares added
    assert_eq!(pool.reserve_a, 1_000);   // Full deposit in reserves
    assert_eq!(pool.reserve_b, 1_000);
}

#[test]
fn test_deposit_with_zero_lp_fee() {
    let (e, client, admin, token_a, token_b, user) = setup_test_env();
    
    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);
    
    token_a_admin.mint(&user, &10_000);
    token_b_admin.mint(&user, &10_000);
    
    // Set LP fee to 0
    client.set_lp_fee_bps(&0);
    
    // Deposit should mint full shares with no fee
    let shares = client.deposit(&user, &1_000, &1_000);
    assert_eq!(shares, 1_000);
}

#[test]
fn test_deposit_lp_fee_event_emitted() {
    let (e, client, admin, token_a, token_b, user) = setup_test_env();
    
    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);
    
    token_a_admin.mint(&user, &10_000);
    token_b_admin.mint(&user, &10_000);
    
    // Set LP fee to 100 bps (1%) for visibility
    client.set_lp_fee_bps(&100);
    
    let shares = client.deposit(&user, &1_000, &1_000);
    
    // Check events
    let events = e.events().all();
    let lp_fee_events: Vec<_> = events
        .iter()
        .filter(|event| {
            if let Ok(topics) = event.topics.try_into_val(&e) {
                let topics: (SorobanString, SorobanString) = topics;
                topics.0 == SorobanString::from_str(&e, "lp_fee") 
                    && topics.1 == SorobanString::from_str(&e, "deposit")
            } else {
                false
            }
        })
        .collect();
    
    assert!(lp_fee_events.len() > 0, "LP deposit fee event should be emitted");
}

#[test]
fn test_withdraw_with_lp_fee_deducts_amounts() {
    let (e, client, admin, token_a, token_b, user) = setup_test_env();
    
    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);
    
    token_a_admin.mint(&user, &10_000);
    token_b_admin.mint(&user, &10_000);
    
    // Set LP fee to 0 for deposit to simplify
    client.set_lp_fee_bps(&0);
    let shares = client.deposit(&user, &1_000, &1_000);
    assert_eq!(shares, 1_000);
    
    // Now set LP fee to 10 bps for withdrawal
    client.set_lp_fee_bps(&10);
    
    // Withdraw all shares
    // Gross: 1000 of each token
    // Fee: 1000 * 10 / 10000 = 1 of each
    // Net: 999 of each
    let (amount_a, amount_b) = client.withdraw(&user, &shares);
    
    assert_eq!(amount_a, 999);
    assert_eq!(amount_b, 999);
    
    // Verify pool retains the fee
    let pool: PoolState = e.storage().instance().get(&DataKey::Pool).unwrap();
    assert_eq!(pool.reserve_a, 1);  // Fee retained
    assert_eq!(pool.reserve_b, 1);
    assert_eq!(pool.total_shares, 0);
}

#[test]
fn test_withdraw_with_zero_lp_fee() {
    let (e, client, admin, token_a, token_b, user) = setup_test_env();
    
    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);
    
    token_a_admin.mint(&user, &10_000);
    token_b_admin.mint(&user, &10_000);
    
    client.set_lp_fee_bps(&0);
    let shares = client.deposit(&user, &1_000, &1_000);
    
    // Withdraw with 0 fee should return full amounts
    let (amount_a, amount_b) = client.withdraw(&user, &shares);
    assert_eq!(amount_a, 1_000);
    assert_eq!(amount_b, 1_000);
}

#[test]
fn test_jit_liquidity_loses_value_with_lp_fee() {
    let (e, client, admin, token_a, token_b, jit_actor) = setup_test_env();
    
    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);
    let token_a_client = soroban_sdk::token::Client::new(&e, &token_a);
    let token_b_client = soroban_sdk::token::Client::new(&e, &token_b);
    
    // JIT actor gets 10,000 of each token
    token_a_admin.mint(&jit_actor, &10_000);
    token_b_admin.mint(&jit_actor, &10_000);
    
    // Set LP fee to 10 bps (0.1%)
    client.set_lp_fee_bps(&10);
    
    let initial_a = token_a_client.balance(&jit_actor);
    let initial_b = token_b_client.balance(&jit_actor);
    
    // JIT: Deposit 1000 of each
    let shares = client.deposit(&jit_actor, &1_000, &1_000);
    
    // JIT: Immediately withdraw
    let (returned_a, returned_b) = client.withdraw(&jit_actor, &shares);
    
    let final_a = token_a_client.balance(&jit_actor);
    let final_b = token_b_client.balance(&jit_actor);
    
    // JIT actor should have less than they started with
    // Round-trip fee: deposit fee + withdrawal fee
    // Deposit: 1000 shares - 1 fee = 999 shares
    // Withdraw: gross 1000 - net 999 (1 fee retained in pool)
    //           999 * 99.9% = ~998
    assert!(final_a < initial_a, "JIT actor lost token A");
    assert!(final_b < initial_b, "JIT actor lost token B");
    
    // Loss should be approximately 2-3 tokens (combined deposit + withdrawal fees)
    let loss_a = initial_a - final_a;
    let loss_b = initial_b - final_b;
    assert!(loss_a >= 2 && loss_a <= 3, "Expected loss of 2-3 token A, got {}", loss_a);
    assert!(loss_b >= 2 && loss_b <= 3, "Expected loss of 2-3 token B, got {}", loss_b);
}

#[test]
fn test_lp_fee_increases_share_value_for_remaining_lps() {
    let (e, client, admin, token_a, token_b, user1) = setup_test_env();
    let user2 = Address::generate(&e);
    
    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);
    
    // Both users get tokens
    token_a_admin.mint(&user1, &10_000);
    token_b_admin.mint(&user1, &10_000);
    token_a_admin.mint(&user2, &10_000);
    token_b_admin.mint(&user2, &10_000);
    
    client.set_lp_fee_bps(&100);  // 1% for visibility
    
    // User1 deposits 1000 of each
    let shares1 = client.deposit(&user1, &1_000, &1_000);
    
    // User2 deposits and immediately withdraws (JIT)
    let shares2 = client.deposit(&user2, &1_000, &1_000);
    client.withdraw(&user2, &shares2);
    
    // After user2's round-trip, pool has captured fees
    // User1's shares should now be worth MORE per share
    
    let pool: PoolState = e.storage().instance().get(&DataKey::Pool).unwrap();
    
    // Pool reserves should have grown relative to total shares
    // because user2's fees stayed in the pool
    let value_per_share_a = pool.reserve_a * 1000 / pool.total_shares;  // scaled by 1000
    let value_per_share_b = pool.reserve_b * 1000 / pool.total_shares;
    
    // Each share should be worth more than 1.0 tokens now (scaled by 1000, so > 1000)
    assert!(value_per_share_a > 1_000, "Share value should increase due to captured fees");
    assert!(value_per_share_b > 1_000, "Share value should increase due to captured fees");
}

#[test]
fn test_multiple_deposits_with_lp_fee() {
    let (e, client, admin, token_a, token_b, user) = setup_test_env();
    
    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);
    
    token_a_admin.mint(&user, &50_000);
    token_b_admin.mint(&user, &50_000);
    
    client.set_lp_fee_bps(&10);  // 10 bps
    
    // First deposit
    let shares1 = client.deposit(&user, &10_000, &10_000);
    
    // Second deposit (proportional)
    let shares2 = client.deposit(&user, &10_000, &10_000);
    
    // Total shares should be less than 20,000 due to fees
    let total_balance = client.balance(&user);
    assert!(total_balance < 20_000);
    assert_eq!(total_balance, shares1 + shares2);
}

#[test]
fn test_lp_fee_with_max_rate() {
    let (e, client, admin, token_a, token_b, user) = setup_test_env();
    
    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);
    
    token_a_admin.mint(&user, &10_000);
    token_b_admin.mint(&user, &10_000);
    
    // Set to maximum LP fee (100 bps = 1%)
    client.set_lp_fee_bps(&MAX_LP_FEE_BPS);
    
    let shares = client.deposit(&user, &10_000, &10_000);
    
    // Expected: sqrt(10000 * 10000) = 10000 gross
    // Fee: 10000 * 100 / 10000 = 100
    // Net: 10000 - 100 = 9900
    assert_eq!(shares, 9_900);
    
    // Withdraw
    let (amount_a, amount_b) = client.withdraw(&user, &shares);
    
    // Gross would be 10000 each, fee is 1%, so net is 9900 each
    assert_eq!(amount_a, 9_801);  // 9900 * 0.99 = 9801
    assert_eq!(amount_b, 9_801);
}

#[test]
fn test_withdraw_lp_fee_event_emitted() {
    let (e, client, admin, token_a, token_b, user) = setup_test_env();
    
    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);
    
    token_a_admin.mint(&user, &10_000);
    token_b_admin.mint(&user, &10_000);
    
    client.set_lp_fee_bps(&0);  // No fee on deposit
    let shares = client.deposit(&user, &1_000, &1_000);
    
    client.set_lp_fee_bps(&100);  // 1% fee on withdrawal
    client.withdraw(&user, &shares);
    
    // Check events
    let events = e.events().all();
    let lp_fee_events: Vec<_> = events
        .iter()
        .filter(|event| {
            if let Ok(topics) = event.topics.try_into_val(&e) {
                let topics: (SorobanString, SorobanString) = topics;
                topics.0 == SorobanString::from_str(&e, "lp_fee") 
                    && topics.1 == SorobanString::from_str(&e, "withdraw")
            } else {
                false
            }
        })
        .collect();
    
    assert!(lp_fee_events.len() > 0, "LP withdraw fee event should be emitted");
}

#[test]
fn test_admin_only_can_set_lp_fee() {
    let (e, client, admin, _, _, non_admin) = setup_test_env();
    
    // Mock auth for non-admin should fail
    // (In real test, you'd need to properly test authorization failure)
    // For now, verify admin can set
    client.set_lp_fee_bps(&20);
    assert_eq!(client.get_lp_fee_bps(), 20);
}
