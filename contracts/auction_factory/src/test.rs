//! Admin rotation integration tests for AuctionFactory (issue #452).
//!
//! These tests verify that admin rotation via EmergencyGuard works correctly:
//!  - rotate_admin atomically swaps old admin for new admin
//!  - the new admin gains full privileges
//!  - the old admin loses all privileges
//!  - edge cases: rotating to the same admin, rotating with insufficient signers

#![cfg(test)]

use super::*;
use emergency_guard::GuardError;
use soroban_sdk::{testutils::Address as _, vec, Address, Env};

// ── helpers ──────────────────────────────────────────────────────────────────

/// Register AuctionFactory and initialize it with `n_admins` admins at the given
/// `threshold`. Returns `(env, client, admins_vec)`.
fn setup(
    n_admins: u32,
    threshold: u32,
) -> (
    Env,
    AuctionFactoryClient<'static>,
    soroban_sdk::Vec<Address>,
) {
    let env = Env::default();
    env.mock_all_auths();

    let factory_id = env.register(AuctionFactory, ());
    let client = AuctionFactoryClient::new(&env, &factory_id);

    let mut admins = soroban_sdk::Vec::new(&env);
    for _ in 0..n_admins {
        admins.push_back(Address::generate(&env));
    }

    // initialize returns () on the generated client (panics on error)
    client.initialize(&admins, &threshold);

    (env, client, admins)
}

// ── initialization sanity ────────────────────────────────────────────────────

#[test]
fn test_initialize_stores_admins_and_threshold() {
    let (_env, client, admins) = setup(3, 2);

    assert_eq!(client.get_admins().len(), 3);
    assert_eq!(client.get_threshold(), 2);

    for a in admins.iter() {
        assert!(client.is_admin(&a));
    }
}

#[test]
fn test_initialize_rejects_zero_threshold() {
    let env = Env::default();
    env.mock_all_auths();

    let factory_id = env.register(AuctionFactory, ());
    let client = AuctionFactoryClient::new(&env, &factory_id);

    let admins = vec![&env, Address::generate(&env)];
    let result = client.try_initialize(&admins, &0);
    assert_eq!(result, Err(Ok(GuardError::InvalidThreshold)));
}

#[test]
fn test_initialize_rejects_threshold_exceeding_admin_count() {
    let env = Env::default();
    env.mock_all_auths();

    let factory_id = env.register(AuctionFactory, ());
    let client = AuctionFactoryClient::new(&env, &factory_id);

    let admins = vec![&env, Address::generate(&env), Address::generate(&env)];
    let result = client.try_initialize(&admins, &3);
    assert_eq!(result, Err(Ok(GuardError::InvalidThreshold)));
}

#[test]
fn test_initialize_cannot_be_called_twice() {
    let (env, client, _admins) = setup(2, 1);
    let dummy = vec![&env, Address::generate(&env)];
    let result = client.try_initialize(&dummy, &1);
    assert_eq!(result, Err(Ok(GuardError::AlreadyInitialized)));
}

// ── rotate_admin: happy path ─────────────────────────────────────────────────

#[test]
fn test_rotate_admin_succeeds_with_sufficient_approvers() {
    // 3 admins, threshold 2 → two approvers suffice for rotation
    let (env, client, admins) = setup(3, 2);
    let new_admin = Address::generate(&env);

    let old_admin = admins.get(0).unwrap();
    let approvers = vec![&env, admins.get(1).unwrap(), admins.get(2).unwrap()];

    // rotate_admin returns () on the client; panics on error
    client.rotate_admin(&approvers, &old_admin, &new_admin);

    // Admin list size is unchanged
    assert_eq!(client.get_admins().len(), 3);
}

#[test]
fn test_new_admin_is_in_admin_list_after_rotation() {
    let (env, client, admins) = setup(3, 2);
    let new_admin = Address::generate(&env);
    let old_admin = admins.get(0).unwrap();
    let approvers = vec![&env, admins.get(1).unwrap(), admins.get(2).unwrap()];

    client.rotate_admin(&approvers, &old_admin, &new_admin);

    assert!(client.is_admin(&new_admin));
}

#[test]
fn test_old_admin_is_removed_from_admin_list_after_rotation() {
    let (env, client, admins) = setup(3, 2);
    let new_admin = Address::generate(&env);
    let old_admin = admins.get(0).unwrap();
    let approvers = vec![&env, admins.get(1).unwrap(), admins.get(2).unwrap()];

    client.rotate_admin(&approvers, &old_admin, &new_admin);

    assert!(!client.is_admin(&old_admin));
}

// ── new admin has full privileges ────────────────────────────────────────────

#[test]
fn test_new_admin_can_approve_multi_sig_operations() {
    // Start with threshold=2, 3 admins.  Rotate admin[0] → new_admin.
    // Then use new_admin + admin[1] (2 approvers) to add yet another admin.
    let (env, client, admins) = setup(3, 2);
    let new_admin = Address::generate(&env);
    let another_admin = Address::generate(&env);

    let old_admin = admins.get(0).unwrap();
    let rotate_approvers = vec![&env, admins.get(1).unwrap(), admins.get(2).unwrap()];

    client.rotate_admin(&rotate_approvers, &old_admin, &new_admin);

    // new_admin participates as an approver in a subsequent add_admin call
    let add_approvers = vec![&env, new_admin.clone(), admins.get(1).unwrap()];
    client.add_admin(&add_approvers, &another_admin);

    assert!(client.is_admin(&another_admin));
    assert_eq!(client.get_admins().len(), 4);
}

#[test]
fn test_new_admin_can_trigger_single_admin_operations() {
    // After rotation, new_admin can act as the sole approver when threshold=1.
    let (env, client, admins) = setup(3, 1);
    let new_admin = Address::generate(&env);
    let old_admin = admins.get(0).unwrap();

    // Rotate with single approver (threshold=1)
    let approvers = vec![&env, admins.get(1).unwrap()];
    client.rotate_admin(&approvers, &old_admin, &new_admin);

    // new_admin should be able to act as sole approver for add_admin
    let extra_admin = Address::generate(&env);
    let solo_approver = vec![&env, new_admin.clone()];
    client.add_admin(&solo_approver, &extra_admin);

    assert!(client.is_admin(&extra_admin));
}

// ── old admin loses privileges ───────────────────────────────────────────────

#[test]
fn test_old_admin_cannot_approve_multi_sig_after_rotation() {
    // Rotate admin[0] out. Threshold=2. old_admin + admin[1] is 1 valid + 1 invalid → fail.
    let (env, client, admins) = setup(3, 2);
    let new_admin = Address::generate(&env);
    let old_admin = admins.get(0).unwrap();
    let rotate_approvers = vec![&env, admins.get(1).unwrap(), admins.get(2).unwrap()];

    client.rotate_admin(&rotate_approvers, &old_admin, &new_admin);

    // old_admin is no longer in the set → its vote doesn't count
    let extra_admin = Address::generate(&env);
    let bad_approvers = vec![&env, old_admin.clone(), admins.get(1).unwrap()];
    let result = client.try_add_admin(&bad_approvers, &extra_admin);
    assert_eq!(result, Err(Ok(GuardError::InsufficientSignatures)));
}

#[test]
fn test_old_admin_is_not_recognized_as_admin() {
    let (env, client, admins) = setup(2, 1);
    let new_admin = Address::generate(&env);
    let old_admin = admins.get(0).unwrap();
    let approvers = vec![&env, admins.get(1).unwrap()];

    client.rotate_admin(&approvers, &old_admin, &new_admin);

    assert!(!client.is_admin(&old_admin));
}

#[test]
fn test_old_admin_cannot_be_sole_approver_for_rotate_admin() {
    // After being rotated out, old_admin tries to rotate another admin.
    let (env, client, admins) = setup(3, 1);
    let new_admin = Address::generate(&env);
    let old_admin = admins.get(0).unwrap();

    // Rotate old_admin out using admin[1]
    let approvers = vec![&env, admins.get(1).unwrap()];
    client.rotate_admin(&approvers, &old_admin, &new_admin);

    // old_admin tries to rotate admin[1] → should fail
    let another_new = Address::generate(&env);
    let bad_approvers = vec![&env, old_admin.clone()];
    let result = client.try_rotate_admin(&bad_approvers, &admins.get(1).unwrap(), &another_new);
    assert_eq!(result, Err(Ok(GuardError::InsufficientSignatures)));
}

// ── edge case: rotating to the same admin ────────────────────────────────────

#[test]
fn test_rotate_admin_to_same_address_is_a_no_op() {
    // Rotating admin[0] → admin[0] should succeed: found=true, list unchanged.
    let (env, client, admins) = setup(3, 2);
    let same_admin = admins.get(0).unwrap();
    let approvers = vec![&env, admins.get(1).unwrap(), admins.get(2).unwrap()];

    client.rotate_admin(&approvers, &same_admin, &same_admin);

    // Admin is still present, list still has 3 admins
    assert!(client.is_admin(&same_admin));
    assert_eq!(client.get_admins().len(), 3);
}

// ── edge case: rotating to an already-existing admin ─────────────────────────

#[test]
fn test_rotate_admin_to_existing_admin_does_not_duplicate() {
    // Rotating admin[0] → admin[1] removes admin[0] and keeps admin[1] exactly once.
    let (env, client, admins) = setup(3, 2);
    let old_admin = admins.get(0).unwrap();
    let existing_admin = admins.get(1).unwrap();
    let approvers = vec![&env, admins.get(1).unwrap(), admins.get(2).unwrap()];

    client.rotate_admin(&approvers, &old_admin, &existing_admin);

    // old_admin is gone, existing_admin appears only once, total = 2
    assert!(!client.is_admin(&old_admin));
    assert!(client.is_admin(&existing_admin));
    assert_eq!(client.get_admins().len(), 2);
}

// ── edge case: rotating a non-existent old admin ─────────────────────────────

#[test]
fn test_rotate_admin_fails_when_old_admin_not_found() {
    let (env, client, admins) = setup(3, 2);
    let not_an_admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let approvers = vec![&env, admins.get(0).unwrap(), admins.get(1).unwrap()];

    let result = client.try_rotate_admin(&approvers, &not_an_admin, &new_admin);
    assert_eq!(result, Err(Ok(GuardError::AdminNotFound)));
}

// ── edge case: insufficient signers ─────────────────────────────────────────

#[test]
fn test_rotate_admin_fails_with_zero_approvers() {
    let (env, client, admins) = setup(3, 2);
    let new_admin = Address::generate(&env);
    let old_admin = admins.get(0).unwrap();

    let empty_approvers: soroban_sdk::Vec<Address> = soroban_sdk::Vec::new(&env);
    let result = client.try_rotate_admin(&empty_approvers, &old_admin, &new_admin);
    assert_eq!(result, Err(Ok(GuardError::InsufficientSignatures)));
}

#[test]
fn test_rotate_admin_fails_with_one_approver_when_threshold_is_two() {
    let (env, client, admins) = setup(3, 2);
    let new_admin = Address::generate(&env);
    let old_admin = admins.get(0).unwrap();

    // Only one valid approver, but threshold is 2
    let approvers = vec![&env, admins.get(1).unwrap()];
    let result = client.try_rotate_admin(&approvers, &old_admin, &new_admin);
    assert_eq!(result, Err(Ok(GuardError::InsufficientSignatures)));
}

#[test]
fn test_rotate_admin_fails_with_duplicate_approvers_below_threshold() {
    // Passing the same admin twice must not count as two distinct approvals.
    let (env, client, admins) = setup(3, 2);
    let new_admin = Address::generate(&env);
    let old_admin = admins.get(0).unwrap();

    let approvers = vec![&env, admins.get(1).unwrap(), admins.get(1).unwrap()];
    let result = client.try_rotate_admin(&approvers, &old_admin, &new_admin);
    assert_eq!(result, Err(Ok(GuardError::InsufficientSignatures)));
}

#[test]
fn test_rotate_admin_fails_with_non_admin_approvers() {
    let (env, client, admins) = setup(3, 2);
    let new_admin = Address::generate(&env);
    let old_admin = admins.get(0).unwrap();

    // Two outsiders — neither is in the admin set
    let outsider1 = Address::generate(&env);
    let outsider2 = Address::generate(&env);
    let approvers = vec![&env, outsider1, outsider2];
    let result = client.try_rotate_admin(&approvers, &old_admin, &new_admin);
    assert_eq!(result, Err(Ok(GuardError::InsufficientSignatures)));
}

// ── full rotation cycle ──────────────────────────────────────────────────────

#[test]
fn test_full_rotation_cycle_preserves_threshold() {
    // Rotate all original admins one by one; threshold must be unchanged throughout.
    let (env, client, admins) = setup(3, 2);

    let new0 = Address::generate(&env);
    let new1 = Address::generate(&env);
    let new2 = Address::generate(&env);

    // Rotate admin[0] → new0
    let approvers = vec![&env, admins.get(1).unwrap(), admins.get(2).unwrap()];
    client.rotate_admin(&approvers, &admins.get(0).unwrap(), &new0);
    assert_eq!(client.get_threshold(), 2);

    // Rotate admin[1] → new1 (use new0 + admin[2])
    let approvers2 = vec![&env, new0.clone(), admins.get(2).unwrap()];
    client.rotate_admin(&approvers2, &admins.get(1).unwrap(), &new1);
    assert_eq!(client.get_threshold(), 2);

    // Rotate admin[2] → new2 (use new0 + new1)
    let approvers3 = vec![&env, new0.clone(), new1.clone()];
    client.rotate_admin(&approvers3, &admins.get(2).unwrap(), &new2);
    assert_eq!(client.get_threshold(), 2);

    // All original admins are gone; all new admins are present
    for old in admins.iter() {
        assert!(!client.is_admin(&old));
    }
    assert!(client.is_admin(&new0));
    assert!(client.is_admin(&new1));
    assert!(client.is_admin(&new2));
    assert_eq!(client.get_admins().len(), 3);
}

#[test]
fn test_rotate_admin_then_new_admin_can_rotate_again() {
    // A newly rotated-in admin can itself participate in further rotations.
    let (env, client, admins) = setup(2, 1);

    let new_admin = Address::generate(&env);
    let next_admin = Address::generate(&env);

    // Rotate admin[0] → new_admin using admin[1]
    client.rotate_admin(
        &vec![&env, admins.get(1).unwrap()],
        &admins.get(0).unwrap(),
        &new_admin,
    );

    // new_admin rotates admin[1] → next_admin
    client.rotate_admin(
        &vec![&env, new_admin.clone()],
        &admins.get(1).unwrap(),
        &next_admin,
    );

    assert!(client.is_admin(&new_admin));
    assert!(client.is_admin(&next_admin));
    assert!(!client.is_admin(&admins.get(0).unwrap()));
    assert!(!client.is_admin(&admins.get(1).unwrap()));
}
