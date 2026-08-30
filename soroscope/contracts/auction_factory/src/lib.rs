#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, xdr::ToXdr, Address, BytesN, Env, Symbol, Vec};

#[contracttype]
pub enum AuctionType {
    English,
    Dutch,
}

#[contracttype]
pub enum DataKey {
    Auction(Address), // Auction address -> type
}

#[contract]
pub struct AuctionFactory;

#[contractimpl]
impl AuctionFactory {
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
    ) -> Address {
        // Generate salt based on seller, nft, token_id, and type
        let salt = env.crypto().sha256(
            &(seller.clone(), nft_contract.clone(), token_id, AuctionType::English).to_xdr(&env)
        );

        let deployed_address = env
            .deployer()
            .with_current_contract(salt)
            .deploy_v2(english_wasm_hash, Vec::<soroban_sdk::Val>::new(&env));

        // Initialize the auction
        let init_args = Vec::from_array(&env, [
            seller.to_val(),
            nft_contract.to_val(),
            token_id.into_val(&env),
            payment_token.to_val(),
            starting_price.into_val(&env),
            reserve_price.into_val(&env),
            duration_ledgers.into_val(&env),
        ]);

        env.invoke_contract(
            &deployed_address,
            &Symbol::new(&env, "initialize"),
            init_args,
        );

        // Store the auction type
        env.storage().persistent().set(&DataKey::Auction(deployed_address.clone()), &AuctionType::English);

        deployed_address
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
    ) -> Address {
        let salt = env.crypto().sha256(
            &(seller.clone(), nft_contract.clone(), token_id, AuctionType::Dutch).to_xdr(&env)
        );

        let deployed_address = env
            .deployer()
            .with_current_contract(salt)
            .deploy_v2(dutch_wasm_hash, Vec::<soroban_sdk::Val>::new(&env));

        // Initialize
        let init_args = Vec::from_array(&env, [
            seller.to_val(),
            nft_contract.to_val(),
            token_id.into_val(&env),
            payment_token.to_val(),
            start_price.into_val(&env),
            end_price.into_val(&env),
            duration_ledgers.into_val(&env),
        ]);

        env.invoke_contract(
            &deployed_address,
            &Symbol::new(&env, "initialize"),
            init_args,
        );

        env.storage().persistent().set(&DataKey::Auction(deployed_address.clone()), &AuctionType::Dutch);

        deployed_address
    }

    pub fn get_auction_type(env: Env, auction_address: Address) -> Option<AuctionType> {
        env.storage().persistent().get(&DataKey::Auction(auction_address))
    }
}

mod test;