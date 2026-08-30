#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Vec};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotEnoughSources = 1,
    NotEnoughValidPrices = 2,
    NotEnoughReliableSources = 3,
    InvalidPrice = 4,
}

pub trait PriceOracle {
    fn latest_price(env: Env) -> Result<i128, Error>;
}

soroban_sdk::contractclient!(name = "PriceOracleClient", trait = PriceOracle);

#[contract]
pub struct OracleAggregator;

#[contractimpl]
impl OracleAggregator {
    pub fn aggregate_price(env: Env, sources: Vec<Address>) -> Result<i128, Error> {
        if sources.len() < 3 {
            return Err(Error::NotEnoughSources);
        }

        let mut prices = Vec::new(&env);
        for idx in 0..sources.len() {
            let source = sources.get(idx).unwrap();
            let client = PriceOracleClient::new(&env, &source);

            if let Ok(price) = client.latest_price() {
                if price > 0 {
                    prices.push_back(price);
                }
            }
        }

        if prices.len() < 3 {
            return Err(Error::NotEnoughValidPrices);
        }

        let mut sorted = Self::sort_prices(prices);
        let median = Self::median(&sorted);
        let filtered = Self::filter_outliers(&env, &sorted, median);

        if filtered.len() < 3 {
            return Err(Error::NotEnoughReliableSources);
        }

        Ok(Self::median(&filtered))
    }

    fn sort_prices(mut prices: Vec<i128>) -> Vec<i128> {
        let n = prices.len();
        for i in 0..n {
            for j in 0..n - i - 1 {
                let current = prices.get(j).unwrap();
                let next = prices.get(j + 1).unwrap();
                if current > next {
                    prices.set(j, next);
                    prices.set(j + 1, current);
                }
            }
        }
        prices
    }

    fn median(prices: &Vec<i128>) -> i128 {
        let len = prices.len();
        let mid = len / 2;
        if len % 2 == 1 {
            prices.get(mid).unwrap()
        } else {
            let low = prices.get(mid - 1).unwrap();
            let high = prices.get(mid).unwrap();
            (low + high) / 2
        }
    }

    fn filter_outliers(env: &Env, prices: &Vec<i128>, median: i128) -> Vec<i128> {
        let mut filtered = Vec::new(env);
        let threshold = median.saturating_mul(5);

        for idx in 0..prices.len() {
            let price = prices.get(idx).unwrap();
            if Self::abs_diff(price, median).saturating_mul(100) <= threshold {
                filtered.push_back(price);
            }
        }

        filtered
    }

    fn abs_diff(left: i128, right: i128) -> i128 {
        if left > right {
            left - right
        } else {
            right - left
        }
    }
}

#[cfg(test)]
mod test;
