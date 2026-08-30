//! Threshold multi-sig tests for the token contract.
//!
//! These tests verify N-of-M multi-sig admin capabilities using the
//! EmergencyGuard contract as the token's admin, exercising add_admin,
//! remove_admin, emergency_pause, and resume with threshold enforcement.

use soroban_sdk::{testutils::Address as _, vec, Address, Env};

// Import EmergencyGuard directly from its crate
use emergency_guard::{EmergencyGuard, EmergencyGuardClient};

fn setup_guard<'a>(
    env: &'a Env,
    admins: &[Address],
    threshold: u32,
) -> (EmergencyGuardClient<'a>, Address) {
    let contract_id = env.register(EmergencyGuard, ());
    let client = EmergencyGuardClient::new(env, &contract_id);
    let admins_vec = {
        let mut v = vec![env];
        for a in admins {
            v.push_back(a.clone());
        }
        v
    };
    client.initialize(&admins_vec, &threshold);
    (client, contract_id)
}

/// 2-of-3: emergency_pause succeeds with exactly 2 approvers.
#[test]
fn test_multisig_2_of_3_pause_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let a1 = Address::generate(&env);
    let a2 = Address::generate(&env);
    let a3 = Address::generate(&env);
    let (client, _) = setup_guard(&env, &[a1.clone(), a2.clone(), a3.clone()], 2);

    let approvers = vec![&env, a1.clone(), a2.clone()];
    client.emergency_pause(&approvers);

    assert!(client.is_paused(&emergency_guard::PauseType::MINT));
}

/// 2-of-3: emergency_pause fails with only 1 approver.
#[test]
fn test_multisig_2_of_3_pause_fails_insufficient() {
    let env = Env::default();
    env.mock_all_auths();

    let a1 = Address::generate(&env);
    let a2 = Address::generate(&env);
    let a3 = Address::generate(&env);
    let (client, _) = setup_guard(&env, &[a1.clone(), a2.clone(), a3.clone()], 2);

    let approvers = vec![&env, a1.clone()];
    let result = client.try_emergency_pause(&approvers);
    assert!(result.is_err());
}

/// 3-of-3: all admins required; succeeds with all 3.
#[test]
fn test_multisig_3_of_3_all_required() {
    let env = Env::default();
    env.mock_all_auths();

    let a1 = Address::generate(&env);
    let a2 = Address::generate(&env);
    let a3 = Address::generate(&env);
    let (client, _) = setup_guard(&env, &[a1.clone(), a2.clone(), a3.clone()], 3);

    let approvers = vec![&env, a1.clone(), a2.clone(), a3.clone()];
    client.emergency_pause(&approvers);
    assert!(client.is_paused(&emergency_guard::PauseType::MINT));
}

/// add_admin requires multi-sig; new admin appears in list.
#[test]
fn test_multisig_add_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let a1 = Address::generate(&env);
    let a2 = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let (client, _) = setup_guard(&env, &[a1.clone(), a2.clone()], 2);

    let approvers = vec![&env, a1.clone(), a2.clone()];
    client.add_admin(&approvers, &new_admin);

    let admins = client.get_admins();
    assert!(admins.iter().any(|a| a == new_admin));
}

/// remove_admin requires multi-sig; removed admin no longer in list.
#[test]
fn test_multisig_remove_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let a1 = Address::generate(&env);
    let a2 = Address::generate(&env);
    let a3 = Address::generate(&env);
    let (client, _) = setup_guard(&env, &[a1.clone(), a2.clone(), a3.clone()], 2);

    let approvers = vec![&env, a1.clone(), a2.clone()];
    client.remove_admin(&approvers, &a3);

    let admins = client.get_admins();
    assert!(!admins.iter().any(|a| a == a3));
}

/// resume requires multi-sig; unpauses after emergency_pause.
#[test]
fn test_multisig_resume_after_pause() {
    let env = Env::default();
    env.mock_all_auths();

    let a1 = Address::generate(&env);
    let a2 = Address::generate(&env);
    let (client, _) = setup_guard(&env, &[a1.clone(), a2.clone()], 2);

    let approvers = vec![&env, a1.clone(), a2.clone()];
    client.emergency_pause(&approvers);
    assert!(client.is_paused(&emergency_guard::PauseType::MINT));

    client.resume(&approvers);
    assert!(!client.is_paused(&emergency_guard::PauseType::MINT));
}

/// Duplicate approvers do not count twice toward threshold.
#[test]
fn test_multisig_duplicate_approvers_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let a1 = Address::generate(&env);
    let a2 = Address::generate(&env);
    let (client, _) = setup_guard(&env, &[a1.clone(), a2.clone()], 2);

    // Provide a1 twice — should only count as 1 unique approver
    let approvers = vec![&env, a1.clone(), a1.clone()];
    let result = client.try_emergency_pause(&approvers);
    assert!(result.is_err());
}
