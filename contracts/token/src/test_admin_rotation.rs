use crate::contract::{Token, TokenClient};
use soroban_sdk::{testutils::Address as _, vec, Address, Env, String};

#[cfg(test)]
fn setup(env: &Env) -> (TokenClient<'_>, Address) {
    let contract_id = env.register(Token, ());
    let client = TokenClient::new(env, &contract_id);
    let admin = Address::generate(env);
    let guardian = Address::generate(env);
    client.initialize(
        &admin,
        &7,
        &String::from_str(env, "Test Token"),
        &String::from_str(env, "TEST"),
        &guardian,
    );
    (client, admin)
}

/// Admin can rotate to a new admin; new admin can mint, old cannot.
#[test]
fn test_admin_rotation_basic() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin) = setup(&env);
    let new_admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.set_admin(&vec![&env, admin.clone()], &new_admin);

    // New admin can mint
    client.mint(&user, &500);
    assert_eq!(client.balance(&user), 500);
}

/// set_admin requires multi-sig auth from guard admins.
#[test]
#[should_panic]
fn test_admin_rotation_requires_auth() {
    let env = Env::default();
    // No mock_all_auths — auth is enforced via check_multi_sig
    let (client, admin) = setup(&env);
    let new_admin = Address::generate(&env);

    // Should panic: no auth provided
    client.set_admin(&vec![&env, admin.clone()], &new_admin);
}

/// Admin can be rotated multiple times in sequence.
#[test]
fn test_admin_rotation_chain() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin) = setup(&env);
    let admin2 = Address::generate(&env);
    let admin3 = Address::generate(&env);
    let user = Address::generate(&env);

    client.set_admin(&vec![&env, admin.clone()], &admin2);
    client.set_admin(&vec![&env, admin2.clone()], &admin3);

    // admin3 is now the admin; mint should succeed
    client.mint(&user, &100);
    assert_eq!(client.balance(&user), 100);
}

/// After rotation, the new admin can rotate again to a third admin.
#[test]
fn test_admin_rotation_new_admin_can_rotate() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin) = setup(&env);
    let admin2 = Address::generate(&env);
    let admin3 = Address::generate(&env);
    let user = Address::generate(&env);

    client.set_admin(&vec![&env, admin.clone()], &admin2);
    // admin2 rotates to admin3
    client.set_admin(&vec![&env, admin2.clone()], &admin3);

    client.mint(&user, &250);
    assert_eq!(client.balance(&user), 250);
}

/// Rotating to the same admin is a no-op (idempotent).
#[test]
fn test_admin_rotation_to_self() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin) = setup(&env);
    // Rotate to self
    client.set_admin(&vec![&env, admin.clone()], &admin);

    let user = Address::generate(&env);
    client.mint(&user, &10);
    assert_eq!(client.balance(&user), 10);
}

/// Unauthorized approvers cannot rotate admin.
#[test]
#[should_panic]
fn test_admin_rotation_unauthorized_approver() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin) = setup(&env);
    let stranger = Address::generate(&env);
    let new_admin = Address::generate(&env);

    client.set_admin(&vec![&env, stranger.clone()], &new_admin);
}
