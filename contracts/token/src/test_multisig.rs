#![cfg(test)]

//! Threshold multi-sig tests exercising the EmergencyGuard RBAC model:
//! - Guardians can trigger emergency_pause unilaterally.
//! - Admins (multi-sig) can resume and manage roles.

use soroban_sdk::{testutils::Address as _, vec, Address, Env};

use emergency_guard::{EmergencyGuard, EmergencyGuardClient};

fn setup_guard(
    env: &Env,
    admins: &[Address],
    threshold: u32,
    guardian: &Address,
) -> EmergencyGuardClient<'_> {
    let contract_id = env.register(EmergencyGuard, ());
    let client = EmergencyGuardClient::new(env, &contract_id);
    let admins_vec = {
        let mut v = vec![env];
        for a in admins {
            v.push_back(a.clone());
        }
        v
    };
    client.initialize(&admins_vec, &threshold, guardian);
    client
}

/// A guardian can trigger emergency_pause without admin multi-sig.
#[test]
fn test_guardian_single_sig_emergency_pause() {
    let env = Env::default();
    env.mock_all_auths();

    let a1 = Address::generate(&env);
    let a2 = Address::generate(&env);
    let a3 = Address::generate(&env);
    let guardian = Address::generate(&env);
    let client = setup_guard(&env, &[a1.clone(), a2.clone(), a3.clone()], 2, &guardian);

    // Guardian acts alone — no admin multi-sig needed
    client.emergency_pause(&guardian);
    assert!(client.is_paused(&emergency_guard::PauseType::MINT));
}

/// An admin can also trigger emergency_pause unilaterally.
#[test]
fn test_admin_single_sig_emergency_pause() {
    let env = Env::default();
    env.mock_all_auths();

    let a1 = Address::generate(&env);
    let a2 = Address::generate(&env);
    let guardian = Address::generate(&env);
    let client = setup_guard(&env, &[a1.clone(), a2.clone()], 2, &guardian);

    client.emergency_pause(&a1);
    assert!(client.is_paused(&emergency_guard::PauseType::MINT));
}

/// A non-guardian, non-admin cannot trigger emergency_pause.
#[test]
fn test_outsider_cannot_emergency_pause() {
    let env = Env::default();
    env.mock_all_auths();

    let a1 = Address::generate(&env);
    let a2 = Address::generate(&env);
    let guardian = Address::generate(&env);
    let outsider = Address::generate(&env);
    let client = setup_guard(&env, &[a1.clone(), a2.clone()], 2, &guardian);

    let result = client.try_emergency_pause(&outsider);
    assert!(result.is_err());
}

/// 2-of-3: resume succeeds with exactly 2 admin approvers.
#[test]
fn test_multisig_2_of_3_resume_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let a1 = Address::generate(&env);
    let a2 = Address::generate(&env);
    let a3 = Address::generate(&env);
    let guardian = Address::generate(&env);
    let client = setup_guard(&env, &[a1.clone(), a2.clone(), a3.clone()], 2, &guardian);

    // Guardian pauses
    client.emergency_pause(&guardian);
    assert!(client.is_paused(&emergency_guard::PauseType::MINT));

    // 2-of-3 admins resume
    let approvers = vec![&env, a1.clone(), a2.clone()];
    client.resume(&approvers);
    assert!(!client.is_paused(&emergency_guard::PauseType::MINT));
}

/// resume fails with only 1 approver when threshold is 2.
#[test]
fn test_multisig_resume_fails_insufficient() {
    let env = Env::default();
    env.mock_all_auths();

    let a1 = Address::generate(&env);
    let a2 = Address::generate(&env);
    let a3 = Address::generate(&env);
    let guardian = Address::generate(&env);
    let client = setup_guard(&env, &[a1.clone(), a2.clone(), a3.clone()], 2, &guardian);

    client.emergency_pause(&guardian);

    let approvers = vec![&env, a1.clone()];
    let result = client.try_resume(&approvers);
    assert!(result.is_err());
}

/// add_admin requires multi-sig; new admin appears in list.
#[test]
fn test_multisig_add_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let a1 = Address::generate(&env);
    let a2 = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let guardian = Address::generate(&env);
    let client = setup_guard(&env, &[a1.clone(), a2.clone()], 2, &guardian);

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
    let guardian = Address::generate(&env);
    let client = setup_guard(&env, &[a1.clone(), a2.clone(), a3.clone()], 2, &guardian);

    let approvers = vec![&env, a1.clone(), a2.clone()];
    client.remove_admin(&approvers, &a3);

    let admins = client.get_admins();
    assert!(!admins.iter().any(|a| a == a3));
}

/// 3-of-3: all admins required for resume; succeeds with all 3.
#[test]
fn test_multisig_3_of_3_all_required() {
    let env = Env::default();
    env.mock_all_auths();

    let a1 = Address::generate(&env);
    let a2 = Address::generate(&env);
    let a3 = Address::generate(&env);
    let guardian = Address::generate(&env);
    let client = setup_guard(&env, &[a1.clone(), a2.clone(), a3.clone()], 3, &guardian);

    client.emergency_pause(&guardian);

    let approvers = vec![&env, a1.clone(), a2.clone(), a3.clone()];
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
    let guardian = Address::generate(&env);
    let client = setup_guard(&env, &[a1.clone(), a2.clone()], 2, &guardian);

    client.emergency_pause(&guardian);

    // Provide a1 twice — should only count as 1 unique approver
    let approvers = vec![&env, a1.clone(), a1.clone()];
    let result = client.try_resume(&approvers);
    assert!(result.is_err());
}
