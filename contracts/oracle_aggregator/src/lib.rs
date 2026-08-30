#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Vec};

/// Default rolling window for TWAP samples (1 hour).
const DEFAULT_TWAP_WINDOW_SECONDS: u64 = 3_600;
/// Reject prices that deviate by this many standard deviations from the median.
const STDDEV_MULTIPLIER: i128 = 2;
/// Secondary cap: reject prices that deviate more than 5% from the median.
const MAX_DEVIATION_PERCENT: i128 = 5;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotEnoughSources = 1,
    NotEnoughValidPrices = 2,
    NotEnoughReliableSources = 3,
    InvalidPrice = 4,
    /// Returned when fewer than three non-stale oracle records are available
    /// after applying the `max_age_seconds` threshold.
    OracleStaleness = 5,
    /// Returned when `window_seconds` is zero during TWAP initialization.
    InvalidWindow = 6,
}

/// A price record returned by an oracle source, including a Unix timestamp
/// (seconds) indicating when the price was last updated.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleRecord {
    /// The current price (must be > 0).
    pub price: i128,
    /// Unix timestamp (seconds) of the last price update.
    pub timestamp: u64,
}

/// A single aggregated price observation used by the rolling TWAP engine.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TwapSample {
    pub price: i128,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    TwapWindowSeconds,
    TwapSamples,
}

/// Oracle interface that returns only a price (legacy / simple sources).
#[soroban_sdk::contractclient(name = "PriceOracleClient")]
pub trait PriceOracle {
    fn latest_price(env: Env) -> Result<i128, Error>;
}

/// Oracle interface that returns both a price and the timestamp of the last
/// update, enabling staleness validation by the aggregator.
#[soroban_sdk::contractclient(name = "PriceOracleWithTimestampClient")]
pub trait PriceOracleWithTimestamp {
    fn latest_price_with_timestamp(env: Env) -> Result<OracleRecord, Error>;
}

#[contract]
pub struct OracleAggregator;

#[contractimpl]
impl OracleAggregator {
    /// Configures the rolling TWAP window. Optional — a 1-hour window is used
    /// until this is called. Passing `0` is rejected.
    pub fn initialize(env: Env, window_seconds: u64) -> Result<(), Error> {
        if window_seconds == 0 {
            return Err(Error::InvalidWindow);
        }
        env.storage()
            .instance()
            .set(&DataKey::TwapWindowSeconds, &window_seconds);
        env.storage()
            .instance()
            .set(&DataKey::TwapSamples, &Vec::<TwapSample>::new(&env));
        Ok(())
    }

    /// Aggregates prices from multiple oracle sources, rejecting any source
    /// whose price timestamp is older than `max_age_seconds` relative to the
    /// current ledger timestamp.
    ///
    /// Fresh, valid prices are then filtered with a median-centered standard
    /// deviation threshold (and a 5% sanity cap). The remaining median is
    /// recorded as a TWAP sample.
    ///
    /// # Parameters
    /// - `sources`:         Addresses of oracle contracts implementing
    ///                      `PriceOracleWithTimestamp`.
    /// - `max_age_seconds`: Maximum allowed age (in seconds) of a price record
    ///                      before it is considered stale and ignored.
    ///
    /// # Errors
    /// - `NotEnoughSources`        – fewer than 3 source addresses provided.
    /// - `OracleStaleness`         – fewer than 3 sources returned a fresh price.
    /// - `NotEnoughValidPrices`    – fewer than 3 sources returned a valid (> 0) price.
    /// - `NotEnoughReliableSources`– after outlier filtering, fewer than 3 prices remain.
    pub fn aggregate_price(
        env: Env,
        sources: Vec<Address>,
        max_age_seconds: u64,
    ) -> Result<i128, Error> {
        if sources.len() < 3 {
            return Err(Error::NotEnoughSources);
        }

        let now = env.ledger().timestamp();
        let mut fresh_count: u32 = 0;
        let mut prices = Vec::new(&env);

        for idx in 0..sources.len() {
            let source = sources.get(idx).unwrap();
            let client = PriceOracleWithTimestampClient::new(&env, &source);

            let record = match client.try_latest_price_with_timestamp() {
                Ok(Ok(record)) => record,
                _ => continue,
            };
            // Reject stale prices: price is stale if its timestamp is older
            // than `max_age_seconds` before the current ledger time.
            let age = now.saturating_sub(record.timestamp);
            if age > max_age_seconds {
                continue;
            }

            fresh_count += 1;

            if record.price > 0 {
                prices.push_back(record.price);
            }
        }

        // Need at least 3 fresh (non-stale) sources even before validity check.
        if fresh_count < 3 {
            return Err(Error::OracleStaleness);
        }

        if prices.len() < 3 {
            return Err(Error::NotEnoughValidPrices);
        }

        let sorted = Self::sort_prices(prices);
        let median = Self::median(&sorted);
        let filtered = Self::filter_outliers(&env, &sorted, median);

        if filtered.len() < 3 {
            return Err(Error::NotEnoughReliableSources);
        }

        let snapshot = Self::median(&filtered);
        Self::record_sample(&env, snapshot);
        Ok(snapshot)
    }

    /// Rolling time-weighted average of aggregated (outlier-filtered) prices
    /// over the configured window. Returns `0` before the first observation.
    ///
    /// Each sample is weighted by the seconds it remained in effect, including
    /// the open interval from the latest sample until the current ledger time.
    pub fn get_twap(env: Env) -> i128 {
        let now = env.ledger().timestamp();
        let window: u64 = env
            .storage()
            .instance()
            .get(&DataKey::TwapWindowSeconds)
            .unwrap_or(DEFAULT_TWAP_WINDOW_SECONDS);
        let samples: Vec<TwapSample> = env
            .storage()
            .instance()
            .get(&DataKey::TwapSamples)
            .unwrap_or(Vec::new(&env));

        Self::compute_twap(&samples, now, window)
    }

    fn record_sample(env: &Env, price: i128) {
        let now = env.ledger().timestamp();
        let window: u64 = env
            .storage()
            .instance()
            .get(&DataKey::TwapWindowSeconds)
            .unwrap_or(DEFAULT_TWAP_WINDOW_SECONDS);
        let mut samples: Vec<TwapSample> = env
            .storage()
            .instance()
            .get(&DataKey::TwapSamples)
            .unwrap_or(Vec::new(env));

        samples.push_back(TwapSample {
            price,
            timestamp: now,
        });

        let pruned = Self::prune_samples(env, &samples, now, window);
        env.storage()
            .instance()
            .set(&DataKey::TwapSamples, &pruned);
        env.storage()
            .instance()
            .set(&DataKey::TwapWindowSeconds, &window);
    }

    /// Keep samples inside the window, plus the last sample that started
    /// before `window_start` so the price in effect at the window boundary
    /// is still weighted.
    fn prune_samples(env: &Env, samples: &Vec<TwapSample>, now: u64, window: u64) -> Vec<TwapSample> {
        let window_start = now.saturating_sub(window);
        let mut pruned = Vec::new(env);
        let mut last_before: Option<TwapSample> = None;

        for idx in 0..samples.len() {
            let sample = samples.get(idx).unwrap();
            if sample.timestamp < window_start {
                last_before = Some(sample);
            } else {
                if let Some(prev) = last_before.take() {
                    pruned.push_back(prev);
                }
                pruned.push_back(sample);
            }
        }

        if pruned.is_empty() {
            if let Some(prev) = last_before {
                pruned.push_back(prev);
            }
        }

        pruned
    }

    fn compute_twap(samples: &Vec<TwapSample>, now: u64, window: u64) -> i128 {
        if samples.is_empty() {
            return 0;
        }

        let window_start = now.saturating_sub(window);
        let mut weighted: i128 = 0;
        let mut duration: i128 = 0;

        for idx in 0..samples.len() {
            let sample = samples.get(idx).unwrap();
            let start = if sample.timestamp < window_start {
                window_start
            } else {
                sample.timestamp
            };
            let end = if idx + 1 < samples.len() {
                samples.get(idx + 1).unwrap().timestamp
            } else {
                now
            };
            if end > start {
                let dt = (end - start) as i128;
                weighted = weighted.saturating_add(sample.price.saturating_mul(dt));
                duration = duration.saturating_add(dt);
            }
        }

        if duration == 0 {
            samples.get(samples.len() - 1).unwrap().price
        } else {
            weighted / duration
        }
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

    /// Drops prices that exceed 2σ from the median, or that deviate by more
    /// than `MAX_DEVIATION_PERCENT` of the median. Integer stddev of 0 (tight
    /// cluster) keeps every observation.
    fn filter_outliers(env: &Env, prices: &Vec<i128>, median: i128) -> Vec<i128> {
        let std = Self::stddev(prices, median);
        let mut filtered = Vec::new(env);

        for idx in 0..prices.len() {
            let price = prices.get(idx).unwrap();
            let diff = Self::abs_diff(price, median);
            let exceeds_stddev = std > 0 && diff >= std.saturating_mul(STDDEV_MULTIPLIER);
            let exceeds_pct = diff.saturating_mul(100) > median.saturating_mul(MAX_DEVIATION_PERCENT);
            if !exceeds_stddev && !exceeds_pct {
                filtered.push_back(price);
            }
        }

        filtered
    }

    fn stddev(prices: &Vec<i128>, center: i128) -> i128 {
        let n = prices.len() as i128;
        if n == 0 {
            return 0;
        }
        let mut var_sum: i128 = 0;
        for idx in 0..prices.len() {
            let diff = Self::abs_diff(prices.get(idx).unwrap(), center);
            var_sum = var_sum.saturating_add(diff.saturating_mul(diff));
        }
        Self::isqrt(var_sum / n)
    }

    fn isqrt(n: i128) -> i128 {
        if n <= 0 {
            return 0;
        }
        let mut x = n;
        let mut y = (x + 1) / 2;
        while y < x {
            x = y;
            y = (x + n / x) / 2;
        }
        x
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
