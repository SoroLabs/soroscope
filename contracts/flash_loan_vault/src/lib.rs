#![no_std]
//! # Flash Loan Vault
//!
//! A dedicated vault contract that enables atomic flash loans on Soroban.
//! Users can borrow the full vault balance for 0 fee (or a small configurable
//! fee) as long as they return the borrowed amount (+ fee) before the
//! invocation completes.
//!
//! ## Safety Model
//!
//! 1. **Atomic rollback**: Soroban reverts all state changes if any step panics.
//!    If the borrower fails to repay, the balance check at step 9 fails and
//!    the transfer at step 6 is rolled back — funds never leave the vault.
//!
//! 2. **Reentrancy guard**: A `FlashLoanActive` flag prevents nested flash
//!    loans from the same vault during a callback.
//!
//! ## Flow
//!
//! ```text
//! 1. if BorrowPaused → Err(BorrowPaused)
//! 2. check_not_paused(PauseType::BORROW)
//! 3. if FlashLoanActive → Err(Reentrancy)
//! 4. set FlashLoanActive = true
//! 5. pre_balance = token.balance(self)
//! 6. fee = amount * fee_bps / 10_000
//! 7. token.transfer(self → receiver, amount)
//! 8. ReceiverClient::execute_operation(token, amount, fee, initiator)
//! 9. post_balance = token.balance(self)
//! 10. assert post_balance >= pre_balance + fee
//! 11. set FlashLoanActive = false
//! 12. emit FlashLoanEvent
//! ```

use emergency_guard::{EmergencyGuard, PauseType};
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, String, Vec};

#[cfg(test)]
mod test;

// ── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// Contract has already been initialized.
    AlreadyInitialized = 1,
    /// Contract has not been initialized yet.
    NotInitialized = 2,
    /// Caller is not the vault admin.
    Unauthorized = 3,
    /// Flash loan is already in progress (reentrancy).
    Reentrancy = 4,
    /// Borrower did not repay the required amount.
    LoanNotRepaid = 5,
    /// Requested borrow exceeds available vault balance.
    InsufficientVaultBalance = 6,
    /// Invalid amount (zero or negative).
    InvalidAmount = 7,
    /// Fee basis points out of valid range (0–100).
    InvalidFee = 8,
    /// Contract is paused.
    Paused = 9,
    /// Requested withdrawal exceeds deposited balance.
    InsufficientDeposit = 10,
    /// Borrow (flash loan) is paused.
    BorrowPaused = 11,
    /// Receiver contract does not implement the FlashLoanReceiver interface.
    InvalidReceiver = 12,
}

// ── Event types ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlashLoanEvent {
    pub receiver: Address,
    pub token: Address,
    pub amount: i128,
    pub fee: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultDepositEvent {
    pub admin: Address,
    pub amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultWithdrawEvent {
    pub admin: Address,
    pub amount: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeeChangedEvent {
    pub admin: Address,
    pub old_fee_bps: i128,
    pub new_fee_bps: i128,
}

// ── Storage keys ─────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Vault admin address.
    Admin,
    /// The token this vault lends.
    Token,
    /// Flash loan fee in basis points (0–100).
    FeeBps,
    /// Reentrancy guard: true while a flash loan callback is executing.
    FlashLoanActive,
    /// Granular pause: blocks BORROW/flash_loan only, without affecting admin ops.
    BorrowPaused,
    /// Total amount the admin has deposited (tracked for withdrawal cap).
    TotalDeposited,
    /// Temporary record of the last borrow for a borrower address.
    BorrowRecord(Address),
}

// ── Flash loan receiver interface ────────────────────────────────────────────

/// Trait that borrower contracts must implement.
///
/// The vault calls `execute_operation` after transferring the borrowed tokens.
/// The receiver must ensure that `amount + fee` is transferred back to the
/// vault address before this function returns.
#[soroban_sdk::contractclient(name = "FlashLoanReceiverClient")]
pub trait FlashLoanReceiver {
    /// Called by the vault during a flash loan.
    ///
    /// # Arguments
    /// * `token`     – address of the borrowed token
    /// * `amount`    – amount borrowed
    /// * `fee`       – fee owed to the vault (may be 0)
    /// * `initiator` – address that initiated the flash loan
    fn execute_operation(e: Env, token: Address, amount: i128, fee: i128, initiator: Address);
}

// ── Constants ────────────────────────────────────────────────────────────────

/// Maximum fee in basis points (1%).
pub const MAX_FEE_BPS: i128 = 100;

/// Default fee: 0 bps (free flash loans).
pub const DEFAULT_FEE_BPS: i128 = 0;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn check_not_paused(e: &Env, operation: u32) -> Result<(), Error> {
    if EmergencyGuard::is_paused(e.clone(), operation) {
        Err(Error::Paused)
    } else {
        Ok(())
    }
}

fn load_admin(e: &Env) -> Result<Address, Error> {
    e.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(Error::NotInitialized)
}

fn load_token(e: &Env) -> Result<Address, Error> {
    e.storage()
        .instance()
        .get(&DataKey::Token)
        .ok_or(Error::NotInitialized)
}

fn get_fee_bps(e: &Env) -> i128 {
    e.storage()
        .instance()
        .get(&DataKey::FeeBps)
        .unwrap_or(DEFAULT_FEE_BPS)
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BorrowRecord {
    pub fee: i128,
    pub total_repayment: i128,
}

/// Helper: calculate fee for a given amount using configured bps.
/// Uses ceiling division when fee_bps > 0 to prevent fee evasion on small amounts.
fn calculate_fee(e: &Env, amount: i128) -> i128 {
    let fee_bps = get_fee_bps(e);
    if fee_bps == 0 || amount <= 0 {
        0
    } else {
        (amount * fee_bps + 9_999) / 10_000
    }
}

fn is_flash_loan_active(e: &Env) -> bool {
    e.storage()
        .instance()
        .get(&DataKey::FlashLoanActive)
        .unwrap_or(false)
}

fn check_no_flash_loan_active(e: &Env) -> Result<(), Error> {
    if is_flash_loan_active(e) {
        Err(Error::Reentrancy)
    } else {
        Ok(())
    }
}

fn set_flash_loan_active(e: &Env, active: bool) {
    e.storage()
        .instance()
        .set(&DataKey::FlashLoanActive, &active);
}

fn get_total_deposited(e: &Env) -> i128 {
    e.storage()
        .instance()
        .get(&DataKey::TotalDeposited)
        .unwrap_or(0)
}

fn set_total_deposited(e: &Env, amount: i128) {
    e.storage()
        .instance()
        .set(&DataKey::TotalDeposited, &amount);
}

fn is_borrow_paused(e: &Env) -> bool {
    e.storage()
        .instance()
        .get(&DataKey::BorrowPaused)
        .unwrap_or(false)
}

fn set_borrow_paused(e: &Env, paused: bool) {
    e.storage().instance().set(&DataKey::BorrowPaused, &paused);
}

// ── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct FlashLoanVault;

#[contractimpl]
impl FlashLoanVault {
    // ── Initialization ───────────────────────────────────────────────────────

    /// Initialize the vault with an admin and the token to lend.
    ///
    /// Can only be called once. Sets the fee to 0 bps by default.
    pub fn initialize(e: Env, admin: Address, token: Address) -> Result<(), Error> {
        if e.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }

        e.storage().instance().set(&DataKey::Admin, &admin);
        e.storage().instance().set(&DataKey::Token, &token);
        e.storage()
            .instance()
            .set(&DataKey::FeeBps, &DEFAULT_FEE_BPS);
        e.storage()
            .instance()
            .set(&DataKey::FlashLoanActive, &false);
        e.storage().instance().set(&DataKey::BorrowPaused, &false);
        e.storage().instance().set(&DataKey::TotalDeposited, &0i128);

        Ok(())
    }

    // ── Admin operations ─────────────────────────────────────────────────────

    /// Admin deposits tokens into the vault, making them available for flash loans.
    pub fn deposit(e: Env, from: Address, amount: i128) -> Result<(), Error> {
        check_no_flash_loan_active(&e)?;

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let admin = load_admin(&e)?;
        if from != admin {
            return Err(Error::Unauthorized);
        }
        from.require_auth();

        let token = load_token(&e)?;
        soroban_sdk::token::Client::new(&e, &token).transfer(
            &from,
            &e.current_contract_address(),
            &amount,
        );

        let deposited = get_total_deposited(&e);
        set_total_deposited(&e, deposited + amount);

        e.events().publish(
            (String::from_str(&e, "vault_deposit"), from.clone()),
            VaultDepositEvent {
                admin: from,
                amount,
            },
        );

        Ok(())
    }

    /// Admin withdraws tokens from the vault.
    pub fn withdraw(e: Env, to: Address, amount: i128) -> Result<(), Error> {
        check_no_flash_loan_active(&e)?;

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let admin = load_admin(&e)?;
        if to != admin {
            return Err(Error::Unauthorized);
        }
        to.require_auth();

        let deposited = get_total_deposited(&e);
        if amount > deposited {
            return Err(Error::InsufficientDeposit);
        }

        let token = load_token(&e)?;
        soroban_sdk::token::Client::new(&e, &token).transfer(
            &e.current_contract_address(),
            &to,
            &amount,
        );

        set_total_deposited(&e, deposited - amount);

        e.events().publish(
            (String::from_str(&e, "vault_withdraw"), to.clone()),
            VaultWithdrawEvent { admin: to, amount },
        );

        Ok(())
    }

    /// Admin-only: set the flash loan fee in basis points (0–100).
    pub fn set_fee(e: Env, fee_bps: i128) -> Result<(), Error> {
        check_no_flash_loan_active(&e)?;

        if !(0..=MAX_FEE_BPS).contains(&fee_bps) {
            return Err(Error::InvalidFee);
        }
        let admin = load_admin(&e)?;
        admin.require_auth();

        let old_fee = get_fee_bps(&e);
        e.storage().instance().set(&DataKey::FeeBps, &fee_bps);

        e.events().publish(
            (String::from_str(&e, "fee_changed"), admin.clone()),
            FeeChangedEvent {
                admin,
                old_fee_bps: old_fee,
                new_fee_bps: fee_bps,
            },
        );

        Ok(())
    }

    /// Admin-only: pause or unpause flash loan operations.
    pub fn set_paused(e: Env, admin: Address, paused: bool) -> Result<(), Error> {
        check_no_flash_loan_active(&e)?;

        EmergencyGuard::set_pause(e, admin, PauseType::BORROW, paused)
            .map_err(|_| Error::Unauthorized)
    }

    /// Admin-only: pause BORROW/flash_loan without affecting other operations.
    pub fn pause_borrow(e: Env) -> Result<(), Error> {
        let admin = load_admin(&e)?;
        admin.require_auth();
        set_borrow_paused(&e, true);
        Ok(())
    }

    /// Admin-only: resume BORROW/flash_loan.
    pub fn resume_borrow(e: Env) -> Result<(), Error> {
        let admin = load_admin(&e)?;
        admin.require_auth();
        set_borrow_paused(&e, false);
        Ok(())
    }

    /// View: is borrow paused?
    pub fn get_borrow_paused(e: Env) -> bool {
        is_borrow_paused(&e)
    }

    /// Admin-only: emergency pause all operations.
    pub fn emergency_pause(e: Env, approvers: Vec<Address>) -> Result<(), Error> {
        check_no_flash_loan_active(&e)?;

        EmergencyGuard::emergency_pause(e, approvers).map_err(|_| Error::Unauthorized)
    }

    // ── Flash loan ───────────────────────────────────────────────────────────

    /// Execute a flash loan.
    ///
    /// Transfers `amount` of the vault's token to `receiver`, then calls
    /// `receiver.execute_operation(token, amount, fee, initiator)`.
    ///
    /// After the callback returns, the vault verifies that its token balance
    /// is at least `pre_balance + fee`, which proves the callback returned the
    /// original `amount` plus the required fee. If not, the entire transaction
    /// reverts.
    ///
    /// # Arguments
    /// * `initiator` – the address initiating the flash loan (must authorize)
    /// * `receiver`  – the contract that will receive funds and be called back
    /// * `amount`    – amount to borrow
    ///
    /// # Returns
    /// The fee charged (may be 0).
    pub fn flash_loan(
        e: Env,
        initiator: Address,
        receiver: Address,
        amount: i128,
    ) -> Result<i128, Error> {
        // Granular pause check: BORROW only.
        if is_borrow_paused(&e) {
            return Err(Error::BorrowPaused);
        }

        // 1. Pause check.
        check_not_paused(&e, PauseType::BORROW)?;

        // 2. Validate amount.
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        // 3. Reentrancy guard.
        check_no_flash_loan_active(&e)?;

        // 4. Auth: the initiator must have signed.
        initiator.require_auth();

        // 4b. Verify receiver implements the flash loan callback interface.
        if !receiver.is_contract() {
            return Err(Error::InvalidReceiver);
        }

        let token_addr = load_token(&e)?;
        let token = soroban_sdk::token::Client::new(&e, &token_addr);

        // 5. Check vault has enough balance.
        let pre_balance = token.balance(&e.current_contract_address());
        if amount > pre_balance {
            return Err(Error::InsufficientVaultBalance);
        }

        // 6. Calculate fee using configured basis points with ceiling division.
        let fee = calculate_fee(&e, amount);

        // 7. Set reentrancy guard.
        set_flash_loan_active(&e, true);

        // 8. Transfer borrowed tokens to the receiver.
        token.transfer(&e.current_contract_address(), &receiver, &amount);

        // 9. Call the receiver's callback so it can use the funds.
        let receiver_client = FlashLoanReceiverClient::new(&e, &receiver);
        if receiver_client
            .try_execute_operation(&token_addr, &amount, &fee, &initiator)
            .is_err()
        {
            set_flash_loan_active(&e, false);
            return Err(Error::InvalidReceiver);
        }

        // 10. Verify repayment: after lending `amount`, the receiver must
        // return `amount + fee`, leaving the vault contract address with its original balance
        // plus the dynamically verified fee.
        let expected_fee = calculate_fee(&e, amount);
        let required_balance = pre_balance + expected_fee;
        let post_balance = token.balance(&e.current_contract_address());
        if post_balance < required_balance {
            // The transfer at step 8 is rolled back — funds are safe.
            set_flash_loan_active(&e, false);
            return Err(Error::LoanNotRepaid);
        }

        // 11. Clear reentrancy guard.
        set_flash_loan_active(&e, false);

        // 12. If fee was collected, update total deposited to reflect new capital.
        if fee > 0 {
            let deposited = get_total_deposited(&e);
            set_total_deposited(&e, deposited + fee);
        }

        // 13. Emit event.
        e.events().publish(
            (String::from_str(&e, "flash_loan"), receiver.clone()),
            FlashLoanEvent {
                receiver,
                token: token_addr,
                amount,
                fee,
            },
        );

        Ok(fee)
    }

    /// Execute a borrow (flash-style): transfer `amount` to `borrower`, call
    /// borrower callback, and verify repayment of `amount + fee` before
    /// returning. Stores a temporary `BorrowRecord` with `fee` and
    /// `total_repayment` while the callback executes.
    pub fn borrow(e: Env, borrower: Address, amount: i128) -> Result<i128, Error> {
        // Granular pause check: BORROW only.
        if is_borrow_paused(&e) {
            return Err(Error::BorrowPaused);
        }

        // 1. Pause check.
        check_not_paused(&e, PauseType::BORROW)?;

        // 2. Validate amount > 0.
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        // 3. Reentrancy guard.
        if is_flash_loan_active(&e) {
            return Err(Error::Reentrancy);
        }

        let token_addr = load_token(&e)?;
        let token = soroban_sdk::token::Client::new(&e, &token_addr);

        // 4. Check vault has enough balance.
        let pre_balance = token.balance(&e.current_contract_address());
        if amount > pre_balance {
            return Err(Error::InsufficientVaultBalance);
        }

        // 5. Calculate fee and total repayment.
        let fee = calculate_fee(&e, amount);
        let total_repayment = amount + fee;

        // 6. Effects: set reentrancy guard and store borrow record.
        set_flash_loan_active(&e, true);
        e.storage().instance().set(
            &DataKey::BorrowRecord(borrower.clone()),
            &BorrowRecord {
                fee,
                total_repayment,
            },
        );

        // 7. Interaction: transfer funds to borrower.
        token.transfer(&e.current_contract_address(), &borrower, &amount);

        // 8. Call borrower's callback so it can use the funds and repay.
        let receiver_client = FlashLoanReceiverClient::new(&e, &borrower);
        if receiver_client
            .try_execute_operation(&token_addr, &amount, &fee, &borrower)
            .is_err()
        {
            set_flash_loan_active(&e, false);
            e.storage().instance().set(
                &DataKey::BorrowRecord(borrower.clone()),
                &BorrowRecord { fee: 0, total_repayment: 0 },
            );
            return Err(Error::InvalidReceiver);
        }

        // 9. Verify repayment: vault balance must be >= pre_balance + expected_fee.
        let expected_fee = calculate_fee(&e, amount);
        let required_balance = pre_balance + expected_fee;
        let post_balance = token.balance(&e.current_contract_address());
        if post_balance < pre_balance + fee {
            set_flash_loan_active(&e, false);
            e.storage().instance().set(
                &DataKey::BorrowRecord(borrower.clone()),
                &BorrowRecord { fee: 0, total_repayment: 0 },
            );
        if post_balance < required_balance {
            e.storage()
                .instance()
                .remove(&DataKey::BorrowRecord(borrower));
            return Err(Error::LoanNotRepaid);
        }

        // 10. Clear reentrancy guard and borrow record.
        set_flash_loan_active(&e, false);
        // Overwrite the temporary record with zeros to avoid stale data.
        e.storage().instance().set(
            &DataKey::BorrowRecord(borrower.clone()),
            &BorrowRecord {
                fee: 0,
                total_repayment: 0,
            },
        );

        // 11. If fee collected, update total deposited.
        if fee > 0 {
            let deposited = get_total_deposited(&e);
            set_total_deposited(&e, deposited + fee);
        }

        // 12. Emit same event as flash_loan for observability.
        e.events().publish(
            (String::from_str(&e, "borrow"), borrower.clone()),
            FlashLoanEvent {
                receiver: borrower,
                token: token_addr,
                amount,
                fee,
            },
        );

        Ok(fee)
    }

    // ── Views ────────────────────────────────────────────────────────────────

    /// Returns the current fee in basis points.
    pub fn get_fee(e: Env) -> i128 {
        get_fee_bps(&e)
    }

    /// Returns the amount of tokens available for flash loans.
    pub fn get_available(e: Env) -> Result<i128, Error> {
        let token_addr = load_token(&e)?;
        let token = soroban_sdk::token::Client::new(&e, &token_addr);
        Ok(token.balance(&e.current_contract_address()))
    }

    /// Returns the token address this vault lends.
    pub fn get_token(e: Env) -> Result<Address, Error> {
        load_token(&e)
    }

    /// Returns the vault admin.
    pub fn get_admin(e: Env) -> Result<Address, Error> {
        load_admin(&e)
    }

    /// Returns the total amount deposited by the admin.
    pub fn get_total_deposited(e: Env) -> i128 {
        get_total_deposited(&e)
    }
}
