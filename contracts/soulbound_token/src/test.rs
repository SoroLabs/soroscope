use crate::contract::{Error, SoulboundToken, SoulboundTokenClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn init(env: &Env) -> (SoulboundTokenClient<'_>, Address) {
    env.mock_all_auths();

    let contract_id = env.register(SoulboundToken, ());
    let client = SoulboundTokenClient::new(env, &contract_id);
    let admin = Address::generate(env);
    let user1 = Address::generate(env);

    client.initialize(
        &admin,
        &0,
        &String::from_str(env, "Soulbound Token"),
        &String::from_str(env, "SBT"),
    );

    (client, user1)
}

#[test]
fn test_mint_and_balance() {
    let env = Env::default();
    let (client, user1) = init(&env);

    client.mint(&user1);
    assert_eq!(client.balance(&user1), 1);
}

#[test]
fn test_transfer_returns_unauthorized() {
    let env = Env::default();
    let (client, user1) = init(&env);
    let user2 = Address::generate(&env);

    client.mint(&user1);

    let err = client.try_transfer(&user1, &user2, &1);
    assert_eq!(err, Err(Ok(Error::Unauthorized)));
    assert_eq!(client.balance(&user1), 1);
    assert_eq!(client.balance(&user2), 0);

    let err = client.try_transfer_from(&user2, &user1, &user2, &1);
    assert_eq!(err, Err(Ok(Error::Unauthorized)));
    assert_eq!(client.balance(&user1), 1);
}

#[test]
fn test_admin_transfer() {
    let env = Env::default();
    let (client, user1) = init(&env);
    let user2 = Address::generate(&env);

    client.mint(&user1);
    assert_eq!(client.balance(&user1), 1);
    assert_eq!(client.balance(&user2), 0);

    client.admin_transfer(&user1, &user2);
    assert_eq!(client.balance(&user1), 0);
    assert_eq!(client.balance(&user2), 1);
}

#[test]
fn test_burn() {
    let env = Env::default();
    let (client, user1) = init(&env);

    client.mint(&user1);
    assert_eq!(client.balance(&user1), 1);

    client.burn(&user1);
    assert_eq!(client.balance(&user1), 0);
}

#[test]
#[should_panic(expected = "cannot hold more than one soulbound token")]
fn test_mint_twice_panics() {
    let env = Env::default();
    let (client, user1) = init(&env);

    client.mint(&user1);
    client.mint(&user1);
}

#[test]
fn test_revoke_by_admin() {
    let env = Env::default();
    let (client, user1) = init(&env);

    client.mint(&user1);
    assert_eq!(client.balance(&user1), 1);

    client.revoke(&user1);
    assert_eq!(client.balance(&user1), 0);
}

#[test]
#[should_panic(expected = "no token to revoke")]
fn test_revoke_no_token_panics() {
    let env = Env::default();
    let (client, user1) = init(&env);
    client.revoke(&user1);
}

#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn test_revoke_non_admin_panics() {
    let env = Env::default();
    let contract_id = env.register(SoulboundToken, ());
    let client = SoulboundTokenClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);

    client.initialize(
        &admin,
        &0,
        &String::from_str(&env, "Soulbound Token"),
        &String::from_str(&env, "SBT"),
    );

    env.mock_all_auths();
    client.mint(&user1);
    assert_eq!(client.balance(&user1), 1);

    env.mock_auths(&[]);
    client.revoke(&user1);
}

#[test]
fn test_mint_identity_badge_tied_to_address() {
    let env = Env::default();
    let (client, user1) = init(&env);
    let metadata = String::from_str(&env, "ipfs://identity/kyc-v1");

    client.mint_identity(&user1, &metadata);
    assert_eq!(client.balance(&user1), 1);
    assert_eq!(client.identity_metadata(&user1), metadata);
}

#[test]
fn test_issuer_metadata_update() {
    let env = Env::default();
    let (client, user1) = init(&env);
    client.mint_identity(&user1, &String::from_str(&env, "ipfs://v1"));

    let updated = String::from_str(&env, "ipfs://v2");
    client.update_metadata(&user1, &updated);
    assert_eq!(client.identity_metadata(&user1), updated);
    assert_eq!(client.balance(&user1), 1);
}

#[test]
#[should_panic(expected = "no token to update")]
fn test_update_metadata_without_badge_panics() {
    let env = Env::default();
    let (client, user1) = init(&env);
    client.update_metadata(&user1, &String::from_str(&env, "ipfs://missing"));
}

#[test]
fn test_revoke_clears_identity_metadata() {
    let env = Env::default();
    let (client, user1) = init(&env);
    client.mint_identity(&user1, &String::from_str(&env, "ipfs://v1"));
    client.revoke(&user1);
    assert_eq!(client.balance(&user1), 0);
    assert_eq!(client.identity_metadata(&user1), String::from_str(&env, ""));
}
