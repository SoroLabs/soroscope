use super::*;
use soroban_sdk::{
    contract, contractimpl, contracttype,
    testutils::{Address as _, Events, Ledger},
    vec, Address, Env, String as SorobanString, TryIntoVal,
};

// Import Vec from alloc for no_std environment
extern crate alloc;
use alloc::vec::Vec;

#[contract]
struct MockOracle;

#[contracttype]
#[derive(Clone)]
enum OracleDataKey {
    Price,
}

#[contractimpl]
impl MockOracle {
    pub fn set_price(e: Env, price: i128) {
        e.storage().instance().set(&OracleDataKey::Price, &price);
    }

    pub fn latest_price(e: Env) -> i128 {
        e.storage()
            .instance()
            .get(&OracleDataKey::Price)
            .unwrap_or(100_000_000i128)
    }
}

#[test]
fn test_basic_flow() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    // Setup tokens
    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_client = soroban_sdk::token::Client::new(&e, &token_a);
    let token_b_client = soroban_sdk::token::Client::new(&e, &token_b);

    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    // Check initialize
    client.initialize(&admin, &token_a, &token_b);

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

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    client.initialize(&admin, &token_a, &token_b);
    // Should panic with AlreadyInitialized error
    client.initialize(&admin, &token_a, &token_b);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_swap_insufficient_liquidity() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

    // Mint and deposit
    token_a_admin.mint(&user, &1000);
    token_b_admin.mint(&user, &1000);
    client.deposit(&user, &1000, &1000);

    // Try to swap more than reserve
    client.swap(&user, &false, &1000, &10000); // Should panic with InsufficientLiquidity
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_swap_slippage_exceeded() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

    // Mint and deposit
    token_a_admin.mint(&user, &1000);
    token_b_admin.mint(&user, &1000);
    client.deposit(&user, &1000, &1000);

    // Try to swap with very low slippage tolerance
    client.swap(&user, &false, &100, &1); // Should panic with SlippageExceeded
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_withdraw_insufficient_shares() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

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

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user1 = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

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
    let _shares = client.deposit(&user1, &1000, &1000);

    // Check balances
    assert_eq!(client.total_supply(), _shares);
    assert_eq!(client.balance(&user1), _shares);
}

#[test]
fn test_transfer() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

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
#[should_panic(expected = "Error(Contract, #4)")]
fn test_transfer_insufficient_balance() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

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

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

    // Mint tokens to users
    token_a_admin.mint(&user1, &2000);
    token_b_admin.mint(&user1, &2000);
    token_a_admin.mint(&user2, &1000);
    token_b_admin.mint(&user2, &1000);

    // === Test Deposit Event ===
    let deposit_shares = client.deposit(&user1, &1000, &1000);

    let events = e.events().all();
    let deposit_event_name = String::from_str(&e, "deposit");
    let deposit_events: Vec<_> = events
        .iter()
        .filter(|(_, topics, _)| {
            if topics.len() != 2 {
                return false;
            }
            // Compare by converting Val to String
            let topic_str: Result<SorobanString, _> = topics.get(0).unwrap().try_into_val(&e);
            topic_str.is_ok() && topic_str.unwrap() == deposit_event_name
        })
        .collect();

    // Should have exactly one deposit event
    assert_eq!(deposit_events.len(), 1);

    // Verify deposit event data
    let (contract_addr, topics, data) = &deposit_events[0];
    assert_eq!(
        contract_addr, &contract_id,
        "Event should be emitted from liquidity pool contract"
    );

    // Convert topic Val to Address for comparison
    let topic_user: Address = topics.get(1).unwrap().try_into_val(&e).unwrap();
    assert_eq!(
        topic_user, user1,
        "Deposit event should contain user1 address in topics"
    );

    // Convert data Val to DepositEvent
    let deposit_event: DepositEvent = data.try_into_val(&e).unwrap();
    assert_eq!(deposit_event.user, user1);
    assert_eq!(deposit_event.amount_a, 1000);
    assert_eq!(deposit_event.amount_b, 1000);
    assert_eq!(deposit_event.shares_minted, deposit_shares);

    // === Test Swap Event ===
    let out_amount = 100;
    let in_max = 150;
    let amount_paid = client.swap(&user2, &false, &out_amount, &in_max);

    let events = e.events().all();
    let swap_event_name = String::from_str(&e, "swap");
    let swap_events: Vec<_> = events
        .iter()
        .filter(|(_, topics, _)| {
            if topics.len() != 2 {
                return false;
            }
            // Compare by converting Val to String
            let topic_str: Result<SorobanString, _> = topics.get(0).unwrap().try_into_val(&e);
            topic_str.is_ok() && topic_str.unwrap() == swap_event_name
        })
        .collect();

    // Should have exactly one swap event
    assert_eq!(swap_events.len(), 1);

    // Verify swap event data
    let (contract_addr, topics, data) = &swap_events[0];
    assert_eq!(
        contract_addr, &contract_id,
        "Event should be emitted from liquidity pool contract"
    );

    // Convert topic Val to Address for comparison
    let topic_user: Address = topics.get(1).unwrap().try_into_val(&e).unwrap();
    assert_eq!(
        topic_user, user2,
        "Swap event should contain user2 address in topics"
    );

    // Convert data Val to SwapEvent
    let swap_event: SwapEvent = data.try_into_val(&e).unwrap();
    assert_eq!(swap_event.user, user2);
    // buy_a = false means we're buying token B (token_out) by paying token A (token_in)
    assert_eq!(swap_event.token_in, token_a);
    assert_eq!(swap_event.token_out, token_b);
    assert_eq!(swap_event.amount_in, amount_paid);
    assert_eq!(swap_event.amount_out, out_amount);

    // === Test Withdraw Event ===
    let withdraw_shares = 500;
    let (withdrawn_a, withdrawn_b) = client.withdraw(&user1, &withdraw_shares);

    let events = e.events().all();
    let withdraw_event_name = String::from_str(&e, "withdraw");
    let withdraw_events: Vec<_> = events
        .iter()
        .filter(|(_, topics, _)| {
            if topics.len() != 2 {
                return false;
            }
            // Compare by converting Val to String
            let topic_str: Result<SorobanString, _> = topics.get(0).unwrap().try_into_val(&e);
            topic_str.is_ok() && topic_str.unwrap() == withdraw_event_name
        })
        .collect();

    // Should have exactly one withdraw event
    assert_eq!(withdraw_events.len(), 1);

    // Verify withdraw event data
    let (contract_addr, topics, data) = &withdraw_events[0];
    assert_eq!(
        contract_addr, &contract_id,
        "Event should be emitted from liquidity pool contract"
    );

    // Convert topic Val to Address for comparison
    let topic_user: Address = topics.get(1).unwrap().try_into_val(&e).unwrap();
    assert_eq!(
        topic_user, user1,
        "Withdraw event should contain user1 address in topics"
    );

    // Convert data Val to WithdrawEvent
    let withdraw_event: WithdrawEvent = data.try_into_val(&e).unwrap();
    assert_eq!(withdraw_event.user, user1);
    assert_eq!(withdraw_event.shares_burned, withdraw_shares);
    assert_eq!(withdraw_event.amount_a, withdrawn_a);
    assert_eq!(withdraw_event.amount_b, withdrawn_b);

    // Verify all event types are present
    assert!(!deposit_events.is_empty());
    assert!(!swap_events.is_empty());
    assert!(!withdraw_events.is_empty());
}

// ===== Allowance and TransferFrom Tests =====

#[test]
fn test_approve() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user1 = Address::generate(&e);
    let spender = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

    // Mint and deposit to get shares
    token_a_admin.mint(&user1, &1000);
    token_b_admin.mint(&user1, &1000);
    let _shares = client.deposit(&user1, &1000, &1000);

    // Approve spender to use 500 shares
    let expiration_ledger = e.ledger().sequence() + 1000;
    client.approve(&user1, &spender, &500, &expiration_ledger);

    // Check allowance
    assert_eq!(client.allowance(&user1, &spender), 500);

    // Try to approve more - should overwrite
    client.approve(&user1, &spender, &300, &expiration_ledger);
    assert_eq!(client.allowance(&user1, &spender), 300);
}

#[test]
fn test_approve_expired() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user1 = Address::generate(&e);
    let spender = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

    // Mint and deposit to get shares
    token_a_admin.mint(&user1, &1000);
    token_b_admin.mint(&user1, &1000);
    client.deposit(&user1, &1000, &1000);

    // Approve with short expiration
    let expiration_ledger = e.ledger().sequence() + 10;
    client.approve(&user1, &spender, &500, &expiration_ledger);

    // Advance ledger to expire allowance
    let mut ledger_info = e.ledger().get();
    ledger_info.sequence_number += 15;
    e.ledger().set(ledger_info);

    // Check that allowance is now 0 (expired)
    assert_eq!(client.allowance(&user1, &spender), 0);
}

#[test]
fn test_transfer_from() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);
    let spender = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

    // Mint and deposit to get shares
    token_a_admin.mint(&user1, &1000);
    token_b_admin.mint(&user1, &1000);
    let shares = client.deposit(&user1, &1000, &1000);

    // Approve spender to use 500 shares
    let expiration_ledger = e.ledger().sequence() + 1000;
    client.approve(&user1, &spender, &500, &expiration_ledger);

    // Spender transfers 200 shares from user1 to user2
    client.transfer_from(&spender, &user1, &user2, &200);

    // Check balances
    assert_eq!(client.balance(&user1), shares - 200);
    assert_eq!(client.balance(&user2), 200);
    assert_eq!(client.allowance(&user1, &spender), 300); // 500 - 200 = 300 remaining

    // Spender transfers remaining 300 shares
    client.transfer_from(&spender, &user1, &user2, &300);

    // Check final balances
    assert_eq!(client.balance(&user1), shares - 500);
    assert_eq!(client.balance(&user2), 500);
    assert_eq!(client.allowance(&user1, &spender), 0); // Allowance depleted
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_transfer_from_insufficient_allowance() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);
    let spender = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

    // Mint and deposit to get shares
    token_a_admin.mint(&user1, &1000);
    token_b_admin.mint(&user1, &1000);
    client.deposit(&user1, &1000, &1000);

    // Approve only 100 shares
    let expiration_ledger = e.ledger().sequence() + 1000;
    client.approve(&user1, &spender, &100, &expiration_ledger);

    // Try to transfer 200 shares (more than approved)
    client.transfer_from(&spender, &user1, &user2, &200); // Should panic with InsufficientBalance
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_transfer_from_insufficient_balance() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);
    let spender = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

    // Mint and deposit to get shares
    token_a_admin.mint(&user1, &1000);
    token_b_admin.mint(&user1, &1000);
    let shares = client.deposit(&user1, &1000, &1000);

    // Approve more shares than user has (should still fail on balance check)
    let expiration_ledger = e.ledger().sequence() + 1000;
    let allowance_amount = shares + 100;
    client.approve(&user1, &spender, &allowance_amount, &expiration_ledger);

    // Try to transfer more than user's balance
    let transfer_amount = shares + 50;
    client.transfer_from(&spender, &user1, &user2, &transfer_amount); // Should panic with InsufficientBalance
}

// ===== Pausable Functionality Tests =====

#[test]
fn test_pause_and_unpause() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

    // Mint tokens
    token_a_admin.mint(&user, &1000);
    token_b_admin.mint(&user, &1000);

    // Deposit should work when not paused
    let shares = client.deposit(&user, &1000, &1000);
    assert_eq!(shares, 1000);

    // Admin pauses deposits only.
    client.set_operation_paused(&admin, &emergency_guard::PauseType::DEPOSIT, &true);
    assert_eq!(client.try_deposit(&user, &1, &1), Err(Ok(Error::Paused)));

    // Unpause deposits.
    client.set_operation_paused(&admin, &emergency_guard::PauseType::DEPOSIT, &false);
    client.guard_pause(&admin, &pause_op::DEPOSIT, &true);
    assert!(client.guard_is_paused(&pause_op::DEPOSIT));
    assert!(!client.guard_is_paused(&pause_op::SWAP));

    // Unpause deposits.
    client.guard_pause(&admin, &pause_op::DEPOSIT, &false);
    assert!(!client.guard_is_paused(&pause_op::DEPOSIT));
    // Admin pauses deposits and swaps together.
    client.guard_pause(
        &admin,
        &(emergency_guard::PauseType::DEPOSIT | emergency_guard::PauseType::SWAP),
        &true,
    );
    assert!(client.guard_is_paused(&emergency_guard::PauseType::DEPOSIT));
    assert!(client.guard_is_paused(&emergency_guard::PauseType::SWAP));
    assert!(!client.guard_is_paused(&emergency_guard::PauseType::WITHDRAW));

    // Unpause deposits only, leaving swaps paused.
    client.guard_pause(&admin, &emergency_guard::PauseType::DEPOSIT, &false);
    assert!(!client.guard_is_paused(&emergency_guard::PauseType::DEPOSIT));
    assert!(client.guard_is_paused(&emergency_guard::PauseType::SWAP));

    // Operations should work again
    token_a_admin.mint(&user, &500);
    token_b_admin.mint(&user, &500);
    let more_shares = client.deposit(&user, &500, &500);
    assert!(more_shares > 0);
}

#[test]
fn test_emergency_guard_trait_impl() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);
    let admin3 = Address::generate(&e);
    let _admins = soroban_sdk::vec![&e, admin1.clone(), admin2.clone(), admin3.clone()];

    let token_a = e
        .register_stellar_asset_contract_v2(admin1.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin1.clone())
        .address();

    client.initialize(&admin1, &token_a, &token_b);

    // `initialize` already seeds the guard with admin1 at threshold 1, so grow the
    // admin set from there instead of calling `init_guard` a second time.
    client.add_admin(&soroban_sdk::vec![&e, admin1.clone()], &admin2);
    client.add_admin(
        &soroban_sdk::vec![&e, admin1.clone(), admin2.clone()],
        &admin3,
    );
    // Lower threshold by rotating to a 3-admin setup — just verify via get_admins/threshold.
    assert!(!client.get_guard_admins().is_empty());

    // Pause SWAP via single admin.
    client.guard_pause(&admin1, &PauseType::SWAP, &true);
    assert!(client.guard_is_paused(&PauseType::SWAP));
    assert!(!client.guard_is_paused(&PauseType::DEPOSIT));

    let approvers = soroban_sdk::vec![&e, admin1.clone(), admin2.clone()];
    // Emergency pause all via multi-sig.
    client.emergency_pause_all(&approvers);
    assert_eq!(client.get_pause_state(), u32::MAX);

    // Resume all via multi-sig.
    client.resume_all(&approvers);
    assert_eq!(client.get_pause_state(), 0);

    // Add and remove admin3 (already added above, so remove it).
    client.remove_admin(
        &soroban_sdk::vec![&e, admin1.clone(), admin2.clone()],
        &admin3,
    );
    assert!(!client.get_guard_admins().iter().any(|a| a == admin3));
}

#[test]
#[should_panic(expected = "Error(Contract, #14)")]
fn test_deposit_when_paused() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

    // Mint tokens
    token_a_admin.mint(&user, &1000);
    token_b_admin.mint(&user, &1000);

    // Pause deposits only.
    client.set_operation_paused(&admin, &emergency_guard::PauseType::DEPOSIT, &true);
    client.guard_pause(&admin, &pause_op::DEPOSIT, &true);

    // Try to deposit - should panic with Paused error
    client.deposit(&user, &1000, &1000);
}

#[test]
#[should_panic(expected = "Error(Contract, #14)")]
fn test_swap_when_paused() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

    // Mint and deposit liquidity first
    token_a_admin.mint(&user, &2000);
    token_b_admin.mint(&user, &2000);
    client.deposit(&user, &1000, &1000);

    // Pause swaps only.
    client.set_operation_paused(&admin, &emergency_guard::PauseType::SWAP, &true);
    client.guard_pause(&admin, &pause_op::SWAP, &true);

    // Try to swap - should panic with Paused error
    client.swap(&user, &false, &100, &200);
}

#[test]
#[should_panic(expected = "Error(Contract, #14)")]
fn test_withdraw_when_paused() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

    // Mint and deposit liquidity first
    token_a_admin.mint(&user, &1000);
    token_b_admin.mint(&user, &1000);
    let shares = client.deposit(&user, &1000, &1000);

    // Pause withdrawals only.
    client.set_operation_paused(&admin, &emergency_guard::PauseType::WITHDRAW, &true);
    client.guard_pause(&admin, &pause_op::WITHDRAW, &true);

    // Try to withdraw - should panic with Paused error
    client.withdraw(&user, &shares);
}

#[test]
fn test_pause_deposit_only_allows_swap_and_withdraw() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);
    token_a_admin.mint(&user, &3_000);
    token_b_admin.mint(&user, &3_000);
    let shares = client.deposit(&user, &1_000, &1_000);

    client.set_operation_paused(&admin, &emergency_guard::PauseType::DEPOSIT, &true);

    assert_eq!(
        client.try_deposit(&user, &100, &100),
        Err(Ok(Error::Paused))
    );
    assert!(client.swap(&user, &false, &50, &100) > 0);
    let (withdrawn_a, withdrawn_b) = client.withdraw(&user, &(shares / 10));
    assert!(withdrawn_a > 0);
    assert!(withdrawn_b > 0);
}

#[test]
fn test_pause_swap_only_allows_deposit_and_withdraw() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);
    token_a_admin.mint(&user, &3_000);
    token_b_admin.mint(&user, &3_000);
    client.deposit(&user, &1_000, &1_000);

    client.set_operation_paused(&admin, &emergency_guard::PauseType::SWAP, &true);

    assert_eq!(
        client.try_swap(&user, &false, &50, &100),
        Err(Ok(Error::Paused))
    );
    let added_shares = client.deposit(&user, &500, &500);
    assert!(added_shares > 0);
    assert_eq!(client.withdraw(&user, &100), (100, 100));
}

#[test]
fn test_pause_withdraw_only_allows_deposit_and_swap() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);
    token_a_admin.mint(&user, &3_000);
    token_b_admin.mint(&user, &3_000);
    let shares = client.deposit(&user, &1_000, &1_000);

    client.set_operation_paused(&admin, &emergency_guard::PauseType::WITHDRAW, &true);

    assert_eq!(
        client.try_withdraw(&user, &(shares / 10)),
        Err(Ok(Error::Paused))
    );
    let added_shares = client.deposit(&user, &500, &500);
    assert!(added_shares > 0);
    assert!(client.swap(&user, &false, &50, &100) > 0);
}

// ===== Admin Fee Control Tests =====

#[test]
fn test_get_fee_default() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    client.initialize(&admin, &token_a, &token_b);

    // Default fee should be 30 bps
    assert_eq!(client.get_fee(), 30);
}

#[test]
fn test_set_fee_valid() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    client.initialize(&admin, &token_a, &token_b);

    // Admin updates fee to 10 bps
    client.set_fee(&10);
    assert_eq!(client.get_fee(), 10);

    // 0 bps (free swaps)
    client.set_fee(&0);
    assert_eq!(client.get_fee(), 0);

    // 100 bps (1%)
    client.set_fee(&100);
    assert_eq!(client.get_fee(), 100);
}

#[test]
#[should_panic(expected = "Error(Contract, #9)")]
fn test_set_fee_above_max() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    client.initialize(&admin, &token_a, &token_b);

    // 101 bps — should panic with InvalidFee
    client.set_fee(&101);
}

#[test]
fn test_oracle_fee_scheduling_and_timelock_execution() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    client.initialize(&admin, &token_a, &token_b);

    let oracle_id = e.register(MockOracle, ());
    let oracle = MockOracleClient::new(&e, &oracle_id);
    oracle.set_price(&100_000_000);

    client.configure_fee_oracle(&oracle_id, &30, &5);

    // First sync seeds the initial price and does not schedule.
    let first_sync = client.sync_fee_from_oracle();
    assert!(first_sync.is_none());

    // 10% jump (1000 bps) should schedule high-volatility fee (100 bps).
    oracle.set_price(&110_000_000);
    let scheduled = client.sync_fee_from_oracle();
    assert!(scheduled.is_some());
    let pending = scheduled.unwrap();
    assert_eq!(pending.new_fee_bps, 100);
    assert_eq!(client.get_last_volatility_bps(), 1000);

    // Not executable before timelock.
    let early = client.try_execute_fee_update();
    assert_eq!(early, Err(Ok(Error::TimelockNotElapsed)));

    // Advance ledgers and execute.
    let mut ledger_info = e.ledger().get();
    ledger_info.sequence_number = pending.executable_after_ledger;
    e.ledger().set(ledger_info);

    let new_fee = client.execute_fee_update();
    assert_eq!(new_fee, 100);
    assert_eq!(client.get_fee(), 100);
    assert!(client.get_pending_fee_update().is_none());
}

#[test]
fn test_oracle_low_volatility_keeps_base_fee() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    client.initialize(&admin, &token_a, &token_b);

    let oracle_id = e.register(MockOracle, ());
    let oracle = MockOracleClient::new(&e, &oracle_id);
    oracle.set_price(&100_000_000);
    client.configure_fee_oracle(&oracle_id, &30, &3);

    assert!(client.sync_fee_from_oracle().is_none());
    // 0.2% move => 20 bps, below low threshold.
    oracle.set_price(&100_200_000);
    assert!(client.sync_fee_from_oracle().is_none());
    assert_eq!(client.get_fee(), 30);
}

#[test]
fn test_burn() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

    // Mint and deposit
    token_a_admin.mint(&user, &1000);
    token_b_admin.mint(&user, &1000);
    let _shares = client.deposit(&user, &1000, &1000);

    let supply_before = client.total_supply();
    let balance_before = client.balance(&user);

    // Burn 400 shares
    client.burn(&user, &400);

    let supply_after = client.total_supply();
    let balance_after = client.balance(&user);

    assert_eq!(supply_before - 400, supply_after);
    assert_eq!(balance_before - 400, balance_after);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_burn_insufficient_shares() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

    token_a_admin.mint(&user, &1000);
    token_b_admin.mint(&user, &1000);
    client.deposit(&user, &1000, &1000);

    // Try to burn more than user has
    client.burn(&user, &2000);
}

// ===== EmergencyGuard Admin Rotation Tests =====

#[test]
fn test_emergency_guard_admin_initialized_with_pool_admin() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    client.initialize(&admin, &token_a, &token_b);

    let admins = client.get_admins();
    assert_eq!(admins.len(), 1);
    assert_eq!(admins.get(0).unwrap(), admin);
    assert_eq!(client.get_admin_threshold(), 1);
    assert_eq!(client.get_admin(), admin);
}

#[test]
fn test_rotate_admin_replaces_guard_and_pool_admin() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let new_admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    client.initialize(&admin, &token_a, &token_b);
    client.rotate_admin(&vec![&e, admin.clone()], &admin, &new_admin);

    let admins = client.get_admins();
    assert_eq!(admins.len(), 1);
    assert_eq!(admins.get(0).unwrap(), new_admin);
    assert_eq!(client.get_admin(), new_admin);

    assert_eq!(
        client.try_set_operation_paused(&admin, &pause_op::SWAP, &true),
        Err(Ok(Error::Unauthorized))
    );

    client.set_operation_paused(&new_admin, &pause_op::SWAP, &true);
    assert_eq!(
        client.try_set_operation_paused(&new_admin, &pause_op::SWAP, &false),
        Ok(Ok(()))
    );
}

#[test]
fn test_rotated_admin_controls_pool_admin_functions() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let new_admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    client.initialize(&admin, &token_a, &token_b);
    client.rotate_admin(&vec![&e, admin.clone()], &admin, &new_admin);

    assert_eq!(client.try_set_fee(&99), Ok(Ok(())));
    assert_eq!(client.get_fee(), 99);
    assert_eq!(client.get_admin(), new_admin);
}

#[test]
fn test_failed_rotate_admin_does_not_add_new_admin() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let missing_old_admin = Address::generate(&e);
    let new_admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    client.initialize(&admin, &token_a, &token_b);

    assert_eq!(
        client.try_rotate_admin(&vec![&e, admin.clone()], &missing_old_admin, &new_admin),
        Err(Ok(emergency_guard::GuardError::Unauthorized))
    );

    let admins = client.get_admins();
    assert_eq!(admins.len(), 1);
    assert_eq!(admins.get(0).unwrap(), admin);
    assert!(!admins.iter().any(|a| a == new_admin));
    assert_eq!(client.get_admin(), admin);
}

#[test]
fn test_add_then_remove_admin_enforces_rotation_membership() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let new_admin = Address::generate(&e);
    let stranger = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    client.initialize(&admin, &token_a, &token_b);

    assert_eq!(
        client.try_add_admin(&vec![&e, stranger.clone()], &new_admin),
        Err(Ok(emergency_guard::GuardError::Unauthorized))
    );

    client.add_admin(&vec![&e, admin.clone()], &new_admin);
    let admins = client.get_admins();
    assert_eq!(admins.len(), 2);
    assert!(admins.iter().any(|a| a == admin));
    assert!(admins.iter().any(|a| a == new_admin));

    client.remove_admin(&vec![&e, admin.clone()], &admin);
    let admins = client.get_admins();
    assert_eq!(admins.len(), 1);
    assert_eq!(admins.get(0).unwrap(), new_admin);
    assert_eq!(client.get_admin(), new_admin);

    assert_eq!(
        client.try_remove_admin(&vec![&e, new_admin.clone()], &new_admin),
        Err(Ok(emergency_guard::GuardError::Unauthorized))
    );
}

#[test]
fn test_rotated_admin_controls_emergency_pause_and_guard_unpause() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let new_admin = Address::generate(&e);
    let user = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    client.initialize(&admin, &token_a, &token_b);
    client.rotate_admin(&vec![&e, admin.clone()], &admin, &new_admin);

    token_a_admin.mint(&user, &2_000);
    token_b_admin.mint(&user, &2_000);

    client.emergency_pause(&vec![&e, new_admin.clone()]);
    assert_eq!(
        client.try_deposit(&user, &1_000, &1_000),
        Err(Ok(Error::Paused))
    );

    assert_eq!(
        client.try_guard_unpause(&vec![&e, admin.clone()]),
        Err(Ok(Error::Unauthorized))
    );

    client.guard_unpause(&vec![&e, new_admin.clone()]);
    assert_eq!(client.deposit(&user, &1_000, &1_000), 1_000);
}

// ===== Zero-Value Edge Case Tests =====

#[test]
fn test_deposit_zero_amount() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

    // Mint tokens so the user has balance for subsequent tests
    token_a_admin.mint(&user, &10000);
    token_b_admin.mint(&user, &10000);

    // --- Scenario 1: First deposit with both amounts = 0 ---
    // sqrt(0 * 0) = 0, so 0 shares should be minted without panicking.
    let shares = client.deposit(&user, &0, &0);
    assert_eq!(
        shares, 0,
        "Depositing (0, 0) as first liquidity must mint 0 shares"
    );
    assert_eq!(
        client.total_supply(),
        0,
        "Total supply must remain 0 after zero deposit"
    );

    // --- Scenario 2: Seed the pool with real liquidity, then deposit zero ---
    let initial_shares = client.deposit(&user, &1000, &1000);
    assert_eq!(
        initial_shares, 1000,
        "Initial deposit should mint sqrt(1000*1000) = 1000 shares"
    );

    let token_a_client = soroban_sdk::token::Client::new(&e, &token_a);
    let token_b_client = soroban_sdk::token::Client::new(&e, &token_b);
    let balance_a_before = token_a_client.balance(&user);
    let balance_b_before = token_b_client.balance(&user);

    // Deposit 0 of each into a pool that already has reserves
    // Proportional formula: min(0 * total / reserve_a, 0 * total / reserve_b) = 0
    let zero_shares = client.deposit(&user, &0, &0);
    assert_eq!(
        zero_shares, 0,
        "Depositing (0, 0) into funded pool must mint 0 shares"
    );
    assert_eq!(
        client.total_supply(),
        initial_shares,
        "Total supply must be unchanged after zero deposit"
    );

    // Verify user token balances are unchanged (no tokens transferred)
    assert_eq!(
        token_a_client.balance(&user),
        balance_a_before,
        "Token A balance must be unchanged after zero deposit"
    );
    assert_eq!(
        token_b_client.balance(&user),
        balance_b_before,
        "Token B balance must be unchanged after zero deposit"
    );

    // --- Scenario 3: Only one amount is zero on initial-like deposit ---
    // Deposit with amount_a = 0 and amount_b > 0 into the funded pool
    // min(0 * total / reserve_a, amount_b * total / reserve_b) = 0
    let one_zero_shares = client.deposit(&user, &0, &500);
    assert_eq!(
        one_zero_shares, 0,
        "Depositing (0, 500) must mint 0 shares (limited by zero side)"
    );
}

// ===== Staking and Rewards Tests =====

#[test]
fn test_stake_basic() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

    // Mint and deposit to get shares
    token_a_admin.mint(&user, &1000);
    token_b_admin.mint(&user, &1000);
    let shares = client.deposit(&user, &1000, &1000);

    // Verify balance before staking
    assert_eq!(client.balance(&user), shares);
    assert_eq!(client.get_staked_balance(&user), 0);

    // Stake half of the shares
    let stake_amount = shares / 2;
    client.stake(&user, &stake_amount);

    // Verify balances after staking
    assert_eq!(client.balance(&user), shares - stake_amount);
    assert_eq!(client.get_staked_balance(&user), stake_amount);
    assert_eq!(client.get_total_staked(), stake_amount);
}

#[test]
fn test_stake_insufficient_balance() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

    token_a_admin.mint(&user, &1000);
    token_b_admin.mint(&user, &1000);
    let shares = client.deposit(&user, &1000, &1000);

    // Try to stake more than available
    assert_eq!(
        client.try_stake(&user, &(shares + 1)),
        Err(Ok(Error::InsufficientBalance))
    );
    assert!(client.try_stake(&user, &(shares + 1)).is_err());
}

#[test]
#[should_panic(expected = "Error(Contract, #14)")]
fn test_stake_when_paused() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

    token_a_admin.mint(&user, &1000);
    token_b_admin.mint(&user, &1000);
    let shares = client.deposit(&user, &1000, &1000);

    // Pause the contract
    client.set_paused(&true);

    // Try to stake - should panic with Paused error
    client.stake(&user, &shares);
}

#[test]
fn test_unstake_basic() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

    token_a_admin.mint(&user, &1000);
    token_b_admin.mint(&user, &1000);
    let shares = client.deposit(&user, &1000, &1000);

    // Stake all shares
    client.stake(&user, &shares);
    assert_eq!(client.get_staked_balance(&user), shares);

    // Unstake half
    let unstake_amount = shares / 2;
    client.unstake(&user, &unstake_amount);

    // Verify balances after unstaking
    assert_eq!(client.balance(&user), unstake_amount);
    assert_eq!(client.get_staked_balance(&user), shares - unstake_amount);
    assert_eq!(client.get_total_staked(), shares - unstake_amount);
}

#[test]
fn test_unstake_insufficient_staked() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

    token_a_admin.mint(&user, &1000);
    token_b_admin.mint(&user, &1000);
    let shares = client.deposit(&user, &1000, &1000);

    client.stake(&user, &(shares / 2));

    // Try to unstake more than staked
    assert_eq!(
        client.try_unstake(&user, &shares),
        Err(Ok(Error::InsufficientShares))
    );
    assert!(client.try_unstake(&user, &shares).is_err());
}

#[test]
#[should_panic(expected = "Error(Contract, #14)")]
fn test_unstake_when_paused() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

    token_a_admin.mint(&user, &1000);
    token_b_admin.mint(&user, &1000);
    let shares = client.deposit(&user, &1000, &1000);

    client.stake(&user, &shares);

    // Pause the contract
    client.set_paused(&true);

    // Try to unstake - should panic with Paused error
    client.unstake(&user, &shares);
}

#[test]
fn test_claim_rewards_basic() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

    token_a_admin.mint(&user, &1000);
    token_b_admin.mint(&user, &1000);
    let shares = client.deposit(&user, &1000, &1000);

    // Stake shares
    client.stake(&user, &shares);

    // No rewards initially
    assert_eq!(client.get_pending_rewards(&user), 0);

    // Advance ledger to accumulate rewards
    {
        let mut info = e.ledger().get();
        info.sequence_number = 100;
        e.ledger().set(info);
    }
    let mut ledger_info = e.ledger().get();
    ledger_info.sequence_number = 100;
    e.ledger().set(ledger_info);

    // Now there should be pending rewards
    let pending = client.get_pending_rewards(&user);
    assert!(pending > 0);

    // Claim rewards
    let claimed = client.claim_rewards(&user);
    assert_eq!(claimed, pending);

    // After claiming, pending should be 0
    assert_eq!(client.get_pending_rewards(&user), 0);
}

#[test]
fn test_claim_rewards_no_stake() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let user = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

    // Try to claim rewards with no stake
    let claimed = client.claim_rewards(&user);
    assert_eq!(claimed, 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #14)")]
fn test_claim_rewards_when_paused() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

    token_a_admin.mint(&user, &1000);
    token_b_admin.mint(&user, &1000);
    let shares = client.deposit(&user, &1000, &1000);

    client.stake(&user, &shares);
    {
        let mut info = e.ledger().get();
        info.sequence_number = 100;
        e.ledger().set(info);
    }
    let mut ledger_info = e.ledger().get();
    ledger_info.sequence_number = 100;
    e.ledger().set(ledger_info);

    // Pause the contract
    client.set_paused(&true);

    // Try to claim rewards - should panic with Paused error
    client.claim_rewards(&user);
}

#[test]
fn test_stake_unstake_claim_full_cycle() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

    // Deposit
    token_a_admin.mint(&user, &1000);
    token_b_admin.mint(&user, &1000);
    let shares = client.deposit(&user, &1000, &1000);

    // Stake
    client.stake(&user, &shares);
    assert_eq!(client.get_staked_balance(&user), shares);

    // Advance ledger
    {
        let mut info = e.ledger().get();
        info.sequence_number = 50;
        e.ledger().set(info);
    }
    let mut ledger_info = e.ledger().get();
    ledger_info.sequence_number = 50;
    e.ledger().set(ledger_info);

    // Claim some rewards
    let first_claim = client.claim_rewards(&user);
    assert!(first_claim > 0);

    // Advance more
    {
        let mut info = e.ledger().get();
        info.sequence_number = 100;
        e.ledger().set(info);
    }
    let mut ledger_info = e.ledger().get();
    ledger_info.sequence_number = 100;
    e.ledger().set(ledger_info);

    // Claim more rewards
    let second_claim = client.claim_rewards(&user);
    assert!(second_claim > 0);

    // Unstake all
    client.unstake(&user, &shares);
    assert_eq!(client.get_staked_balance(&user), 0);
    assert_eq!(client.balance(&user), shares);
}

#[test]
fn test_multiple_users_staking() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(&e, &contract_id);

    let admin = Address::generate(&e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);

    e.cost_estimate().budget().reset_unlimited();

    client.initialize(&admin, &token_a, &token_b);

    // User 1 deposits and stakes
    token_a_admin.mint(&user1, &1000);
    token_b_admin.mint(&user1, &1000);
    let shares1 = client.deposit(&user1, &1000, &1000);
    client.stake(&user1, &shares1);

    // User 2 deposits and stakes
    token_a_admin.mint(&user2, &2000);
    token_b_admin.mint(&user2, &2000);
    let shares2 = client.deposit(&user2, &2000, &2000);
    client.stake(&user2, &shares2);

    // Verify total staked
    assert_eq!(client.get_total_staked(), shares1 + shares2);

    // Advance and claim
    let mut ledger_info = e.ledger().get();
    ledger_info.sequence_number = 100;
    e.ledger().set(ledger_info);

    let rewards1 = client.claim_rewards(&user1);
    let rewards2 = client.claim_rewards(&user2);

    // User2 staked more, so should get more rewards (approximately 2x)
    assert!(rewards2 > rewards1);
}

// ── Slippage protection (issue #650) ─────────────────────────────────────────

/// Pool seeded with `reserve` of each token, plus two traders each funded with
/// `trader_funds` of both tokens. Returns the pool client, the two traders, and
/// both token clients for balance assertions.
fn slippage_fixture<'a>(
    e: &'a Env,
    reserve: i128,
    trader_funds: i128,
) -> (
    LiquidityPoolClient<'a>,
    Address,
    Address,
    soroban_sdk::token::Client<'a>,
    soroban_sdk::token::Client<'a>,
) {
    let contract_id = e.register(LiquidityPool, ());
    let client = LiquidityPoolClient::new(e, &contract_id);

    let admin = Address::generate(e);
    let token_a = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_b = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(e, &token_b);

    let lp = Address::generate(e);
    let trader = Address::generate(e);
    let other = Address::generate(e);

    e.cost_estimate().budget().reset_unlimited();
    client.initialize(&admin, &token_a, &token_b);

    token_a_admin.mint(&lp, &reserve);
    token_b_admin.mint(&lp, &reserve);
    client.deposit(&lp, &reserve, &reserve);

    for account in [&trader, &other] {
        token_a_admin.mint(account, &trader_funds);
        token_b_admin.mint(account, &trader_funds);
    }

    (
        client,
        trader,
        other,
        soroban_sdk::token::Client::new(e, &token_a),
        soroban_sdk::token::Client::new(e, &token_b),
    )
}

#[test]
fn test_swap_exact_in_delivers_quoted_output() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, trader, _other, token_a, token_b) = slippage_fixture(&e, 1000, 1000);

    // 100 A in against 1000/1000 reserves at the default 30 bps fee.
    let quote = client.get_amount_out(&false, &100);
    assert_eq!(quote, 90);

    // Quoting min_amount_out at exactly the quote must still fill: the state the
    // quote was read from is the state the swap executes against.
    let received = client.swap_exact_in(&trader, &false, &100, &quote);
    assert_eq!(received, quote);

    // Exactly `amount_in` leaves the trader, exactly `received` arrives.
    assert_eq!(token_a.balance(&trader), 1000 - 100);
    assert_eq!(token_b.balance(&trader), 1000 + received);
}

#[test]
fn test_swap_exact_in_rejects_output_below_minimum() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, trader, _other, token_a, token_b) = slippage_fixture(&e, 1000, 1000);

    let quote = client.get_amount_out(&false, &100);

    // Asking for one unit more than the pool can deliver must revert, not fill.
    assert_eq!(
        client.try_swap_exact_in(&trader, &false, &100, &(quote + 1)),
        Err(Ok(Error::SlippageExceeded))
    );

    // A rejected swap moves nothing.
    assert_eq!(token_a.balance(&trader), 1000);
    assert_eq!(token_b.balance(&trader), 1000);
    assert_eq!(client.get_amount_out(&false, &100), quote);
}

/// The sandwich this parameter exists to stop: a quote is taken, an attacker
/// front-runs to move the price, and the victim's swap must abort rather than
/// fill at the worse rate.
#[test]
fn test_swap_exact_in_min_amount_out_blocks_front_run() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, victim, attacker, _token_a, token_b) = slippage_fixture(&e, 1000, 1000);

    // Victim quotes, then the attacker moves the price ahead of them.
    let quote = client.get_amount_out(&false, &100);
    client.swap_exact_in(&attacker, &false, &500, &0);

    // The same input now buys materially less.
    let degraded = client.get_amount_out(&false, &100);
    assert!(
        degraded < quote,
        "front-run should degrade the quote: {degraded} vs {quote}"
    );

    // With min_amount_out pinned to the original quote, the victim is protected.
    assert_eq!(
        client.try_swap_exact_in(&victim, &false, &100, &quote),
        Err(Ok(Error::SlippageExceeded))
    );
    assert_eq!(token_b.balance(&victim), 1000);

    // A min the victim would actually have accepted still fills.
    let received = client.swap_exact_in(&victim, &false, &100, &degraded);
    assert_eq!(received, degraded);
}

#[test]
fn test_swap_exact_in_zero_minimum_accepts_any_fill() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, trader, _other, _token_a, token_b) = slippage_fixture(&e, 1000, 1000);

    // min_amount_out = 0 opts out of the check entirely.
    let received = client.swap_exact_in(&trader, &false, &100, &0);
    assert!(received > 0);
    assert_eq!(token_b.balance(&trader), 1000 + received);
}

#[test]
fn test_swap_exact_in_buy_a_direction() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, trader, _other, token_a, token_b) = slippage_fixture(&e, 1000, 1000);

    // buy_a: token B goes in, token A comes out.
    let quote = client.get_amount_out(&true, &100);
    let received = client.swap_exact_in(&trader, &true, &100, &quote);

    assert_eq!(received, quote);
    assert_eq!(token_b.balance(&trader), 1000 - 100);
    assert_eq!(token_a.balance(&trader), 1000 + received);

    // Slippage is enforced in this direction too.
    assert_eq!(
        client.try_swap_exact_in(&trader, &true, &100, &(quote * 2)),
        Err(Ok(Error::SlippageExceeded))
    );
}

#[test]
fn test_swap_exact_in_rejects_invalid_amounts() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, trader, _other, _token_a, _token_b) = slippage_fixture(&e, 1000, 1000);

    assert_eq!(
        client.try_swap_exact_in(&trader, &false, &0, &0),
        Err(Ok(Error::InvalidAmount))
    );
    assert_eq!(
        client.try_swap_exact_in(&trader, &false, &-100, &0),
        Err(Ok(Error::InvalidAmount))
    );
    // A negative floor would silently disable the check, so reject it.
    assert_eq!(
        client.try_swap_exact_in(&trader, &false, &100, &-1),
        Err(Ok(Error::InvalidAmount))
    );
    assert_eq!(
        client.try_get_amount_out(&false, &0),
        Err(Ok(Error::InvalidAmount))
    );
    assert_eq!(
        client.try_get_amount_in(&false, &0),
        Err(Ok(Error::InvalidAmount))
    );
}

#[test]
fn test_swap_exact_in_input_too_small_to_fill() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, trader, _other, _token_a, _token_b) = slippage_fixture(&e, 1_000_000, 1000);

    // 1 unit against a 1M/1M pool rounds down to zero output. Settling that would
    // take the input and hand back nothing.
    assert_eq!(client.get_amount_out(&false, &1), 0);
    assert_eq!(
        client.try_swap_exact_in(&trader, &false, &1, &0),
        Err(Ok(Error::InsufficientLiquidity))
    );
}

#[test]
fn test_swap_exact_in_respects_swap_pause() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, trader, _other, _token_a, _token_b) = slippage_fixture(&e, 1000, 1000);

    client.pause_swaps();
    assert_eq!(
        client.try_swap_exact_in(&trader, &false, &100, &0),
        Err(Ok(Error::Paused))
    );

    client.resume_swaps();
    assert!(client.swap_exact_in(&trader, &false, &100, &0) > 0);
}

#[test]
fn test_swap_exact_in_preserves_constant_product() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, trader, _other, _token_a, _token_b) = slippage_fixture(&e, 1000, 1000);

    let before = client.get_reserves();
    let product_before = before.0 * before.1;

    client.swap_exact_in(&trader, &false, &100, &0);

    let after = client.get_reserves();
    // Fees plus truncation in the pool's favour mean the invariant only grows.
    assert!(
        after.0 * after.1 >= product_before,
        "invariant weakened: {} -> {}",
        product_before,
        after.0 * after.1
    );
}

#[test]
fn test_swap_exact_in_charges_the_pool_fee() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, _trader, _other, _token_a, _token_b) = slippage_fixture(&e, 1_000_000, 1000);

    client.set_fee(&0);
    let without_fee = client.get_amount_out(&false, &100_000);

    client.set_fee(&MAX_FEE_BPS);
    let with_fee = client.get_amount_out(&false, &100_000);

    assert!(
        with_fee < without_fee,
        "fee should reduce output: {with_fee} vs {without_fee}"
    );
}

#[test]
fn test_amount_in_quote_round_trips_against_amount_out() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, _trader, _other, _token_a, _token_b) = slippage_fixture(&e, 1000, 1000);

    let quoted_out = client.get_amount_out(&false, &100);
    let quoted_in = client.get_amount_in(&false, &quoted_out);

    // Rounding always favours the pool, so re-quoting never asks for less than
    // the original input.
    assert!(
        quoted_in >= 100,
        "round trip under-charged: {quoted_in} for {quoted_out} out"
    );
}
