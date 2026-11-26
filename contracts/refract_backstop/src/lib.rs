#![no_std]
//! # RefractBackstop — SEP-41 Receipt Token Backstop Pool
//!
//! Accepts deposits of a backing (backstop) token and mints **receipt tokens**
//! 1:1 on deposit.  The receipt implements the standard [`TokenInterface`]
//! (SEP-41) so positions are **transferable and composable**: any contract or
//! integrator can read balances, transfer, approve, and burn receipts through
//! the familiar `soroban_sdk::token::Client` interface.
//!
//! ## Loss socialization
//!
//! When the backstop must absorb a loss, an admin calls [`socialize_loss`],
//! which:
//!
//! 1. Transfers `loss_amount` of the backing token to the `destination`
//!    (e.g. the protocol being rescued).
//! 2. Burns receipt tokens **pro-rata** across every holder so that the
//!    1:1 backing ratio is preserved for the remaining receipts.
//!
//! [`socialize_loss`]: RefractBackstop::socialize_loss
//! [`TokenInterface`]: soroban_sdk::token::TokenInterface

use soroban_sdk::{
    contract, contractimpl, contracttype, token, token::TokenInterface, Address, Env, String,
    Symbol, Vec,
};

use emergency_guard::{
    DefaultEmergencyGuard, EmergencyGuard, EmergencyGuardTrait,
};
pub use soroscope_error_codes::ContractError;

// ── Pause bits ────────────────────────────────────────────────────────────────
// Reuses standard PauseType bits (DEPOSIT, WITHDRAW, TRANSFER, BURN) and adds
// a contract-specific bit for loss socialization.
const PAUSE_SOCIALIZE_LOSS: u32 = 1 << 10;

const TTL_LEDGERS: u32 = 10_000;

// ── Storage types ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub struct AllowanceDataKey {
    pub from: Address,
    pub spender: Address,
}

#[derive(Clone)]
#[contracttype]
pub struct AllowanceValue {
    pub amount: i128,
    pub expiration_ledger: u32,
}

#[derive(Clone)]
#[contracttype]
pub struct TokenMetadata {
    pub name: String,
    pub symbol: String,
    pub decimals: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BackstopConfig {
    pub admin: Address,
    pub backstop_token: Address,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Config,
    Metadata,
    TotalSupply,
    Balance(Address),
    Allowance(AllowanceDataKey),
    Holders,
}

// ── Event structs ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DepositEvent {
    pub staker: Address,
    pub amount: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct WithdrawEvent {
    pub staker: Address,
    pub receipt_amount: i128,
    pub backing_amount: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SocializeLossEvent {
    pub admin: Address,
    pub loss_amount: i128,
    pub destination: Address,
    pub receipts_burned: i128,
}

// ── Balance helpers ───────────────────────────────────────────────────────────

fn read_balance(e: &Env, addr: &Address) -> i128 {
    e.storage()
        .persistent()
        .get(&DataKey::Balance(addr.clone()))
        .unwrap_or(0)
}

fn write_balance(e: &Env, addr: &Address, amount: i128) {
    let key = DataKey::Balance(addr.clone());
    e.storage().persistent().set(&key, &amount);
    e.storage().persistent().extend_ttl(&key, TTL_LEDGERS, TTL_LEDGERS);
}

fn receive_balance(e: &Env, addr: &Address, amount: i128) {
    let balance = read_balance(e, addr);
    write_balance(e, addr, balance + amount);
}

fn spend_balance(e: &Env, addr: &Address, amount: i128) -> Result<(), ContractError> {
    let balance = read_balance(e, addr);
    if balance < amount {
        return Err(ContractError::InsufficientBalance);
    }
    write_balance(e, addr, balance - amount);
    Ok(())
}

// ── Supply helpers ────────────────────────────────────────────────────────────

fn read_total_supply(e: &Env) -> i128 {
    e.storage().instance().get(&DataKey::TotalSupply).unwrap_or(0)
}

fn write_total_supply(e: &Env, amount: i128) {
    e.storage().instance().set(&DataKey::TotalSupply, &amount);
}

// ── Allowance helpers ─────────────────────────────────────────────────────────

fn read_allowance(e: &Env, from: &Address, spender: &Address) -> AllowanceValue {
    let key = DataKey::Allowance(AllowanceDataKey {
        from: from.clone(),
        spender: spender.clone(),
    });
    e.storage()
        .temporary()
        .get(&key)
        .unwrap_or(AllowanceValue {
            amount: 0,
            expiration_ledger: 0,
        })
}

fn write_allowance(
    e: &Env,
    from: &Address,
    spender: &Address,
    amount: i128,
    expiration_ledger: u32,
) {
    let key = DataKey::Allowance(AllowanceDataKey {
        from: from.clone(),
        spender: spender.clone(),
    });
    if amount > 0 {
        e.storage().temporary().set(
            &key,
            &AllowanceValue {
                amount,
                expiration_ledger,
            },
        );
    } else {
        e.storage().temporary().remove(&key);
    }
}

fn spend_allowance(
    e: &Env,
    from: &Address,
    spender: &Address,
    amount: i128,
) -> Result<(), ContractError> {
    let allowance = read_allowance(e, from, spender);
    if allowance.amount < amount {
        return Err(ContractError::InsufficientAllowance);
    }
    if allowance.expiration_ledger < e.ledger().sequence() && amount > 0 {
        return Err(ContractError::InsufficientAllowance);
    }
    write_allowance(
        e,
        from,
        spender,
        allowance.amount - amount,
        allowance.expiration_ledger,
    );
    Ok(())
}

// ── Holder tracking ──────────────────────────────────────────────────────────

fn add_holder(e: &Env, addr: &Address) {
    let key = DataKey::Holders;
    let mut holders: Vec<Address> = e.storage().instance().get(&key).unwrap_or(Vec::new(e));
    // Avoid duplicates — linear scan is fine for backstop-scale holder sets.
    for i in 0..holders.len() {
        if &holders.get(i).unwrap() == addr {
            return;
        }
    }
    holders.push_back(addr.clone());
    e.storage().instance().set(&key, &holders);
}

// ── Metadata helpers ─────────────────────────────────────────────────────────

fn read_metadata(e: &Env) -> TokenMetadata {
    e.storage().instance().get(&DataKey::Metadata).unwrap()
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct RefractBackstop;

#[contractimpl]
impl RefractBackstop {
    // ── Lifecycle ─────────────────────────────────────────────────────────────

    /// Initialise the backstop pool.
    ///
    /// * `admin`          — address authorised to call `socialize_loss` and
    ///   toggle the emergency guard.
    /// * `backstop_token` — SEP-41 token that backs the receipts.
    /// * `name` / `symbol` / `decimals` — metadata for the receipt token.
    pub fn initialize(
        e: Env,
        admin: Address,
        backstop_token: Address,
        name: String,
        symbol: String,
        decimals: u32,
    ) -> Result<(), ContractError> {
        if e.storage().instance().has(&DataKey::Config) {
            return Err(ContractError::AlreadyInitialized);
        }

        let config = BackstopConfig {
            admin: admin.clone(),
            backstop_token,
        };
        e.storage().instance().set(&DataKey::Config, &config);
        e.storage().instance().set(
            &DataKey::Metadata,
            &TokenMetadata {
                name,
                symbol,
                decimals,
            },
        );
        write_total_supply(&e, 0);
        e.storage()
            .instance()
            .set(&DataKey::Holders, &Vec::<Address>::new(&e));

        // Initialise the embedded EmergencyGuard (threshold 1 = single admin).
        EmergencyGuard::initialize(e.clone(), Vec::from_array(&e, [admin]), 1)
            .map_err(|_| ContractError::AlreadyInitialized)?;

        e.storage().instance().extend_ttl(TTL_LEDGERS, TTL_LEDGERS);
        Ok(())
    }

    // ── Backstop operations ────────────────────────────────────────────────────

    /// Deposit `amount` of the backing token and receive receipt tokens 1:1.
    pub fn deposit(e: Env, staker: Address, amount: i128) -> Result<(), ContractError> {
        DefaultEmergencyGuard::check_not_paused(&e, emergency_guard::PauseType::DEPOSIT)
            .map_err(|_| ContractError::Paused)?;

        if amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }
        staker.require_auth();

        let config = Self::get_config(e.clone())?;

        // Pull backing tokens from staker.
        token::Client::new(&e, &config.backstop_token).transfer(
            &staker,
            &e.current_contract_address(),
            &amount,
        );

        // Mint receipts 1:1.
        receive_balance(&e, &staker, amount);
        write_total_supply(&e, read_total_supply(&e) + amount);
        add_holder(&e, &staker);

        e.storage().instance().extend_ttl(TTL_LEDGERS, TTL_LEDGERS);

        e.events().publish(
            (Symbol::new(&e, "deposit"), staker.clone()),
            DepositEvent { staker, amount },
        );

        Ok(())
    }

    /// Burn `receipt_amount` receipts and receive the same amount of backing
    /// tokens back.
    pub fn withdraw(e: Env, staker: Address, receipt_amount: i128) -> Result<(), ContractError> {
        DefaultEmergencyGuard::check_not_paused(&e, emergency_guard::PauseType::WITHDRAW)
            .map_err(|_| ContractError::Paused)?;

        if receipt_amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }
        staker.require_auth();

        let config = Self::get_config(e.clone())?;

        // Burn receipts.
        spend_balance(&e, &staker, receipt_amount)?;
        write_total_supply(&e, read_total_supply(&e) - receipt_amount);

        // Return backing tokens 1:1.
        token::Client::new(&e, &config.backstop_token).transfer(
            &e.current_contract_address(),
            &staker,
            &receipt_amount,
        );

        e.storage().instance().extend_ttl(TTL_LEDGERS, TTL_LEDGERS);

        e.events().publish(
            (Symbol::new(&e, "withdraw"), staker.clone()),
            WithdrawEvent {
                staker,
                receipt_amount,
                backing_amount: receipt_amount,
            },
        );

        Ok(())
    }

    /// Socialise a loss across all receipt holders.
    ///
    /// Transfers `loss_amount` of the backing token to `destination` and burns
    /// receipts pro-rata so the 1:1 ratio is preserved for remaining holders.
    /// Admin-only; rejects if the loss exceeds total supply.
    pub fn socialize_loss(
        e: Env,
        admin: Address,
        loss_amount: i128,
        destination: Address,
    ) -> Result<i128, ContractError> {
        DefaultEmergencyGuard::check_not_paused(&e, PAUSE_SOCIALIZE_LOSS)
            .map_err(|_| ContractError::Paused)?;

        admin.require_auth();

        let config = Self::get_config(e.clone())?;

        let total = read_total_supply(&e);
        if loss_amount <= 0 {
            return Err(ContractError::InvalidAmount);
        }
        if loss_amount > total {
            return Err(ContractError::LossExceedsSupply);
        }

        // Transfer backing tokens to the destination (protocol being rescued).
        token::Client::new(&e, &config.backstop_token).transfer(
            &e.current_contract_address(),
            &destination,
            &loss_amount,
        );

        // Burn receipts pro-rata across all holders.
        let holders: Vec<Address> = e
            .storage()
            .instance()
            .get(&DataKey::Holders)
            .unwrap_or(Vec::new(&e));

        let mut total_burned: i128 = 0;
        for i in 0..holders.len() {
            let holder = holders.get(i).unwrap();
            let balance = read_balance(&e, &holder);
            if balance == 0 {
                continue;
            }
            // Pro-rata burn: floor(balance * loss_amount / total)
            let burn = (balance as u128 * loss_amount as u128 / total as u128) as i128;
            if burn > 0 {
                write_balance(&e, &holder, balance - burn);
                total_burned += burn;
            }
        }

        write_total_supply(&e, total - total_burned);
        e.storage().instance().extend_ttl(TTL_LEDGERS, TTL_LEDGERS);

        e.events().publish(
            (Symbol::new(&e, "socialize_loss"), admin.clone()),
            SocializeLossEvent {
                admin,
                loss_amount,
                destination,
                receipts_burned: total_burned,
            },
        );

        Ok(total_burned)
    }

    // ── Views ──────────────────────────────────────────────────────────────────

    /// Returns the contract configuration.
    pub fn get_config(e: Env) -> Result<BackstopConfig, ContractError> {
        e.storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(ContractError::NotInitialized)
    }

    /// Returns the total receipt token supply.
    pub fn total_supply(e: Env) -> i128 {
        read_total_supply(&e)
    }

    /// Returns the list of all holder addresses (for off-chain inspection).
    pub fn get_holders(e: Env) -> Vec<Address> {
        e.storage()
            .instance()
            .get(&DataKey::Holders)
            .unwrap_or(Vec::new(&e))
    }

    // ── Emergency-guard passthrough ────────────────────────────────────────────

    /// Granular pause/unpause for a specific operation (admin only).
    pub fn set_pause(
        e: Env,
        admin: Address,
        operation: u32,
        paused: bool,
    ) -> Result<(), ContractError> {
        EmergencyGuard::set_pause(e, admin, operation, paused)
            .map_err(|_| ContractError::Paused)
    }

    /// Emergency pause all operations (requires multi-sig).
    pub fn emergency_pause_all(e: Env, approvers: Vec<Address>) -> Result<(), ContractError> {
        EmergencyGuard::emergency_pause(e, approvers).map_err(|_| ContractError::Paused)
    }

    /// Resume all operations (requires multi-sig).
    pub fn resume_all(e: Env, approvers: Vec<Address>) -> Result<(), ContractError> {
        EmergencyGuard::resume(e, approvers).map_err(|_| ContractError::Paused)
    }

    /// Get the current pause bitmask.
    pub fn get_pause_state(e: Env) -> u32 {
        DefaultEmergencyGuard::get_pause_state(&e)
    }

    /// Check whether a specific operation is paused.
    pub fn is_paused(e: Env, operation: u32) -> bool {
        EmergencyGuard::is_paused(e, operation)
    }

    /// Get list of guard admins.
    pub fn get_admins(e: Env) -> Vec<Address> {
        EmergencyGuard::get_admins(e)
    }

    /// Get multi-sig threshold.
    pub fn get_threshold(e: Env) -> u32 {
        EmergencyGuard::get_threshold(e)
    }

    /// Add a guard admin (multi-sig required).
    pub fn add_admin(
        e: Env,
        approvers: Vec<Address>,
        new_admin: Address,
    ) -> Result<(), ContractError> {
        EmergencyGuard::add_admin(e, approvers, new_admin).map_err(|_| ContractError::Paused)
    }

    /// Remove a guard admin (multi-sig required).
    pub fn remove_admin(
        e: Env,
        approvers: Vec<Address>,
        admin: Address,
    ) -> Result<(), ContractError> {
        EmergencyGuard::remove_admin(e, approvers, admin).map_err(|_| ContractError::Paused)
    }
}

// ── SEP-41 TokenInterface implementation ──────────────────────────────────────

#[contractimpl]
impl TokenInterface for RefractBackstop {
    fn allowance(e: Env, from: Address, spender: Address) -> i128 {
        read_allowance(&e, &from, &spender).amount
    }

    fn approve(e: Env, from: Address, spender: Address, amount: i128, expiration_ledger: u32) {
        from.require_auth();
        e.storage().instance().extend_ttl(TTL_LEDGERS, TTL_LEDGERS);

        write_allowance(&e, &from, &spender, amount, expiration_ledger);

        e.events().publish(
            (
                Symbol::new(&e, "approve"),
                from.clone(),
                spender.clone(),
            ),
            (amount, expiration_ledger),
        );
    }

    fn balance(e: Env, id: Address) -> i128 {
        read_balance(&e, &id)
    }

    fn transfer(e: Env, from: Address, to: Address, amount: i128) {
        DefaultEmergencyGuard::check_not_paused(&e, emergency_guard::PauseType::TRANSFER)
            .unwrap_or_else(|_| panic!("operation paused"));
        from.require_auth();
        e.storage().instance().extend_ttl(TTL_LEDGERS, TTL_LEDGERS);

        spend_balance(&e, &from, amount).unwrap_or_else(|_| panic!("insufficient balance"));
        receive_balance(&e, &to, amount);
        add_holder(&e, &to);

        e.events().publish(
            (
                Symbol::new(&e, "transfer"),
                from.clone(),
                to.clone(),
            ),
            amount,
        );
    }

    fn transfer_from(e: Env, spender: Address, from: Address, to: Address, amount: i128) {
        DefaultEmergencyGuard::check_not_paused(&e, emergency_guard::PauseType::TRANSFER)
            .unwrap_or_else(|_| panic!("operation paused"));
        spender.require_auth();
        e.storage().instance().extend_ttl(TTL_LEDGERS, TTL_LEDGERS);

        spend_allowance(&e, &from, &spender, amount)
            .unwrap_or_else(|_| panic!("insufficient allowance"));
        spend_balance(&e, &from, amount).unwrap_or_else(|_| panic!("insufficient balance"));
        receive_balance(&e, &to, amount);
        add_holder(&e, &to);

        e.events().publish(
            (
                Symbol::new(&e, "transfer"),
                from.clone(),
                to.clone(),
            ),
            amount,
        );
    }

    fn burn(e: Env, from: Address, amount: i128) {
        DefaultEmergencyGuard::check_not_paused(&e, emergency_guard::PauseType::BURN)
            .unwrap_or_else(|_| panic!("operation paused"));
        from.require_auth();
        e.storage().instance().extend_ttl(TTL_LEDGERS, TTL_LEDGERS);

        spend_balance(&e, &from, amount).unwrap_or_else(|_| panic!("insufficient balance"));
        write_total_supply(&e, read_total_supply(&e) - amount);

        e.events().publish((Symbol::new(&e, "burn"), from.clone()), amount);
    }

    fn burn_from(e: Env, spender: Address, from: Address, amount: i128) {
        DefaultEmergencyGuard::check_not_paused(&e, emergency_guard::PauseType::BURN)
            .unwrap_or_else(|_| panic!("operation paused"));
        spender.require_auth();
        e.storage().instance().extend_ttl(TTL_LEDGERS, TTL_LEDGERS);

        spend_allowance(&e, &from, &spender, amount)
            .unwrap_or_else(|_| panic!("insufficient allowance"));
        spend_balance(&e, &from, amount).unwrap_or_else(|_| panic!("insufficient balance"));
        write_total_supply(&e, read_total_supply(&e) - amount);

        e.events().publish((Symbol::new(&e, "burn"), from.clone()), amount);
    }

    fn decimals(e: Env) -> u32 {
        e.storage().instance().extend_ttl(TTL_LEDGERS, TTL_LEDGERS);
        read_metadata(&e).decimals
    }

    fn name(e: Env) -> String {
        e.storage().instance().extend_ttl(TTL_LEDGERS, TTL_LEDGERS);
        read_metadata(&e).name
    }

    fn symbol(e: Env) -> String {
        e.storage().instance().extend_ttl(TTL_LEDGERS, TTL_LEDGERS);
        read_metadata(&e).symbol
    }
}

#[cfg(test)]
mod test;
