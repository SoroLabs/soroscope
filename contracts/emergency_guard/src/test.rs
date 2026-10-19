extern crate std;

use crate::{EmergencyGuard, EmergencyGuardClient, GuardError, PauseType};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    vec, Address, Env,
};

fn setup() -> (
    Env,
    EmergencyGuardClient<'static>,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(EmergencyGuard, ());
    let client = EmergencyGuardClient::new(&env, &contract_id);
    let admin_a = Address::generate(&env);
    let admin_b = Address::generate(&env);
    let guardian = Address::generate(&env);
    let admins = vec![&env, admin_a.clone(), admin_b.clone()];
    let guardians = vec![&env, guardian.clone()];
    client.initialize_with_roles(&admins, &guardians, &2);
    (env, client, admin_a, admin_b, guardian)
}

#[test]
fn pause_type_reports_unpaused_partial_and_full_modes() {
    assert_eq!(PauseType::new(0).pause_type(), PauseType::Unpaused);
    assert_eq!(
        PauseType::new(PauseType::SWAP).pause_type(),
        PauseType::PartialPause
    );
    assert_eq!(PauseType::new(u32::MAX).pause_type(), PauseType::FullPause);
}

#[test]
fn guardians_can_pause_but_cannot_unpause_or_manage_roles() {
    let (env, client, admin_a, admin_b, guardian) = setup();
    assert!(client.is_guardian(&guardian));

    client.set_pause(&guardian, &PauseType::SWAP, &true);
    assert!(client.is_paused(&PauseType::SWAP));
    assert_eq!(
        client.try_set_pause(&guardian, &PauseType::SWAP, &false),
        Err(Ok(GuardError::Unauthorized))
    );

    let approvers = vec![&env, admin_a.clone(), admin_b.clone()];
    let extra_guardian = Address::generate(&env);
    client.add_guardian(&approvers, &extra_guardian);
    assert!(client.is_guardian(&extra_guardian));
    client.remove_guardian(&approvers, &extra_guardian);
    assert!(!client.is_guardian(&extra_guardian));
}

#[test]
fn non_guardians_cannot_trigger_pause() {
    let (env, client, _admin_a, _admin_b, _guardian) = setup();
    let outsider = Address::generate(&env);
    assert_eq!(
        client.try_set_pause(&outsider, &PauseType::TRANSFER, &true),
        Err(Ok(GuardError::Unauthorized)),
    );
}

#[test]
fn emergency_pause_auto_unpauses_only_after_delay() {
    let (env, client, admin_a, admin_b, guardian) = setup();
    let admins = vec![&env, admin_a, admin_b];
    client.set_auto_unpause_delay(&admins, &100);
    client.guardian_emergency_pause(&guardian);
    assert_eq!(client.get_pause_type(), PauseType::FullPause);

    assert_eq!(
        client.try_auto_unpause(),
        Err(Ok(GuardError::TimelockNotElapsed))
    );
    env.ledger().set_timestamp(env.ledger().timestamp() + 99);
    assert!(client.is_paused(&PauseType::SWAP));
    env.ledger().set_timestamp(env.ledger().timestamp() + 1);
    assert!(!client.is_paused(&PauseType::SWAP));
    assert_eq!(client.get_pause_type(), PauseType::Unpaused);
    client.auto_unpause();
}

#[test]
fn resume_clears_auto_unpause_schedule() {
    let (env, client, admin_a, admin_b, guardian) = setup();
    let admins = vec![&env, admin_a, admin_b];
    client.set_auto_unpause_delay(&admins, &100);
    client.guardian_emergency_pause(&guardian);
    client.resume(&admins);
    assert_eq!(client.get_pause_type(), PauseType::Unpaused);
    assert_eq!(
        client.try_auto_unpause(),
        Err(Ok(GuardError::TimelockNotElapsed))
    );
}

#[test]
fn test_m_of_n_guardian_multi_sig_pause_and_unpause() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(EmergencyGuard, ());
    let client = EmergencyGuardClient::new(&env, &contract_id);

    let g1 = Address::generate(&env);
    let g2 = Address::generate(&env);
    let g3 = Address::generate(&env);
    let outsider = Address::generate(&env);

    let guardians = vec![&env, g1.clone(), g2.clone(), g3.clone()];
    // 2-of-3 threshold
    client.init(&guardians, &2);

    assert_eq!(client.get_threshold(), 2);
    assert_eq!(client.get_pause_votes(), 0);
    assert_eq!(client.get_unpause_votes(), 0);
    assert!(!client.is_paused(&PauseType::SWAP));

    // Non-guardian cannot vote
    assert_eq!(
        client.try_vote_pause(&outsider),
        Err(Ok(GuardError::Unauthorized))
    );

    // First guardian votes to pause: 1/2 votes -> not paused yet
    let count1 = client.vote_pause(&g1);
    assert_eq!(count1, 1);
    assert_eq!(client.get_pause_votes(), 1);
    assert!(!client.is_paused(&PauseType::SWAP));
    assert_eq!(client.get_pause_type(), PauseType::Unpaused);

    // Duplicate vote from g1 should not increment
    let count_dup = client.vote_pause(&g1);
    assert_eq!(count_dup, 1);
    assert_eq!(client.get_pause_votes(), 1);

    // Second guardian votes to pause: 2/2 votes -> threshold reached, auto-pauses!
    let count2 = client.vote_pause(&g2);
    assert_eq!(count2, 2);
    assert_eq!(client.get_pause_votes(), 2);
    assert!(client.is_paused(&PauseType::SWAP));
    assert_eq!(client.get_pause_type(), PauseType::FullPause);

    // Now test vote_unpause:
    // Non-guardian cannot vote unpause
    assert_eq!(
        client.try_vote_unpause(&outsider),
        Err(Ok(GuardError::Unauthorized))
    );

    // First guardian votes to unpause: 1/2 votes -> still paused
    let unpause_count1 = client.vote_unpause(&g1);
    assert_eq!(unpause_count1, 1);
    assert_eq!(client.get_unpause_votes(), 1);
    assert!(client.is_paused(&PauseType::SWAP));

    // Third guardian votes to unpause: 2/2 votes -> threshold reached, auto-unpauses!
    let unpause_count2 = client.vote_unpause(&g3);
    assert_eq!(unpause_count2, 2);
    assert!(!client.is_paused(&PauseType::SWAP));
    assert_eq!(client.get_pause_type(), PauseType::Unpaused);
    assert_eq!(client.get_pause_votes(), 0); // pause votes reset after unpause
}

