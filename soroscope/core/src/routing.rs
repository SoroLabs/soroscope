//! Pure routing algorithms for picking an RPC provider by measured RTT.
//!
//! The registry (`crate::rpc_provider`) owns the actual state — this
//! module hosts the small algorithmic pieces (least-EMA selection,
//! inverse-RTT weight computation) as free functions operating on
//! caller-supplied slices, which makes them exhaustively testable
//! without spinning up a full registry.
//!
//! The production dispatch path is
//! [`crate::rpc_provider::ProviderRegistry::providers_by_latency`]; this
//! module is the algorithmic core of that method plus extra helpers
//! (weighted round-robin) that the registry may adopt later.

use crate::rpc_provider::MIN_SAMPLES_FOR_EMA;

/// View of one provider that the routing algorithms care about. All
/// pure functions in this module take slices of `ProviderView`, which
/// lets tests synthesise scenarios without any real RPC state.
#[derive(Debug, Clone, Copy)]
pub struct ProviderView<'a> {
    pub name: &'a str,
    pub is_healthy: bool,
    pub ema_rtt_us: u64,
    pub sample_count: u64,
}

/// Return the index (into `providers`) of the best provider to send the
/// next request to, or `None` when no provider is healthy.
///
/// - Primary strategy: pick the healthy provider with the **lowest**
///   EMA RTT. Requires every healthy provider to have reached
///   [`MIN_SAMPLES_FOR_EMA`] samples so we don't bias against providers
///   with short EMAs purely because they're new.
/// - Fallback: while any healthy provider is below the threshold, use
///   round-robin over the healthy set. `round_robin_cursor` is the
///   caller's monotonically-advancing counter (typically an
///   `AtomicUsize::fetch_add(1, …)` from the registry).
pub fn select_provider_index(
    providers: &[ProviderView<'_>],
    round_robin_cursor: usize,
) -> Option<usize> {
    let healthy: Vec<(usize, ProviderView<'_>)> = providers
        .iter()
        .enumerate()
        .filter(|(_, p)| p.is_healthy)
        .map(|(i, p)| (i, *p))
        .collect();

    if healthy.is_empty() {
        return None;
    }

    let all_warmed = healthy
        .iter()
        .all(|(_, p)| p.sample_count >= MIN_SAMPLES_FOR_EMA);

    if !all_warmed {
        // Round-robin across the healthy subset. Modulo against
        // `healthy.len()` (not `providers.len()`) so unhealthy
        // providers are genuinely skipped rather than producing a
        // no-op tick.
        let pick = round_robin_cursor % healthy.len();
        return Some(healthy[pick].0);
    }

    // Least-EMA. Ties break by original index — the first provider
    // declared in config wins on equal latency.
    healthy
        .into_iter()
        .min_by_key(|(i, p)| (p.ema_rtt_us, *i))
        .map(|(i, _)| i)
}

/// Compute weighted round-robin weights for the healthy subset of
/// `providers`. Returns one weight per healthy provider, in input order.
/// Faster providers receive higher weights: `weight = max_rtt / rtt`.
///
/// Skips unhealthy providers entirely — the output length equals the
/// count of healthy providers, so callers pairing the weights with
/// indices must track the filter themselves.
pub fn compute_inverse_rtt_weights(providers: &[ProviderView<'_>]) -> Vec<u64> {
    let rtts: Vec<u64> = providers
        .iter()
        .filter(|p| p.is_healthy)
        .map(|p| p.ema_rtt_us.max(1))
        .collect();

    if rtts.is_empty() {
        return Vec::new();
    }

    let max_rtt = *rtts.iter().max().unwrap();
    rtts.into_iter().map(|r| max_rtt / r).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper — build a healthy, fully-warmed provider view.
    fn warm(name: &str, ema_us: u64) -> ProviderView<'_> {
        ProviderView {
            name,
            is_healthy: true,
            ema_rtt_us: ema_us,
            sample_count: MIN_SAMPLES_FOR_EMA,
        }
    }

    fn cold(name: &str) -> ProviderView<'_> {
        ProviderView {
            name,
            is_healthy: true,
            ema_rtt_us: 0,
            sample_count: 0,
        }
    }

    fn down(name: &str, ema_us: u64) -> ProviderView<'_> {
        ProviderView {
            name,
            is_healthy: false,
            ema_rtt_us: ema_us,
            sample_count: MIN_SAMPLES_FOR_EMA,
        }
    }

    #[test]
    fn empty_pool_returns_none() {
        assert_eq!(select_provider_index(&[], 0), None);
    }

    #[test]
    fn all_unhealthy_returns_none() {
        let pool = [down("a", 50), down("b", 100)];
        assert_eq!(select_provider_index(&pool, 0), None);
    }

    #[test]
    fn select_provider_picks_fastest_healthy() {
        // slow at 500ms, fast at 50ms; fast must win on every cursor value.
        let pool = [warm("slow", 500_000), warm("fast", 50_000)];
        assert_eq!(select_provider_index(&pool, 0), Some(1));
        assert_eq!(select_provider_index(&pool, 42), Some(1));
    }

    #[test]
    fn select_provider_falls_back_to_round_robin_before_warmup() {
        // `a` has no samples; we must use round-robin across both.
        let pool = [cold("a"), warm("b", 50_000)];
        assert_eq!(select_provider_index(&pool, 0), Some(0));
        assert_eq!(select_provider_index(&pool, 1), Some(1));
        assert_eq!(select_provider_index(&pool, 2), Some(0));
    }

    #[test]
    fn round_robin_skips_unhealthy() {
        // `a` and `c` healthy, `b` unhealthy; cursor should cycle a → c → a → …
        let pool = [cold("a"), down("b", 100), cold("c")];
        assert_eq!(select_provider_index(&pool, 0), Some(0));
        assert_eq!(select_provider_index(&pool, 1), Some(2));
        assert_eq!(select_provider_index(&pool, 2), Some(0));
    }

    #[test]
    fn unhealthy_providers_are_excluded_from_ema_pick() {
        // Fast but unhealthy — must not be picked. Slow but healthy
        // is the only remaining candidate.
        let pool = [down("fast", 10_000), warm("slow", 500_000)];
        assert_eq!(select_provider_index(&pool, 0), Some(1));
    }

    #[test]
    fn ties_broken_by_lowest_index() {
        // Two providers, identical EMA — the first declared wins.
        let pool = [warm("first", 100_000), warm("second", 100_000)];
        assert_eq!(select_provider_index(&pool, 0), Some(0));
    }

    #[test]
    fn inverse_rtt_weights_favour_faster_providers() {
        // fast=50us, slow=500us → max/fast=10, max/slow=1
        let pool = [warm("slow", 500), warm("fast", 50)];
        assert_eq!(compute_inverse_rtt_weights(&pool), vec![1, 10]);
    }

    #[test]
    fn inverse_rtt_weights_skip_unhealthy() {
        let pool = [warm("a", 100), down("b", 10), warm("c", 50)];
        assert_eq!(compute_inverse_rtt_weights(&pool), vec![1, 2]);
    }

    #[test]
    fn inverse_rtt_weights_on_empty_healthy_pool_return_empty() {
        let pool = [down("a", 100), down("b", 50)];
        assert!(compute_inverse_rtt_weights(&pool).is_empty());
    }

    #[test]
    fn zero_rtt_is_clamped_to_one_in_weight_calc() {
        // A provider with an EMA of exactly zero (no samples) must not
        // divide-by-zero the weight formula; it's clamped to 1us so it
        // receives weight = max_rtt.
        let pool = [
            ProviderView {
                name: "zero",
                is_healthy: true,
                ema_rtt_us: 0,
                sample_count: MIN_SAMPLES_FOR_EMA,
            },
            warm("real", 200),
        ];
        let weights = compute_inverse_rtt_weights(&pool);
        assert_eq!(weights.len(), 2);
        assert!(weights[0] >= weights[1]);
    }
}
