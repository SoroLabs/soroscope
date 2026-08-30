#![cfg(test)]

use super::*;
use soroban_sdk::{contract, contractimpl, contracttype, testutils::Ledger as _, Address, Env, Vec};

// ---------------------------------------------------------------------------
// Stub oracle contracts – each lives in its own module to prevent
// soroban-sdk's `#[contractimpl]` from emitting duplicate `__fn_name`
// module-level symbols when multiple contracts share the same method name.
// ---------------------------------------------------------------------------

/// Returns price=100 timestamped at the current ledger time (always fresh).
mod fresh_100 {
    use super::*;

    #[contract]
    pub struct FreshSource100;

    #[contractimpl]
    impl FreshSource100 {
        pub fn latest_price_with_timestamp(env: Env) -> Result<OracleRecord, Error> {
            Ok(OracleRecord {
                price: 100,
                timestamp: env.ledger().timestamp(),
            })
        }
    }

    pub use FreshSource100 as Contract;
}

/// Returns price=101 timestamped at the current ledger time.
mod fresh_101 {
    use super::*;

    #[contract]
    pub struct FreshSource101;

    #[contractimpl]
    impl FreshSource101 {
        pub fn latest_price_with_timestamp(env: Env) -> Result<OracleRecord, Error> {
            Ok(OracleRecord {
                price: 101,
                timestamp: env.ledger().timestamp(),
            })
        }
    }

    pub use FreshSource101 as Contract;
}

/// Returns price=99 timestamped at the current ledger time.
mod fresh_99 {
    use super::*;

    #[contract]
    pub struct FreshSource99;

    #[contractimpl]
    impl FreshSource99 {
        pub fn latest_price_with_timestamp(env: Env) -> Result<OracleRecord, Error> {
            Ok(OracleRecord {
                price: 99,
                timestamp: env.ledger().timestamp(),
            })
        }
    }

    pub use FreshSource99 as Contract;
}

/// Simulates a source whose last update was 500 s ago (stale if max_age < 500).
mod stale_source {
    use super::*;

    #[contract]
    pub struct StaleSource;

    #[contractimpl]
    impl StaleSource {
        pub fn latest_price_with_timestamp(env: Env) -> Result<OracleRecord, Error> {
            let stale_ts = env.ledger().timestamp().saturating_sub(500);
            Ok(OracleRecord {
                price: 100,
                timestamp: stale_ts,
            })
        }
    }

    pub use StaleSource as Contract;
}

/// Always returns an error – simulates an unresponsive oracle node.
mod unresponsive_source {
    use super::*;

    #[contract]
    pub struct UnresponsiveSource;

    #[contractimpl]
    impl UnresponsiveSource {
        pub fn latest_price_with_timestamp(_env: Env) -> Result<OracleRecord, Error> {
            Err(Error::InvalidPrice)
        }
    }

    pub use UnresponsiveSource as Contract;
}

/// Returns a wildly outlier price with a fresh timestamp.
mod outlier_source {
    use super::*;

    #[contract]
    pub struct OutlierSource;

    #[contractimpl]
    impl OutlierSource {
        pub fn latest_price_with_timestamp(env: Env) -> Result<OracleRecord, Error> {
            Ok(OracleRecord {
                price: 150_000,
                timestamp: env.ledger().timestamp(),
            })
        }
    }

    pub use OutlierSource as Contract;
}

// ---------------------------------------------------------------------------
// Helper: register all stubs and return their addresses
// ---------------------------------------------------------------------------

fn register_all(env: &Env) -> (Address, Address, Address, Address, Address, Address) {
    let fresh_100 = env.register(fresh_100::Contract, ());
    let fresh_101 = env.register(fresh_101::Contract, ());
    let fresh_99 = env.register(fresh_99::Contract, ());
    let stale = env.register(stale_source::Contract, ());
    let unresponsive = env.register(unresponsive_source::Contract, ());
    let outlier = env.register(outlier_source::Contract, ());
    (fresh_100, fresh_101, fresh_99, stale, unresponsive, outlier)
}

// ---------------------------------------------------------------------------
// Tests – happy path
// ---------------------------------------------------------------------------

#[test]
fn test_aggregate_three_fresh_sources() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.timestamp = 1_000);

    let (fresh_100, fresh_101, fresh_99, _, _, _) = register_all(&env);
    let aggregator_id = env.register(OracleAggregator, ());
    let client = OracleAggregatorClient::new(&env, &aggregator_id);

    let sources = Vec::from_array(&env, [fresh_100, fresh_101, fresh_99]);
    // All three fresh; median of sorted [99, 100, 101] = 100.
        assert_eq!(client.aggregate_price(&sources, &60), 100);
}

#[test]
fn test_aggregate_ignores_outlier_keeps_fresh_median() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.timestamp = 1_000);

    let (fresh_100, fresh_101, fresh_99, _, _, outlier) = register_all(&env);
    let aggregator_id = env.register(OracleAggregator, ());
    let client = OracleAggregatorClient::new(&env, &aggregator_id);

    let sources = Vec::from_array(&env, [fresh_100, fresh_101, fresh_99, outlier]);
    // Outlier (150_000) is filtered as an extreme outlier; median = 100.
        assert_eq!(client.aggregate_price(&sources, &60), 100);
}

#[test]
fn test_aggregate_skips_unresponsive_source() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.timestamp = 1_000);

    let (fresh_100, fresh_101, fresh_99, _, unresponsive, _) = register_all(&env);
    let aggregator_id = env.register(OracleAggregator, ());
    let client = OracleAggregatorClient::new(&env, &aggregator_id);

    let sources = Vec::from_array(&env, [fresh_100, fresh_101, fresh_99, unresponsive]);
    // Unresponsive skipped; three fresh remain → median = 100.
        assert_eq!(client.aggregate_price(&sources, &60), 100);
}

// ---------------------------------------------------------------------------
// Tests – staleness enforcement
// ---------------------------------------------------------------------------

#[test]
fn test_stale_source_excluded_when_three_fresh_remain() {
    let env = Env::default();
    // Ledger time = 1000; StaleSource timestamp = 1000 - 500 = 500.
    env.ledger().with_mut(|li| li.timestamp = 1_000);

    let (fresh_100, fresh_101, fresh_99, stale, _, _) = register_all(&env);
    let aggregator_id = env.register(OracleAggregator, ());
    let client = OracleAggregatorClient::new(&env, &aggregator_id);

    // max_age_seconds = 60 → stale source (age 500 s) is excluded.
    // Three fresh sources remain → median = 100.
    let sources = Vec::from_array(&env, [fresh_100, fresh_101, fresh_99, stale]);
        assert_eq!(client.aggregate_price(&sources, &60), 100);
}

#[test]
fn test_returns_oracle_staleness_when_too_few_fresh_sources() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.timestamp = 1_000);

    let (fresh_100, _, _, stale, _, _) = register_all(&env);
    let stale2 = env.register(stale_source::Contract, ());
    let aggregator_id = env.register(OracleAggregator, ());
    let client = OracleAggregatorClient::new(&env, &aggregator_id);

    // Only 1 fresh source; 2 stale → OracleStaleness (need >= 3 fresh).
    let sources = Vec::from_array(&env, [fresh_100, stale, stale2]);
    assert_eq!(
        client.try_aggregate_price(&sources, &60),
        Err(Ok(Error::OracleStaleness))
    );
}

#[test]
fn test_all_stale_returns_oracle_staleness() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.timestamp = 1_000);

    let (_, _, _, stale, _, _) = register_all(&env);
    let stale2 = env.register(stale_source::Contract, ());
    let stale3 = env.register(stale_source::Contract, ());
    let aggregator_id = env.register(OracleAggregator, ());
    let client = OracleAggregatorClient::new(&env, &aggregator_id);

    let sources = Vec::from_array(&env, [stale, stale2, stale3]);
    assert_eq!(
        client.try_aggregate_price(&sources, &60),
        Err(Ok(Error::OracleStaleness))
    );
}

#[test]
fn test_stale_source_accepted_when_max_age_is_generous() {
    let env = Env::default();
    // Ledger time = 1000; StaleSource timestamp = 500 (age = 500 s).
    env.ledger().with_mut(|li| li.timestamp = 1_000);

    let (fresh_100, fresh_101, _, stale, _, _) = register_all(&env);
    let aggregator_id = env.register(OracleAggregator, ());
    let client = OracleAggregatorClient::new(&env, &aggregator_id);

    // max_age_seconds = 600 → age 500 s is within threshold.
    let sources = Vec::from_array(&env, [fresh_100, fresh_101, stale]);
    // Prices: [100, 101, 100] → sorted [100, 100, 101] → median = 100.
    assert_eq!(client.aggregate_price(&sources, &600), 100);
}

#[test]
fn test_exact_boundary_age_is_fresh() {
    let env = Env::default();
    // StaleSource age = 500 s; max_age_seconds = 500 → age == threshold → fresh.
    env.ledger().with_mut(|li| li.timestamp = 1_000);

    let (fresh_100, fresh_101, _, stale, _, _) = register_all(&env);
    let aggregator_id = env.register(OracleAggregator, ());
    let client = OracleAggregatorClient::new(&env, &aggregator_id);

    let sources = Vec::from_array(&env, [fresh_100, fresh_101, stale]);
    // age = 500 == max_age_seconds → still accepted.
    assert_eq!(client.aggregate_price(&sources, &500), 100);
}

#[test]
fn test_one_second_over_threshold_is_stale() {
    let env = Env::default();
    // StaleSource age = 500 s; max_age_seconds = 499 → excluded (age > threshold).
    env.ledger().with_mut(|li| li.timestamp = 1_000);

    let (fresh_100, fresh_101, _, stale, _, _) = register_all(&env);
    let stale2 = env.register(stale_source::Contract, ());
    let aggregator_id = env.register(OracleAggregator, ());
    let client = OracleAggregatorClient::new(&env, &aggregator_id);

    // Only 2 fresh sources remain → OracleStaleness.
    let sources = Vec::from_array(&env, [fresh_100, fresh_101, stale, stale2]);
    assert_eq!(
        client.try_aggregate_price(&sources, &499),
        Err(Ok(Error::OracleStaleness))
    );
}

// ---------------------------------------------------------------------------
// Tests – existing error conditions (updated for new signature)
// ---------------------------------------------------------------------------

#[test]
fn test_reject_when_not_enough_sources() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.timestamp = 1_000);

    let (fresh_100, fresh_101, _, _, _, _) = register_all(&env);
    let aggregator_id = env.register(OracleAggregator, ());
    let client = OracleAggregatorClient::new(&env, &aggregator_id);

    // Fewer than 3 source addresses → NotEnoughSources.
    let sources = Vec::from_array(&env, [fresh_100, fresh_101]);
    assert_eq!(
        client.try_aggregate_price(&sources, &60),
        Err(Ok(Error::NotEnoughSources))
    );
}

#[test]
fn test_aggregate_median_even_number_sources() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.timestamp = 1_000);

    let (fresh_100, fresh_101, fresh_99, _, _, _) = register_all(&env);
    let extra_101 = env.register(fresh_101::Contract, ());
    let aggregator_id = env.register(OracleAggregator, ());
    let client = OracleAggregatorClient::new(&env, &aggregator_id);

    // 4 sources: 100, 101, 99, 101 → sorted: 99, 100, 101, 101
    // Median should be (100 + 101) / 2 = 100
    let sources = Vec::from_array(&env, [fresh_100, fresh_101, fresh_99, extra_101]);
        assert_eq!(client.aggregate_price(&sources, &60), 100);
}

// ---------------------------------------------------------------------------
// Stub oracle whose price can be rewritten between aggregator calls
// ---------------------------------------------------------------------------

mod tunable_source {
    use super::*;

    #[contracttype]
    #[derive(Clone)]
    enum DataKey {
        Record,
    }

    #[contract]
    pub struct TunableSource;

    #[contractimpl]
    impl TunableSource {
        pub fn set_record(env: Env, price: i128, timestamp: u64) {
            env.storage()
                .instance()
                .set(&DataKey::Record, &OracleRecord { price, timestamp });
        }

        pub fn latest_price_with_timestamp(env: Env) -> Result<OracleRecord, Error> {
            env.storage()
                .instance()
                .get(&DataKey::Record)
                .ok_or(Error::InvalidPrice)
        }
    }

    pub use TunableSource as Contract;
    pub use TunableSourceClient as Client;
}

fn register_tunable_trio(env: &Env, price: i128, timestamp: u64) -> (Address, Address, Address) {
    let a = env.register(tunable_source::Contract, ());
    let b = env.register(tunable_source::Contract, ());
    let c = env.register(tunable_source::Contract, ());
    tunable_source::Client::new(env, &a).set_record(&price, &timestamp);
    tunable_source::Client::new(env, &b).set_record(&price, &timestamp);
    tunable_source::Client::new(env, &c).set_record(&price, &timestamp);
    (a, b, c)
}

// ---------------------------------------------------------------------------
// Tests – standard-deviation outlier filter & rolling TWAP
// ---------------------------------------------------------------------------

#[test]
fn test_stddev_filter_rejects_extreme_outlier() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.timestamp = 1_000);

    let (fresh_100, fresh_101, fresh_99, _, _, outlier) = register_all(&env);
    let aggregator_id = env.register(OracleAggregator, ());
    let client = OracleAggregatorClient::new(&env, &aggregator_id);

    let sources = Vec::from_array(&env, [fresh_100, fresh_101, fresh_99, outlier]);
    // 150_000 is many σ away from the median (~100) and is dropped.
        assert_eq!(client.aggregate_price(&sources, &60), 100);
}

#[test]
fn test_twap_equals_snapshot_when_no_time_elapsed() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.timestamp = 1_000);

    let (fresh_100, fresh_101, fresh_99, _, _, _) = register_all(&env);
    let aggregator_id = env.register(OracleAggregator, ());
    let client = OracleAggregatorClient::new(&env, &aggregator_id);

    assert_eq!(client.get_twap(), 0);
    assert_eq!(
        client.aggregate_price(
            &Vec::from_array(&env, [fresh_100, fresh_101, fresh_99]),
            &60
        ),
        100
    );
    // Single observation with dt = 0 falls back to the latest snapshot.
    assert_eq!(client.get_twap(), 100);
}

#[test]
fn test_rolling_twap_weights_prices_by_elapsed_time() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.timestamp = 1_000);

    let (a, b, c) = register_tunable_trio(&env, 100, 1_000);
    let aggregator_id = env.register(OracleAggregator, ());
    let client = OracleAggregatorClient::new(&env, &aggregator_id);
    client.initialize(&3_600);

    let sources = Vec::from_array(&env, [a.clone(), b.clone(), c.clone()]);
    assert_eq!(client.aggregate_price(&sources, &60), 100);

    // 100 seconds later the sources print 200.
    env.ledger().with_mut(|li| li.timestamp = 1_100);
    tunable_source::Client::new(&env, &a).set_record(&200, &1_100);
    tunable_source::Client::new(&env, &b).set_record(&200, &1_100);
    tunable_source::Client::new(&env, &c).set_record(&200, &1_100);
    assert_eq!(client.aggregate_price(&sources, &60), 200);

    // Immediately after the 200 snapshot the 200 price has zero duration, so
    // TWAP is still 100 (100 * 100s / 100s).
    assert_eq!(client.get_twap(), 100);

    // Hold 200 for another 200 seconds: TWAP = (100*100 + 200*200) / 300 = 166.
    env.ledger().with_mut(|li| li.timestamp = 1_300);
    assert_eq!(client.get_twap(), 166);
}

#[test]
fn test_twap_window_drops_expired_samples() {
    let env = Env::default();
    env.ledger().with_mut(|li| li.timestamp = 1_000);

    let (a, b, c) = register_tunable_trio(&env, 50, 1_000);
    let aggregator_id = env.register(OracleAggregator, ());
    let client = OracleAggregatorClient::new(&env, &aggregator_id);
    // 200-second rolling window.
    client.initialize(&200);

    let sources = Vec::from_array(&env, [a.clone(), b.clone(), c.clone()]);
    assert_eq!(client.aggregate_price(&sources, &60), 50);

    env.ledger().with_mut(|li| li.timestamp = 1_250);
    tunable_source::Client::new(&env, &a).set_record(&150, &1_250);
    tunable_source::Client::new(&env, &b).set_record(&150, &1_250);
    tunable_source::Client::new(&env, &c).set_record(&150, &1_250);
    assert_eq!(client.aggregate_price(&sources, &60), 150);

    // Window start = 1250 - 200 = 1050. The 50-price sample at t=1000 is kept
    // as the pre-window boundary price and weighted only from 1050 → 1250
    // (200s), then 150 is open until now (dt = 0) → TWAP = 50.
    assert_eq!(client.get_twap(), 50);

    env.ledger().with_mut(|li| li.timestamp = 1_500);
    // Window start = 1300. The 50-price interval (1000→1250) no longer
    // overlaps the window, so only 150 (1300→1500) is weighted → TWAP = 150.
    assert_eq!(client.get_twap(), 150);
}

#[test]
fn test_initialize_rejects_zero_window() {
    let env = Env::default();
    let aggregator_id = env.register(OracleAggregator, ());
    let client = OracleAggregatorClient::new(&env, &aggregator_id);
    assert_eq!(
        client.try_initialize(&0),
        Err(Ok(Error::InvalidWindow))
    );
}
