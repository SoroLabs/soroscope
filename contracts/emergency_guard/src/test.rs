extern crate std;

use crate::{EmergencyGuard, EmergencyGuardClient, GuardError, PauseType};
use soroban_sdk::{testutils::Address as _, vec, Address, Env, Vec as SorobanVec};
use std::vec::Vec;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_addresses(env: &Env, n: u32) -> SorobanVec<Address> {
    let mut v = SorobanVec::new(env);
    for _ in 0..n {
        v.push_back(Address::generate(env));
    }
    v
}

/// Returns (env, client, admins, guardian).
fn setup(
    threshold: u32,
    n_admins: u32,
) -> (Env, EmergencyGuardClient<'static>, Vec<Address>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(EmergencyGuard, ());
    let client = EmergencyGuardClient::new(&env, &contract_id);
    let admins = make_addresses(&env, n_admins);
    let guardian = Address::generate(&env);
    client.initialize(&admins, &threshold, &guardian);
    let std_admins: Vec<Address> = admins.iter().collect();
    (env, client, std_admins, guardian)
}

// ── PauseType bitmask unit tests ──────────────────────────────────────────────
fn setup_with_roles(
    threshold: u32,
    admins: Vec<Address>,
    guardians: Vec<Address>,
) -> (Env, EmergencyGuardClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(EmergencyGuard, ());
    let client = EmergencyGuardClient::new(&env, &contract_id);
    client.initialize_with_roles(&admins, &guardians, &threshold);
    (env, client)
}

#[test]
fn test_granular_pause_types() {
    let mut pause = PauseType::new(0);

    pause.set_paused(PauseType::SWAP, true);
    assert!(pause.is_paused(PauseType::SWAP));
    assert!(!pause.is_paused(PauseType::DEPOSIT));

    pause.set_paused(PauseType::DEPOSIT, true);
    assert!(pause.is_paused(PauseType::SWAP));
    assert!(pause.is_paused(PauseType::DEPOSIT));

    pause.set_paused(PauseType::WITHDRAW, true);
    assert!(pause.is_paused(PauseType::WITHDRAW));

    pause.set_paused(PauseType::SWAP, false);
    assert!(!pause.is_paused(PauseType::SWAP));
    assert!(pause.is_paused(PauseType::DEPOSIT));
    assert!(pause.is_paused(PauseType::WITHDRAW));
}

#[test]
fn test_bitwise_pause_logic() {
    let mut pause = PauseType::new(0);

    pause.set_paused(PauseType::SWAP, true);
    assert!(pause.is_paused(PauseType::SWAP));
    assert!(!pause.is_paused(PauseType::DEPOSIT));

    pause.set_paused(PauseType::MINT, true);
    assert!(pause.is_paused(PauseType::SWAP));
    assert!(pause.is_paused(PauseType::MINT));

    pause.set_paused(PauseType::SWAP, false);
    assert!(!pause.is_paused(PauseType::SWAP));
    assert!(pause.is_paused(PauseType::MINT));

    pause.set_paused(PauseType::MINT, false);
    assert_eq!(pause.as_u32(), 0);
}

#[test]
fn test_pause_all_and_unpause_all() {
    let mut pause = PauseType::new(0);
    pause.pause_all();
    for op in [
        PauseType::SWAP,
        PauseType::DEPOSIT,
        PauseType::WITHDRAW,
        PauseType::TRANSFER,
        PauseType::MINT,
        PauseType::BURN,
    ] {
        assert!(pause.is_paused(op));
    }
    pause.unpause_all();
    for op in [
        PauseType::SWAP,
        PauseType::DEPOSIT,
        PauseType::WITHDRAW,
        PauseType::TRANSFER,
        PauseType::MINT,
        PauseType::BURN,
    ] {
        assert!(!pause.is_paused(op));
    }
}

#[test]
fn test_multiple_pause_types() {
    let mut pause = PauseType::new(0);
    let combined = PauseType::SWAP | PauseType::DEPOSIT | PauseType::MINT;
    pause.set_paused(combined, true);
    assert!(pause.is_paused(PauseType::SWAP));
    assert!(pause.is_paused(PauseType::DEPOSIT));
    assert!(!pause.is_paused(PauseType::WITHDRAW));
    assert!(pause.is_paused(PauseType::MINT));
    assert!(!pause.is_paused(PauseType::BURN));
}

// ── Initialization ────────────────────────────────────────────────────────────

#[test]
fn test_initialize_stores_admins_and_threshold() {
    let (_env, client, _admins, _guardian) = setup(2, 3);
    let stored: Vec<Address> = client.get_admins().iter().collect();
    assert_eq!(stored.len(), 3);
    assert_eq!(client.get_threshold(), 2);
    assert!(!client.is_paused(&PauseType::SWAP));
}

#[test]
fn test_initialize_stores_guardian() {
    let (_env, client, _admins, guardian) = setup(1, 2);
    let guardians: Vec<Address> = client.get_guardians().iter().collect();
    assert_eq!(guardians.len(), 1);
    assert!(guardians.contains(&guardian));
fn test_initialize_with_distinct_guardians_and_admins() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(EmergencyGuard, ());
    let client = EmergencyGuardClient::new(&env, &contract_id);
    let admins = vec![&env, Address::generate(&env), Address::generate(&env)];
    let guardians = vec![&env, Address::generate(&env)];
    client.initialize_with_roles(&admins, &guardians, &1);

    assert!(client.is_admin(&admins.get(0).unwrap()));
    assert!(client.is_guardian(&guardians.get(0).unwrap()));
    assert!(!client.is_guardian(&admins.get(0).unwrap()));
}

#[test]
fn test_initialize_rejects_zero_threshold() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(EmergencyGuard, ());
    let client = EmergencyGuardClient::new(&env, &contract_id);
    let admins = vec![&env, Address::generate(&env)];
    let guardian = Address::generate(&env);
    let result = client.try_initialize(&admins, &0, &guardian);
    assert_eq!(result, Err(Ok(GuardError::InvalidThreshold)));
}

#[test]
fn test_initialize_rejects_threshold_greater_than_admin_count() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(EmergencyGuard, ());
    let client = EmergencyGuardClient::new(&env, &contract_id);
    let admins = vec![&env, Address::generate(&env), Address::generate(&env)];
    let guardian = Address::generate(&env);
    let result = client.try_initialize(&admins, &3, &guardian);
    assert_eq!(result, Err(Ok(GuardError::InvalidThreshold)));
}

#[test]
fn test_initialize_cannot_be_called_twice() {
    let (env, client, _admins, _guardian) = setup(1, 2);
    let guardian2 = Address::generate(&env);
    let result = client.try_initialize(&soroban_sdk::Vec::new(&env), &1, &guardian2);
    assert_eq!(result, Err(Ok(GuardError::AlreadyInitialized)));
}

// ── Guardian role: emergency pause ───────────────────────────────────────────

#[test]
fn test_guardian_can_trigger_emergency_pause() {
    let (_env, client, _admins, guardian) = setup(2, 3);
    client.emergency_pause(&guardian);
    for op in [
        PauseType::SWAP,
        PauseType::DEPOSIT,
        PauseType::WITHDRAW,
        PauseType::TRANSFER,
        PauseType::MINT,
        PauseType::BURN,
    ] {
        assert!(client.is_paused(&op));
    }
}

#[test]
fn test_guardian_cannot_pause_specific_operation() {
    let (_env, client, _admins, guardian) = setup(2, 3);
    let result = client.try_set_pause(&guardian, &PauseType::TRANSFER, &true);
    assert_eq!(result, Err(Ok(GuardError::Unauthorized)));
}

#[test]
fn test_guardian_cannot_unpause() {
    let (env, client, admins, guardian) = setup(2, 3);

    // Admin pauses first
    client.set_pause(&admins[0], &PauseType::SWAP, &true);
    assert!(client.is_paused(&PauseType::SWAP));

    // Guardian tries to unpause a single operation — should be rejected
    let result = client.try_set_pause(&guardian, &PauseType::SWAP, &false);
    assert_eq!(result, Err(Ok(GuardError::Unauthorized)));

    // Guardian tries resume (multi-sig) — guardian is not an admin so check_multi_sig rejects it
    let approvers = vec![&env, guardian.clone()];
    let result = client.try_resume(&approvers);
    assert_eq!(result, Err(Ok(GuardError::Unauthorized)));
}

#[test]
fn test_guardian_cannot_manage_roles() {
    let (env, client, admins, guardian) = setup(1, 2);
    let new_admin = Address::generate(&env);

    // Guardian tries to add an admin — should fail
    let bad_approvers = vec![&env, guardian.clone()];
    let result = client.try_add_admin(&bad_approvers, &new_admin);
    assert_eq!(result, Err(Ok(GuardError::Unauthorized)));

    // Guardian tries to remove an admin — should fail
    let result = client.try_remove_admin(&bad_approvers, &admins[0]);
    assert_eq!(result, Err(Ok(GuardError::Unauthorized)));
}

#[test]
fn test_non_guardian_non_admin_cannot_emergency_pause() {
    let (env, client, _admins, _guardian) = setup(1, 2);
    let outsider = Address::generate(&env);
    let result = client.try_emergency_pause(&outsider);
    assert_eq!(result, Err(Ok(GuardError::Unauthorized)));
}

// ── Admin role: unpause and role management ───────────────────────────────────

#[test]
fn test_admin_can_pause_and_unpause_specific_operation() {
    let (_env, client, admins, _guardian) = setup(1, 1);
    let admin = admins[0].clone();

    client.set_pause(&admin, &PauseType::SWAP, &true);
    assert!(client.is_paused(&PauseType::SWAP));

    client.set_pause(&admin, &PauseType::SWAP, &false);
    assert!(!client.is_paused(&PauseType::SWAP));
}

#[test]
fn test_admin_cannot_trigger_emergency_pause() {
    let (_env, client, admins, _guardian) = setup(1, 2);
    let result = client.try_emergency_pause(&admins[0]);
    assert_eq!(result, Err(Ok(GuardError::Unauthorized)));
}

#[test]
fn test_resume_requires_multi_sig() {
    let (env, client, admins, guardian) = setup(2, 3);

    // Guardian triggers pause
    client.emergency_pause(&guardian);

    // Single admin tries to resume — should fail
    let approvers1 = vec![&env, admins[0].clone()];
    let result = client.try_resume(&approvers1);
    assert_eq!(result, Err(Ok(GuardError::InsufficientSignatures)));

    // Two admins resume — should succeed
    let approvers2 = vec![&env, admins[0].clone(), admins[1].clone()];
    client.resume(&approvers2);
    assert!(!client.is_paused(&PauseType::SWAP));
    assert!(!client.is_paused(&PauseType::DEPOSIT));
}

#[test]
fn test_set_pause_rejected_for_non_admin_non_guardian() {
    let (env, client, _admins, _guardian) = setup(1, 2);
    let outsider = Address::generate(&env);
    let result = client.try_set_pause(&outsider, &PauseType::SWAP, &true);
    assert_eq!(result, Err(Ok(GuardError::Unauthorized)));
}

// ── Guardian management ───────────────────────────────────────────────────────

#[test]
fn test_add_guardian_requires_admin_multi_sig() {
    let (env, client, admins, _guardian) = setup(2, 3);
    let new_guardian = Address::generate(&env);

    // Only 1 approver — should fail
    let approvers1 = vec![&env, admins[0].clone()];
    let result = client.try_add_guardian(&approvers1, &new_guardian);
    assert_eq!(result, Err(Ok(GuardError::InsufficientSignatures)));

    // 2 approvers — should succeed
    let approvers2 = vec![&env, admins[0].clone(), admins[1].clone()];
    client.add_guardian(&approvers2, &new_guardian);

    let guardians: Vec<Address> = client.get_guardians().iter().collect();
    assert!(guardians.contains(&new_guardian));
}

#[test]
fn test_add_guardian_idempotent() {
    let (env, client, admins, guardian) = setup(1, 2);
    let approvers = vec![&env, admins[0].clone()];
    // Adding the same guardian twice should not create a duplicate
    client.add_guardian(&approvers, &guardian);
    let guardians: Vec<Address> = client.get_guardians().iter().collect();
    assert_eq!(guardians.iter().filter(|g| **g == guardian).count(), 1);
}

#[test]
fn test_remove_guardian_requires_admin_multi_sig() {
    let (env, client, admins, guardian) = setup(2, 3);

    // Only 1 approver — should fail
    let approvers1 = vec![&env, admins[0].clone()];
    let result = client.try_remove_guardian(&approvers1, &guardian);
    assert_eq!(result, Err(Ok(GuardError::InsufficientSignatures)));

    // 2 approvers — should succeed
    let approvers2 = vec![&env, admins[0].clone(), admins[1].clone()];
    client.remove_guardian(&approvers2, &guardian);
    let guardians: Vec<Address> = client.get_guardians().iter().collect();
    assert!(!guardians.contains(&guardian));
}

#[test]
fn test_remove_guardian_fails_when_not_found() {
    let (env, client, admins, _guardian) = setup(1, 2);
    let outsider = Address::generate(&env);
    let approvers = vec![&env, admins[0].clone()];
    let result = client.try_remove_guardian(&approvers, &outsider);
    assert_eq!(result, Err(Ok(GuardError::GuardianNotFound)));
}

#[test]
fn test_removed_guardian_cannot_pause() {
    let (env, client, admins, guardian) = setup(1, 2);

    // Remove the guardian
    let approvers = vec![&env, admins[0].clone()];
    client.remove_guardian(&approvers, &guardian);

    // Removed guardian tries to emergency pause — should fail
    let result = client.try_emergency_pause(&guardian);
    assert_eq!(result, Err(Ok(GuardError::Unauthorized)));
}

#[test]
fn test_newly_added_guardian_can_pause() {
    let (env, client, admins, _guardian) = setup(1, 2);
    let new_guardian = Address::generate(&env);

    let approvers = vec![&env, admins[0].clone()];
    client.add_guardian(&approvers, &new_guardian);

    client.emergency_pause(&new_guardian);
    assert!(client.is_paused(&PauseType::SWAP));
}

// ── Admin rotation ────────────────────────────────────────────────────────────

#[test]
fn test_guardian_can_pause_but_admin_cannot_pause() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(EmergencyGuard, ());
    let client = EmergencyGuardClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let guardian = Address::generate(&env);
    let admins = vec![&env, admin.clone()];
    let guardians = vec![&env, guardian.clone()];
    client.initialize_with_roles(&admins, &guardians, &1);

    client.set_pause(&guardian, &PauseType::SWAP, &true);
    assert!(client.is_paused(&PauseType::SWAP));

    let result = client.try_set_pause(&admin, &PauseType::DEPOSIT, &true);
    assert_eq!(result, Err(Ok(GuardError::Unauthorized)));
}

#[test]
fn test_admin_can_resume_but_guardian_cannot_resume() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(EmergencyGuard, ());
    let client = EmergencyGuardClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let guardian = Address::generate(&env);
    let admins = vec![&env, admin.clone()];
    let guardians = vec![&env, guardian.clone()];
    client.initialize_with_roles(&admins, &guardians, &1);

    let guardians_vec = vec![&env, guardian.clone()];
    client.emergency_pause(&guardians_vec);
    assert!(client.is_paused(&PauseType::SWAP));

    let result = client.try_resume(&vec![&env, guardian.clone()]);
    assert_eq!(result, Err(Ok(GuardError::Unauthorized)));

    client.resume(&vec![&env, admin.clone()]);
    assert!(!client.is_paused(&PauseType::SWAP));
}

#[test]
fn test_add_admin_with_sufficient_approvers() {
    let (env, client, admins, _guardian) = setup(2, 3);
    let new_admin = Address::generate(&env);
    let approvers = vec![&env, admins[0].clone(), admins[1].clone()];
    client.add_admin(&approvers, &new_admin);
    let stored: Vec<Address> = client.get_admins().iter().collect();
    assert_eq!(stored.len(), 4);
    assert!(stored.contains(&new_admin));
}

#[test]
fn test_add_admin_fails_with_insufficient_approvers() {
    let (env, client, admins, _guardian) = setup(2, 3);
    let new_admin = Address::generate(&env);
    let approvers = vec![&env, admins[0].clone()];
    let result = client.try_add_admin(&approvers, &new_admin);
    assert_eq!(result, Err(Ok(GuardError::InsufficientSignatures)));
}

#[test]
fn test_add_admin_fails_with_non_admin_approvers() {
    let (env, client, _admins, _guardian) = setup(1, 2);
    let new_admin = Address::generate(&env);
    let outsider = Address::generate(&env);
    let approvers = vec![&env, outsider];
    let result = client.try_add_admin(&approvers, &new_admin);
    // Approver count meets the threshold, so this fails on the approver not
    // being an admin rather than on the signature count.
    assert_eq!(result, Err(Ok(GuardError::Unauthorized)));
}

#[test]
fn test_add_admin_deduplicates_approvers() {
    let (env, client, admins, _guardian) = setup(2, 3);
    let new_admin = Address::generate(&env);
    let approvers = vec![&env, admins[0].clone(), admins[0].clone()];
    let result = client.try_add_admin(&approvers, &new_admin);
    assert_eq!(result, Err(Ok(GuardError::InsufficientSignatures)));
}

#[test]
fn test_add_admin_idempotent_for_existing_admin() {
    let (env, client, admins, _guardian) = setup(1, 2);
    let existing = admins[0].clone();
    let approvers = vec![&env, admins[0].clone()];
    client.add_admin(&approvers, &existing);
    let stored: Vec<Address> = client.get_admins().iter().collect();
    assert_eq!(stored.len(), 2, "duplicate admin must not be inserted");
}

#[test]
fn test_remove_admin_with_sufficient_approvers() {
    let (env, client, admins, _guardian) = setup(2, 3);
    let to_remove = admins[2].clone();
    let approvers = vec![&env, admins[0].clone(), admins[1].clone()];
    client.remove_admin(&approvers, &to_remove);
    let stored: Vec<Address> = client.get_admins().iter().collect();
    assert_eq!(stored.len(), 2);
    assert!(!stored.contains(&to_remove));
}

#[test]
fn test_remove_admin_fails_with_insufficient_approvers() {
    let (env, client, admins, _guardian) = setup(2, 3);
    let to_remove = admins[2].clone();
    let approvers = vec![&env, admins[0].clone()];
    let result = client.try_remove_admin(&approvers, &to_remove);
    assert_eq!(result, Err(Ok(GuardError::InsufficientSignatures)));
}

#[test]
fn test_remove_admin_fails_when_admin_not_found() {
    let (env, client, admins, _guardian) = setup(2, 3);
    let outsider = Address::generate(&env);
    let approvers = vec![&env, admins[0].clone(), admins[1].clone()];
    let result = client.try_remove_admin(&approvers, &outsider);
    assert_eq!(result, Err(Ok(GuardError::AdminNotFound)));
}

#[test]
fn test_remove_admin_fails_when_would_drop_below_threshold() {
    let (env, client, admins, _guardian) = setup(2, 2);
    let to_remove = admins[1].clone();
    let approvers = vec![&env, admins[0].clone(), admins[1].clone()];
    let result = client.try_remove_admin(&approvers, &to_remove);
    assert_eq!(result, Err(Ok(GuardError::InvalidThreshold)));
}

#[test]
fn test_unauthorized_admin_removal() {
    let (env, client, admins) = setup(1, 2);
    let outsider = Address::generate(&env);
    let approvers = vec![&env, outsider];
    let result = client.try_remove_admin(&approvers, &admins[1]);
    // The outsider satisfies the count but is not an admin.
    assert_eq!(result, Err(Ok(GuardError::Unauthorized)));
}

// â”€â”€â”€ Full rotation cycle â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[test]
fn test_full_admin_rotation_add_then_remove_old() {
    let (env, client, admins) = setup(2, 3);
    let new_admin = Address::generate(&env);

    // Step 1: add new admin
    let approvers = vec![&env, admins[0].clone(), admins[1].clone()];
    client.add_admin(&approvers, &new_admin);
    assert_eq!(client.get_admins().len(), 4);

    // Step 2: remove one of the original admins using new quorum
    let approvers2 = vec![&env, admins[0].clone(), new_admin.clone()];
    client.remove_admin(&approvers2, &admins[2]);

    let stored: Vec<Address> = client.get_admins().iter().collect();
    assert_eq!(stored.len(), 3);
    assert!(!stored.contains(&admins[2]));
    assert!(stored.contains(&new_admin));
}

#[test]
fn test_rotate_admin() {
    let (env, client, admins, _guardian) = setup(2, 3);
    let old_admin = admins[2].clone();
    let new_admin = Address::generate(&env);

    let approvers = vec![&env, admins[0].clone(), admins[1].clone()];
    client.rotate_admin(&approvers, &old_admin, &new_admin);

    let stored: Vec<Address> = client.get_admins().iter().collect();
    assert_eq!(stored.len(), 3);
    assert!(!stored.contains(&old_admin));
    assert!(stored.contains(&new_admin));
}

#[test]
fn test_rotate_admin_duplicate_prevented() {
    let (env, client, admins, _guardian) = setup(2, 3);
    let old_admin = admins[2].clone();
    let new_admin = admins[1].clone(); // already an admin

    let approvers = vec![&env, admins[0].clone(), admins[1].clone()];
    client.rotate_admin(&approvers, &old_admin, &new_admin);

    let stored: Vec<Address> = client.get_admins().iter().collect();
    assert_eq!(stored.len(), 2);
    assert!(!stored.contains(&old_admin));
    assert!(stored.contains(&new_admin));
}

#[test]
fn test_removed_admin_cannot_approve_operations() {
    let (env, client, admins, _guardian) = setup(1, 3);

    let approvers = vec![&env, admins[0].clone()];
    client.remove_admin(&approvers, &admins[2]);

    let new_admin = Address::generate(&env);
    let bad_approvers = vec![&env, admins[2].clone()];
    let result = client.try_add_admin(&bad_approvers, &new_admin);
    // Once removed, the former admin is rejected as unauthorized rather than
    // counting toward the signature threshold.
    assert_eq!(result, Err(Ok(GuardError::Unauthorized)));
}

#[test]
fn test_newly_added_admin_can_approve_operations() {
    let (env, client, admins) = setup(1, 2);
    let new_admin = Address::generate(&env);

    // Add new_admin
    let approvers = vec![&env, admins[0].clone()];
    client.add_admin(&approvers, &new_admin);

    // new_admin approves a pause operation
    client.set_pause(&new_admin, &PauseType::SWAP, &true);
    assert!(client.is_paused(&PauseType::SWAP));
}

// â”€â”€â”€ get_admins / get_threshold â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[test]
fn test_get_admins_returns_all_admins() {
    let (_env, client, admins) = setup(1, 3);
    let stored: Vec<Address> = client.get_admins().iter().collect();
    assert_eq!(stored.len(), 3);
    for a in &admins {
        assert!(stored.contains(a));
    }
}

#[test]
fn test_get_threshold_returns_correct_value() {
    let (_env, client, _admins) = setup(2, 4);
    assert_eq!(client.get_threshold(), 2);
}

// â”€â”€â”€ Pause / resume integration â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[test]
fn test_set_pause_by_single_admin() {
    let (_env, client, admins) = setup(1, 2);
    client.set_pause(&admins[0], &PauseType::DEPOSIT, &true);
    assert!(client.is_paused(&PauseType::DEPOSIT));
    assert!(!client.is_paused(&PauseType::SWAP));
}

#[test]
fn test_set_pause_rejected_for_non_admin() {
    let (env, client, _admins) = setup(1, 2);
    let outsider = Address::generate(&env);
    let result = client.try_set_pause(&outsider, &PauseType::SWAP, &true);
    assert_eq!(result, Err(Ok(GuardError::Unauthorized)));
}

#[test]
fn test_full_admin_rotation_add_then_remove_old() {
    let (env, client, admins, _guardian) = setup(2, 3);
    let new_admin = Address::generate(&env);

    let approvers = vec![&env, admins[0].clone(), admins[1].clone()];
    client.add_admin(&approvers, &new_admin);
    assert_eq!(client.get_admins().len(), 4);

    let approvers2 = vec![&env, admins[0].clone(), new_admin.clone()];
    client.remove_admin(&approvers2, &admins[2]);

    let stored: Vec<Address> = client.get_admins().iter().collect();
    assert_eq!(stored.len(), 3);
    assert!(!stored.contains(&admins[2]));
    assert!(stored.contains(&new_admin));
}
