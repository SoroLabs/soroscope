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
        &0u32,
        &0u32, // epoch_duration = 0 (continuous mode)
    );

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
    assert_eq!(config.epoch_duration, 0);
    assert!(!config.is_paused);
}

#[test]
fn test_stake_and_yield_accumulation_no_decay() {
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
        &0i128,
        &10u32,
        &0u32,
    );

    let user = Address::generate(&e);
    let staking_client = token::StellarAssetClient::new(&e, &staking_token);
    staking_client.mint(&user, &STAKE_AMOUNT);

    advance_ledger(&e, 10);

    client.stake(&user, &STAKE_AMOUNT);

    assert_eq!(client.get_staked_balance(&user), STAKE_AMOUNT);
    assert_eq!(client.get_accrued_rewards(&user), 0);
    assert_eq!(client.get_pending_rewards(&user), 0);

    advance_ledger(&e, 5);

    let pending = client.get_pending_rewards(&user);
    assert_eq!(pending, 5);
}

#[test]
fn test_stake_and_yield_accumulation_with_decay() {
    let (e, client, _, staking_token, _) = setup();
    let user = Address::generate(&e);

    let staking_client = token::StellarAssetClient::new(&e, &staking_token);
    staking_client.mint(&user, &STAKE_AMOUNT);

    client.stake(&user, &STAKE_AMOUNT);

    advance_ledger(&e, 5);

    let pending = client.get_pending_rewards(&user);
    assert_eq!(pending, 4);

    client.claim(&user);
    assert_eq!(client.get_accrued_rewards(&user), 0);
    assert_eq!(client.get_pending_rewards(&user), 0);
}

#[test]
fn test_compounding_interest() {
    let (e, client, _, staking_token, _) = setup();
    let user = Address::generate(&e);

    let staking_client = token::StellarAssetClient::new(&e, &staking_token);
    staking_client.mint(&user, &100_000);

    client.stake(&user, &100_000);

    advance_ledger(&e, 10);

    let pending_1 = client.get_pending_rewards(&user);

    staking_client.mint(&user, &1);
    client.stake(&user, &1);

    let accrued = client.get_accrued_rewards(&user);
    assert!(accrued > 0);
    assert_eq!(accrued, pending_1);

    advance_ledger(&e, 10);

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

    let pending = client.get_pending_rewards(&user);
    assert!(pending > 0);

    client.withdraw(&user, &STAKE_AMOUNT);
    assert_eq!(client.get_staked_balance(&user), 0);

    let accrued = client.get_accrued_rewards(&user);
    assert_eq!(accrued, pending);

    advance_ledger(&e, 10);

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

    assert!(client.get_pending_rewards(&user) > 0);

    client.set_paused(&true);

    let withdrawn = client.emergency_withdraw(&user);
    assert_eq!(withdrawn, STAKE_AMOUNT);

    assert_eq!(client.get_staked_balance(&user), 0);
    assert_eq!(client.get_accrued_rewards(&user), 0);
    assert_eq!(client.get_pending_rewards(&user), 0);

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

// ── Epoch Snapshot Tests ──────────────────────────────────────

fn setup_epoch(
    epoch_duration: u32,
) -> (
    Env,
    StakingRewardsClient<'static>,
    Address,
    Address,
    Address,
) {
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
        &0i128,
        &0u32,
        &epoch_duration,
    );

    let reward_client = token::StellarAssetClient::new(&e, &reward_token);
    reward_client.mint(&contract_id, &1_000_000_000);

    (e, client, owner, staking_token, reward_token)
}

#[test]
fn test_epoch_initialization() {
    let (_, client, _owner, _staking_token, _reward_token) = setup_epoch(100);
    let config = client.get_config();

    assert_eq!(config.epoch_duration, 100);
    assert_eq!(config.initial_rate.0, INITIAL_RATE);
    assert_eq!(config.decay_rate.0, 0);
}

#[test]
fn test_epoch_snapshot_no_decay() {
    let (e, client, _, staking_token, _) = setup_epoch(10);
    let user = Address::generate(&e);

    let staking_client = token::StellarAssetClient::new(&e, &staking_token);
    staking_client.mint(&user, &STAKE_AMOUNT);

    // Stake at block 0
    client.stake(&user, &STAKE_AMOUNT);

    // Advance to block 15: epoch 0 spans [0,9], epoch 1 spans [10,19]
    advance_ledger(&e, 15);

    let pending = client.get_pending_rewards(&user);

    // Epoch 0 (blocks 0-9): multiplier = exp(r0 * 10) = exp(0.0001 * 10) = exp(0.001) = 1.0010005
    // Virtual balance after epoch 0: 10000 * 1.0010005 = 10010.005
    // Epoch 1 (blocks 10-15): from epoch start (10) to curr (15), 5 blocks
    // multiplier = exp(r0 * 5) = exp(0.0005) = 1.000500125
    // Final virtual: 10010.005 * 1.000500125 = 10015.00625...
    // Rewards = 10015 - 10000 = 15
    assert!(pending > 0, "Epoch mode should produce positive rewards");
}

#[test]
fn test_epoch_snapshot_matches_continuous_within_same_epoch() {
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

    // Continuous mode contract
    let continuous_id = e.register(StakingRewards, ());
    let continuous = StakingRewardsClient::new(&e, &continuous_id);
    continuous.initialize(
        &owner,
        &staking_token,
        &reward_token,
        &INITIAL_RATE,
        &0i128,
        &0u32,
        &0u32,
    );

    // Epoch mode contract (same parameters, but with epoch_duration=100)
    let epoch_id = e.register(StakingRewards, ());
    let epoch = StakingRewardsClient::new(&e, &epoch_id);
    epoch.initialize(
        &owner,
        &staking_token,
        &reward_token,
        &INITIAL_RATE,
        &0i128,
        &0u32,
        &100u32,
    );

    let reward_continuous = token::StellarAssetClient::new(&e, &reward_token);
    reward_continuous.mint(&continuous_id, &1_000_000_000);
    let reward_epoch = token::StellarAssetClient::new(&e, &reward_token);
    reward_epoch.mint(&epoch_id, &1_000_000_000);

    let user_cont = Address::generate(&e);
    let user_epoch = Address::generate(&e);

    let staking_cont = token::StellarAssetClient::new(&e, &staking_token);
    staking_cont.mint(&user_cont, &100_000);
    staking_cont.mint(&user_epoch, &100_000);

    // Both stake at block 5, same amount
    advance_ledger(&e, 5);
    continuous.stake(&user_cont, &100_000);
    epoch.stake(&user_epoch, &100_000);

    // Advance to block 55 (still within first epoch for epoch mode)
    advance_ledger(&e, 50);

    let pending_cont = continuous.get_pending_rewards(&user_cont);
    let pending_epoch = epoch.get_pending_rewards(&user_epoch);

    // Within the same epoch, results should match continuous mode
    assert_eq!(
        pending_epoch, pending_cont,
        "Within same epoch, epoch mode should match continuous mode"
    );
}

#[test]
fn test_epoch_snapshot_accuracy_at_epoch_boundary() {
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

    let continuous_id = e.register(StakingRewards, ());
    let continuous = StakingRewardsClient::new(&e, &continuous_id);
    continuous.initialize(
        &owner,
        &staking_token,
        &reward_token,
        &INITIAL_RATE,
        &0i128,
        &0u32,
        &0u32,
    );

    let epoch_id = e.register(StakingRewards, ());
    let epoch = StakingRewardsClient::new(&e, &epoch_id);
    epoch.initialize(
        &owner,
        &staking_token,
        &reward_token,
        &INITIAL_RATE,
        &0i128,
        &0u32,
        &50u32,
    );

    let reward_continuous = token::StellarAssetClient::new(&e, &reward_token);
    reward_continuous.mint(&continuous_id, &1_000_000_000);
    let reward_epoch = token::StellarAssetClient::new(&e, &reward_token);
    reward_epoch.mint(&epoch_id, &1_000_000_000);

    let user_cont = Address::generate(&e);
    let user_epoch = Address::generate(&e);

    let staking = token::StellarAssetClient::new(&e, &staking_token);
    staking.mint(&user_cont, &100_000);
    staking.mint(&user_epoch, &100_000);

    // Both stake at block 0
    continuous.stake(&user_cont, &100_000);
    epoch.stake(&user_epoch, &100_000);

    // Advance to block 50 (exactly one epoch boundary)
    advance_ledger(&e, 50);

    let pending_cont = continuous.get_pending_rewards(&user_cont);
    let pending_epoch = epoch.get_pending_rewards(&user_epoch);

    assert_eq!(
        pending_epoch, pending_cont,
        "At epoch boundary, epoch mode should exactly match continuous mode"
    );
}

#[test]
fn test_epoch_snapshot_storage() {
    let (e, client, _, staking_token, _) = setup_epoch(50);
    let user = Address::generate(&e);

    let staking_client = token::StellarAssetClient::new(&e, &staking_token);
    staking_client.mint(&user, &STAKE_AMOUNT);

    client.stake(&user, &STAKE_AMOUNT);

    // Advance to block 120 (epoch 2 started at block 100)
    advance_ledger(&e, 120);

    // Get pending rewards (should trigger snapshot computation)
    let _pending = client.get_pending_rewards(&user);

    // Snapshot at epoch 0 should exist and be > 1.0
    let snap_0 = client.get_epoch_snapshot(&0);
    assert!(snap_0 > SCALE, "Epoch 0 snapshot should be > 1.0");

    // Snapshot at epoch 1 should exist and be > epoch 0
    let snap_1 = client.get_epoch_snapshot(&1);
    assert!(
        snap_1 > snap_0,
        "Epoch 1 snapshot should be > epoch 0 snapshot"
    );
}

#[test]
fn test_epoch_multi_epoch_rewards() {
    let (e, client, _, staking_token, _) = setup_epoch(100);
    let user = Address::generate(&e);

    let staking_client = token::StellarAssetClient::new(&e, &staking_token);
    staking_client.mint(&user, &100_000);

    client.stake(&user, &100_000);

    // Advance through 3 full epochs (300 blocks)
    advance_ledger(&e, 300);

    let pending = client.get_pending_rewards(&user);
    assert!(pending > 0, "Should accumulate rewards across epochs");

    // Claim and verify
    let claimed = client.claim(&user);
    assert!(claimed > 0, "Should claim rewards");
    assert_eq!(client.get_accrued_rewards(&user), 0);
}

#[test]
fn test_epoch_set_duration() {
    let (_e, client, _, _, _) = setup_epoch(0);

    let config = client.get_config();
    assert_eq!(config.epoch_duration, 0);

    client.set_epoch_duration(&200);

    let config = client.get_config();
    assert_eq!(config.epoch_duration, 200);
}
