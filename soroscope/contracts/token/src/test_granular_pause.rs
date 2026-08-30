//! Granular pausing tests for MINT, BURN, and TRANSFER operations.
//!
//! These tests verify that pausing specific operations via EmergencyGuard
//! works independently — pausing MINT does not affect BURN or TRANSFER,
//! and vice versa.

use emergency_guard::{EmergencyGuard, EmergencyGuardClient, PauseType};
use soroban_sdk::{testutils::Address as _, vec, Address, Env};

fn setup_guard<'a>(env: &'a Env, admin: &'a Address) -> EmergencyGuardClient<'a> {
    let contract_id = env.register(EmergencyGuard, ());
    let client = EmergencyGuardClient::new(env, &contract_id);
    client.initialize(&vec![env, admin.clone()], &1);
    client
}

/// Pausing MINT does not affect BURN or TRANSFER.
#[test]
fn test_pause_mint_only() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let client = setup_guard(&env, &admin);

    client.set_pause(&admin, &PauseType::MINT, &true);

    assert!(client.is_paused(&PauseType::MINT));
    assert!(!client.is_paused(&PauseType::BURN));
    assert!(!client.is_paused(&PauseType::TRANSFER));
}

/// Pausing BURN does not affect MINT or TRANSFER.
#[test]
fn test_pause_burn_only() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let client = setup_guard(&env, &admin);

    client.set_pause(&admin, &PauseType::BURN, &true);

    assert!(!client.is_paused(&PauseType::MINT));
    assert!(client.is_paused(&PauseType::BURN));
    assert!(!client.is_paused(&PauseType::TRANSFER));
}

/// Pausing TRANSFER does not affect MINT or BURN.
#[test]
fn test_pause_transfer_only() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let client = setup_guard(&env, &admin);

    client.set_pause(&admin, &PauseType::TRANSFER, &true);

    assert!(!client.is_paused(&PauseType::MINT));
    assert!(!client.is_paused(&PauseType::BURN));
    assert!(client.is_paused(&PauseType::TRANSFER));
}

/// All three can be paused simultaneously and independently unpaused.
#[test]
fn test_pause_all_three_then_unpause_individually() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let client = setup_guard(&env, &admin);

    client.set_pause(&admin, &PauseType::MINT, &true);
    client.set_pause(&admin, &PauseType::BURN, &true);
    client.set_pause(&admin, &PauseType::TRANSFER, &true);

    assert!(client.is_paused(&PauseType::MINT));
    assert!(client.is_paused(&PauseType::BURN));
    assert!(client.is_paused(&PauseType::TRANSFER));

    // Unpause MINT only
    client.set_pause(&admin, &PauseType::MINT, &false);
    assert!(!client.is_paused(&PauseType::MINT));
    assert!(client.is_paused(&PauseType::BURN));
    assert!(client.is_paused(&PauseType::TRANSFER));

    // Unpause BURN only
    client.set_pause(&admin, &PauseType::BURN, &false);
    assert!(!client.is_paused(&PauseType::MINT));
    assert!(!client.is_paused(&PauseType::BURN));
    assert!(client.is_paused(&PauseType::TRANSFER));

    // Unpause TRANSFER
    client.set_pause(&admin, &PauseType::TRANSFER, &false);
    assert!(!client.is_paused(&PauseType::TRANSFER));
}

/// Unpausing an already-unpaused operation is a no-op.
#[test]
fn test_unpause_when_not_paused_is_noop() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let client = setup_guard(&env, &admin);

    // None paused initially
    client.set_pause(&admin, &PauseType::MINT, &false);
    assert!(!client.is_paused(&PauseType::MINT));
    assert!(!client.is_paused(&PauseType::BURN));
    assert!(!client.is_paused(&PauseType::TRANSFER));
}

/// set_pause requires admin auth; non-admin cannot pause.
#[test]
fn test_pause_requires_admin_auth() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    let client = setup_guard(&env, &admin);

    let result = client.try_set_pause(&non_admin, &PauseType::MINT, &true);
    assert!(result.is_err());
}
