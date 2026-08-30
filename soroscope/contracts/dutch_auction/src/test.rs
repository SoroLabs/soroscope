#![cfg(test)]
use super::*;
use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
use soroban_sdk::{token, vec, Address, Env, IntoVal, Symbol, Vec};
use token_contract::TokenClient;

#[test]
fn test_dutch_auction() {
    let env = Env::default();
    env.mock_all_auths();

    let seller = Address::generate(&env);
    let buyer = Address::generate(&env);

    // Deploy NFT contract
    let nft_contract_id = env.register(token_contract::Token, ());
    let nft_client = TokenClient::new(&env, &nft_contract_id);
    nft_client.initialize(&seller, &0, &"NFT".into_val(&env), &"NFT".into_val(&env));
    nft_client.mint(&seller, &1);

    // Deploy payment token
    let payment_contract_id = env.register(token_contract::Token, ());
    let payment_client = TokenClient::new(&env, &payment_contract_id);
    payment_client.initialize(&seller, &7, &"USD".into_val(&env), &"USD".into_val(&env));
    payment_client.mint(&buyer, &1000);

    // Deploy auction contract
    let auction_contract_id = env.register(DutchAuction, ());
    let auction_client = DutchAuctionClient::new(&env, &auction_contract_id);

    // Transfer NFT to auction
    nft_client.approve(&seller, &auction_contract_id, &1, &1000);
    nft_client.transfer(&seller, &auction_contract_id, &1);

    // Initialize auction: start 200, end 100, duration 10 ledgers
    auction_client.initialize(&seller, &nft_contract_id, &1, &payment_contract_id, &200, &100, &10);

    // At start, price should be 200
    assert_eq!(auction_client.get_current_price(), 200);

    // Advance 5 ledgers, price should be 150
    env.ledger().set(LedgerInfo {
        timestamp: 500000,
        protocol_version: 1,
        sequence_number: 5,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 10,
        min_persistent_entry_ttl: 10,
        max_entry_ttl: 3110400,
    });
    assert_eq!(auction_client.get_current_price(), 150);

    // Buy
    auction_client.buy(&buyer);

    // Check NFT transferred
    assert_eq!(nft_client.balance(&buyer), 1);
    // Check payment
    assert_eq!(payment_client.balance(&seller), 150);
    assert_eq!(auction_client.is_sold(), true);
}
