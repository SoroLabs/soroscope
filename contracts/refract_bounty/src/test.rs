#![cfg(test)]

use super::*;
use payout_velocity::VelocityConfig;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, Address, Env,
};

const FUND_AMOUNT: i128 = 1_000_000_000;

fn advance_ledger(e: &Env, to: u32) {
    let mut info = e.ledger().get();
    info.sequence_number = to;
    e.ledger().set(info);
}

fn velocity_config() -> VelocityConfig {
    VelocityConfig {
        epoch_length: 100,
        max_per_recipient_per_epoch: 50_000_000,
        min_spacing_ledgers: 5,
        rolling_window_ledgers: 500,
        max_per_rolling_window: 200_000_000,
    }
}

fn setup() -> (
    Env,
    RefractBountyClient<'static>,
    Address,
    Address,
    Address,
) {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&e);
    let payout_token = e
        .register_stellar_asset_contract_v2(Address::generate(&e))
        .address();

    let contract_id = e.register(RefractBounty, ());
    let client = RefractBountyClient::new(&e, &contract_id);

    client.initialize(&admin, &payout_token, &velocity_config());

    // Fund the bounty pool.
    let token_client = token::StellarAssetClient::new(&e, &payout_token);
    token_client.mint(&admin, &FUND_AMOUNT);
    client.fund(&admin, &FUND_AMOUNT);

    (e, client, admin, payout_token, contract_id)
}

#[test]
fn test_initialize() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&e);
    let payout_token = e
        .register_stellar_asset_contract_v2(Address::generate(&e))
        .address();

    let contract_id = e.register(RefractBounty, ());
    let client = RefractBountyClient::new(&e, &contract_id);

    client.initialize(&admin, &payout_token, &velocity_config());

    let config = client.get_config();
    assert_eq!(config.admin, admin);
    assert_eq!(config.payout_token, payout_token);

    let vc = client.get_velocity_config();
    assert_eq!(vc, Some(velocity_config()));
}

#[test]
fn test_fund_and_payout() {
    let (e, client, admin, payout_token, _) = setup();
    let recipient = Address::generate(&e);

    advance_ledger(&e, 10);

    client.payout(&admin, &recipient, &10_000_000);

    let balance = token::Client::new(&e, &payout_token).balance(&recipient);
    assert_eq!(balance, 10_000_000);

    assert_eq!(client.get_pool_balance(), FUND_AMOUNT - 10_000_000);
}

#[test]
fn test_payout_epoch_cap() {
    let (e, client, admin, payout_token, _) = setup();
    let recipient = Address::generate(&e);

    advance_ledger(&e, 10);

    // First payout: 30M within 50M cap → ok
    client.payout(&admin, &recipient, &30_000_000);

    // Second payout: 30M + 30M = 60M > 50M → rejected
    let result = client.try_payout(&admin, &recipient, &30_000_000);
    assert!(result.is_err());

    // Verify the token balance didn't change after rejected payout
    let balance = token::Client::new(&e, &payout_token).balance(&recipient);
    assert_eq!(balance, 30_000_000);
}

#[test]
fn test_payout_epoch_cap_resets_next_epoch() {
    let (e, client, admin, _, _) = setup();
    let recipient = Address::generate(&e);

    advance_ledger(&e, 10);
    client.payout(&admin, &recipient, &50_000_000);

    // Move to next epoch (epoch_length = 100)
    advance_ledger(&e, 110);
    // Cap should reset
    assert_eq!(client.get_recipient_epoch_outflow(&recipient), 0);

    client.payout(&admin, &recipient, &50_000_000);
}

#[test]
fn test_payout_min_spacing() {
    let (e, client, admin, _, _) = setup();
    let recipient = Address::generate(&e);

    advance_ledger(&e, 10);
    client.payout(&admin, &recipient, &10_000_000);

    // Only 2 ledgers later → rejected (min_spacing = 5)
    advance_ledger(&e, 12);
    let result = client.try_payout(&admin, &recipient, &10_000_000);
    assert!(result.is_err());

    // Exactly 5 ledgers later → ok
    advance_ledger(&e, 15);
    client.payout(&admin, &recipient, &10_000_000);
}

#[test]
fn test_payout_rolling_window_cap() {
    let (e, client, admin, _, _) = setup();
    let r1 = Address::generate(&e);
    let r2 = Address::generate(&e);

    advance_ledger(&e, 10);
    client.payout(&admin, &r1, &100_000_000);

    advance_ledger(&e, 20);
    client.payout(&admin, &r2, &100_000_000);

    // Total = 200M = window cap → third payout should fail
    advance_ledger(&e, 25);
    let result = client.try_payout(&admin, &r1, &10_000_000);
    assert!(result.is_err());

    // Advance past the first payout's ledger (window = 500)
    // First payout at ledger 10, window covers (505-500, 505] = (5, 505]
    // Ledger 10 is inside the window → need to go past 510
    advance_ledger(&e, 511);
    client.payout(&admin, &r1, &10_000_000);
}

#[test]
fn test_payout_insufficient_funds() {
    let (e, client, admin, _, _) = setup();
    let recipient = Address::generate(&e);
    advance_ledger(&e, 10);

    // Try to pay more than the pool has
    let result = client.try_payout(&admin, &recipient, &(FUND_AMOUNT + 1));
    assert!(result.is_err());
}

#[test]
fn test_payout_zero_amount() {
    let (e, client, admin, _, _) = setup();
    let recipient = Address::generate(&e);
    advance_ledger(&e, 10);

    let result = client.try_payout(&admin, &recipient, &0);
    assert!(result.is_err());
}

#[test]
fn test_different_recipients_independent() {
    let (e, client, admin, _, _) = setup();
    let r1 = Address::generate(&e);
    let r2 = Address::generate(&e);

    advance_ledger(&e, 10);
    // r1 hits epoch cap
    client.payout(&admin, &r1, &50_000_000);

    // r2 is independent — should still be able to receive
    advance_ledger(&e, 15);
    client.payout(&admin, &r2, &50_000_000);
}

#[test]
fn test_set_velocity_config() {
    let (e, client, admin, _, _) = setup();

    let new_config = VelocityConfig {
        epoch_length: 50,
        max_per_recipient_per_epoch: 25_000_000,
        min_spacing_ledgers: 10,
        rolling_window_ledgers: 200,
        max_per_rolling_window: 100_000_000,
    };
    client.set_velocity_config(&admin, &new_config.clone());

    assert_eq!(client.get_velocity_config(), Some(new_config));
}

#[test]
#[should_panic(expected = "operation paused")]
fn test_pause_blocks_payout() {
    let (e, client, admin, _, _) = setup();
    let recipient = Address::generate(&e);

    advance_ledger(&e, 10);
    client.set_pause(&admin, &PAUSE_PAYOUT, &true);
    client.payout(&admin, &recipient, &10_000_000);
}

#[test]
fn test_velocity_introspection() {
    let (e, client, admin, _, _) = setup();
    let recipient = Address::generate(&e);

    advance_ledger(&e, 10);
    client.payout(&admin, &recipient, &15_000_000);

    assert_eq!(client.get_recipient_epoch_outflow(&recipient), 15_000_000);
    assert_eq!(client.get_recipient_last_payout(&recipient), 10);
    assert_eq!(client.get_rolling_window_total(), 15_000_000);
}

#[test]
fn test_no_state_mutation_on_velocity_error() {
    let (e, client, admin, payout_token, _) = setup();
    let recipient = Address::generate(&e);

    advance_ledger(&e, 10);
    client.payout(&admin, &recipient, &50_000_000);

    // Try to exceed epoch cap → should fail and not change state
    let result = client.try_payout(&admin, &recipient, &10_000_000);
    assert!(result.is_err());

    // Balance unchanged
    let balance = token::Client::new(&e, &payout_token).balance(&recipient);
    assert_eq!(balance, 50_000_000);

    // Epoch outflow unchanged
    assert_eq!(client.get_recipient_epoch_outflow(&recipient), 50_000_000);
}
