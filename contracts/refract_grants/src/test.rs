#![cfg(test)]

use super::*;
use payout_velocity::VelocityConfig;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, Address, Env,
};

const FUND_AMOUNT: i128 = 1_000_000_000;
const GRANT_AMOUNT: i128 = 100_000_000;

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
    RefractGrantsClient<'static>,
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

    let contract_id = e.register(RefractGrants, ());
    let client = RefractGrantsClient::new(&e, &contract_id);

    client.initialize(&admin, &payout_token, &velocity_config());

    // Fund the grants pool.
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

    let contract_id = e.register(RefractGrants, ());
    let client = RefractGrantsClient::new(&e, &contract_id);

    client.initialize(&admin, &payout_token, &velocity_config());

    let config = client.get_config();
    assert_eq!(config.admin, admin);
    assert_eq!(config.payout_token, payout_token);
}

#[test]
fn test_approve_and_payout() {
    let (e, client, admin, payout_token, _) = setup();
    let recipient = Address::generate(&e);

    // Approve grant
    client.approve_grant(&admin, &recipient, &GRANT_AMOUNT);
    assert_eq!(client.get_approved_grant(&recipient), GRANT_AMOUNT);
    assert_eq!(client.get_total_approved(&recipient), GRANT_AMOUNT);

    advance_ledger(&e, 10);

    // Payout 30M from the approved 100M
    client.payout(&admin, &recipient, &30_000_000);

    let balance = token::Client::new(&e, &payout_token).balance(&recipient);
    assert_eq!(balance, 30_000_000);

    // Approved amount reduced
    assert_eq!(client.get_approved_grant(&recipient), GRANT_AMOUNT - 30_000_000);
}

#[test]
fn test_payout_exceeds_approved_grant() {
    let (e, client, admin, _, _) = setup();
    let recipient = Address::generate(&e);

    client.approve_grant(&admin, &recipient, &50_000_000);
    advance_ledger(&e, 10);

    // Try to pay more than approved → fail
    let result = client.try_payout(&admin, &recipient, &60_000_000);
    assert!(result.is_err());

    // Approved unchanged
    assert_eq!(client.get_approved_grant(&recipient), 50_000_000);
}

#[test]
fn test_payout_without_approval() {
    let (e, client, admin, _, _) = setup();
    let recipient = Address::generate(&e);

    advance_ledger(&e, 10);

    // No grant approved → fail
    let result = client.try_payout(&admin, &recipient, &10_000_000);
    assert!(result.is_err());
}

#[test]
fn test_payout_epoch_cap() {
    let (e, client, admin, _, _) = setup();
    let recipient = Address::generate(&e);

    client.approve_grant(&admin, &recipient, &GRANT_AMOUNT);
    advance_ledger(&e, 10);

    // First payout: 30M within 50M cap → ok
    client.payout(&admin, &recipient, &30_000_000);

    // Second payout: 30M + 30M = 60M > 50M → rejected
    let result = client.try_payout(&admin, &recipient, &30_000_000);
    assert!(result.is_err());

    // Approved amount only reduced by successful payout
    assert_eq!(client.get_approved_grant(&recipient), GRANT_AMOUNT - 30_000_000);
}

#[test]
fn test_payout_min_spacing() {
    let (e, client, admin, _, _) = setup();
    let recipient = Address::generate(&e);

    client.approve_grant(&admin, &recipient, &GRANT_AMOUNT);
    advance_ledger(&e, 10);

    client.payout(&admin, &recipient, &10_000_000);

    // Only 2 ledgers later → rejected
    advance_ledger(&e, 12);
    let result = client.try_payout(&admin, &recipient, &10_000_000);
    assert!(result.is_err());

    // 5 ledgers later → ok
    advance_ledger(&e, 15);
    client.payout(&admin, &recipient, &10_000_000);
}

#[test]
fn test_payout_rolling_window_cap() {
    let (e, client, admin, _, _) = setup();
    let r1 = Address::generate(&e);
    let r2 = Address::generate(&e);

    client.approve_grant(&admin, &r1, &GRANT_AMOUNT);
    client.approve_grant(&admin, &r2, &GRANT_AMOUNT);

    advance_ledger(&e, 10);
    client.payout(&admin, &r1, &100_000_000);

    advance_ledger(&e, 20);
    client.payout(&admin, &r2, &100_000_000);

    // Total = 200M = window cap → third should fail
    advance_ledger(&e, 25);
    let result = client.try_payout(&admin, &r1, &10_000_000);
    assert!(result.is_err());

    // Advance past first payout's window
    advance_ledger(&e, 511);
    client.payout(&admin, &r1, &10_000_000);
}

#[test]
fn test_multiple_drawdowns() {
    let (e, client, admin, _, _) = setup();
    let recipient = Address::generate(&e);

    client.approve_grant(&admin, &recipient, &GRANT_AMOUNT);
    advance_ledger(&e, 10);

    // Draw down in increments
    for i in 0..3 {
        advance_ledger(&e, 10 + i as u32 * 10);
        let remaining = client.get_approved_grant(&recipient);
        let draw = remaining.min(20_000_000);
        if draw > 0 {
            client.payout(&admin, &recipient, &draw);
        }
    }

    // Should have paid out 20M * 3 = 60M (3 payouts with 10-ledger spacing)
    assert_eq!(
        client.get_approved_grant(&recipient),
        GRANT_AMOUNT - 60_000_000
    );
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

    client.approve_grant(&admin, &recipient, &GRANT_AMOUNT);
    advance_ledger(&e, 10);

    client.set_pause(&admin, &PAUSE_PAYOUT, &true);
    client.payout(&admin, &recipient, &10_000_000);
}

#[test]
fn test_velocity_introspection() {
    let (e, client, admin, _, _) = setup();
    let recipient = Address::generate(&e);

    client.approve_grant(&admin, &recipient, &GRANT_AMOUNT);
    advance_ledger(&e, 10);

    client.payout(&admin, &recipient, &15_000_000);

    assert_eq!(client.get_recipient_epoch_outflow(&recipient), 15_000_000);
    assert_eq!(client.get_recipient_last_payout(&recipient), 10);
    assert_eq!(client.get_rolling_window_total(), 15_000_000);
}

#[test]
fn test_no_state_mutation_on_velocity_error() {
    let (e, client, admin, _, _) = setup();
    let recipient = Address::generate(&e);

    client.approve_grant(&admin, &recipient, &GRANT_AMOUNT);
    advance_ledger(&e, 10);

    client.payout(&admin, &recipient, &50_000_000);

    // Try to exceed epoch cap → should fail and not change state
    let result = client.try_payout(&admin, &recipient, &10_000_000);
    assert!(result.is_err());

    // Approved unchanged from the last successful payout
    assert_eq!(
        client.get_approved_grant(&recipient),
        GRANT_AMOUNT - 50_000_000
    );
}

#[test]
fn test_pool_balance() {
    let (e, client, _, _, _) = setup();
    assert_eq!(client.get_pool_balance(), FUND_AMOUNT);
}

#[test]
fn test_zero_payout() {
    let (e, client, admin, _, _) = setup();
    let recipient = Address::generate(&e);

    client.approve_grant(&admin, &recipient, &GRANT_AMOUNT);
    advance_ledger(&e, 10);

    let result = client.try_payout(&admin, &recipient, &0);
    assert!(result.is_err());
}
