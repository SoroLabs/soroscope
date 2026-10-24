#![cfg(test)]

use super::*;
use soroban_sdk::{
    contract, contractimpl, testutils::{Address as _, Ledger}, Address, Env, Vec,
};

/// Minimal contract registered so that `as_contract` provides a storage
/// context for the library functions.
#[contract]
pub struct DummyContract;

#[contractimpl]
impl DummyContract {
    pub fn _noop(_e: Env) {}
}

fn advance_ledger(e: &Env, to: u32) {
    let mut info = e.ledger().get();
    info.sequence_number = to;
    e.ledger().set(info);
}

fn setup_env() -> (Env, Address) {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();
    let addr = e.register(DummyContract, ());
    (e, addr)
}

fn init(e: &Env, addr: &Address, config: VelocityConfig) -> Result<(), VelocityError> {
    e.as_contract(addr, || PayoutVelocity::initialize(e, config))
}

fn set_config(e: &Env, addr: &Address, config: VelocityConfig) -> Result<(), VelocityError> {
    e.as_contract(addr, || PayoutVelocity::set_config(e, config))
}

fn get_config(e: &Env, addr: &Address) -> Option<VelocityConfig> {
    e.as_contract(addr, || PayoutVelocity::get_config(e))
}

fn check_and_record(
    e: &Env,
    addr: &Address,
    recipient: &Address,
    amount: i128,
) -> Result<(), VelocityError> {
    e.as_contract(addr, || PayoutVelocity::check_and_record(e, recipient, amount))
}

fn get_epoch_outflow(e: &Env, addr: &Address, recipient: &Address, ledger: u32) -> i128 {
    e.as_contract(addr, || {
        PayoutVelocity::get_recipient_epoch_outflow(e, recipient, ledger)
    })
}

fn get_last_payout(e: &Env, addr: &Address, recipient: &Address) -> u32 {
    e.as_contract(addr, || PayoutVelocity::get_recipient_last_payout(e, recipient))
}

fn get_window_total(e: &Env, addr: &Address) -> i128 {
    e.as_contract(addr, || PayoutVelocity::get_rolling_window_total(e))
}

fn get_records(e: &Env, addr: &Address) -> Vec<WindowRecord> {
    e.as_contract(addr, || {
        e.storage()
            .instance()
            .get(&VelocityKey::RollingWindowRecords)
            .unwrap_or(Vec::new(e))
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn test_epoch_cap_allows_within_limit() {
    let (e, addr) = setup_env();
    init(
        &e,
        &addr,
        VelocityConfig {
            epoch_length: 10,
            max_per_recipient_per_epoch: 1_000,
            ..Default::default()
        },
    )
    .unwrap();

    let recipient = Address::generate(&e);
    advance_ledger(&e, 3);

    assert!(check_and_record(&e, &addr, &recipient, 600).is_ok());
    assert_eq!(get_epoch_outflow(&e, &addr, &recipient, 3), 600);

    assert!(check_and_record(&e, &addr, &recipient, 400).is_ok());
    assert_eq!(get_epoch_outflow(&e, &addr, &recipient, 3), 1_000);
}

#[test]
fn test_epoch_cap_rejects_over_limit() {
    let (e, addr) = setup_env();
    init(
        &e,
        &addr,
        VelocityConfig {
            epoch_length: 10,
            max_per_recipient_per_epoch: 1_000,
            ..Default::default()
        },
    )
    .unwrap();

    let recipient = Address::generate(&e);
    advance_ledger(&e, 5);

    assert!(check_and_record(&e, &addr, &recipient, 600).is_ok());

    let err = check_and_record(&e, &addr, &recipient, 500).unwrap_err();
    assert_eq!(err, VelocityError::EpochCapExceeded);
}

#[test]
fn test_epoch_cap_resets_across_epoch_boundary() {
    let (e, addr) = setup_env();
    init(
        &e,
        &addr,
        VelocityConfig {
            epoch_length: 10,
            max_per_recipient_per_epoch: 1_000,
            ..Default::default()
        },
    )
    .unwrap();

    let recipient = Address::generate(&e);
    advance_ledger(&e, 8);
    assert!(check_and_record(&e, &addr, &recipient, 1_000).is_ok());

    advance_ledger(&e, 12);
    assert_eq!(get_epoch_outflow(&e, &addr, &recipient, 12), 0);
    assert!(check_and_record(&e, &addr, &recipient, 1_000).is_ok());
}

#[test]
fn test_min_spacing_rejects_too_soon() {
    let (e, addr) = setup_env();
    init(
        &e,
        &addr,
        VelocityConfig {
            min_spacing_ledgers: 5,
            ..Default::default()
        },
    )
    .unwrap();

    let recipient = Address::generate(&e);
    advance_ledger(&e, 10);
    assert!(check_and_record(&e, &addr, &recipient, 100).is_ok());
    assert_eq!(get_last_payout(&e, &addr, &recipient), 10);

    advance_ledger(&e, 13);
    let err = check_and_record(&e, &addr, &recipient, 100).unwrap_err();
    assert_eq!(err, VelocityError::MinSpacingNotMet);

    advance_ledger(&e, 15);
    assert!(check_and_record(&e, &addr, &recipient, 100).is_ok());
}

#[test]
fn test_rolling_window_cap() {
    let (e, addr) = setup_env();
    init(
        &e,
        &addr,
        VelocityConfig {
            rolling_window_ledgers: 20,
            max_per_rolling_window: 5_000,
            ..Default::default()
        },
    )
    .unwrap();

    let r1 = Address::generate(&e);
    let r2 = Address::generate(&e);

    advance_ledger(&e, 10);
    assert!(check_and_record(&e, &addr, &r1, 2_000).is_ok());
    assert_eq!(get_window_total(&e, &addr), 2_000);

    advance_ledger(&e, 15);
    assert!(check_and_record(&e, &addr, &r2, 2_000).is_ok());
    assert_eq!(get_window_total(&e, &addr), 4_000);

    // 4,000 + 2,000 = 6,000 > 5,000 → rejected
    let err = check_and_record(&e, &addr, &r1, 2_000).unwrap_err();
    assert_eq!(err, VelocityError::RollingWindowCapExceeded);

    // Advance past the first payout (ledger 10).
    // Window covers (31-20, 31] = (11, 31], so ledger 10 is pruned.
    advance_ledger(&e, 31);
    assert!(check_and_record(&e, &addr, &r1, 2_000).is_ok());
    assert_eq!(get_window_total(&e, &addr), 4_000);
}

#[test]
fn test_all_controls_combined() {
    let (e, addr) = setup_env();
    init(
        &e,
        &addr,
        VelocityConfig {
            epoch_length: 100,
            max_per_recipient_per_epoch: 3_000,
            min_spacing_ledgers: 5,
            rolling_window_ledgers: 50,
            max_per_rolling_window: 10_000,
        },
    )
    .unwrap();

    let r1 = Address::generate(&e);
    advance_ledger(&e, 10);
    assert!(check_and_record(&e, &addr, &r1, 1_000).is_ok());

    advance_ledger(&e, 12);
    assert_eq!(
        check_and_record(&e, &addr, &r1, 1_000).unwrap_err(),
        VelocityError::MinSpacingNotMet
    );

    advance_ledger(&e, 16);
    assert!(check_and_record(&e, &addr, &r1, 1_500).is_ok());

    advance_ledger(&e, 22);
    assert_eq!(
        check_and_record(&e, &addr, &r1, 1_000).unwrap_err(),
        VelocityError::EpochCapExceeded
    );
}

#[test]
fn test_no_state_mutation_on_error() {
    let (e, addr) = setup_env();
    init(
        &e,
        &addr,
        VelocityConfig {
            epoch_length: 10,
            max_per_recipient_per_epoch: 500,
            ..Default::default()
        },
    )
    .unwrap();

    let recipient = Address::generate(&e);
    advance_ledger(&e, 3);
    assert!(check_and_record(&e, &addr, &recipient, 300).is_ok());
    assert_eq!(get_epoch_outflow(&e, &addr, &recipient, 3), 300);

    assert!(check_and_record(&e, &addr, &recipient, 300).is_err());
    assert_eq!(get_epoch_outflow(&e, &addr, &recipient, 3), 300);
}

#[test]
fn test_disabled_controls_allow_all() {
    let (e, addr) = setup_env();
    init(&e, &addr, VelocityConfig::default()).unwrap();

    let recipient = Address::generate(&e);
    advance_ledger(&e, 1);
    assert!(check_and_record(&e, &addr, &recipient, 999_999_999).is_ok());
    advance_ledger(&e, 2);
    assert!(check_and_record(&e, &addr, &recipient, 999_999_999).is_ok());
}

#[test]
fn test_invalid_config_rejected() {
    let (e, addr) = setup_env();

    let bad_config = VelocityConfig {
        epoch_length: 0,
        max_per_recipient_per_epoch: 1_000,
        ..Default::default()
    };
    assert_eq!(
        init(&e, &addr, bad_config).unwrap_err(),
        VelocityError::InvalidConfig
    );
}

#[test]
fn test_set_config() {
    let (e, addr) = setup_env();
    init(&e, &addr, VelocityConfig::default()).unwrap();

    let new_config = VelocityConfig {
        epoch_length: 10,
        max_per_recipient_per_epoch: 5_000,
        min_spacing_ledgers: 3,
        rolling_window_ledgers: 100,
        max_per_rolling_window: 20_000,
    };
    set_config(&e, &addr, new_config.clone()).unwrap();
    assert_eq!(get_config(&e, &addr), Some(new_config));
}

#[test]
fn test_zero_amount_rejected() {
    let (e, addr) = setup_env();
    init(&e, &addr, VelocityConfig::default()).unwrap();
    let recipient = Address::generate(&e);
    advance_ledger(&e, 1);

    assert_eq!(
        check_and_record(&e, &addr, &recipient, 0).unwrap_err(),
        VelocityError::InvalidAmount
    );
    assert_eq!(
        check_and_record(&e, &addr, &recipient, -1).unwrap_err(),
        VelocityError::InvalidAmount
    );
}

#[test]
fn test_different_recipients_independent() {
    let (e, addr) = setup_env();
    init(
        &e,
        &addr,
        VelocityConfig {
            epoch_length: 100,
            max_per_recipient_per_epoch: 1_000,
            min_spacing_ledgers: 5,
            ..Default::default()
        },
    )
    .unwrap();

    let r1 = Address::generate(&e);
    let r2 = Address::generate(&e);

    advance_ledger(&e, 10);
    assert!(check_and_record(&e, &addr, &r1, 1_000).is_ok());

    advance_ledger(&e, 11);
    assert!(check_and_record(&e, &addr, &r2, 1_000).is_ok());

    advance_ledger(&e, 12);
    assert_eq!(
        check_and_record(&e, &addr, &r1, 100).unwrap_err(),
        VelocityError::MinSpacingNotMet
    );
}

#[test]
fn test_rolling_window_pruning_on_read() {
    let (e, addr) = setup_env();
    init(
        &e,
        &addr,
        VelocityConfig {
            rolling_window_ledgers: 10,
            max_per_rolling_window: 100_000,
            ..Default::default()
        },
    )
    .unwrap();

    let r = Address::generate(&e);
    advance_ledger(&e, 5);
    assert!(check_and_record(&e, &addr, &r, 100).is_ok());
    advance_ledger(&e, 10);
    assert!(check_and_record(&e, &addr, &r, 200).is_ok());

    advance_ledger(&e, 100);
    assert!(check_and_record(&e, &addr, &r, 300).is_ok());

    assert_eq!(get_window_total(&e, &addr), 300);

    let records = get_records(&e, &addr);
    assert_eq!(records.len(), 1);
    assert_eq!(records.get(0).unwrap().ledger, 100);
}
