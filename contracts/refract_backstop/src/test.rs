#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, Address, Env, String,
};

const DECIMALS: u32 = 7;
const DEPOSIT_AMOUNT: i128 = 10_000_000;

fn advance_ledger(e: &Env, to: u32) {
    let mut info = e.ledger().get();
    info.sequence_number = to;
    e.ledger().set(info);
}

fn setup() -> (
    Env,
    RefractBackstopClient<'static>,
    Address,
    Address,
    Address,
) {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&e);
    let backstop_token = e
        .register_stellar_asset_contract_v2(Address::generate(&e))
        .address();

    let contract_id = e.register(RefractBackstop, ());
    let client = RefractBackstopClient::new(&e, &contract_id);

    client.initialize(
        &admin,
        &backstop_token,
        &String::from_str(&e, "Refract Backstop Receipt"),
        &String::from_str(&e, "rBACKSTOP"),
        &DECIMALS,
    );

    (e, client, admin, backstop_token, contract_id)
}

fn mint_backstop_token(e: &Env, token_addr: &Address, to: &Address, amount: i128) {
    let token_client = token::StellarAssetClient::new(e, token_addr);
    token_client.mint(to, &amount);
}

// ── Initialization ────────────────────────────────────────────────────────────

#[test]
fn test_initialize() {
    let (e, client, admin, backstop_token, _) = setup();

    let config = client.get_config();
    assert_eq!(config.admin, admin);
    assert_eq!(config.backstop_token, backstop_token);

    assert_eq!(client.total_supply(), 0);
    assert_eq!(client.decimals(), DECIMALS);
    assert_eq!(client.name(), String::from_str(&e, "Refract Backstop Receipt"));
    assert_eq!(client.symbol(), String::from_str(&e, "rBACKSTOP"));
}

#[test]
#[should_panic(expected = "Contract, #1")]
fn test_double_initialize() {
    let (_e, client, admin, backstop_token, _) = setup();
    client.initialize(
        &admin,
        &backstop_token,
        &String::from_str(&_e, "Duplicate"),
        &String::from_str(&_e, "DUP"),
        &DECIMALS,
    );
}

// ── Deposit & Withdraw ───────────────────────────────────────────────────────

#[test]
fn test_deposit_mints_receipts_1to1() {
    let (e, client, _admin, backstop_token, contract_id) = setup();
    let user = Address::generate(&e);

    mint_backstop_token(&e, &backstop_token, &user, DEPOSIT_AMOUNT);
    client.deposit(&user, &DEPOSIT_AMOUNT);

    // Receipt balance equals deposit amount (1:1).
    assert_eq!(client.balance(&user), DEPOSIT_AMOUNT);
    assert_eq!(client.total_supply(), DEPOSIT_AMOUNT);

    // Backing tokens are held by the contract.
    let backing = token::Client::new(&e, &backstop_token).balance(&contract_id);
    assert_eq!(backing, DEPOSIT_AMOUNT);

    // User's backstop token balance decreased.
    let user_backing = token::Client::new(&e, &backstop_token).balance(&user);
    assert_eq!(user_backing, 0);
}

#[test]
fn test_withdraw_burns_receipts_returns_backing() {
    let (e, client, _admin, backstop_token, contract_id) = setup();
    let user = Address::generate(&e);

    mint_backstop_token(&e, &backstop_token, &user, DEPOSIT_AMOUNT);
    client.deposit(&user, &DEPOSIT_AMOUNT);

    let withdraw_amount = DEPOSIT_AMOUNT / 2;
    client.withdraw(&user, &withdraw_amount);

    assert_eq!(client.balance(&user), DEPOSIT_AMOUNT - withdraw_amount);
    assert_eq!(client.total_supply(), DEPOSIT_AMOUNT - withdraw_amount);

    let user_backing = token::Client::new(&e, &backstop_token).balance(&user);
    assert_eq!(user_backing, withdraw_amount);

    let contract_backing = token::Client::new(&e, &backstop_token).balance(&contract_id);
    assert_eq!(contract_backing, DEPOSIT_AMOUNT - withdraw_amount);
}

#[test]
#[should_panic(expected = "Contract, #4")]
fn test_withdraw_insufficient_receipts() {
    let (e, client, _admin, backstop_token, _) = setup();
    let user = Address::generate(&e);

    mint_backstop_token(&e, &backstop_token, &user, 1_000);
    client.deposit(&user, &1_000);

    client.withdraw(&user, &2_000);
}

#[test]
#[should_panic(expected = "Contract, #18")]
fn test_deposit_zero() {
    let (e, client, _admin, backstop_token, _) = setup();
    let user = Address::generate(&e);
    mint_backstop_token(&e, &backstop_token, &user, 1_000);
    client.deposit(&user, &0);
}

#[test]
#[should_panic(expected = "Contract, #18")]
fn test_withdraw_zero() {
    let (e, client, _admin, backstop_token, _) = setup();
    let user = Address::generate(&e);
    mint_backstop_token(&e, &backstop_token, &user, 1_000);
    client.deposit(&user, &1_000);
    client.withdraw(&user, &0);
}

// ── SEP-41 transfer / transfer_from ───────────────────────────────────────────

#[test]
fn test_transfer_receipts() {
    let (e, client, _admin, backstop_token, _) = setup();
    let alice = Address::generate(&e);
    let bob = Address::generate(&e);

    mint_backstop_token(&e, &backstop_token, &alice, DEPOSIT_AMOUNT);
    client.deposit(&alice, &DEPOSIT_AMOUNT);

    let transfer_amount = 3_000_000;
    client.transfer(&alice, &bob, &transfer_amount);

    assert_eq!(client.balance(&alice), DEPOSIT_AMOUNT - transfer_amount);
    assert_eq!(client.balance(&bob), transfer_amount);
    assert_eq!(client.total_supply(), DEPOSIT_AMOUNT); // supply unchanged
}

#[test]
fn test_transfer_from_receipts() {
    let (e, client, _admin, backstop_token, _) = setup();
    let alice = Address::generate(&e);
    let bob = Address::generate(&e);
    let spender = Address::generate(&e);

    mint_backstop_token(&e, &backstop_token, &alice, DEPOSIT_AMOUNT);
    client.deposit(&alice, &DEPOSIT_AMOUNT);

    let approved_amount = 5_000_000;
    let exp_ledger = e.ledger().sequence() + 100;
    client.approve(&alice, &spender, &approved_amount, &exp_ledger);

    assert_eq!(client.allowance(&alice, &spender), approved_amount);

    let transfer_amount = 2_000_000;
    client.transfer_from(&spender, &alice, &bob, &transfer_amount);

    assert_eq!(client.balance(&alice), DEPOSIT_AMOUNT - transfer_amount);
    assert_eq!(client.balance(&bob), transfer_amount);
    assert_eq!(
        client.allowance(&alice, &spender),
        approved_amount - transfer_amount
    );
}

#[test]
fn test_composability_via_token_client() {
    let (e, client, _admin, backstop_token, contract_id) = setup();
    let alice = Address::generate(&e);
    let bob = Address::generate(&e);

    mint_backstop_token(&e, &backstop_token, &alice, DEPOSIT_AMOUNT);
    client.deposit(&alice, &DEPOSIT_AMOUNT);

    // Interact with receipts through the standard SEP-41 token::Client.
    let receipt_client = token::Client::new(&e, &contract_id);
    assert_eq!(receipt_client.balance(&alice), DEPOSIT_AMOUNT);

    receipt_client.transfer(&alice, &bob, &4_000_000);
    assert_eq!(receipt_client.balance(&alice), DEPOSIT_AMOUNT - 4_000_000);
    assert_eq!(receipt_client.balance(&bob), 4_000_000);

    // Metadata is also readable via standard client.
    assert_eq!(receipt_client.decimals(), DECIMALS);
    assert_eq!(receipt_client.name(), String::from_str(&e, "Refract Backstop Receipt"));
    assert_eq!(receipt_client.symbol(), String::from_str(&e, "rBACKSTOP"));
}

// ── Burn ─────────────────────────────────────────────────────────────────────

#[test]
fn test_burn_receipts() {
    let (e, client, _admin, backstop_token, _) = setup();
    let user = Address::generate(&e);

    mint_backstop_token(&e, &backstop_token, &user, DEPOSIT_AMOUNT);
    client.deposit(&user, &DEPOSIT_AMOUNT);

    let burn_amount = 3_000_000;
    client.burn(&user, &burn_amount);

    assert_eq!(client.balance(&user), DEPOSIT_AMOUNT - burn_amount);
    assert_eq!(client.total_supply(), DEPOSIT_AMOUNT - burn_amount);
}

#[test]
fn test_burn_from_receipts() {
    let (e, client, _admin, backstop_token, _) = setup();
    let alice = Address::generate(&e);
    let spender = Address::generate(&e);

    mint_backstop_token(&e, &backstop_token, &alice, DEPOSIT_AMOUNT);
    client.deposit(&alice, &DEPOSIT_AMOUNT);

    let exp_ledger = e.ledger().sequence() + 100;
    client.approve(&alice, &spender, &5_000_000, &exp_ledger);

    client.burn_from(&spender, &alice, &2_000_000);

    assert_eq!(client.balance(&alice), DEPOSIT_AMOUNT - 2_000_000);
    assert_eq!(client.total_supply(), DEPOSIT_AMOUNT - 2_000_000);
    assert_eq!(client.allowance(&alice, &spender), 3_000_000);
}

// ── Loss socialization ───────────────────────────────────────────────────────

#[test]
fn test_socialize_loss_pro_rata() {
    let (e, client, admin, backstop_token, contract_id) = setup();

    // Three depositors with different amounts.
    let alice = Address::generate(&e);
    let bob = Address::generate(&e);
    let carol = Address::generate(&e);

    let alice_dep = 40_000_000;
    let bob_dep = 30_000_000;
    let carol_dep = 30_000_000;
    let total = alice_dep + bob_dep + carol_dep;

    mint_backstop_token(&e, &backstop_token, &alice, alice_dep);
    mint_backstop_token(&e, &backstop_token, &bob, bob_dep);
    mint_backstop_token(&e, &backstop_token, &carol, carol_dep);

    client.deposit(&alice, &alice_dep);
    client.deposit(&bob, &bob_dep);
    client.deposit(&carol, &carol_dep);

    assert_eq!(client.total_supply(), total);

    let loss_amount = 10_000_000;
    let destination = Address::generate(&e);

    let burned = client.socialize_loss(&admin, &loss_amount, &destination);
    // Due to rounding, the total burned may differ slightly from loss_amount.
    assert!(burned > 0);
    assert!(burned <= loss_amount);

    // Backing tokens were transferred to destination.
    let dest_balance = token::Client::new(&e, &backstop_token).balance(&destination);
    assert_eq!(dest_balance, loss_amount);

    // Contract backing decreased by loss_amount.
    let contract_balance = token::Client::new(&e, &backstop_token).balance(&contract_id);
    assert_eq!(contract_balance, total - loss_amount);

    // Total supply decreased by the burned amount.
    assert_eq!(client.total_supply(), total - burned);

    // Each holder's receipt was reduced pro-rata.
    let alice_balance = client.balance(&alice);
    let bob_balance = client.balance(&bob);
    let carol_balance = client.balance(&carol);

    // Alice had 40% → should lose ~40% of the burn.
    let alice_expected_burn = alice_dep * loss_amount / total;
    assert_eq!(alice_balance, alice_dep - alice_expected_burn);

    let bob_expected_burn = bob_dep * loss_amount / total;
    assert_eq!(bob_balance, bob_dep - bob_expected_burn);

    let carol_expected_burn = carol_dep * loss_amount / total;
    assert_eq!(carol_balance, carol_dep - carol_expected_burn);

    // The 1:1 ratio is preserved: backing / supply ≈ 1.
    let ratio_num = contract_balance;
    let ratio_den = client.total_supply();
    assert!(ratio_num <= ratio_den); // slight rounding favors holders
}

#[test]
#[should_panic(expected = "Contract, #21")]
fn test_socialize_loss_exceeds_supply() {
    let (e, client, admin, backstop_token, _) = setup();
    let user = Address::generate(&e);

    mint_backstop_token(&e, &backstop_token, &user, 1_000);
    client.deposit(&user, &1_000);

    let dest = Address::generate(&e);
    client.socialize_loss(&admin, &2_000, &dest);
}

#[test]
#[should_panic(expected = "Contract, #18")]
fn test_socialize_loss_zero() {
    let (e, client, admin, backstop_token, _) = setup();
    let user = Address::generate(&e);

    mint_backstop_token(&e, &backstop_token, &user, 1_000);
    client.deposit(&user, &1_000);

    let dest = Address::generate(&e);
    client.socialize_loss(&admin, &0, &dest);
}

#[test]
fn test_socialize_loss_preserves_ratio_after_partial_withdraw() {
    let (e, client, admin, backstop_token, contract_id) = setup();

    let alice = Address::generate(&e);
    let bob = Address::generate(&e);

    mint_backstop_token(&e, &backstop_token, &alice, 50_000_000);
    mint_backstop_token(&e, &backstop_token, &bob, 50_000_000);

    client.deposit(&alice, &50_000_000);
    client.deposit(&bob, &50_000_000);

    // Alice withdraws half.
    client.withdraw(&alice, &25_000_000);

    assert_eq!(client.total_supply(), 75_000_000);

    let loss = 15_000_000;
    let dest = Address::generate(&e);
    client.socialize_loss(&admin, &loss, &dest);

    // Alice should lose: 25M * 15M / 75M = 5M
    assert_eq!(client.balance(&alice), 25_000_000 - 5_000_000);
    // Bob should lose: 50M * 15M / 75M = 10M
    assert_eq!(client.balance(&bob), 50_000_000 - 10_000_000);

    let contract_backing = token::Client::new(&e, &backstop_token).balance(&contract_id);
    assert_eq!(contract_backing, 75_000_000 - 15_000_000);
}

// ── Emergency guard integration ───────────────────────────────────────────────

#[test]
#[should_panic(expected = "operation paused")]
fn test_pause_blocks_deposit() {
    let (e, client, admin, backstop_token, _) = setup();
    let user = Address::generate(&e);

    mint_backstop_token(&e, &backstop_token, &user, 1_000);

    client.set_pause(&admin, &emergency_guard::PauseType::DEPOSIT, &true);
    client.deposit(&user, &1_000);
}

#[test]
#[should_panic(expected = "operation paused")]
fn test_pause_blocks_withdraw() {
    let (e, client, admin, backstop_token, _) = setup();
    let user = Address::generate(&e);

    mint_backstop_token(&e, &backstop_token, &user, 1_000);
    client.deposit(&user, &1_000);

    client.set_pause(&admin, &emergency_guard::PauseType::WITHDRAW, &true);
    client.withdraw(&user, &500);
}

#[test]
#[should_panic(expected = "operation paused")]
fn test_pause_blocks_transfer() {
    let (e, client, admin, backstop_token, _) = setup();
    let alice = Address::generate(&e);
    let bob = Address::generate(&e);

    mint_backstop_token(&e, &backstop_token, &alice, 1_000);
    client.deposit(&alice, &1_000);

    client.set_pause(&admin, &emergency_guard::PauseType::TRANSFER, &true);
    client.transfer(&alice, &bob, &500);
}

#[test]
#[should_panic(expected = "operation paused")]
fn test_pause_blocks_burn() {
    let (e, client, admin, backstop_token, _) = setup();
    let user = Address::generate(&e);

    mint_backstop_token(&e, &backstop_token, &user, 1_000);
    client.deposit(&user, &1_000);

    client.set_pause(&admin, &emergency_guard::PauseType::BURN, &true);
    client.burn(&user, &500);
}

#[test]
#[should_panic(expected = "operation paused")]
fn test_pause_blocks_socialize_loss() {
    let (e, client, admin, backstop_token, _) = setup();
    let user = Address::generate(&e);

    mint_backstop_token(&e, &backstop_token, &user, 1_000);
    client.deposit(&user, &1_000);

    client.set_pause(&admin, &PAUSE_SOCIALIZE_LOSS, &true);
    let dest = Address::generate(&e);
    client.socialize_loss(&admin, &500, &dest);
}

#[test]
fn test_get_holders() {
    let (e, client, _admin, backstop_token, _) = setup();
    let alice = Address::generate(&e);
    let bob = Address::generate(&e);

    mint_backstop_token(&e, &backstop_token, &alice, 1_000);
    mint_backstop_token(&e, &backstop_token, &bob, 1_000);

    client.deposit(&alice, &1_000);
    client.deposit(&bob, &1_000);

    let holders = client.get_holders();
    assert_eq!(holders.len(), 2);
}

#[test]
fn test_multiple_deposits_same_holder_not_duplicated() {
    let (e, client, _admin, backstop_token, _) = setup();
    let alice = Address::generate(&e);

    mint_backstop_token(&e, &backstop_token, &alice, 5_000);
    client.deposit(&alice, &1_000);
    client.deposit(&alice, &1_000);
    client.deposit(&alice, &1_000);

    let holders = client.get_holders();
    assert_eq!(holders.len(), 1);
    assert_eq!(client.balance(&alice), 3_000);
}
