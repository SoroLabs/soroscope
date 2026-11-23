
#![no_std]

use emergency_guard::{EmergencyGuard, GuardError};

use soroban_sdk::{
    contract,
    contracterror,
    contractimpl,
    contracttype,
    token,
    xdr::ToXdr,
    Address,
    BytesN,
    Env,
    IntoVal,
    Symbol,
    Val,
    Vec,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuctionType {
    English,
    Dutch,
}

#[contracttype]
pub enum DataKey {
    Auction(Address),
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
    InvalidTokenId = 4,
    InvalidPrice = 5,
    InvalidDuration = 6,
    InvalidParameters = 7,
}

const MAX_PAGE_SIZE: u32 = 100;

// -----------------------------------------------------------------------------
// Validation
// -----------------------------------------------------------------------------

fn validate_token_id(token_id: i128) -> Result<(), FactoryError> {
    if token_id < 0 {
        return Err(FactoryError::InvalidTokenId);
    }

    Ok(())
}

fn validate_duration(duration: u32) -> Result<(), FactoryError> {
    if duration == 0 {
        return Err(FactoryError::InvalidDuration);
    }

    Ok(())
}

fn validate_english_auction(
    starting_price: i128,
    reserve_price: i128,
) -> Result<(), FactoryError> {
    if starting_price <= 0 || reserve_price < 0 {
        return Err(FactoryError::InvalidPrice);
    }

    // A configured reserve price should not be below the starting price.
    if reserve_price > 0 && reserve_price < starting_price {
        return Err(FactoryError::InvalidParameters);
    }

    Ok(())
}

fn validate_dutch_auction(
    start_price: i128,
    end_price: i128,
) -> Result<(), FactoryError> {
    if start_price <= 0 || end_price <= 0 {
        return Err(FactoryError::InvalidPrice);
    }

    // Dutch auctions must decrease in price.
    if start_price <= end_price {
        return Err(FactoryError::InvalidParameters);
    }

    Ok(())
}

// -----------------------------------------------------------------------------
// Deployment Fee Management
// -----------------------------------------------------------------------------

fn collect_deployment_fee(
    env: &Env,
    seller: &Address,
) -> Result<(), FactoryError> {
    let fee: DeploymentFee = env
        .storage()
        .persistent()
        .get(&DataKey::DeploymentFee)
        .ok_or(FactoryError::FeeNotConfigured)?;

    if fee.amount <= 0 {
        return Err(FactoryError::InvalidFee);
    }

    seller.require_auth();

    let token_client = token::Client::new(env, &fee.token);

    token_client.transfer(
        seller,
        &fee.recipient,
        &fee.amount,
    );

    Ok(())
}

// -----------------------------------------------------------------------------
// Auction Registry
// -----------------------------------------------------------------------------

fn register_auction(
    env: &Env,
    address: &Address,
    auction_type: &AuctionType,
) {
    let index: u32 = env
        .storage()
        .persistent()
        .get(&DataKey::AuctionCount)
        .unwrap_or(0);

    env.storage()
        .persistent()
        .set(
            &DataKey::Auction(address.clone()),
            auction_type,
        );

    env.storage()
        .persistent()
        .set(
            &DataKey::AuctionByIndex(index),
            address,
        );

    env.storage()
        .persistent()
        .set(
            &DataKey::AuctionCount,
            &(index + 1),
        );
}

// -----------------------------------------------------------------------------
// Shared Auction Deployment
// -----------------------------------------------------------------------------

fn deploy_auction(
    env: &Env,
    seller: &Address,
    nft_contract: &Address,
    token_id: i128,
    auction_type: AuctionType,
    wasm_hash: BytesN<32>,
    init_args: Vec<Val>,
) -> Result<Address, FactoryError> {
    // Collect the deployment fee.
    // Any subsequent contract failure propagates and rolls back the
    // transaction, including the fee transfer and registry updates.
    collect_deployment_fee(env, seller)?;

    // Generate a deterministic deployment salt.
    let salt = env.crypto().sha256(
        &(
            seller.clone(),
            nft_contract.clone(),
            token_id,
            auction_type.clone(),
        )
            .to_xdr(env),
    );

    // Deploy the auction contract.
    let deployed_address = env
        .deployer()
        .with_current_contract(salt)
        .deploy_v2(
            wasm_hash,
            Vec::<Val>::new(env),
        );

    // Initialize the deployed auction.
    env.invoke_contract::<Val>(
        &deployed_address,
        &Symbol::new(env, "initialize"),
        init_args,
    );

    // Register only after successful initialization.
    register_auction(
        env,
        &deployed_address,
        &auction_type,
    );

    Ok(deployed_address)
}

// -----------------------------------------------------------------------------
// Auction Factory
// -----------------------------------------------------------------------------

#[contract]
pub struct AuctionFactory;

#[contractimpl]
impl AuctionFactory {
    // -------------------------------------------------------------------------
    // EmergencyGuard / Administration
    // -------------------------------------------------------------------------

    /// Initialize the factory's multi-signature admin committee.
    pub fn initialize(
        env: Env,
        admins: Vec<Address>,
        threshold: u32,
    ) -> Result<(), GuardError> {
        EmergencyGuard::initialize(env, admins, threshold)
    }

    /// Replace an existing administrator.
    pub fn rotate_admin(
        env: Env,
        approvers: Vec<Address>,
        old_admin: Address,
        new_admin: Address,
    ) -> Result<(), GuardError> {
        EmergencyGuard::rotate_admin(
            env,
            approvers,
            old_admin,
            new_admin,
        )
    }

    /// Add a new administrator.
    pub fn add_admin(
        env: Env,
        approvers: Vec<Address>,
        new_admin: Address,
    ) -> Result<(), GuardError> {
        EmergencyGuard::add_admin(
            env,
            approvers,
            new_admin,
        )
    }

    /// Remove an administrator.
    pub fn remove_admin(
        env: Env,
        approvers: Vec<Address>,
        admin: Address,
    ) -> Result<(), GuardError> {
        EmergencyGuard::remove_admin(
            env,
            approvers,
            admin,
        )
    }

    /// Return all current administrators.
    pub fn get_admins(env: Env) -> Vec<Address> {
        EmergencyGuard::get_admins(env)
    }

    /// Return the required multi-signature threshold.
    pub fn get_threshold(env: Env) -> u32 {
        EmergencyGuard::get_threshold(env)
    }

    /// Check whether an address is an administrator.
    pub fn is_admin(env: Env, addr: Address) -> bool {
        EmergencyGuard::is_admin_public(env, addr)
    }

    // -------------------------------------------------------------------------
    // Deployment Fee Configuration
    // -------------------------------------------------------------------------

    /// Configure the fixed SEP-41 deployment fee.
    ///
    /// Requires multi-signature approval.
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

        EmergencyGuard::validate_multi_sig(
            env.clone(),
            approvers,
        )
        .map_err(|_| FactoryError::Unauthorized)?;

        let fee = DeploymentFee {
            token,
            recipient,
            amount,
        };

        env.storage()
            .persistent()
            .set(&DataKey::DeploymentFee, &fee);

        Ok(())
    }

    /// Retrieve the currently configured deployment fee.
    pub fn get_deployment_fee(
        env: Env,
    ) -> Option<DeploymentFee> {
        env.storage()
            .persistent()
            .get(&DataKey::DeploymentFee)
    }

    // -------------------------------------------------------------------------
    // English Auction Deployment
    // -------------------------------------------------------------------------

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
        validate_token_id(token_id)?;

        validate_duration(duration_ledgers)?;

        validate_english_auction(
            starting_price,
            reserve_price,
        )?;

        let init_args = Vec::from_array(
            &env,
            [
                seller.clone().into_val(&env),
                nft_contract.clone().into_val(&env),
                token_id.into_val(&env),
                payment_token.into_val(&env),
                starting_price.into_val(&env),
                reserve_price.into_val(&env),
                duration_ledgers.into_val(&env),
            ],
        );

        deploy_auction(
            &env,
            &seller,
            &nft_contract,
            token_id,
            AuctionType::English,
            english_wasm_hash,
            init_args,
        )
    }

    // -------------------------------------------------------------------------
    // Dutch Auction Deployment
    // -------------------------------------------------------------------------

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
        validate_token_id(token_id)?;

        validate_duration(duration_ledgers)?;

        validate_dutch_auction(
            start_price,
            end_price,
        )?;

        let init_args = Vec::from_array(
            &env,
            [
                seller.clone().into_val(&env),
                nft_contract.clone().into_val(&env),
                token_id.into_val(&env),
                payment_token.into_val(&env),
                start_price.into_val(&env),
                end_price.into_val(&env),
                duration_ledgers.into_val(&env),
            ],
        );

        deploy_auction(
            &env,
            &seller,
            &nft_contract,
            token_id,
            AuctionType::Dutch,
            dutch_wasm_hash,
            init_args,
        )
    }

    // -------------------------------------------------------------------------
    // Auction Registry Queries
    // -------------------------------------------------------------------------

    /// Retrieve the type of a deployed auction.
    pub fn get_auction_type(
        env: Env,
        auction_address: Address,
    ) -> Option<AuctionType> {
        env.storage()
            .persistent()
            .get(&DataKey::Auction(auction_address))
    }

    /// Return the total number of registered auctions.
    pub fn get_auction_count(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::AuctionCount)
            .unwrap_or(0)
    }

    /// Retrieve an auction by its registry index.
    pub fn get_auction_by_index(
        env: Env,
        index: u32,
    ) -> Option<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::AuctionByIndex(index))
    }

    /// Return a bounded page of deployed auction addresses.
    ///
    /// Maximum page size: 100.
    pub fn get_auctions(
        env: Env,
        start: u32,
        limit: u32,
    ) -> Vec<Address> {
        let count = Self::get_auction_count(env.clone());

        let end = start
            .saturating_add(limit.min(MAX_PAGE_SIZE))
            .min(count);

        let mut auctions = Vec::new(&env);

        for index in start..end {
            if let Some(address) =
                Self::get_auction_by_index(env.clone(), index)
            {
                auctions.push_back(address);
            }
        }

        auctions
    }
}

mod test;
