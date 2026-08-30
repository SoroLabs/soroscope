#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, Address, Env,
};

const INITIAL_RATE: i128 = 100_000_000_000_000; // 0.0001 in Fixed (18 decimals)
const DECAY_RATE: i128 = 10_000_000_000_000_000; // 0.01 in Fixed (18 decimals)
const STAKE_AMOUNT: i128 = 10_000;

fn advance_ledger(e: &Env, by: u32) {
    let mut info = e.ledger().get();
    info.sequence_number += by;
    e.ledger().set(info);
}

fn setup() -> (
    Env,
    StakingRewardsClient<'static>,
    Address, // owner
    Address, // staking_token
    Address, // reward_token
) {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let owner = Address::generate(&e);
    let staking_token_admin = Address::generate(&e);
    let staking_token = e
        .register_stellar_asset_contract_v2(staking_token_admin)
        .address();

    let reward_token_admin = Address::generate(&e);
    let reward_token = e
        .register_stellar_asset_contract_v2(reward_token_admin)
        .address();

    let contract_id = e.register(StakingRewards, ());
    let client = StakingRewardsClient::new(&e, &contract_id);

    client.initialize(
        &owner,
        &staking_token,
        &reward_token,
        &INITIAL_RATE,
        &DECAY_RATE,
        &0u32, // start block
    );

    // Mint staking tokens to users later, and mint reward tokens to the contract
    let reward_client = token::StellarAssetClient::new(&e, &reward_token);
    reward_client.mint(&contract_id, &1_000_000_000);

    (e, client, owner, staking_token, reward_token)
}

#[test]
fn test_initialization() {
    let (_, client, owner, staking_token, reward_token) = setup();
    let config = client.get_config();

    assert_eq!(config.owner, owner);
    assert_eq!(config.staking_token, staking_token);
    assert_eq!(config.reward_token, reward_token);
    assert_eq!(config.initial_rate.0, INITIAL_RATE);
    assert_eq!(config.decay_rate.0, DECAY_RATE);
    assert_eq!(config.start_block, 0);
    assert!(!config.is_paused);
}

#[test]
fn test_stake_and_yield_accumulation_no_decay() {
    // Re-initialize with 0 decay rate (alpha = 1)
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let owner = Address::generate(&e);
    let staking_token = e
        .register_stellar_asset_contract_v2(Address::generate(&e))
        .address();
    let reward_token = e
        .register_stellar_asset_contract_v2(Address::generate(&e))
        .address();

    let contract_id = e.register(StakingRewards, ());
    let client = StakingRewardsClient::new(&e, &contract_id);

    client.initialize(
        &owner,
        &staking_token,
        &reward_token,
        &INITIAL_RATE,
        &0i128, // decay_rate = 0
        &10u32, // start_block = 10
    );

    let user = Address::generate(&e);
    let staking_client = token::StellarAssetClient::new(&e, &staking_token);
    staking_client.mint(&user, &STAKE_AMOUNT);

    // Fast forward ledger to block 10
    advance_ledger(&e, 10);

    // Stake at block 10
    client.stake(&user, &STAKE_AMOUNT);

    assert_eq!(client.get_staked_balance(&user), STAKE_AMOUNT);
    assert_eq!(client.get_accrued_rewards(&user), 0);
    assert_eq!(client.get_pending_rewards(&user), 0);

    // Advance 5 blocks (from 10 to 15)
    advance_ledger(&e, 5);

    // Expected multiplier: exp(r0 * 5)
    // r0 = 0.0001, so r0 * 5 = 0.0005
    // exp(0.0005) = 1.00050012502
    // Expected reward = 10,000 * (1.00050012502 - 1) = 5.0012502
    // Truncated to integer: 5
    let pending = client.get_pending_rewards(&user);
    assert_eq!(pending, 5);
}

#[test]
fn test_stake_and_yield_accumulation_with_decay() {
    let (e, client, _, staking_token, _) = setup();
    let user = Address::generate(&e);

    let staking_client = token::StellarAssetClient::new(&e, &staking_token);
    staking_client.mint(&user, &STAKE_AMOUNT);

    // Stake at block 0
    client.stake(&user, &STAKE_AMOUNT);

    // Advance 5 blocks (from 0 to 5)
    advance_ledger(&e, 5);

    // Expected exponent: (r0 / d) * (1 - alpha^5)
    // r0 = 0.0001, d = 0.01, alpha = 0.99
    // alpha^5 = 0.99^5 = 0.9509900499
    // 1 - alpha^5 = 0.0490099501
    // exponent = (0.0001 / 0.01) * 0.0490099501 = 0.000490099501
    // exp(0.000490099501) = 1.0004902196
    // Expected reward = 10,000 * 0.0004902196 = 4.902196 => truncated to 4
    let pending = client.get_pending_rewards(&user);
    assert_eq!(pending, 4);

    // Claim rewards
    client.claim(&user);
    assert_eq!(client.get_accrued_rewards(&user), 0);
    assert_eq!(client.get_pending_rewards(&user), 0);
}

#[test]
fn test_compounding_interest() {
    let (e, client, _, staking_token, _) = setup();
    let user = Address::generate(&e);

    let staking_client = token::StellarAssetClient::new(&e, &staking_token);
    staking_client.mint(&user, &100_000); // larger stake to see compounding clearly

    client.stake(&user, &100_000);

    // Advance 10 blocks (from 0 to 10)
    advance_ledger(&e, 10);

    // Check rewards without claiming
    let pending_1 = client.get_pending_rewards(&user);

    // Let's do another action to write back accrued rewards to storage (e.g. withdraw 0, or we can just let it update)
    // Staking 1 more token triggers a reward update and stores the accrued reward
    staking_client.mint(&user, &1);
    client.stake(&user, &1);

    let accrued = client.get_accrued_rewards(&user);
    assert!(accrued > 0);
    assert_eq!(accrued, pending_1);

    // Now advance another 10 blocks (from 10 to 20)
    advance_ledger(&e, 10);

    // The new pending rewards should compound on (staked_amount + accrued_rewards)
    let pending_2 = client.get_pending_rewards(&user);
    assert!(pending_2 > accrued);
}

#[test]
fn test_zero_stake_security() {
    let (e, client, _, staking_token, _) = setup();
    let user = Address::generate(&e);

    let staking_client = token::StellarAssetClient::new(&e, &staking_token);
    staking_client.mint(&user, &STAKE_AMOUNT);

    client.stake(&user, &STAKE_AMOUNT);
    advance_ledger(&e, 10);

    // User has accrued rewards
    let pending = client.get_pending_rewards(&user);
    assert!(pending > 0);

    // Withdraw entire principal
    client.withdraw(&user, &STAKE_AMOUNT);
    assert_eq!(client.get_staked_balance(&user), 0);

    // Accrued rewards are saved
    let accrued = client.get_accrued_rewards(&user);
    assert_eq!(accrued, pending);

    // Advance another 10 blocks
    advance_ledger(&e, 10);

    // Since stake is 0, compounding is inactive. Accrued rewards must NOT increase!
    let pending_after = client.get_pending_rewards(&user);
    assert_eq!(pending_after, accrued);
}

#[test]
fn test_emergency_withdraw() {
    let (e, client, _, staking_token, _) = setup();
    let user = Address::generate(&e);

    let staking_client = token::StellarAssetClient::new(&e, &staking_token);
    staking_client.mint(&user, &STAKE_AMOUNT);

    client.stake(&user, &STAKE_AMOUNT);
    advance_ledger(&e, 10);

    // Verify rewards accrued
    assert!(client.get_pending_rewards(&user) > 0);

    // Pause contract to simulate extreme conditions
    client.set_paused(&true);

    // Emergency withdraw should succeed even when paused
    let withdrawn = client.emergency_withdraw(&user);
    assert_eq!(withdrawn, STAKE_AMOUNT);

    // Verify stake balance is zero and user state is cleared (rewards forfeited)
    assert_eq!(client.get_staked_balance(&user), 0);
    assert_eq!(client.get_accrued_rewards(&user), 0);
    assert_eq!(client.get_pending_rewards(&user), 0);

    // Verify principal token is fully returned to the user
    let token_balance = token::Client::new(&e, &staking_token).balance(&user);
    assert_eq!(token_balance, STAKE_AMOUNT);
}

#[test]
#[should_panic(expected = "Contract, #14")]
fn test_pause_safeguards_stake() {
    let (e, client, _, staking_token, _) = setup();
    let user = Address::generate(&e);

    let staking_client = token::StellarAssetClient::new(&e, &staking_token);
    staking_client.mint(&user, &STAKE_AMOUNT);

    client.set_paused(&true);
    client.stake(&user, &STAKE_AMOUNT);
}

#[test]
#[should_panic(expected = "Contract, #14")]
fn test_pause_safeguards_withdraw() {
    let (e, client, _, staking_token, _) = setup();
    let user = Address::generate(&e);

    let staking_client = token::StellarAssetClient::new(&e, &staking_token);
    staking_client.mint(&user, &STAKE_AMOUNT);

    client.stake(&user, &STAKE_AMOUNT);
    client.set_paused(&true);
    client.withdraw(&user, &STAKE_AMOUNT);
}

#[test]
#[should_panic(expected = "Contract, #14")]
fn test_pause_safeguards_claim() {
    let (e, client, _, staking_token, _) = setup();
    let user = Address::generate(&e);

    let staking_client = token::StellarAssetClient::new(&e, &staking_token);
    staking_client.mint(&user, &STAKE_AMOUNT);

    client.stake(&user, &STAKE_AMOUNT);
    client.set_paused(&true);
    client.claim(&user);
}
