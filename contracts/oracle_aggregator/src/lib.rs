
#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Vec};

/// Default rolling window for TWAP samples (1 hour).
const DEFAULT_TWAP_WINDOW_SECONDS: u64 = 3_600;

/// Reject prices that deviate by this many standard deviations from the median.
const STDDEV_MULTIPLIER: i128 = 2;

/// Secondary cap: reject prices that deviate more than 5% from the median.
const MAX_DEVIATION_PERCENT: i128 = 5;

/// Maximum number of price observations retained for any asset.
const MAX_TWAP_OBSERVATIONS: u32 = 1_440;

/// Hard cap on the active oracle provider list.
pub const MAX_ORACLE_PROVIDERS: u32 = 32;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotEnoughSources = 1,
    NotEnoughValidPrices = 2,
    NotEnoughReliableSources = 3,
    InvalidPrice = 4,
    OracleStaleness = 5,
    InvalidWindow = 6,
    InvalidOraclePrice = 7,
    DuplicateProvider = 8,
    TooManyProviders = 9,
    ProviderNotFound = 10,
    Unauthorized = 11,
    AlreadyInitialized = 12,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleRecord {
    pub price: i128,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TwapSample {
    pub price: i128,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LedgerObservation {
    pub price: i128,
    pub ledger: u32,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    TwapWindowSeconds,
    TwapSamples,
    AssetTwapNextIndex(Address),
    AssetTwapLen(Address),
    AssetTwapObservation(Address, u32),
    OracleProviders,
}

/// Legacy/simple oracle interface.
#[soroban_sdk::contractclient(name = "PriceOracleClient")]
pub trait PriceOracle {
    fn latest_price(env: Env) -> Result<i128, Error>;
}

/// Oracle interface that returns a price and its update timestamp.
#[soroban_sdk::contractclient(name = "PriceOracleWithTimestampClient")]
pub trait PriceOracleWithTimestamp {
    fn latest_price_with_timestamp(env: Env) -> Result<OracleRecord, Error>;
}

#[contract]
pub struct OracleAggregator;

#[contractimpl]
impl OracleAggregator {
    // ---------------------------------------------------------
    // INITIALIZATION AND ADMINISTRATION
    // ---------------------------------------------------------

    /// Initializes the aggregator and establishes its administrator.
    ///
    /// Initialization can only happen once.
    /// The admin must authorize this operation.
    pub fn initialize(
        env: Env,
        admin: Address,
        window_seconds: u64,
    ) -> Result<(), Error> {
        if window_seconds == 0 {
            return Err(Error::InvalidWindow);
        }

        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }

        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::TwapWindowSeconds, &window_seconds);

        env.storage()
            .instance()
            .set(&DataKey::TwapSamples, &Vec::<TwapSample>::new(&env));

        env.storage()
            .instance()
            .set(&DataKey::OracleProviders, &Vec::<Address>::new(&env));

        Ok(())
    }

    /// Returns the current administrator.
    pub fn get_admin(env: Env) -> Result<Address, Error> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)
    }

    /// Transfers administrative control to another address.
    ///
    /// The current administrator must authorize the operation.
    pub fn transfer_admin(
        env: Env,
        new_admin: Address,
    ) -> Result<(), Error> {
        Self::require_admin(&env)?;

        new_admin.require_auth();

        env.storage()
            .instance()
            .set(&DataKey::Admin, &new_admin);

        Ok(())
    }

    // ---------------------------------------------------------
    // ORACLE PROVIDER MANAGEMENT
    // ---------------------------------------------------------

    /// Registers an oracle provider.
    ///
    /// Only the administrator can register providers.
    pub fn register_provider(
        env: Env,
        provider: Address,
    ) -> Result<(), Error> {
        Self::require_admin(&env)?;

        let mut providers = Self::stored_providers(&env);

        if Self::provider_index(&providers, &provider).is_some() {
            return Err(Error::DuplicateProvider);
        }

        if providers.len() >= MAX_ORACLE_PROVIDERS {
            return Err(Error::TooManyProviders);
        }

        providers.push_back(provider);

        env.storage()
            .instance()
            .set(&DataKey::OracleProviders, &providers);

        Ok(())
    }

    /// Removes an oracle provider.
    ///
    /// Only the administrator can remove providers.
    pub fn remove_provider(
        env: Env,
        provider: Address,
    ) -> Result<(), Error> {
        Self::require_admin(&env)?;

        let providers = Self::stored_providers(&env);

        let Some(index) = Self::provider_index(&providers, &provider) else {
            return Err(Error::ProviderNotFound);
        };

        let mut updated = Vec::new(&env);

        for idx in 0..providers.len() {
            if idx != index {
                if let Some(address) = providers.get(idx) {
                    updated.push_back(address);
                }
            }
        }

        env.storage()
            .instance()
            .set(&DataKey::OracleProviders, &updated);

        Ok(())
    }

    /// Returns the registered oracle providers.
    pub fn get_providers(env: Env) -> Vec<Address> {
        Self::stored_providers(&env)
    }

    // ---------------------------------------------------------
    // PRICE AGGREGATION
    // ---------------------------------------------------------

    /// Aggregates prices from the registered provider list.
    pub fn get_price(
        env: Env,
        max_age_seconds: u64,
    ) -> Result<i128, Error> {
        let providers = Self::stored_providers(&env);

        Self::aggregate_price(env, providers, max_age_seconds)
    }

    /// Aggregates prices from multiple oracle sources.
    ///
    /// Stale, future-dated, failed, and invalid price responses are ignored.
    /// At least three fresh, valid sources must remain before filtering.
    pub fn aggregate_price(
        env: Env,
        sources: Vec<Address>,
        max_age_seconds: u64,
    ) -> Result<i128, Error> {
        if sources.len() > MAX_ORACLE_PROVIDERS {
            return Err(Error::TooManyProviders);
        }

        if sources.len() < 3 {
            return Err(Error::NotEnoughSources);
        }

        let now = env.ledger().timestamp();

        let mut fresh_count: u32 = 0;
        let mut prices = Vec::new(&env);

        for idx in 0..sources.len() {
            let Some(source) = sources.get(idx) else {
                continue;
            };

            let client =
                PriceOracleWithTimestampClient::new(&env, &source);

            let record = match client.try_latest_price_with_timestamp() {
                Ok(Ok(record)) => record,
                _ => continue,
            };

            // Reject future-dated records.
            if record.timestamp > now {
                continue;
            }

            let age = now - record.timestamp;

            // Ignore stale records rather than failing the entire request.
            if age > max_age_seconds {
                continue;
            }

            fresh_count += 1;

            if record.price > 0 {
                prices.push_back(record.price);
            }
        }

        if fresh_count < 3 {
            return Err(Error::OracleStaleness);
        }

        if prices.len() < 3 {
            return Err(Error::NotEnoughValidPrices);
        }

        let sorted = Self::sort_prices(prices);
        let median = Self::median(&sorted);

        let filtered =
            Self::filter_outliers(&env, &sorted, median);

        if filtered.len() < 3 {
            return Err(Error::NotEnoughReliableSources);
        }

        let snapshot = Self::median(&filtered);

        Self::record_sample(&env, snapshot);

        Ok(snapshot)
    }

    // ---------------------------------------------------------
    // ROLLING TIME-WEIGHTED AVERAGE PRICE
    // ---------------------------------------------------------

    /// Returns the rolling time-weighted average price.
    ///
    /// Returns zero when no observations exist.
    pub fn get_rolling_twap(env: Env) -> i128 {
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

    /// Updates the rolling TWAP window.
    ///
    /// Only the administrator can change the window.
    pub fn update_twap_window(
        env: Env,
        window_seconds: u64,
    ) -> Result<(), Error> {
        Self::require_admin(&env)?;

        if window_seconds == 0 {
            return Err(Error::InvalidWindow);
        }

        env.storage()
            .instance()
            .set(&DataKey::TwapWindowSeconds, &window_seconds);

        let now = env.ledger().timestamp();

        let samples: Vec<TwapSample> = env
            .storage()
            .instance()
            .get(&DataKey::TwapSamples)
            .unwrap_or(Vec::new(&env));

        let pruned =
            Self::prune_samples(&env, &samples, now, window_seconds);

        env.storage()
            .instance()
            .set(&DataKey::TwapSamples, &pruned);

        Ok(())
    }

    // ---------------------------------------------------------
    // ASSET PRICE OBSERVATIONS
    // ---------------------------------------------------------

    /// Records an asset price for ledger-based TWAP queries.
    ///
    /// Only the administrator can record asset prices.
    /// A second update in the same ledger replaces the previous observation.
    pub fn record_asset_price(
        env: Env,
        asset: Address,
        price: i128,
    ) -> Result<(), Error> {
        Self::require_admin(&env)?;

        if price <= 0 {
            return Err(Error::InvalidPrice);
        }

        let ledger = env.ledger().sequence();

        let len: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::AssetTwapLen(asset.clone()))
            .unwrap_or(0);

        let next: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::AssetTwapNextIndex(asset.clone()))
            .unwrap_or(0);

        if len > 0 {
            let last_idx =
                (next + MAX_TWAP_OBSERVATIONS - 1)
                    % MAX_TWAP_OBSERVATIONS;

            let key =
                DataKey::AssetTwapObservation(asset.clone(), last_idx);

            if let Some(last) =
                env.storage().persistent().get::<_, LedgerObservation>(&key)
            {
                if last.ledger == ledger {
                    env.storage().persistent().set(
                        &key,
                        &LedgerObservation { price, ledger },
                    );

                    return Ok(());
                }
            }
        }

        env.storage().persistent().set(
            &DataKey::AssetTwapObservation(asset.clone(), next),
            &LedgerObservation { price, ledger },
        );

        let next_index =
            (next + 1) % MAX_TWAP_OBSERVATIONS;

        env.storage().persistent().set(
            &DataKey::AssetTwapNextIndex(asset.clone()),
            &next_index,
        );

        let updated_len =
            core::cmp::min(len.saturating_add(1), MAX_TWAP_OBSERVATIONS);

        env.storage().persistent().set(
            &DataKey::AssetTwapLen(asset),
            &updated_len,
        );

        Ok(())
    }

    /// Returns the time-weighted average asset price over the last N ledgers.
    ///
    /// The observation in effect at the window boundary is included.
    pub fn get_twap(
        env: Env,
        asset: Address,
        window_ledgers: u32,
    ) -> i128 {
        if window_ledgers == 0 {
            return 0;
        }

        let len: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::AssetTwapLen(asset.clone()))
            .unwrap_or(0);

        if len == 0 {
            return 0;
        }

        let next: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::AssetTwapNextIndex(asset.clone()))
            .unwrap_or(0);

        let oldest =
            (next + MAX_TWAP_OBSERVATIONS - len)
                % MAX_TWAP_OBSERVATIONS;

        let mut samples = Vec::new(&env);

        for offset in 0..len {
            let idx =
                (oldest + offset) % MAX_TWAP_OBSERVATIONS;

            let key =
                DataKey::AssetTwapObservation(asset.clone(), idx);

            if let Some(observation) =
                env.storage().persistent().get::<_, LedgerObservation>(&key)
            {
                samples.push_back(TwapSample {
                    price: observation.price,
                    timestamp: observation.ledger as u64,
                });
            }
        }

        if samples.is_empty() {
            return 0;
        }

        Self::compute_twap(
            &samples,
            env.ledger().sequence() as u64,
            window_ledgers as u64,
        )
    }

    // ---------------------------------------------------------
    // INTERNAL STORAGE HELPERS
    // ---------------------------------------------------------

    fn require_admin(env: &Env) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::Unauthorized)?;

        admin.require_auth();

        Ok(())
    }

    fn stored_providers(env: &Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&DataKey::OracleProviders)
            .unwrap_or(Vec::new(env))
    }

    fn provider_index(
        providers: &Vec<Address>,
        provider: &Address,
    ) -> Option<u32> {
        (0..providers.len()).find(|&idx| {
            providers
                .get(idx)
                .map(|address| address == *provider)
                .unwrap_or(false)
        })
    }

    // ---------------------------------------------------------
    // ROLLING TWAP INTERNAL HELPERS
    // ---------------------------------------------------------

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

        let pruned =
            Self::prune_samples(env, &samples, now, window);

        env.storage()
            .instance()
            .set(&DataKey::TwapSamples, &pruned);
    }

    /// Retains samples inside the window and the last sample before it.
    fn prune_samples(
        env: &Env,
        samples: &Vec<TwapSample>,
        now: u64,
        window: u64,
    ) -> Vec<TwapSample> {
        let window_start = now.saturating_sub(window);

        let mut pruned = Vec::new(env);
        let mut last_before: Option<TwapSample> = None;

        for idx in 0..samples.len() {
            let Some(sample) = samples.get(idx) else {
                continue;
            };

            if sample.timestamp < window_start {
                last_before = Some(sample);
            } else {
                if let Some(previous) = last_before.take() {
                    pruned.push_back(previous);
                }

                pruned.push_back(sample);
            }
        }

        if pruned.is_empty() {
            if let Some(previous) = last_before {
                pruned.push_back(previous);
            }
        }

        pruned
    }

    /// Computes a time-weighted average from chronologically ordered samples.
    fn compute_twap(
        samples: &Vec<TwapSample>,
        now: u64,
        window: u64,
    ) -> i128 {
        if samples.is_empty() {
            return 0;
        }

        if window == 0 {
            return samples
                .get(samples.len() - 1)
                .map(|sample| sample.price)
                .unwrap_or(0);
        }

        let window_start = now.saturating_sub(window);

        let mut weighted: i128 = 0;
        let mut duration: i128 = 0;

        for idx in 0..samples.len() {
            let Some(sample) = samples.get(idx) else {
                continue;
            };

            let start = core::cmp::max(
                sample.timestamp,
                window_start,
            );

            let end = if idx + 1 < samples.len() {
                samples
                    .get(idx + 1)
                    .map(|next| next.timestamp)
                    .unwrap_or(now)
            } else {
                now
            };

            if end > start {
                let dt = (end - start) as i128;

                weighted = weighted.saturating_add(
                    sample.price.saturating_mul(dt),
                );

                duration = duration.saturating_add(dt);
            }
        }

        if duration == 0 {
            samples
                .get(samples.len() - 1)
                .map(|sample| sample.price)
                .unwrap_or(0)
        } else {
            weighted / duration
        }
    }

    // ---------------------------------------------------------
    // PRICE SORTING AND OUTLIER FILTERING
    // ---------------------------------------------------------

    /// Bubble sort. Safe for the bounded provider count.
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

    /// Returns the median of a sorted price vector.
    fn median(prices: &Vec<i128>) -> i128 {
        let len = prices.len();
        let mid = len / 2;

        if len % 2 == 1 {
            prices.get(mid).unwrap()
        } else {
            let low = prices.get(mid - 1).unwrap();
            let high = prices.get(mid).unwrap();

            // Avoid overflowing when adding two large positive prices.
            low + (high - low) / 2
        }
    }

    /// Filters prices outside the standard-deviation and percentage limits.
    fn filter_outliers(
        env: &Env,
        prices: &Vec<i128>,
        median: i128,
    ) -> Vec<i128> {
        let std = Self::stddev(prices, median);

        let mut filtered = Vec::new(env);

        for idx in 0..prices.len() {
            let price = prices.get(idx).unwrap();

            let diff = Self::abs_diff(price, median);

            let exceeds_stddev =
                std > 0
                    && diff >= std.saturating_mul(STDDEV_MULTIPLIER);

            let exceeds_pct =
                diff.saturating_mul(100)
                    > median.saturating_mul(MAX_DEVIATION_PERCENT);

            if !exceeds_stddev && !exceeds_pct {
                filtered.push_back(price);
            }
        }

        filtered
    }

    /// Calculates integer standard deviation around the median.
    fn stddev(
        prices: &Vec<i128>,
        center: i128,
    ) -> i128 {
        let n = prices.len() as i128;

        if n == 0 {
            return 0;
        }

        let mut variance_sum: i128 = 0;

        for idx in 0..prices.len() {
            let diff = Self::abs_diff(
                prices.get(idx).unwrap(),
                center,
            );

            variance_sum = variance_sum.saturating_add(
                diff.saturating_mul(diff),
            );
        }

        Self::isqrt(variance_sum / n)
    }

    /// Integer square root using Newton's method.
    fn isqrt(n: i128) -> i128 {
        if n <= 0 {
            return 0;
        }

        let mut x = n;
        let mut y = x / 2 + 1;

        while y < x {
            x = y;
            y = (x + n / x) / 2;
        }

        x
    }

    /// Absolute difference for positive oracle prices.
    fn abs_diff(
        left: i128,
        right: i128,
    ) -> i128 {
        if left >= right {
            left - right
        } else {
            right - left
        }
    }
}

#[cfg(test)]
mod test;
