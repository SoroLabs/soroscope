#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, IntoVal, Symbol, Vec};

/// Stellar ledger close time is ~5 seconds, so 5 minutes ≈ 60 ledgers.
const ANTI_SNIPE_WINDOW_LEDGERS: u32 = 60;
const ANTI_SNIPE_EXTENSION_LEDGERS: u32 = 60;
/// Anti-snipe window and extension in seconds (5 minutes).
const ANTI_SNIPE_WINDOW_SECONDS: u64 = 300;
const ANTI_SNIPE_EXTENSION_SECONDS: u64 = 300;
const SECONDS_PER_LEDGER: u64 = 5;

#[contracttype]
pub enum DataKey {
    Seller,
    NftContract,
    TokenId,
    PaymentToken,
    StartingPrice,
    ReservePrice,
    EndLedger,
    EndTimestamp,
    HighestBidder,
    HighestBid,
    Bids,
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
        if env.storage().instance().has(&DataKey::Seller) {
            panic!("Already initialized");
        }
        if duration_ledgers == 0 {
            panic!("duration_ledgers must be greater than zero");
        }

        seller.require_auth();

        let end_ledger = env.ledger().sequence() + duration_ledgers;
        let end_timestamp =
            env.ledger().timestamp() + (duration_ledgers as u64).saturating_mul(SECONDS_PER_LEDGER);

        env.storage().instance().set(&DataKey::Seller, &seller);
        env.storage()
            .instance()
            .set(&DataKey::NftContract, &nft_contract);
        env.storage().instance().set(&DataKey::TokenId, &token_id);
        env.storage()
            .instance()
            .set(&DataKey::PaymentToken, &payment_token);
        env.storage()
            .instance()
            .set(&DataKey::StartingPrice, &starting_price);
        env.storage()
            .instance()
            .set(&DataKey::ReservePrice, &reserve_price);
        env.storage().instance().set(&DataKey::EndLedger, &end_ledger);
        env.storage()
            .instance()
            .set(&DataKey::EndTimestamp, &end_timestamp);
        env.storage().instance().set(&DataKey::HighestBid, &0i128);
        env.storage()
            .instance()
            .set(&DataKey::Bids, &Vec::<Bid>::new(&env));

        // Escrow the NFT in the auction contract for the duration of the sale.
        env.invoke_contract::<()>(
            &nft_contract,
            &Symbol::new(&env, "transfer"),
            Vec::from_array(
                &env,
                [
                    seller.to_val(),
                    env.current_contract_address().to_val(),
                    token_id.into_val(&env),
                ],
            ),
        );
    }

    pub fn bid(env: Env, bidder: Address, amount: i128) {
        bidder.require_auth();

        let end_ledger: u32 = env.storage().instance().get(&DataKey::EndLedger).unwrap();
        if env.ledger().sequence() >= end_ledger {
            panic!("Auction ended");
        }

        let starting_price: i128 = env
            .storage()
            .instance()
            .get(&DataKey::StartingPrice)
            .unwrap();
        if amount < starting_price {
            panic!("Bid too low");
        }

        let highest_bid: i128 = env.storage().instance().get(&DataKey::HighestBid).unwrap();
        if amount <= highest_bid {
            panic!("Bid not higher than current highest");
        }

        let payment_token: Address = env
            .storage()
            .instance()
            .get(&DataKey::PaymentToken)
            .unwrap();

        // Pull the new bid into escrow.
        env.invoke_contract::<()>(
            &payment_token,
            &Symbol::new(&env, "transfer"),
            Vec::from_array(
                &env,
                [
                    bidder.to_val(),
                    env.current_contract_address().to_val(),
                    amount.into_val(&env),
                ],
            ),
        );

        // Immediately refund the previous highest bidder.
        if highest_bid > 0 {
            let prev_bidder: Address = env
                .storage()
                .instance()
                .get(&DataKey::HighestBidder)
                .unwrap();
            env.invoke_contract::<()>(
                &payment_token,
                &Symbol::new(&env, "transfer"),
                Vec::from_array(
                    &env,
                    [
                        env.current_contract_address().to_val(),
                        prev_bidder.to_val(),
                        highest_bid.into_val(&env),
                    ],
                ),
            );
        }

        env.storage().instance().set(&DataKey::HighestBidder, &bidder);
        env.storage().instance().set(&DataKey::HighestBid, &amount);

        let mut bids: Vec<Bid> = env.storage().instance().get(&DataKey::Bids).unwrap();
        bids.push_back(Bid {
            bidder: bidder.clone(),
            amount,
        });
        env.storage().instance().set(&DataKey::Bids, &bids);

        Self::maybe_extend_deadline(&env, end_ledger);
    }

    /// If this bid arrives in the final 5 minutes (or the final 60 ledgers),
    /// push the deadline out by another 5 minutes / 60 ledgers.
    fn maybe_extend_deadline(env: &Env, end_ledger: u32) {
        let remaining_ledgers = end_ledger.saturating_sub(env.ledger().sequence());
        let end_timestamp: u64 = env
            .storage()
            .instance()
            .get(&DataKey::EndTimestamp)
            .unwrap_or(0);
        let remaining_seconds = end_timestamp.saturating_sub(env.ledger().timestamp());

        if remaining_ledgers <= ANTI_SNIPE_WINDOW_LEDGERS
            || remaining_seconds <= ANTI_SNIPE_WINDOW_SECONDS
        {
            env.storage().instance().set(
                &DataKey::EndLedger,
                &end_ledger.saturating_add(ANTI_SNIPE_EXTENSION_LEDGERS),
            );
            env.storage().instance().set(
                &DataKey::EndTimestamp,
                &end_timestamp.saturating_add(ANTI_SNIPE_EXTENSION_SECONDS),
            );
        }
    }

    pub fn end_auction(env: Env) {
        let end_ledger: u32 = env.storage().instance().get(&DataKey::EndLedger).unwrap();
        if env.ledger().sequence() < end_ledger {
            panic!("Auction not ended yet");
        }

        let highest_bid: i128 = env.storage().instance().get(&DataKey::HighestBid).unwrap();
        let reserve_price: i128 = env
            .storage()
            .instance()
            .get(&DataKey::ReservePrice)
            .unwrap();

        if highest_bid >= reserve_price {
            let seller: Address = env.storage().instance().get(&DataKey::Seller).unwrap();
            let highest_bidder: Address = env
                .storage()
                .instance()
                .get(&DataKey::HighestBidder)
                .unwrap();
            let nft_contract: Address = env
                .storage()
                .instance()
                .get(&DataKey::NftContract)
                .unwrap();
            let token_id: i128 = env.storage().instance().get(&DataKey::TokenId).unwrap();
            let payment_token: Address = env
                .storage()
                .instance()
                .get(&DataKey::PaymentToken)
                .unwrap();

            env.invoke_contract::<()>(
                &nft_contract,
                &Symbol::new(&env, "transfer"),
                Vec::from_array(
                    &env,
                    [
                        env.current_contract_address().to_val(),
                        highest_bidder.to_val(),
                        token_id.into_val(&env),
                    ],
                ),
            );

            env.invoke_contract::<()>(
                &payment_token,
                &Symbol::new(&env, "transfer"),
                Vec::from_array(
                    &env,
                    [
                        env.current_contract_address().to_val(),
                        seller.to_val(),
                        highest_bid.into_val(&env),
                    ],
                ),
            );
        } else {
            if highest_bid > 0 {
                let highest_bidder: Address = env
                    .storage()
                    .instance()
                    .get(&DataKey::HighestBidder)
                    .unwrap();
                let payment_token: Address = env
                    .storage()
                    .instance()
                    .get(&DataKey::PaymentToken)
                    .unwrap();
                env.invoke_contract::<()>(
                    &payment_token,
                    &Symbol::new(&env, "transfer"),
                    Vec::from_array(
                        &env,
                        [
                            env.current_contract_address().to_val(),
                            highest_bidder.to_val(),
                            highest_bid.into_val(&env),
                        ],
                    ),
                );
            }
            let seller: Address = env.storage().instance().get(&DataKey::Seller).unwrap();
            let nft_contract: Address = env
                .storage()
                .instance()
                .get(&DataKey::NftContract)
                .unwrap();
            let token_id: i128 = env.storage().instance().get(&DataKey::TokenId).unwrap();
            env.invoke_contract::<()>(
                &nft_contract,
                &Symbol::new(&env, "transfer"),
                Vec::from_array(
                    &env,
                    [
                        env.current_contract_address().to_val(),
                        seller.to_val(),
                        token_id.into_val(&env),
                    ],
                ),
            );
        }
    }

    pub fn get_seller(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Seller).unwrap()
    }

    pub fn get_highest_bid(env: Env) -> i128 {
        env.storage().instance().get(&DataKey::HighestBid).unwrap()
    }

    pub fn get_highest_bidder(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::HighestBidder)
    }

    pub fn get_end_ledger(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::EndLedger).unwrap()
    }

    pub fn get_end_timestamp(env: Env) -> u64 {
        env.storage().instance().get(&DataKey::EndTimestamp).unwrap()
    }
}

#[cfg(test)]
mod test;
