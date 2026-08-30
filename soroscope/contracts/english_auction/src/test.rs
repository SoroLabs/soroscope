#![cfg(test)]
use super::*;
use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
use soroban_sdk::{token, vec, Address, Env, IntoVal, Symbol, Vec};
use token_contract::TokenClient;

#[test]
fn test_english_auction() {
    let env = Env::default();
    env.mock_all_auths();

    let seller = Address::generate(&env);
    let bidder1 = Address::generate(&env);
    let bidder2 = Address::generate(&env);

    // Deploy NFT contract
    let nft_contract_id = env.register(token_contract::Token, ());
    let nft_client = TokenClient::new(&env, &nft_contract_id);

    // Initialize NFT
    nft_client.initialize(&seller, &0, &"NFT".into_val(&env), &"NFT".into_val(&env));

    // Mint NFT to seller
    nft_client.mint(&seller, &1);

    // Deploy payment token
    let payment_contract_id = env.register(token_contract::Token, ());
    let payment_client = TokenClient::new(&env, &payment_contract_id);
    payment_client.initialize(&seller, &7, &"USD".into_val(&env), &"USD".into_val(&env));
    payment_client.mint(&bidder1, &1000);
    payment_client.mint(&bidder2, &1000);

    // Deploy auction contract
    let auction_contract_id = env.register(EnglishAuction, ());
    let auction_client = EnglishAuctionClient::new(&env, &auction_contract_id);

    // Approve NFT transfer
    nft_client.approve(&seller, &auction_contract_id, &1, &1000);

    // Initialize auction
    auction_client.initialize(&seller, &nft_contract_id, &1, &payment_contract_id, &100, &200, &10);

    // Bid
    auction_client.bid(&bidder1, &150);

    // Check highest bid
    assert_eq!(auction_client.get_highest_bid(), 150);

    // Another bid
    auction_client.bid(&bidder2, &180);

    assert_eq!(auction_client.get_highest_bid(), 180);

    // End auction
    env.ledger().set(LedgerInfo {
        timestamp: 1000000,
        protocol_version: 1,
        sequence_number: 20,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 3110400,
    });

    auction_client.end_auction();

    // Check balances
    assert_eq!(payment_client.balance(&seller), 180);
    assert_eq!(nft_client.balance(&bidder2), 1);
}
