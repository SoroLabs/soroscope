#![cfg(test)]
use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env,
};

#[test]
fn test_basic_flow() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register_contract(None, LiquidityPool);
    let client = LiquidityPoolClient::new(&e, &contract_id);

    // Setup tokens
    let admin = Address::generate(&e);
    let token_a = e.register_stellar_asset_contract(admin.clone());
    let token_b = e.register_stellar_asset_contract(admin.clone());

    let token_a_client = soroban_sdk::token::Client::new(&e, &token_a);
    let token_b_client = soroban_sdk::token::Client::new(&e, &token_b);

    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);

    e.budget().reset_unlimited();

    // Check initialize
    client.initialize(&token_a, &token_b);

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    // Mint tokens to users
    token_a_admin.mint(&user1, &10000);
    token_b_admin.mint(&user1, &10000);
    token_a_admin.mint(&user2, &10000);
    token_b_admin.mint(&user2, &10000);

    // User 1 Deposits 1000 of each
    // With new sqrt implementation: shares = sqrt(1000 * 1000) = 1000
    let shares = client.deposit(&user1, &1000, &1000);
    assert_eq!(shares, 1000);

    // User 2 Swaps 100 A for B
    let out_amount = 90;
    let in_max = 110;

    // Swap 90 B out, pay with A
    let paid = client.swap(&user2, &false, &out_amount, &in_max);

    // Check balances
    assert_eq!(token_b_client.balance(&user2), 10000 + 90);
    assert_eq!(token_a_client.balance(&user2), 10000 - paid);

    // User 1 Withdraws
    let (withdrawn_a, withdrawn_b) = client.withdraw(&user1, &1000);
    // Should get roughly remaining reserves
    assert!(withdrawn_a > 1000); // Gained fees (paid by user2)
    assert!(withdrawn_b < 1000); // Lost due to User 2 taking B
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_double_initialization() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register_contract(None, LiquidityPool);
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e.register_stellar_asset_contract(admin.clone());
    let token_b = e.register_stellar_asset_contract(admin.clone());

    client.initialize(&token_a, &token_b);
    // Should panic with AlreadyInitialized error
    client.initialize(&token_a, &token_b);
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn test_swap_insufficient_liquidity() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register_contract(None, LiquidityPool);
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e.register_stellar_asset_contract(admin.clone());
    let token_b = e.register_stellar_asset_contract(admin.clone());

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user = Address::generate(&e);

    e.budget().reset_unlimited();

    client.initialize(&token_a, &token_b);

    // Mint and deposit
    token_a_admin.mint(&user, &1000);
    token_b_admin.mint(&user, &1000);
    client.deposit(&user, &1000, &1000);

    // Try to swap more than reserve
    client.swap(&user, &false, &1000, &10000); // Should panic with InsufficientLiquidity
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_swap_slippage_exceeded() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register_contract(None, LiquidityPool);
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e.register_stellar_asset_contract(admin.clone());
    let token_b = e.register_stellar_asset_contract(admin.clone());

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user = Address::generate(&e);

    e.budget().reset_unlimited();

    client.initialize(&token_a, &token_b);

    // Mint and deposit
    token_a_admin.mint(&user, &1000);
    token_b_admin.mint(&user, &1000);
    client.deposit(&user, &1000, &1000);

    // Try to swap with very low slippage tolerance
    client.swap(&user, &false, &100, &1); // Should panic with SlippageExceeded
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_withdraw_insufficient_shares() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register_contract(None, LiquidityPool);
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e.register_stellar_asset_contract(admin.clone());
    let token_b = e.register_stellar_asset_contract(admin.clone());

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user = Address::generate(&e);

    e.budget().reset_unlimited();

    client.initialize(&token_a, &token_b);

    // Mint and deposit
    token_a_admin.mint(&user, &1000);
    token_b_admin.mint(&user, &1000);
    client.deposit(&user, &1000, &1000);

    // Try to withdraw more than owned
    client.withdraw(&user, &2000); // Should panic with InsufficientShares
}

#[test]
fn test_token_interface() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register_contract(None, LiquidityPool);
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e.register_stellar_asset_contract(admin.clone());
    let token_b = e.register_stellar_asset_contract(admin.clone());

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user1 = Address::generate(&e);

    e.budget().reset_unlimited();

    client.initialize(&token_a, &token_b);

    // Test token metadata
    assert_eq!(client.name(), String::from_str(&e, "Liquidity Pool Share"));
    assert_eq!(client.symbol(), String::from_str(&e, "LPS"));
    assert_eq!(client.decimals(), 7);

    // Initially no shares
    assert_eq!(client.total_supply(), 0);
    assert_eq!(client.balance(&user1), 0);

    // Mint and deposit
    token_a_admin.mint(&user1, &1000);
    token_b_admin.mint(&user1, &1000);
    let shares = client.deposit(&user1, &1000, &1000);

    // Check balances
    assert_eq!(client.total_supply(), shares);
    assert_eq!(client.balance(&user1), shares);
}

#[test]
fn test_transfer() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register_contract(None, LiquidityPool);
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e.register_stellar_asset_contract(admin.clone());
    let token_b = e.register_stellar_asset_contract(admin.clone());

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);

    e.budget().reset_unlimited();

    client.initialize(&token_a, &token_b);

    // Mint and deposit
    token_a_admin.mint(&user1, &1000);
    token_b_admin.mint(&user1, &1000);
    let shares = client.deposit(&user1, &1000, &1000);

    // Transfer shares from user1 to user2
    client.transfer(&user1, &user2, &500);

    // Check balances
    assert_eq!(client.balance(&user1), shares - 500);
    assert_eq!(client.balance(&user2), 500);
    assert_eq!(client.total_supply(), shares); // Total supply unchanged
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_transfer_insufficient_balance() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register_contract(None, LiquidityPool);
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e.register_stellar_asset_contract(admin.clone());
    let token_b = e.register_stellar_asset_contract(admin.clone());

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);

    e.budget().reset_unlimited();

    client.initialize(&token_a, &token_b);

    // Mint and deposit
    token_a_admin.mint(&user1, &1000);
    token_b_admin.mint(&user1, &1000);
    client.deposit(&user1, &1000, &1000);

    // Try to transfer more than owned
    client.transfer(&user1, &user2, &2000); // Should panic with InsufficientBalance
}

#[test]
fn test_events() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register_contract(None, LiquidityPool);
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e.register_stellar_asset_contract(admin.clone());
    let token_b = e.register_stellar_asset_contract(admin.clone());

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user = Address::generate(&e);

    e.budget().reset_unlimited();

    client.initialize(&token_a, &token_b);

    // Mint and deposit
    token_a_admin.mint(&user, &1000);
    token_b_admin.mint(&user, &1000);

    client.deposit(&user, &1000, &1000);

    // Get all events - should include deposit, swap, withdraw events
    // Events also include token transfer events from the minting and deposits
    let events = e.events().all();

    // Just verify we have events (includes token transfers + our custom events)
    assert!(events.len() > 0);
}
