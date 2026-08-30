#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

#[contract]
pub struct PriceSourceOk100;

#[contractimpl]
impl PriceSourceOk100 {
    pub fn latest_price(_env: Env) -> Result<i128, Error> {
        Ok(100)
    }
}

#[contract]
pub struct PriceSourceOk101;

#[contractimpl]
impl PriceSourceOk101 {
    pub fn latest_price(_env: Env) -> Result<i128, Error> {
        Ok(101)
    }
}

#[contract]
pub struct PriceSourceOk99;

#[contractimpl]
impl PriceSourceOk99 {
    pub fn latest_price(_env: Env) -> Result<i128, Error> {
        Ok(99)
    }
}

#[contract]
pub struct PriceSourceOutlier;

#[contractimpl]
impl PriceSourceOutlier {
    pub fn latest_price(_env: Env) -> Result<i128, Error> {
        Ok(150)
    }
}

#[contract]
pub struct PriceSourceUnresponsive;

#[contractimpl]
impl PriceSourceUnresponsive {
    pub fn latest_price(_env: Env) -> Result<i128, Error> {
        Err(Error::InvalidPrice)
    }
}

fn register_sources(env: &Env) -> (Address, Address, Address, Address, Address) {
    let ok_100 = env.register(PriceSourceOk100, ());
    let ok_101 = env.register(PriceSourceOk101, ());
    let ok_99 = env.register(PriceSourceOk99, ());
    let outlier = env.register(PriceSourceOutlier, ());
    let unresponsive = env.register(PriceSourceUnresponsive, ());
    (ok_100, ok_101, ok_99, outlier, unresponsive)
}

#[test]
fn test_aggregate_median_three_sources() {
    let env = Env::default();
    let (ok_100, ok_101, ok_99, _, _) = register_sources(&env);
    let aggregator_id = env.register(OracleAggregator, ());
    let client = OracleAggregatorClient::new(&env, &aggregator_id);
    let sources = Vec::from_array(&env, [ok_100, ok_101, ok_99]);

    assert_eq!(client.aggregate_price(&sources), Ok(100));
}

#[test]
fn test_ignore_outlier_and_use_fallback() {
    let env = Env::default();
    let (ok_100, ok_101, ok_99, outlier, _) = register_sources(&env);
    let aggregator_id = env.register(OracleAggregator, ());
    let client = OracleAggregatorClient::new(&env, &aggregator_id);
    let sources = Vec::from_array(&env, [ok_100, ok_101, ok_99, outlier]);

    assert_eq!(client.aggregate_price(&sources), Ok(100));
}

#[test]
fn test_skip_unresponsive_source() {
    let env = Env::default();
    let (ok_100, ok_101, ok_99, _, unresponsive) = register_sources(&env);
    let aggregator_id = env.register(OracleAggregator, ());
    let client = OracleAggregatorClient::new(&env, &aggregator_id);
    let sources = Vec::from_array(&env, [ok_100, ok_101, ok_99, unresponsive]);

    assert_eq!(client.aggregate_price(&sources), Ok(100));
}

#[test]
fn test_reject_when_not_enough_sources() {
    let env = Env::default();
    let (ok_100, ok_101, _, _, _) = register_sources(&env);
    let aggregator_id = env.register(OracleAggregator, ());
    let client = OracleAggregatorClient::new(&env, &aggregator_id);
    let sources = Vec::from_array(&env, [ok_100, ok_101]);

    assert_eq!(client.aggregate_price(&sources), Err(Error::NotEnoughSources));
}

#[test]
fn test_reject_when_not_enough_valid_prices() {
    let env = Env::default();
    let (ok_100, _, _, outlier, unresponsive) = register_sources(&env);
    let aggregator_id = env.register(OracleAggregator, ());
    let client = OracleAggregatorClient::new(&env, &aggregator_id);
    let sources = Vec::from_array(&env, [ok_100, outlier, unresponsive]);

    assert_eq!(client.aggregate_price(&sources), Err(Error::NotEnoughValidPrices));
}
