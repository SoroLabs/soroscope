use super::*;
use soroban_sdk::{contract, contractimpl, contracttype, testutils::Address as _, Address, Env};

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
fn test_borrow_pause_blocks_flash_loan_and_borrow_and_resume_allows() {
    let s = setup();
    fund_vault(&s, 10_000);

    let initiator = Address::generate(&s.e);
    let receiver_id = s.e.register(good::GoodReceiver, ());
    let receiver = good::GoodReceiverClient::new(&s.e, &receiver_id);
    receiver.set_vault(&s.vault_id);

    assert_eq!(s.vault_client.get_borrow_paused(), false);
    s.vault_client.pause_borrow();
    assert_eq!(s.vault_client.get_borrow_paused(), true);

    // While paused, flash-loan style borrows must fail with BorrowPaused.
    let flash_loan_res = s
        .vault_client
        .try_flash_loan(&initiator, &receiver_id, &5_000);
    assert_eq!(flash_loan_res, Err(Ok(Error::BorrowPaused)));
    // While paused, borrowing must fail with BorrowPaused.
    let res = s
        .vault_client
        .try_flash_loan(&initiator, &receiver_id, &5_000);
    assert_eq!(res, Err(Ok(Error::BorrowPaused)));

    let borrow_res = s.vault_client.try_borrow(&receiver_id, &5_000);
    assert_eq!(borrow_res, Err(Ok(Error::BorrowPaused)));

    // Resume and both borrowing entrypoints work again.
    s.vault_client.resume_borrow();
    assert_eq!(s.vault_client.get_borrow_paused(), false);

    let flash_loan_fee = s.vault_client.flash_loan(&initiator, &receiver_id, &5_000);
    assert_eq!(flash_loan_fee, 0);

    let borrow_fee = s.vault_client.borrow(&receiver_id, &5_000);
    assert_eq!(borrow_fee, 0);
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
