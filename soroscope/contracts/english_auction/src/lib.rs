#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol, Vec};

#[contracttype]
pub enum DataKey {
    Seller,
    NftContract,
    TokenId,
    PaymentToken,
    StartingPrice,
    ReservePrice,
    EndLedger,
    HighestBidder,
    HighestBid,
    Bids, // Vec of (bidder, amount)
}

#[contracttype]
pub struct Bid {
    pub bidder: Address,
    pub amount: i128,
}

#[contract]
pub struct EnglishAuction;

#[contractimpl]
impl EnglishAuction {
    pub fn initialize(
        env: Env,
        seller: Address,
        nft_contract: Address,
        token_id: i128,
        payment_token: Address,
        starting_price: i128,
        reserve_price: i128,
        duration_ledgers: u32,
    ) {
        // Check if already initialized
        if env.storage().instance().has(&DataKey::Seller) {
            panic!("Already initialized");
        }

        let end_ledger = env.ledger().sequence() + duration_ledgers;

        env.storage().instance().set(&DataKey::Seller, &seller);
        env.storage().instance().set(&DataKey::NftContract, &nft_contract);
        env.storage().instance().set(&DataKey::TokenId, &token_id);
        env.storage().instance().set(&DataKey::PaymentToken, &payment_token);
        env.storage().instance().set(&DataKey::StartingPrice, &starting_price);
        env.storage().instance().set(&DataKey::ReservePrice, &reserve_price);
        env.storage().instance().set(&DataKey::EndLedger, &end_ledger);
        env.storage().instance().set(&DataKey::HighestBid, &0i128);
        env.storage().instance().set(&DataKey::Bids, &Vec::<Bid>::new(&env));
    }

    pub fn bid(env: Env, bidder: Address, amount: i128) {
        bidder.require_auth();

        let end_ledger: u32 = env.storage().instance().get(&DataKey::EndLedger).unwrap();
        if env.ledger().sequence() >= end_ledger {
            panic!("Auction ended");
        }

        let starting_price: i128 = env.storage().instance().get(&DataKey::StartingPrice).unwrap();
        if amount < starting_price {
            panic!("Bid too low");
        }

        let highest_bid: i128 = env.storage().instance().get(&DataKey::HighestBid).unwrap();
        if amount <= highest_bid {
            panic!("Bid not higher than current highest");
        }

        let payment_token: Address = env.storage().instance().get(&DataKey::PaymentToken).unwrap();

        // Transfer the bid amount from bidder to contract
        env.invoke_contract(
            &payment_token,
            &Symbol::new(&env, "transfer"),
            Vec::from_array(&env, [bidder.to_val(), env.current_contract_address().to_val(), amount.into_val(&env)]),
        );

        // Refund previous highest bidder
        if highest_bid > 0 {
            let prev_bidder: Address = env.storage().instance().get(&DataKey::HighestBidder).unwrap();
            env.invoke_contract(
                &payment_token,
                &Symbol::new(&env, "transfer"),
                Vec::from_array(&env, [env.current_contract_address().to_val(), prev_bidder.to_val(), highest_bid.into_val(&env)]),
            );
        }

        // Update highest bid
        env.storage().instance().set(&DataKey::HighestBidder, &bidder);
        env.storage().instance().set(&DataKey::HighestBid, &amount);

        // Record bid
        let mut bids: Vec<Bid> = env.storage().instance().get(&DataKey::Bids).unwrap();
        bids.push_back(Bid { bidder: bidder.clone(), amount });
        env.storage().instance().set(&DataKey::Bids, &bids);
    }

    pub fn end_auction(env: Env) {
        let end_ledger: u32 = env.storage().instance().get(&DataKey::EndLedger).unwrap();
        if env.ledger().sequence() < end_ledger {
            panic!("Auction not ended yet");
        }

        let highest_bid: i128 = env.storage().instance().get(&DataKey::HighestBid).unwrap();
        let reserve_price: i128 = env.storage().instance().get(&DataKey::ReservePrice).unwrap();

        if highest_bid >= reserve_price {
            // Successful auction
            let seller: Address = env.storage().instance().get(&DataKey::Seller).unwrap();
            let highest_bidder: Address = env.storage().instance().get(&DataKey::HighestBidder).unwrap();
            let nft_contract: Address = env.storage().instance().get(&DataKey::NftContract).unwrap();
            let token_id: i128 = env.storage().instance().get(&DataKey::TokenId).unwrap();
            let payment_token: Address = env.storage().instance().get(&DataKey::PaymentToken).unwrap();

            // Transfer NFT to winner
            env.invoke_contract(
                &nft_contract,
                &Symbol::new(&env, "transfer"),
                Vec::from_array(&env, [env.current_contract_address().to_val(), highest_bidder.to_val(), token_id.into_val(&env)]),
            );

            // Transfer payment to seller
            env.invoke_contract(
                &payment_token,
                &Symbol::new(&env, "transfer"),
                Vec::from_array(&env, [env.current_contract_address().to_val(), seller.to_val(), highest_bid.into_val(&env)]),
            );
        } else {
            // Reserve not met, refund highest bidder
            if highest_bid > 0 {
                let highest_bidder: Address = env.storage().instance().get(&DataKey::HighestBidder).unwrap();
                let payment_token: Address = env.storage().instance().get(&DataKey::PaymentToken).unwrap();
                env.invoke_contract(
                    &payment_token,
                    &Symbol::new(&env, "transfer"),
                    Vec::from_array(&env, [env.current_contract_address().to_val(), highest_bidder.to_val(), highest_bid.into_val(&env)]),
                );
            }
            // Return NFT to seller
            let seller: Address = env.storage().instance().get(&DataKey::Seller).unwrap();
            let nft_contract: Address = env.storage().instance().get(&DataKey::NftContract).unwrap();
            let token_id: i128 = env.storage().instance().get(&DataKey::TokenId).unwrap();
            env.invoke_contract(
                &nft_contract,
                &Symbol::new(&env, "transfer"),
                Vec::from_array(&env, [env.current_contract_address().to_val(), seller.to_val(), token_id.into_val(&env)]),
            );
        }
    }

    // Getter functions
    pub fn get_seller(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Seller).unwrap()
    }

    pub fn get_highest_bid(env: Env) -> i128 {
        env.storage().instance().get(&DataKey::HighestBid).unwrap()
    }

    pub fn get_end_ledger(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::EndLedger).unwrap()
    }
}

mod test;