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
