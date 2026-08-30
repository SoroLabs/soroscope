#![cfg(test)]
use soroban_sdk::{testutils::Address as _, Address, Env};

use crate::{Error, TwapOracle, TwapOracleClient};

#[test]
fn test_initialize() {
    let e = Env::default();
    let contract_id = e.register(TwapOracle, ());
    let client = TwapOracleClient::new(&e, &contract_id);

    let token_a = Address::generate(&e);
    let token_b = Address::generate(&e);

    assert_eq!(client.initialize(&token_a, &token_b, &60), Ok(()));

    // Try to initialize again
    assert_eq!(client.initialize(&token_a, &token_b, &60), Err(Error::AlreadyInitialized));

    let (a, b) = client.get_tokens();
    assert_eq!(a, token_a);
    assert_eq!(b, token_b);
}

#[test]
fn test_update_and_get_twap() {
    let e = Env::default();
    e.ledger().with_mut(|li| li.timestamp = 1000);

    let contract_id = e.register(TwapOracle, ());
    let client = TwapOracleClient::new(&e, &contract_id);

    let token_a = Address::generate(&e);
    let token_b = Address::generate(&e);

    client.initialize(&token_a, &token_b, &10);

    // First update
    assert_eq!(client.update_price(&100), Ok(()));
    assert_eq!(client.get_twap(), 0); // No time elapsed yet

    // Advance time
    e.ledger().with_mut(|li| li.timestamp = 1010);

    // Second update
    assert_eq!(client.update_price(&110), Ok(()));
    // TWAP = (100 * 10) / 10 = 100
    assert_eq!(client.get_twap(), 100);

    // Advance time again
    e.ledger().with_mut(|li| li.timestamp = 1025);

    // Third update
    assert_eq!(client.update_price(&120), Ok(()));
    // Cumulative = 100*10 + 110*15 = 1000 + 1650 = 2650
    // Total time = 10 + 15 = 25
    // TWAP = 2650 / 25 = 106
    assert_eq!(client.get_twap(), 106);
}

#[test]
fn test_update_too_soon() {
    let e = Env::default();
    e.ledger().with_mut(|li| li.timestamp = 1000);

    let contract_id = e.register(TwapOracle, ());
    let client = TwapOracleClient::new(&e, &contract_id);

    let token_a = Address::generate(&e);
    let token_b = Address::generate(&e);

    client.initialize(&token_a, &token_b, &60);

    client.update_price(&100);

    // Try update immediately
    assert_eq!(client.update_price(&110), Err(Error::InsufficientTimeElapsed));

    // Advance time by 50 seconds
    e.ledger().with_mut(|li| li.timestamp = 1050);

    // Still not enough
    assert_eq!(client.update_price(&110), Err(Error::InsufficientTimeElapsed));

    // Advance to 1060
    e.ledger().with_mut(|li| li.timestamp = 1060);

    assert_eq!(client.update_price(&110), Ok(()));
}

#[test]
fn test_invalid_price() {
    let e = Env::default();
    let contract_id = e.register(TwapOracle, ());
    let client = TwapOracleClient::new(&e, &contract_id);

    let token_a = Address::generate(&e);
    let token_b = Address::generate(&e);

    client.initialize(&token_a, &token_b, &60);

    assert_eq!(client.update_price(&0), Err(Error::InvalidPrice));
    assert_eq!(client.update_price(&-1), Err(Error::InvalidPrice));
}

#[test]
fn test_not_initialized() {
    let e = Env::default();
    let contract_id = e.register(TwapOracle, ());
    let client = TwapOracleClient::new(&e, &contract_id);

    assert_eq!(client.update_price(&100), Err(Error::NotInitialized));
    assert_eq!(client.get_twap(), 0);
}

#[test]
fn test_large_price_accumulation_no_overflow() {
    let e = Env::default();
    e.ledger().with_mut(|li| li.timestamp = 0);

    let contract_id = e.register(TwapOracle, ());
    let client = TwapOracleClient::new(&e, &contract_id);

    let token_a = Address::generate(&e);
    let token_b = Address::generate(&e);

    client.initialize(&token_a, &token_b, &1);

    // Simulate a year of operation with high-precision prices (~18 decimals).
    // Price = 1_000_000_000_000_000_000 (1 token with 18 decimals)
    // Time = 31_536_000 seconds (~1 year)
    // Cumulative = price * time ≈ 3.15 * 10^25, well within u128 range.
    let high_price: i128 = 1_000_000_000_000_000_000; // 1e18
    let year_seconds: u64 = 31_536_000;

    // First update at t=0 sets last_price, but no elapsed time yet.
    assert_eq!(client.update_price(&high_price), Ok(()));

    // Advance one year
    e.ledger().with_mut(|li| li.timestamp = year_seconds);

    // Update again: cumulative = 0 + high_price * year_seconds
    assert_eq!(client.update_price(&high_price), Ok(()));

    let twap = client.get_twap();
    // TWAP should equal the price since it was constant all year.
    assert_eq!(twap, high_price);
}

#[test]
fn test_u128_wrapping_does_not_panic() {
    let e = Env::default();
    e.ledger().with_mut(|li| li.timestamp = 0);

    let contract_id = e.register(TwapOracle, ());
    let client = TwapOracleClient::new(&e, &contract_id);

    let token_a = Address::generate(&e);
    let token_b = Address::generate(&e);

    client.initialize(&token_a, &token_b, &1);

    // Use extremely large values that would overflow i128 but are handled safely by u128 wrapping.
    let max_price: i128 = i128::MAX;

    assert_eq!(client.update_price(&max_price), Ok(()));

    // Advance by a large amount
    e.ledger().with_mut(|li| li.timestamp = 1_000_000);

    // This multiplication would overflow i128, but u128 wrapping handles it gracefully.
    let result = client.update_price(&max_price);
    // The update should succeed without panicking.
    assert_eq!(result, Ok(()));

    // get_twap should not panic either.
    let _twap = client.get_twap();
}

