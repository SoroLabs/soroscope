use super::*;
use soroban_sdk::{
    contract, contractimpl, contracttype, testutils::Address as _, Address, Env,
};

// ── Mock receivers ───────────────────────────────────────────────────────────

pub mod good {
    use super::*;
    /// A well-behaved receiver that repays amount + fee.
    #[contract]
    pub struct GoodReceiver;

    #[contractimpl]
    impl GoodReceiver {
        pub fn execute_operation(
            e: Env,
            token: Address,
            amount: i128,
            fee: i128,
            _initiator: Address,
        ) {
            // Repay amount + fee back to the caller (the vault).
            let vault = e
                .storage()
                .instance()
                .get(&good::GoodReceiverDataKey::Vault)
                .unwrap();
            soroban_sdk::token::Client::new(&e, &token).transfer(
                &e.current_contract_address(),
                &vault,
                &(amount + fee),
            );
        }

        /// Helper: store the vault address so the receiver knows where to repay.
        pub fn set_vault(e: Env, vault: Address) {
            e.storage()
                .instance()
                .set(&good::GoodReceiverDataKey::Vault, &vault);
        }
    }

    #[contracttype]
    #[derive(Clone)]
    enum GoodReceiverDataKey {
        Vault,
    }
}

// ─────────────────────────────────────────────────────────────────────────────

/// A receiver that does NOT repay — the flash loan must revert.
#[contract]
pub struct BadReceiver;

#[contractimpl]
impl BadReceiver {
    pub fn execute_operation(
        _e: Env,
        _token: Address,
        _amount: i128,
        _fee: i128,
        _initiator: Address,
    ) {
        // Intentionally do nothing — don't repay.
    }
}

// ─────────────────────────────────────────────────────────────────────────────

pub mod partial {
    use super::*;
    /// A receiver that only repays part of the loan (amount but not fee).
    #[contract]
    pub struct PartialReceiver;

    #[contractimpl]
    impl PartialReceiver {
        pub fn execute_operation(
            e: Env,
            token: Address,
            amount: i128,
            _fee: i128,
            _initiator: Address,
        ) {
            // Repay only the principal, not the fee.
            let vault: Address = e
                .storage()
                .instance()
                .get(&partial::PartialReceiverDataKey::Vault)
                .unwrap();
            soroban_sdk::token::Client::new(&e, &token).transfer(
                &e.current_contract_address(),
                &vault,
                &amount,
            );
        }

        pub fn set_vault(e: Env, vault: Address) {
            e.storage()
                .instance()
                .set(&partial::PartialReceiverDataKey::Vault, &vault);
        }
    }

    #[contracttype]
    #[derive(Clone)]
    enum PartialReceiverDataKey {
        Vault,
    }
}

// ─────────────────────────────────────────────────────────────────────────────

pub mod overpay {
    use super::*;
    /// A receiver that overpays (returns more than required).
    #[contract]
    pub struct OverpayReceiver;

    #[contractimpl]
    impl OverpayReceiver {
        pub fn execute_operation(
            e: Env,
            token: Address,
            amount: i128,
            fee: i128,
            _initiator: Address,
        ) {
            let vault: Address = e
                .storage()
                .instance()
                .get(&overpay::OverpayReceiverDataKey::Vault)
                .unwrap();
            // Overpay by 100 extra.
            let overpay = amount + fee + 100;
            soroban_sdk::token::Client::new(&e, &token).transfer(
                &e.current_contract_address(),
                &vault,
                &overpay,
            );
        }

        pub fn set_vault(e: Env, vault: Address) {
            e.storage()
                .instance()
                .set(&overpay::OverpayReceiverDataKey::Vault, &vault);
        }
    }

    #[contracttype]
    #[derive(Clone)]
    enum OverpayReceiverDataKey {
        Vault,
    }
}

// ─────────────────────────────────────────────────────────────────────────────

pub mod reentrant {
    use super::*;
    /// A receiver that tries to re-enter flash_loan during the callback.
    #[contract]
    pub struct ReentrantReceiver;

    #[contractimpl]
    impl ReentrantReceiver {
        pub fn execute_operation(
            e: Env,
            token: Address,
            amount: i128,
            fee: i128,
            initiator: Address,
        ) {
            let vault: Address = e
                .storage()
                .instance()
                .get(&reentrant::ReentrantReceiverDataKey::Vault)
                .unwrap();

            // Try to re-enter the vault with another flash loan.
            let vault_client = FlashLoanVaultClient::new(&e, &vault);
            // This should fail with Reentrancy error.
            vault_client.flash_loan(&initiator, &e.current_contract_address(), &amount);

            // If we somehow get here, repay the original loan.
            soroban_sdk::token::Client::new(&e, &token).transfer(
                &e.current_contract_address(),
                &vault,
                &(amount + fee),
            );
        }

        pub fn set_vault(e: Env, vault: Address) {
            e.storage()
                .instance()
                .set(&reentrant::ReentrantReceiverDataKey::Vault, &vault);
        }
    }

    #[contracttype]
    #[derive(Clone)]
    enum ReentrantReceiverDataKey {
        Vault,
    }
}

// ─────────────────────────────────────────────────────────────────────────────

pub mod non_compliant {
    use super::*;
    /// A contract that does NOT implement the FlashLoanReceiver interface.
    #[contract]
    pub struct NonCompliantReceiver;

    #[contractimpl]
    impl NonCompliantReceiver {
        /// Intentionally does not implement execute_operation.
        pub fn dummy(_e: Env) {}
    }
}

// ── Test helpers ─────────────────────────────────────────────────────────────

struct TestSetup {
    e: Env,
    vault_id: Address,
    vault_client: FlashLoanVaultClient<'static>,
    token: Address,
    token_admin: soroban_sdk::token::StellarAssetClient<'static>,
    token_client: soroban_sdk::token::Client<'static>,
    admin: Address,
}

// SAFETY: TestSetup is only used in single-threaded tests.
unsafe impl Send for TestSetup {}
unsafe impl Sync for TestSetup {}

fn setup() -> TestSetup {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&e);
    let token = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token);
    let token_client = soroban_sdk::token::Client::new(&e, &token);

    let vault_id = e.register(FlashLoanVault, ());
    let vault_client = FlashLoanVaultClient::new(&e, &vault_id);

    vault_client.initialize(&admin, &token);

    TestSetup {
        e,
        vault_id,
        vault_client,
        token,
        token_admin,
        token_client,
        admin,
    }
}

fn fund_vault(setup: &TestSetup, amount: i128) {
    setup.token_admin.mint(&setup.admin, &amount);
    setup.vault_client.deposit(&setup.admin, &amount);
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[test]
fn test_initialize() {
    let s = setup();
    assert_eq!(s.vault_client.get_admin(), s.admin);
    assert_eq!(s.vault_client.get_token(), s.token);
    assert_eq!(s.vault_client.get_fee(), DEFAULT_FEE_BPS);
    assert_eq!(s.vault_client.get_total_deposited(), 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_double_initialization() {
    let s = setup();
    let another_admin = Address::generate(&s.e);
    s.vault_client.initialize(&another_admin, &s.token);
}

#[test]
fn test_deposit_withdraw() {
    let s = setup();

    // Deposit 10_000 tokens.
    s.token_admin.mint(&s.admin, &10_000);
    s.vault_client.deposit(&s.admin, &10_000);

    assert_eq!(s.vault_client.get_available(), 10_000);
    assert_eq!(s.vault_client.get_total_deposited(), 10_000);

    // Withdraw 3_000.
    s.vault_client.withdraw(&s.admin, &3_000);
    assert_eq!(s.vault_client.get_available(), 7_000);
    assert_eq!(s.vault_client.get_total_deposited(), 7_000);
    assert_eq!(s.token_client.balance(&s.admin), 3_000);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_deposit_unauthorized() {
    let s = setup();
    let stranger = Address::generate(&s.e);
    s.token_admin.mint(&stranger, &1_000);
    s.vault_client.deposit(&stranger, &1_000);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_withdraw_unauthorized() {
    let s = setup();
    fund_vault(&s, 5_000);
    let stranger = Address::generate(&s.e);
    s.vault_client.withdraw(&stranger, &1_000);
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")]
fn test_withdraw_exceeds_deposited() {
    let s = setup();
    fund_vault(&s, 1_000);
    s.vault_client.withdraw(&s.admin, &2_000);
}

#[test]
fn test_set_fee() {
    let s = setup();
    assert_eq!(s.vault_client.get_fee(), 0);

    s.vault_client.set_fee(&50);
    assert_eq!(s.vault_client.get_fee(), 50);

    s.vault_client.set_fee(&0);
    assert_eq!(s.vault_client.get_fee(), 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_set_fee_invalid() {
    let s = setup();
    s.vault_client.set_fee(&101); // Over MAX_FEE_BPS
}

// ── Granular borrow pause (Issue #320) ───────────────────────────────────────

#[test]
fn test_borrow_pause_blocks_flash_loan_and_resume_allows() {
    let s = setup();
    fund_vault(&s, 10_000);

    let initiator = Address::generate(&s.e);
    let receiver_id = s.e.register(good::GoodReceiver, ());
    let receiver = good::GoodReceiverClient::new(&s.e, &receiver_id);
    receiver.set_vault(&s.vault_id);

    assert_eq!(s.vault_client.get_borrow_paused(), false);
    s.vault_client.pause_borrow();
    assert_eq!(s.vault_client.get_borrow_paused(), true);

    // While paused, borrowing must fail with BorrowPaused.
    let res = s.vault_client.try_flash_loan(&initiator, &receiver_id, &5_000);
    assert_eq!(res, Err(Ok(Error::BorrowPaused)));

    // Resume and borrowing works again.
    s.vault_client.resume_borrow();
    assert_eq!(s.vault_client.get_borrow_paused(), false);
    let fee = s.vault_client.flash_loan(&initiator, &receiver_id, &5_000);
    assert_eq!(fee, 0);
}

#[test]
fn test_borrow_pause_does_not_affect_deposit_withdraw() {
    let s = setup();

    s.vault_client.pause_borrow();

    // Deposit/withdraw should still function normally.
    s.token_admin.mint(&s.admin, &3_000);
    s.vault_client.deposit(&s.admin, &3_000);
    assert_eq!(s.vault_client.get_available(), 3_000);

    s.vault_client.withdraw(&s.admin, &1_000);
    assert_eq!(s.vault_client.get_available(), 2_000);
    assert_eq!(s.token_client.balance(&s.admin), 1_000);
}

// ── Flash loan: success ──────────────────────────────────────────────────────

#[test]
fn test_flash_loan_success_zero_fee() {
    let s = setup();
    fund_vault(&s, 10_000);

    // Deploy good receiver.
    let receiver_id = s.e.register(good::GoodReceiver, ());
    let receiver_client = good::GoodReceiverClient::new(&s.e, &receiver_id);
    receiver_client.set_vault(&s.vault_id);

    // The receiver needs tokens to repay. In a real scenario it would earn
    // them from arbitrage. Here we pre-fund it with enough.
    // With 0 fee, it just needs to return the borrowed amount.
    let initiator = Address::generate(&s.e);

    let fee = s.vault_client.flash_loan(&initiator, &receiver_id, &5_000);
    assert_eq!(fee, 0);

    // Vault balance should be unchanged (5_000 returned).
    assert_eq!(s.vault_client.get_available(), 10_000);
}

#[test]
fn test_flash_loan_with_fee() {
    let s = setup();
    fund_vault(&s, 10_000);

    // Set 1% fee (100 bps).
    s.vault_client.set_fee(&100);

    // Deploy good receiver and pre-fund it with extra tokens for the fee.
    let receiver_id = s.e.register(good::GoodReceiver, ());
    let receiver_client = good::GoodReceiverClient::new(&s.e, &receiver_id);
    receiver_client.set_vault(&s.vault_id);

    // Pre-fund receiver with enough for the fee: 5000 * 100 / 10000 = 50.
    s.token_admin.mint(&receiver_id, &50);

    let initiator = Address::generate(&s.e);
    let fee = s.vault_client.flash_loan(&initiator, &receiver_id, &5_000);
    assert_eq!(fee, 50);

    // Vault should now have 10_000 + 50 = 10_050.
    assert_eq!(s.vault_client.get_available(), 10_050);
    // Total deposited should also reflect the fee.
    assert_eq!(s.vault_client.get_total_deposited(), 10_050);
}

#[test]
fn test_flash_loan_fee_accumulates_total_deposited_over_multiple_loans() {
    let s = setup();
    fund_vault(&s, 10_000);

    s.vault_client.set_fee(&50);

    let receiver_id = s.e.register(good::GoodReceiver, ());
    let receiver_client = good::GoodReceiverClient::new(&s.e, &receiver_id);
    receiver_client.set_vault(&s.vault_id);

    s.token_admin.mint(&receiver_id, &45);
    let initiator = Address::generate(&s.e);

    let mut total_fees = 0;
    for _ in 0..3 {
        let fee = s.vault_client.flash_loan(&initiator, &receiver_id, &3_000);
        assert_eq!(fee, 15);
        total_fees += fee;
    }

    assert_eq!(total_fees, 45);
    assert_eq!(s.vault_client.get_available(), 10_045);
    assert_eq!(s.vault_client.get_total_deposited(), 10_045);
}

#[test]
fn test_flash_loan_fee_accumulation_updates_balance_and_total_deposited() {
    let s = setup();
    fund_vault(&s, 20_000);

    s.vault_client.set_fee(&100);

    let receiver_id = s.e.register(good::GoodReceiver, ());
    let receiver_client = good::GoodReceiverClient::new(&s.e, &receiver_id);
    receiver_client.set_vault(&s.vault_id);

    s.token_admin.mint(&receiver_id, &100);
    let initiator = Address::generate(&s.e);

    let fee = s.vault_client.flash_loan(&initiator, &receiver_id, &5_000);
    assert_eq!(fee, 50);
    assert_eq!(s.vault_client.get_available(), 20_050);
    assert_eq!(s.vault_client.get_total_deposited(), 20_050);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_flash_loan_no_repay_panics() {
    let s = setup();
    fund_vault(&s, 10_000);

    let receiver_id = s.e.register(BadReceiver, ());
    let initiator = Address::generate(&s.e);

    s.vault_client.flash_loan(&initiator, &receiver_id, &5_000);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_flash_loan_partial_repay_panics() {
    let s = setup();
    fund_vault(&s, 10_000);

    s.vault_client.set_fee(&100);

    let receiver_id = s.e.register(partial::PartialReceiver, ());
    let receiver_client = partial::PartialReceiverClient::new(&s.e, &receiver_id);
    receiver_client.set_vault(&s.vault_id);

    let initiator = Address::generate(&s.e);
    s.vault_client.flash_loan(&initiator, &receiver_id, &5_000);
}

#[test]
fn test_flash_loan_borrow_entire_vault() {
    let s = setup();
    fund_vault(&s, 10_000);

    let receiver_id = s.e.register(good::GoodReceiver, ());
    let receiver_client = good::GoodReceiverClient::new(&s.e, &receiver_id);
    receiver_client.set_vault(&s.vault_id);

    let initiator = Address::generate(&s.e);

    // Borrow 100% of the vault.
    let fee = s.vault_client.flash_loan(&initiator, &receiver_id, &10_000);
    assert_eq!(fee, 0);
    assert_eq!(s.vault_client.get_available(), 10_000);
}

// ── Flash loan: failures ─────────────────────────────────────────────────────

// #[test]
// #[should_panic(expected = "flash loan not repaid")]
// fn test_flash_loan_no_repay() {
//     let s = setup();
//     fund_vault(&s, 10_000);
//
//     let receiver_id = s.e.register(bad::BadReceiver, ());
//     let initiator = Address::generate(&s.e);
//
//     // Bad receiver doesn't repay — entire transaction should revert.
//     s.vault_client
//         .flash_loan(&initiator, &receiver_id, &5_000);
// }
//
// #[test]
// #[should_panic(expected = "flash loan not repaid")]
// fn test_flash_loan_partial_repay() {
//     let s = setup();
//     fund_vault(&s, 10_000);
//
//     // Set fee so partial repay (principal only) is insufficient.
//     s.vault_client.set_fee(&100);
//
//     let receiver_id = s.e.register(partial::PartialReceiver, ());
//     let receiver_client = partial::PartialReceiverClient::new(&s.e, &receiver_id);
//     receiver_client.set_vault(&s.vault_id);
//
//     let initiator = Address::generate(&s.e);
//
//     // Partial receiver repays principal but not fee → revert.
//     s.vault_client
//         .flash_loan(&initiator, &receiver_id, &5_000);
// }

#[test]
fn test_flash_loan_no_repay_returns_error() {
    let s = setup();
    fund_vault(&s, 10_000);

    let receiver_id = s.e.register(BadReceiver, ());
    let initiator = Address::generate(&s.e);

    let result = s
        .vault_client
        .try_flash_loan(&initiator, &receiver_id, &5_000);

    assert_eq!(result, Err(Ok(Error::LoanNotRepaid)));
    assert_eq!(s.vault_client.get_available(), 10_000);
    assert_eq!(s.token_client.balance(&receiver_id), 0);
}

#[test]
fn test_flash_loan_partial_repay_returns_error() {
    let s = setup();
    fund_vault(&s, 10_000);

    // Set fee so partial repay (principal only) is insufficient.
    s.vault_client.set_fee(&100);

    let receiver_id = s.e.register(partial::PartialReceiver, ());
    let receiver_client = partial::PartialReceiverClient::new(&s.e, &receiver_id);
    receiver_client.set_vault(&s.vault_id);

    let initiator = Address::generate(&s.e);

    let result = s
        .vault_client
        .try_flash_loan(&initiator, &receiver_id, &5_000);

    assert_eq!(result, Err(Ok(Error::LoanNotRepaid)));
    assert_eq!(s.vault_client.get_available(), 10_000);
    assert_eq!(s.vault_client.get_total_deposited(), 10_000);
}

#[test]
fn test_flash_loan_failure_does_not_accumulate_fee() {
    let s = setup();
    fund_vault(&s, 10_000);

    s.vault_client.set_fee(&100);

    let receiver_id = s.e.register(BadReceiver, ());
    let initiator = Address::generate(&s.e);

    let result = s
        .vault_client
        .try_flash_loan(&initiator, &receiver_id, &5_000);

    assert_eq!(result, Err(Ok(Error::LoanNotRepaid)));
    assert_eq!(s.vault_client.get_available(), 10_000);
    assert_eq!(s.vault_client.get_total_deposited(), 10_000);
}

#[test]
fn test_flash_loan_overpay() {
    let s = setup();
    fund_vault(&s, 10_000);

    let receiver_id = s.e.register(overpay::OverpayReceiver, ());
    let receiver_client = overpay::OverpayReceiverClient::new(&s.e, &receiver_id);
    receiver_client.set_vault(&s.vault_id);

    // Pre-fund receiver with 100 extra tokens for the overpayment.
    s.token_admin.mint(&receiver_id, &100);

    let initiator = Address::generate(&s.e);

    let fee = s.vault_client.flash_loan(&initiator, &receiver_id, &5_000);
    assert_eq!(fee, 0);

    // Vault should now have 10_000 + 100 (overpayment stays in vault).
    assert_eq!(s.vault_client.get_available(), 10_100);
}

// ── Flash loan: reentrancy ───────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Contract re-entry is not allowed")]
#[should_panic(expected = "InvalidAction")]
fn test_reentrancy_guard() {
    let s = setup();
    fund_vault(&s, 10_000);

    let receiver_id = s.e.register(reentrant::ReentrantReceiver, ());
    let receiver_client = reentrant::ReentrantReceiverClient::new(&s.e, &receiver_id);
    receiver_client.set_vault(&s.vault_id);

    let initiator = Address::generate(&s.e);

    // Reentrant receiver tries to call flash_loan again during callback.
    // Should fail with Reentrancy error.
    s.vault_client.flash_loan(&initiator, &receiver_id, &5_000);
}

#[test]
fn test_mutations_reject_while_flash_loan_active() {
    let s = setup();
    fund_vault(&s, 10_000);

    s.e.as_contract(&s.vault_id, || {
        set_flash_loan_active(&s.e, true);
    });

    assert_eq!(
        s.vault_client.try_deposit(&s.admin, &1),
        Err(Ok(Error::Reentrancy))
    );
    assert_eq!(
        s.vault_client.try_withdraw(&s.admin, &1),
        Err(Ok(Error::Reentrancy))
    );
    assert_eq!(s.vault_client.try_set_fee(&25), Err(Ok(Error::Reentrancy)));
    assert_eq!(
        s.vault_client.try_set_paused(&s.admin, &true),
        Err(Ok(Error::Reentrancy))
    );
    assert_eq!(
        s.vault_client
            .try_emergency_pause(&soroban_sdk::vec![&s.e, s.admin.clone()]),
        Err(Ok(Error::Reentrancy))
    );

    s.e.as_contract(&s.vault_id, || {
        set_flash_loan_active(&s.e, false);
    });

    assert_eq!(s.vault_client.get_fee(), 0);
    assert_eq!(s.vault_client.get_available(), 10_000);
    assert_eq!(s.vault_client.get_total_deposited(), 10_000);
    assert_eq!(s.token_client.balance(&s.admin), 0);
}

// ── Flash loan: edge cases ───────────────────────────────────────────────────

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_flash_loan_exceeds_vault_balance() {
    let s = setup();
    fund_vault(&s, 1_000);

    let receiver_id = s.e.register(good::GoodReceiver, ());
    let initiator = Address::generate(&s.e);

    // Borrow more than available.
    s.vault_client.flash_loan(&initiator, &receiver_id, &5_000);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_flash_loan_zero_amount() {
    let s = setup();
    fund_vault(&s, 1_000);

    let receiver_id = s.e.register(good::GoodReceiver, ());
    let initiator = Address::generate(&s.e);

    s.vault_client.flash_loan(&initiator, &receiver_id, &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_flash_loan_negative_amount() {
    let s = setup();
    fund_vault(&s, 1_000);

    let receiver_id = s.e.register(good::GoodReceiver, ());
    let initiator = Address::generate(&s.e);

    s.vault_client.flash_loan(&initiator, &receiver_id, &-100);
}

// ── Multiple flash loans in sequence ─────────────────────────────────────────

#[test]
fn test_sequential_flash_loans() {
    let s = setup();
    fund_vault(&s, 10_000);

    let receiver_id = s.e.register(good::GoodReceiver, ());
    let receiver_client = good::GoodReceiverClient::new(&s.e, &receiver_id);
    receiver_client.set_vault(&s.vault_id);

    let initiator = Address::generate(&s.e);

    // Execute multiple flash loans in sequence (not nested).
    for _ in 0..3 {
        let fee = s.vault_client.flash_loan(&initiator, &receiver_id, &3_000);
        assert_eq!(fee, 0);
    }

    // Vault balance should be unchanged.
    assert_eq!(s.vault_client.get_available(), 10_000);
}

#[test]
fn test_sequential_flash_loans_with_fee() {
    let s = setup();
    fund_vault(&s, 10_000);

    // 50 bps fee = 0.5%.
    s.vault_client.set_fee(&50);

    let receiver_id = s.e.register(good::GoodReceiver, ());
    let receiver_client = good::GoodReceiverClient::new(&s.e, &receiver_id);
    receiver_client.set_vault(&s.vault_id);

    // Pre-fund receiver with enough for 3 fees: 3 * (3000 * 50 / 10000) = 3 * 15 = 45.
    s.token_admin.mint(&receiver_id, &45);

    let initiator = Address::generate(&s.e);

    let mut total_fees: i128 = 0;
    for _ in 0..3 {
        let fee = s.vault_client.flash_loan(&initiator, &receiver_id, &3_000);
        assert_eq!(fee, 15);
        total_fees += fee;
    }

    assert_eq!(total_fees, 45);
    assert_eq!(s.vault_client.get_available(), 10_045);
}

// ── Borrow (alias) tests ───────────────────────────────────────────────────

#[test]
fn test_borrow_success_zero_fee() {
    let s = setup();
    fund_vault(&s, 10_000);

    // Deploy good receiver and have it repay to the vault.
    let receiver_id = s.e.register(good::GoodReceiver, ());
    let receiver_client = good::GoodReceiverClient::new(&s.e, &receiver_id);
    receiver_client.set_vault(&s.vault_id);

    // Borrow 5_000 with 0 fee.
    let fee = s.vault_client.borrow(&receiver_id, &5_000);
    assert_eq!(fee, 0);
    assert_eq!(s.vault_client.get_available(), 10_000);
}

#[test]
fn test_borrow_with_fee() {
    let s = setup();
    fund_vault(&s, 10_000);

    // Set 1% fee (100 bps).
    s.vault_client.set_fee(&100);

    // Deploy good receiver and pre-fund it with extra tokens for the fee.
    let receiver_id = s.e.register(good::GoodReceiver, ());
    let receiver_client = good::GoodReceiverClient::new(&s.e, &receiver_id);
    receiver_client.set_vault(&s.vault_id);

    // Pre-fund receiver with enough for the fee: 5000 * 100 / 10000 = 50.
    s.token_admin.mint(&receiver_id, &50);

    let fee = s.vault_client.borrow(&receiver_id, &5_000);
    assert_eq!(fee, 50);

    // Vault should now have 10_000 + 50 = 10_050.
    assert_eq!(s.vault_client.get_available(), 10_050);
    assert_eq!(s.vault_client.get_total_deposited(), 10_050);
}

// ── View functions ───────────────────────────────────────────────────────────

#[test]
fn test_get_available_empty_vault() {
    let s = setup();
    assert_eq!(s.vault_client.get_available(), 0);
}

#[test]
fn test_get_available_after_deposit() {
    let s = setup();
    fund_vault(&s, 5_000);
    assert_eq!(s.vault_client.get_available(), 5_000);
}

// ── Flash loan: interface verification ──────────────────────────────────────

#[test]
fn test_flash_loan_non_compliant_receiver_returns_error() {
    let s = setup();
    fund_vault(&s, 10_000);

    let receiver_id = s.e.register(non_compliant::NonCompliantReceiver, ());
    let initiator = Address::generate(&s.e);

    let result = s
        .vault_client
        .try_flash_loan(&initiator, &receiver_id, &5_000);

    assert!(result.is_err());
    // Vault funds should remain intact.
    assert_eq!(s.vault_client.get_available(), 10_000);
    assert_eq!(s.token_client.balance(&receiver_id), 0);
}

#[test]
fn test_borrow_non_compliant_receiver_returns_error() {
    let s = setup();
    fund_vault(&s, 10_000);

    let receiver_id = s.e.register(non_compliant::NonCompliantReceiver, ());

    let result = s.vault_client.try_borrow(&receiver_id, &5_000);

    assert!(result.is_err());
    assert_eq!(s.vault_client.get_available(), 10_000);
}
