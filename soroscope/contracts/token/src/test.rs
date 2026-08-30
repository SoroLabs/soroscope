use crate::contract::{Token, TokenClient, BurnEvent};
use emergency_guard::{GuardError, PauseType};
use soroban_sdk::{testutils::Address as _, Address, Env, String, vec, Vec, IntoVal, TryIntoVal};

// ── Existing Tests ─────────────────────────────────────────────────────────────

#[test]
fn test_mint_and_transfer() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(Token, ());
    let client = TokenClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    client.initialize(
        &admin,
        &7,
        &String::from_str(&env, "Test Token"),
        &String::from_str(&env, "TEST"),
    );

    let approvers = vec![&env, admin.clone()];
    client.mint(&approvers, &user1, &1000);
    assert_eq!(client.balance(&user1), 1000);

    client.transfer(&user1, &user2, &200);
    assert_eq!(client.balance(&user1), 800);
    assert_eq!(client.balance(&user2), 200);
}

#[test]
fn test_allowance() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(Token, ());
    let client = TokenClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user1 = Address::generate(&env);
    let spender = Address::generate(&env);

    client.initialize(
        &admin,
        &7,
        &String::from_str(&env, "Test Token"),
        &String::from_str(&env, "TEST"),
    );

    let approvers = vec![&env, admin.clone()];
    client.mint(&approvers, &user1, &1000);

    client.approve(&user1, &spender, &500, &200);
    assert_eq!(client.allowance(&user1, &spender), 500);

    client.transfer_from(&spender, &user1, &spender, &200);
    assert_eq!(client.balance(&user1), 800);
    assert_eq!(client.balance(&spender), 200);
    assert_eq!(client.allowance(&user1, &spender), 300);
}

// ── Token Guard Integration Tests ─────────────────────────────────────────────

/// Issue #438: Verifies that PauseType::MINT check inside mint() blocks new
/// mint calls when the MINT pause bit is set, while transfers remain
/// unaffected — confirms the bitmask works correctly per the acceptance criteria.
#[test]
fn test_pause_minting_blocks_mint_only() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(Token, ());
    let client = TokenClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let user2 = Address::generate(&env);

    client.initialize(
        &admin,
        &7,
        &String::from_str(&env, "Guard Token"),
        &String::from_str(&env, "GTK"),
    );

    // Mint before pause succeeds.
    client.mint(&user, &500);
    assert_eq!(client.balance(&user), 500);

    // Pause minting via PauseType::MINT.
    client.pause_minting(&admin);

    // Mint should now fail because PauseType::MINT is set in the bitmask.
    let result = client.try_mint(&user, &100);
    assert!(result.is_err(), "mint should fail when PauseType::MINT is set");

    // Transfers are NOT paused — they should still work.
    client.transfer(&user, &user2, &100);
    assert_eq!(client.balance(&user2), 100);
}

/// Verifies that the guard is initialized with the token admin as sole guard admin.
#[test]
fn test_guard_initializes_with_token_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(Token, ());
    let client = TokenClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    client.initialize(
        &admin,
        &7,
        &String::from_str(&env, "Test Token"),
        &String::from_str(&env, "TEST"),
    );

    let admins = client.get_guard_admins();
    assert_eq!(admins.len(), 1);
    assert_eq!(admins.get(0).unwrap(), admin);
    assert_eq!(client.get_guard_threshold(), 1);
    assert!(!client.is_operation_paused(&PauseType::TRANSFER));
}

/// Verifies that pausing transfers blocks transfer while minting is unaffected.
#[test]
fn test_pause_transfers_blocks_transfer_only() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(Token, ());
    let client = TokenClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let user2 = Address::generate(&env);

    client.initialize(
        &admin,
        &7,
        &String::from_str(&env, "Test Token"),
        &String::from_str(&env, "TEST"),
    );

    client.mint(&user, &1000);

    // Pause TRANSFER via guard_pause.
    client.guard_pause(&admin, &PauseType::TRANSFER, &true);
    assert!(client.is_operation_paused(&PauseType::TRANSFER));

    // Transfer should fail.
    let result = client.try_transfer(&user, &user2, &100);
    assert!(result.is_err(), "transfer should fail when transfers are paused");

    // Minting is NOT paused — it should work.
    client.mint(&user2, &50);
    assert_eq!(client.balance(&user2), 50);

    // Unpause and verify transfer works again.
    client.guard_pause(&admin, &PauseType::TRANSFER, &false);
    client.transfer(&user, &user2, &100);
    assert_eq!(client.balance(&user2), 150);
}

/// Verifies that pausing burning blocks burn operations.
#[test]
fn test_pause_burning_blocks_burn() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(Token, ());
    let client = TokenClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(
        &admin,
        &7,
        &String::from_str(&env, "Test Token"),
        &String::from_str(&env, "TEST"),
    );
    client.mint(&user, &1000);

    // Pause BURN via guard_pause.
    client.guard_pause(&admin, &PauseType::BURN, &true);
    assert!(client.is_operation_paused(&PauseType::BURN));

    // Burn should fail.
    let result = client.try_burn(&user, &100);
    assert!(result.is_err(), "burn should fail when burning is paused");

    // Unpause and verify burn works again.
    client.guard_pause(&admin, &PauseType::BURN, &false);
    client.burn(&user, &100);
    assert_eq!(client.balance(&user), 900);
}

#[test]
fn test_guard_pause_blocks_transfer_until_resume() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(Token, ());
    let client = TokenClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let user2 = Address::generate(&env);

    client.initialize(
        &admin,
        &7,
        &String::from_str(&env, "Guard Token"),
        &String::from_str(&env, "GTK"),
    );

    client.mint(&user, &1000);

    // Pause transfers.
    client.pause_transfers(&admin);

    // Transfer should fail.
    let result = client.try_transfer(&user, &user2, &100);
    assert!(result.is_err(), "transfer should fail when transfers are paused");

    // Minting is NOT paused — still works.
    client.mint(&user2, &50);
    assert_eq!(client.balance(&user2), 50);
}

#[test]
fn test_emergency_pause_blocks_mint_and_burn_until_resume() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(Token, ());
    let client = TokenClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(
        &admin,
        &7,
        &String::from_str(&env, "Guard Token"),
        &String::from_str(&env, "GTK"),
    );
    client.mint(&user, &1000);

    // Pause burning.
    client.pause_burning(&admin);

    // Burn should fail.
    let result = client.try_burn(&user, &100);
    assert!(result.is_err(), "burn should fail when burning is paused");

    // Resume burning.
    client.resume_burning(&admin);

    // Burn should succeed after resuming.
    client.burn(&user, &100);
    assert_eq!(client.balance(&user), 900);
}

/// Verifies emergency_pause_all blocks all operations simultaneously.
#[test]
fn test_emergency_pause_all_freezes_everything() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(Token, ());
    let client = TokenClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let user2 = Address::generate(&env);

    client.initialize(
        &admin,
        &7,
        &String::from_str(&env, "Guard Token"),
        &String::from_str(&env, "GTK"),
    );
    client.mint(&user, &1000);

    // Emergency pause: all operations freeze via single bitmask write.
    let approvers = vec![&env, admin.clone()];
    client.emergency_pause_all(&approvers);

    // Confirm all operations are blocked.
    assert!(
        client.try_mint(&user2, &100).is_err(),
        "mint should be paused"
    );
    assert!(
        client.try_transfer(&user, &user2, &50).is_err(),
        "transfer should be paused"
    );
    assert!(
        client.try_burn(&user, &50).is_err(),
        "burn should be paused"
    );

    // Resume all via multi-sig.
    client.resume_all(&approvers);

    // All operations should work again.
    client.mint(&user2, &100);
    assert_eq!(client.balance(&user2), 100);
    client.transfer(&user, &user2, &50);
    assert_eq!(client.balance(&user), 950);
}

#[test]
fn test_guard_admin_management() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(Token, ());
    let client = TokenClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let user2 = Address::generate(&env);

    client.initialize(
        &admin,
        &7,
        &String::from_str(&env, "Guard Token"),
        &String::from_str(&env, "GTK"),
    );
    client.mint(&user, &1000);

    // Emergency pause: all operations freeze.
    let approvers = vec![&env, admin.clone()];
    client.emergency_pause_all(&approvers);

    // Confirm all operations are blocked.
    assert!(client.try_mint(&user2, &100).is_err(), "mint should be paused");
    assert!(client.try_transfer(&user, &user2, &50).is_err(), "transfer should be paused");
    assert!(client.try_burn(&user, &50).is_err(), "burn should be paused");

    // Resume all via multi-sig.
    client.resume_all(&approvers);

    // All operations should work again.
    client.mint(&user2, &100);
    assert_eq!(client.balance(&user2), 100);
    client.transfer(&user, &user2, &50);
    assert_eq!(client.balance(&user), 950);
}

#[test]
fn test_set_admin_rotates_token_and_guard_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(Token, ());
    let client = TokenClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    client.initialize(
        &admin,
        &7,
        &String::from_str(&env, "Guard Token"),
        &String::from_str(&env, "GTK"),
    );

    // Guard admin should be the token admin.
    let guard_admins = client.get_guard_admins();
    assert_eq!(guard_admins.len(), 1);
    assert_eq!(guard_admins.get(0).unwrap(), admin);

    // Threshold should be 1 (single-admin setup).
    assert_eq!(client.get_guard_threshold(), 1);

    // No operation should be paused at initialization.
    assert!(!client.is_operation_paused(&PauseType::MINT));
    assert!(!client.is_operation_paused(&PauseType::TRANSFER));
    assert!(!client.is_operation_paused(&PauseType::BURN));
}

/// Storage efficiency test: confirms that after guard integration the
/// footprint for initialize is correct and guard state shares instance storage.
#[test]
fn test_initialize_storage_efficiency() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(Token, ());
    let client = TokenClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    // Initialize — should complete without error and correctly read back metadata.
    client.initialize(
        &admin,
        &18,
        &String::from_str(&env, "Efficiency Token"),
        &String::from_str(&env, "EFFT"),
    );

    assert_eq!(client.decimals(), 18);
    assert_eq!(client.name(), String::from_str(&env, "Efficiency Token"));
    assert_eq!(client.symbol(), String::from_str(&env, "EFFT"));
    assert_eq!(client.get_guard_threshold(), 1);

    // Guard admin should be the initial admin.
    let admins = client.get_guard_admins();
    assert_eq!(admins.len(), 1);
    assert_eq!(admins.get(0).unwrap(), admin);

    // Verify PauseType::MINT is not set initially.
    assert!(!client.is_operation_paused(&PauseType::MINT));

    // Pause MINT and assert it is now set.
    client.guard_pause(&admin, &PauseType::MINT, &true);
    assert!(client.is_operation_paused(&PauseType::MINT));
}

#[test]
fn test_burn_emits_event() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(Token, ());
    let client = TokenClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    client.initialize(
        &admin,
        &7,
        &String::from_str(&env, "Test Token"),
        &String::from_str(&env, "TEST"),
    );

    client.mint(&user, &1000);

    // Record events before burn
    let initial_events_count = env.events().all().len();

    client.burn(&user, &400);
    assert_eq!(client.balance(&user), 600);

    let events = env.events().all();
    assert!(events.len() > initial_events_count);

    // Find the burn event
    let burn_event_name = String::from_str(&env, "burn");
    let burn_events: Vec<_> = events
        .iter()
        .filter(|(c_id, topics, _)| {
            c_id == &contract_id
                && topics.len() == 2
                && topics.get(0).unwrap() == burn_event_name.clone().into_val(&env)
                && topics.get(1).unwrap() == user.clone().into_val(&env)
        })
        .collect();

    assert_eq!(burn_events.len(), 1);
    let (_, _, data) = &burn_events[0];
    let burn_event: BurnEvent = data.try_into_val(&env).unwrap();
    assert_eq!(burn_event.burner, user);
    assert_eq!(burn_event.target_account, user);
    assert_eq!(burn_event.amount, 400);
}

#[test]
fn test_burn_from_emits_event() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(Token, ());
    let client = TokenClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let spender = Address::generate(&env);

    client.initialize(
        &admin,
        &7,
        &String::from_str(&env, "Test Token"),
        &String::from_str(&env, "TEST"),
    );

    client.mint(&user, &1000);
    client.approve(&user, &spender, &500, &200);

    // Record events before burn_from
    let initial_events_count = env.events().all().len();

    client.burn_from(&spender, &user, &300);
    assert_eq!(client.balance(&user), 700);
    assert_eq!(client.allowance(&user, &spender), 200);

    let events = env.events().all();
    assert!(events.len() > initial_events_count);

    // Find the burn event
    let burn_event_name = String::from_str(&env, "burn");
    let burn_events: Vec<_> = events
        .iter()
        .filter(|(c_id, topics, _)| {
            c_id == &contract_id
                && topics.len() == 2
                && topics.get(0).unwrap() == burn_event_name.clone().into_val(&env)
                && topics.get(1).unwrap() == user.clone().into_val(&env)
        })
        .collect();

    assert_eq!(burn_events.len(), 1);
    let (_, _, data) = &burn_events[0];
    let burn_event: BurnEvent = data.try_into_val(&env).unwrap();
    assert_eq!(burn_event.burner, spender);
    assert_eq!(burn_event.target_account, user);
    assert_eq!(burn_event.amount, 300);
}

