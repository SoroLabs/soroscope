#![no_std]

use emergency_guard::{EmergencyGuard, GuardError};
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, token, xdr::ToXdr, Address, BytesN, Env,
    IntoVal, Symbol, Vec,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuctionType {
    English,
    Dutch,
}

#[contracttype]
pub enum DataKey {
    Auction(Address), // Auction address -> type
    AuctionByIndex(u32),
    AuctionCount,
    DeploymentFee,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeploymentFee {
    pub token: Address,
    pub recipient: Address,
    pub amount: i128,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum FactoryError {
    InvalidFee = 1,
    FeeNotConfigured = 2,
    Unauthorized = 3,
}

const MAX_PAGE_SIZE: u32 = 100;

fn collect_deployment_fee(env: &Env, seller: &Address) -> Result<(), FactoryError> {
    let fee: DeploymentFee = env
        .storage()
        .persistent()
        .get(&DataKey::DeploymentFee)
        .ok_or(FactoryError::FeeNotConfigured)?;

    seller.require_auth();
    token::Client::new(env, &fee.token).transfer(seller, &fee.recipient, &fee.amount);
    Ok(())
}

fn register_auction(env: &Env, address: &Address, auction_type: &AuctionType) {
    let index: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::AuctionCount)
        .unwrap_or(0);

    env.storage()
        .persistent()
        .set(&DataKey::Auction(address.clone()), auction_type);
    env.storage()
        .persistent()
        .set(&DataKey::AuctionByIndex(index), address);
    env.storage()
        .persistent()
        .set(&DataKey::AuctionCount, &(index + 1));
}

#[contract]
pub struct AuctionFactory;

#[contractimpl]
impl AuctionFactory {
    // ── Guard / Admin management ─────────────────────────────────────────────

    /// Initialize the factory's admin committee via EmergencyGuard.
    /// Must be called once before any admin-gated operations.
    pub fn initialize(env: Env, admins: Vec<Address>, threshold: u32) -> Result<(), GuardError> {
        EmergencyGuard::initialize(env, admins, threshold)
    }

    /// Atomically rotate a factory admin: replace `old_admin` with `new_admin`.
    /// Requires multi-sig approval from at least `threshold` current admins.
    pub fn rotate_admin(
        env: Env,
        approvers: Vec<Address>,
        old_admin: Address,
        new_admin: Address,
    ) -> Result<(), GuardError> {
        EmergencyGuard::rotate_admin(env, approvers, old_admin, new_admin)
    }

    /// Add a new admin (multi-sig required).
    pub fn add_admin(
        env: Env,
        approvers: Vec<Address>,
        new_admin: Address,
    ) -> Result<(), GuardError> {
        EmergencyGuard::add_admin(env, approvers, new_admin)
    }

    /// Remove an admin (multi-sig required).
    pub fn remove_admin(
        env: Env,
        approvers: Vec<Address>,
        admin: Address,
    ) -> Result<(), GuardError> {
        EmergencyGuard::remove_admin(env, approvers, admin)
    }

    /// Returns all current factory admins.
    pub fn get_admins(env: Env) -> Vec<Address> {
        EmergencyGuard::get_admins(env)
    }

    /// Returns the required multi-signature threshold.
    pub fn get_threshold(env: Env) -> u32 {
        EmergencyGuard::get_threshold(env)
    }

    /// Returns whether `addr` is currently a factory admin.
    pub fn is_admin(env: Env, addr: Address) -> bool {
        EmergencyGuard::is_admin_public(env, addr)
    }

    /// Set the fixed SEP-41 token fee charged for every auction deployment.
    pub fn configure_deployment_fee(
        env: Env,
        approvers: Vec<Address>,
        token: Address,
        recipient: Address,
        amount: i128,
    ) -> Result<(), FactoryError> {
        if amount <= 0 {
            return Err(FactoryError::InvalidFee);
        }

        EmergencyGuard::validate_multi_sig(env.clone(), approvers)
            .map_err(|_| FactoryError::Unauthorized)?;

        env.storage().persistent().set(
            &DataKey::DeploymentFee,
            &DeploymentFee {
                token,
                recipient,
                amount,
            },
        );
        Ok(())
    }

    pub fn get_deployment_fee(env: Env) -> Option<DeploymentFee> {
        env.storage().persistent().get(&DataKey::DeploymentFee)
    }

    // ── Auction deployment ───────────────────────────────────────────────────

    pub fn create_english_auction(
        env: Env,
        seller: Address,
        nft_contract: Address,
        token_id: i128,
        payment_token: Address,
        starting_price: i128,
        reserve_price: i128,
        duration_ledgers: u32,
        english_wasm_hash: BytesN<32>,
    ) -> Result<Address, FactoryError> {
        collect_deployment_fee(&env, &seller)?;

        // Generate salt based on seller, nft, token_id, and type
        let salt = env.crypto().sha256(
            &(
                seller.clone(),
                nft_contract.clone(),
                token_id,
                AuctionType::English,
            )
                .to_xdr(&env),
        );

        let deployed_address = env
            .deployer()
            .with_current_contract(salt)
            .deploy_v2(english_wasm_hash, Vec::<soroban_sdk::Val>::new(&env));

        // Initialize the auction
        let init_args = Vec::from_array(
            &env,
            [
                seller.to_val(),
                nft_contract.to_val(),
                token_id.into_val(&env),
                payment_token.to_val(),
                starting_price.into_val(&env),
                reserve_price.into_val(&env),
                duration_ledgers.into_val(&env),
            ],
        );

        env.invoke_contract::<soroban_sdk::Val>(
            &deployed_address,
            &Symbol::new(&env, "initialize"),
            init_args,
        );

        register_auction(&env, &deployed_address, &AuctionType::English);

        Ok(deployed_address)
    }

    pub fn create_dutch_auction(
        env: Env,
        seller: Address,
        nft_contract: Address,
        token_id: i128,
        payment_token: Address,
        start_price: i128,
        end_price: i128,
        duration_ledgers: u32,
        dutch_wasm_hash: BytesN<32>,
    ) -> Result<Address, FactoryError> {
        collect_deployment_fee(&env, &seller)?;

        let salt = env.crypto().sha256(
            &(
                seller.clone(),
                nft_contract.clone(),
                token_id,
                AuctionType::Dutch,
            )
                .to_xdr(&env),
        );

        let deployed_address = env
            .deployer()
            .with_current_contract(salt)
            .deploy_v2(dutch_wasm_hash, Vec::<soroban_sdk::Val>::new(&env));

        // Initialize
        let init_args = Vec::from_array(
            &env,
            [
                seller.to_val(),
                nft_contract.to_val(),
                token_id.into_val(&env),
                payment_token.to_val(),
                start_price.into_val(&env),
                end_price.into_val(&env),
                duration_ledgers.into_val(&env),
            ],
        );

        env.invoke_contract::<soroban_sdk::Val>(
            &deployed_address,
            &Symbol::new(&env, "initialize"),
            init_args,
        );

        register_auction(&env, &deployed_address, &AuctionType::Dutch);

        Ok(deployed_address)
    }

    pub fn get_auction_type(env: Env, auction_address: Address) -> Option<AuctionType> {
        env.storage()
            .persistent()
            .get(&DataKey::Auction(auction_address))
    }

    pub fn get_auction_count(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::AuctionCount)
            .unwrap_or(0)
    }

    pub fn get_auction_by_index(env: Env, index: u32) -> Option<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::AuctionByIndex(index))
    }

    /// Return a bounded page of deployed auction addresses for indexers.
    pub fn get_auctions(env: Env, start: u32, limit: u32) -> Vec<Address> {
        let count = Self::get_auction_count(env.clone());
        let end = start.saturating_add(limit.min(MAX_PAGE_SIZE)).min(count);
        let mut auctions = Vec::new(&env);

        for index in start..end {
            if let Some(address) = Self::get_auction_by_index(env.clone(), index) {
                auctions.push_back(address);
            }
        }

        auctions
    }
}

mod test;
