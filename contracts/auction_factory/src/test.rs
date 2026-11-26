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
use soroban_sdk::{testutils::Address as _, token, vec, Address, BytesN, Env};

mod english_auction {
    soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/release/english_auction.wasm");
}

mod dutch_auction {
    soroban_sdk::contractimport!(
        file = "../../target/wasm32v1-none/release/soroban_dutch_auction_contract.wasm"
    );
}

fn english_wasm_hash(env: &Env) -> BytesN<32> {
    env.deployer().upload_contract_wasm(english_auction::WASM)
}

fn dutch_wasm_hash(env: &Env) -> BytesN<32> {
    env.deployer().upload_contract_wasm(dutch_auction::WASM)
}

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

#[test]
fn test_factory_deploys_auctions_collects_fees_and_indexes_them() {
    let (env, client, admins) = setup(1, 1);
    let seller = Address::generate(&env);
    let fee_recipient = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let fee_token = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    let nft_token = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();

    token::StellarAssetClient::new(&env, &fee_token).mint(&seller, &100);
    token::StellarAssetClient::new(&env, &nft_token).mint(&seller, &1);
    client.configure_deployment_fee(&admins, &fee_token, &fee_recipient, &50);

    let english = client.create_english_auction(
        &seller,
        &nft_token,
        &1,
        &fee_token,
        &100,
        &200,
        &10,
        &english_wasm_hash(&env),
    );
    let dutch = client.create_dutch_auction(
        &seller,
        &nft_token,
        &1,
        &fee_token,
        &200,
        &100,
        &10,
        &dutch_wasm_hash(&env),
    );

    assert_eq!(
        english_auction::Client::new(&env, &english).get_seller(),
        seller
    );
    assert_eq!(dutch_auction::Client::new(&env, &dutch).is_sold(), false);
    assert_eq!(token::Client::new(&env, &fee_token).balance(&seller), 0);
    assert_eq!(
        token::Client::new(&env, &fee_token).balance(&fee_recipient),
        100
    );
    assert_eq!(client.get_auction_count(), 2);
    assert_eq!(client.get_auction_by_index(&0), Some(english.clone()));
    assert_eq!(client.get_auction_by_index(&1), Some(dutch.clone()));
    assert_eq!(
        client.get_auction_type(&english),
        Some(AuctionType::English)
    );
    assert_eq!(client.get_auction_type(&dutch), Some(AuctionType::Dutch));
    assert_eq!(client.get_auctions(&0, &10), vec![&env, english, dutch]);
}

#[test]
fn test_factory_rejects_deployment_without_fee_configuration() {
    let (env, client, _admins) = setup(1, 1);
    let seller = Address::generate(&env);
    let contract = Address::generate(&env);
    let zero_hash = BytesN::from_array(&env, &[0; 32]);

    let result = client.try_create_dutch_auction(
        &seller, &contract, &1, &contract, &200, &100, &10, &zero_hash,
    );

    assert_eq!(result, Err(Ok(FactoryError::FeeNotConfigured)));
}

#[test]
fn test_factory_rejects_non_positive_deployment_fee() {
    let (env, client, admins) = setup(1, 1);
    let token = Address::generate(&env);
    let recipient = Address::generate(&env);

    assert_eq!(
        client.try_configure_deployment_fee(&admins, &token, &recipient, &0),
        Err(Ok(FactoryError::InvalidFee))
    );
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
    assert_eq!(result, Err(Ok(GuardError::Unauthorized)));
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
    assert_eq!(result, Err(Ok(GuardError::Unauthorized)));
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
    assert_eq!(result, Err(Ok(GuardError::Unauthorized)));
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

// ── deterministic deployment / creator registry / pagination ────────────────

fn setup_with_fee(amount: i128) -> (Env, AuctionFactoryClient<'static>, Address, Address) {
    let (env, client, admins) = setup(1, 1);
    let seller = Address::generate(&env);
    let fee_recipient = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let fee_token = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();

    token::StellarAssetClient::new(&env, &fee_token).mint(&seller, &amount);
    client.configure_deployment_fee(&admins, &fee_token, &fee_recipient, &50);

    (env, client, seller, fee_token)
}

fn create_english(
    env: &Env,
    client: &AuctionFactoryClient<'static>,
    seller: &Address,
    nft_contract: &Address,
    token_id: i128,
    payment_token: &Address,
) -> Address {
    token::StellarAssetClient::new(env, nft_contract).mint(seller, &token_id);
    client.create_english_auction(
        seller,
        nft_contract,
        &token_id,
        payment_token,
        &100,
        &200,
        &10,
        &english_wasm_hash(env),
    )
}

#[test]
fn test_deterministic_nonce_salt_and_creator_registry() {
    let (env, client, seller, payment_token) = setup_with_fee(200);
    let nft = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();

    let first = create_english(&env, &client, &seller, &nft, 1, &payment_token);
    let second = create_english(&env, &client, &seller, &nft, 2, &payment_token);

    assert_ne!(first, second);
    assert_eq!(client.get_creator_nonce(&seller), 2);
    assert_eq!(client.get_creator_auction_count(&seller), 2);
    assert_eq!(
        client.get_auction_by_creator(&seller, &0),
        Some(first.clone())
    );
    assert_eq!(
        client.get_auction_by_creator(&seller, &1),
        Some(second.clone())
    );
    assert_eq!(client.get_auction_creator(&first), Some(seller.clone()));
    assert_eq!(client.get_auction_creator(&second), Some(seller.clone()));
    assert_eq!(
        client.get_auctions_by_creator(&seller, &0, &10),
        vec![&env, first, second]
    );
}

#[test]
fn test_paginated_queries_are_bounded() {
    let (env, client, seller, payment_token) = setup_with_fee(200);
    let nft = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();

    let mut addresses = soroban_sdk::Vec::new(&env);
    for id in 1..=4 {
        addresses.push_back(create_english(
            &env,
            &client,
            &seller,
            &nft,
            id,
            &payment_token,
        ));
    }

    assert_eq!(client.get_auction_count(), 4);
    assert_eq!(
        client.get_auctions(&0, &2),
        vec![&env, addresses.get(0).unwrap(), addresses.get(1).unwrap()]
    );
    assert_eq!(
        client.get_auctions(&2, &10),
        vec![&env, addresses.get(2).unwrap(), addresses.get(3).unwrap()]
    );
    assert_eq!(client.get_auctions(&0, &1000).len(), 4);
    assert_eq!(client.get_auctions(&4, &10).len(), 0);
    assert_eq!(client.get_auctions(&99, &10).len(), 0);
    assert_eq!(
        client.get_auctions_by_creator(&seller, &2, &10),
        vec![&env, addresses.get(2).unwrap(), addresses.get(3).unwrap()]
    );
}

#[test]
fn test_creator_filtering_returns_only_owned_auctions() {
    let (env, client, seller_a, payment_token) = setup_with_fee(100);
    let seller_b = Address::generate(&env);
    token::StellarAssetClient::new(&env, &payment_token).mint(&seller_b, &100);

    let nft = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();

    let auction_a = create_english(&env, &client, &seller_a, &nft, 1, &payment_token);
    let auction_b = create_english(&env, &client, &seller_b, &nft, 2, &payment_token);

    assert_eq!(
        client.get_auctions_by_creator(&seller_a, &0, &10),
        vec![&env, auction_a.clone()]
    );
    assert_eq!(
        client.get_auctions_by_creator(&seller_b, &0, &10),
        vec![&env, auction_b.clone()]
    );
    assert_eq!(
        client.get_auctions(&0, &10),
        vec![&env, auction_a, auction_b]
    );
}

#[test]
fn test_paused_factory_blocks_new_deployments_only() {
    let (env, client, seller, payment_token) = setup_with_fee(100);
    let nft = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    let live = create_english(&env, &client, &seller, &nft, 1, &payment_token);

    let admins = client.get_admins();
    assert!(!client.is_creation_paused());
    client.set_creation_paused(&admins, &true);
    assert!(client.is_creation_paused());

    assert_eq!(
        client.try_create_english_auction(
            &seller,
            &nft,
            &2,
            &payment_token,
            &100,
            &200,
            &10,
            &english_wasm_hash(&env),
        ),
        Err(Ok(FactoryError::Paused))
    );

    // Live auctions remain fully queryable while new deployments are paused.
    assert_eq!(client.get_auction_type(&live), Some(AuctionType::English));
    assert_eq!(client.get_auctions(&0, &10).len(), 1);
    assert_eq!(client.get_creator_auction_count(&seller), 1);

    client.set_creation_paused(&admins, &false);
    let resumed = create_english(&env, &client, &seller, &nft, 2, &payment_token);
    assert_eq!(client.get_auction_count(), 2);
    assert!(resumed != live);
}
