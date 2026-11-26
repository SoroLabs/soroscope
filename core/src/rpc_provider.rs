use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicI64, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Consecutive failures before the circuit opens (issue #1141).
const CIRCUIT_BREAKER_THRESHOLD: u64 = 5;
/// Base cooldown before the half-open `getHealth` probe (issues #1141, #1398).
/// Each consecutive trip doubles it, with jitter, up to
/// [`CIRCUIT_BREAKER_MAX_COOLDOWN`].
const CIRCUIT_BREAKER_COOLDOWN: Duration = Duration::from_secs(15);
/// Upper bound for the exponential cooldown (issue #1398).
const CIRCUIT_BREAKER_MAX_COOLDOWN: Duration = Duration::from_secs(5 * 60);
/// Requests in the rolling error-rate window (issue #1398).
const CIRCUIT_BREAKER_WINDOW: usize = 20;
/// The circuit opens once more than this share of a full window failed (issue #1398).
const CIRCUIT_BREAKER_ERROR_RATE: f64 = 0.5;
/// Cooldown jitter: the backoff is scaled by a factor in `1 ± CIRCUIT_BREAKER_JITTER`.
const CIRCUIT_BREAKER_JITTER: f64 = 0.2;
/// Interval between half-open probe requests (issue #1141).
pub const CIRCUIT_BREAKER_HALF_OPEN_PROBE_INTERVAL: Duration = CIRCUIT_BREAKER_COOLDOWN;
const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(10);
const REMOTE_OBSERVATION_TTL: Duration = Duration::from_secs(5 * 60);
const PEER_STALE_AFTER: Duration = Duration::from_secs(10 * 60);
const MAX_HEALTH_SCORE: i64 = 100;
const MIN_HEALTH_SCORE: i64 = 0;
const LOCAL_PROVIDER_STARTING_SCORE: i64 = 70;
const DISCOVERED_PROVIDER_STARTING_SCORE: i64 = 55;
const PEER_STARTING_SCORE: i64 = 60;
const LOCAL_SUCCESS_BONUS: i64 = 12;
const LOCAL_FAILURE_PENALTY: i64 = 25;
const PROBE_SUCCESS_BONUS: i64 = 6;
const PROBE_FAILURE_PENALTY: i64 = 15;
const PEER_SUCCESS_BONUS: i64 = 8;
const PEER_FAILURE_PENALTY: i64 = 20;
const MIN_PROVIDER_SCORE: i64 = 25;
const MAX_GOSSIP_PROVIDERS: usize = 64;
const MAX_GOSSIP_PEERS: usize = 64;
pub const MIN_SAMPLES_FOR_EMA: u64 = 5;

/// Three-state circuit breaker for Soroban RPC upstream calls (issues #1141, #1398).
///
/// * `Closed` — requests flow normally.
/// * `Open` — 5 consecutive failures, or more than 50% failures over the
///   last 20 requests, tripped the breaker; the provider gets no traffic and
///   callers fail over to healthy providers (or fast-fail with
///   `503 Service Unavailable` + `Retry-After` when none are left).
/// * `HalfOpen` — the cooldown (15 s, doubled with jitter on every
///   consecutive trip) elapsed; the provider still gets no client traffic
///   until a single `getHealth` probe succeeds, which closes the circuit. A
///   failed probe re-opens it with a longer cooldown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug)]
pub struct ProviderStats {
    ema_rtt_us: AtomicU64,
    sample_count: AtomicU64,
    window: u64,
}

impl ProviderStats {
    pub fn new(window: u64) -> Self {
        Self {
            ema_rtt_us: AtomicU64::new(0),
            sample_count: AtomicU64::new(0),
            window: window.max(1),
        }
    }

    pub fn record(&self, rtt_us: u64) {
        let count = self.sample_count.fetch_add(1, Ordering::Relaxed);
        if count == 0 {
            self.ema_rtt_us.store(rtt_us, Ordering::Relaxed);
            return;
        }

        let previous = self.ema_rtt_us.load(Ordering::Relaxed);
        let alpha_num = 2u128;
        let alpha_den = (self.window + 1) as u128;
        let next = ((rtt_us as u128 * alpha_num) + (previous as u128 * (alpha_den - alpha_num)))
            / alpha_den;
        self.ema_rtt_us.store(next as u64, Ordering::Relaxed);
    }

    pub fn ema_rtt_us(&self) -> u64 {
        self.ema_rtt_us.load(Ordering::Relaxed)
    }

    pub fn sample_count(&self) -> u64 {
        self.sample_count.load(Ordering::Relaxed)
    }

    pub fn is_warmed(&self) -> bool {
        self.sample_count() >= MIN_SAMPLES_FOR_EMA
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderStatsSnapshot {
    pub name: String,
    pub url: String,
    pub ema_rtt_us: u64,
    pub sample_count: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RpcProvider {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub auth_header: Option<String>,
    #[serde(default)]
    pub auth_value: Option<String>,
    /// Controls whether this provider can be advertised to peer nodes.
    ///
    /// When omitted, providers with credentials are kept local-only.
    #[serde(default)]
    pub advertise: Option<bool>,
}

impl RpcProvider {
    fn should_advertise(&self) -> bool {
        self.advertise.unwrap_or(self.auth_value.is_none())
    }

    fn public_provider(&self) -> PublicRpcProvider {
        PublicRpcProvider {
            name: self.name.clone(),
            url: self.url.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublicRpcProvider {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct RegistryConfig {
    pub instance_id: String,
    pub public_base_url: Option<String>,
    pub seed_peers: Vec<String>,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            instance_id: "local".to_string(),
            public_base_url: None,
            seed_peers: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerAdvertisement {
    pub instance_id: Option<String>,
    pub base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GossipProviderSnapshot {
    pub provider: PublicRpcProvider,
    pub score: i64,
    pub latest_ledger: Option<u64>,
    pub consecutive_failures: u64,
    pub healthy: bool,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrySnapshot {
    pub instance_id: String,
    pub base_url: Option<String>,
    pub generated_at: DateTime<Utc>,
    pub peers: Vec<PeerAdvertisement>,
    pub providers: Vec<GossipProviderSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderHealthReport {
    pub name: String,
    pub url: String,
    pub effective_score: i64,
    pub local_score: i64,
    pub peer_score: i64,
    pub latest_ledger: u64,
    pub consecutive_failures: u64,
    pub healthy: bool,
    pub source: String,
    pub observation_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PeerHealthReport {
    pub base_url: String,
    pub instance_id: Option<String>,
    pub score: i64,
    pub consecutive_failures: u64,
    pub healthy: bool,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub discovered_from: Vec<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
struct RemoteProviderObservation {
    score: i64,
    latest_ledger: u64,
    consecutive_failures: u64,
    healthy: bool,
    observed_at: DateTime<Utc>,
}

#[derive(Debug)]
struct ProviderState {
    provider: RwLock<RpcProvider>,
    source: &'static str,
    stats: ProviderStats,
    local_score: AtomicI64,
    consecutive_failures: AtomicU64,
    tripped_at: RwLock<Option<Instant>>,
    /// Cooldown of the current trip, in milliseconds (issue #1398).
    cooldown_ms: AtomicU64,
    /// Consecutive trips without a recovery; drives the exponential backoff.
    trip_count: AtomicU32,
    /// Outcomes of the last [`CIRCUIT_BREAKER_WINDOW`] requests, `true` = failed.
    outcomes: std::sync::Mutex<VecDeque<bool>>,
    latest_ledger: AtomicU64,
    last_local_observed_at: RwLock<Option<DateTime<Utc>>>,
    remote_observations: RwLock<HashMap<String, RemoteProviderObservation>>,
}

impl ProviderState {
    fn new(provider: RpcProvider, source: &'static str, local_score: i64) -> Self {
        Self {
            provider: RwLock::new(provider),
            source,
            stats: ProviderStats::new(20),
            local_score: AtomicI64::new(local_score),
            consecutive_failures: AtomicU64::new(0),
            tripped_at: RwLock::new(None),
            cooldown_ms: AtomicU64::new(CIRCUIT_BREAKER_COOLDOWN.as_millis() as u64),
            trip_count: AtomicU32::new(0),
            outcomes: std::sync::Mutex::new(VecDeque::with_capacity(CIRCUIT_BREAKER_WINDOW)),
            latest_ledger: AtomicU64::new(0),
            last_local_observed_at: RwLock::new(None),
            remote_observations: RwLock::new(HashMap::new()),
        }
    }

    /// Record one request outcome and report whether the rolling window is
    /// full and its error rate exceeds [`CIRCUIT_BREAKER_ERROR_RATE`].
    fn record_outcome(&self, failed: bool) -> bool {
        let mut outcomes = self.outcomes.lock().unwrap_or_else(|e| e.into_inner());
        outcomes.push_back(failed);
        while outcomes.len() > CIRCUIT_BREAKER_WINDOW {
            outcomes.pop_front();
        }
        if outcomes.len() < CIRCUIT_BREAKER_WINDOW {
            return false;
        }
        let failures = outcomes.iter().filter(|failed| **failed).count();
        failures as f64 / outcomes.len() as f64 > CIRCUIT_BREAKER_ERROR_RATE
    }

    fn cooldown(&self) -> Duration {
        Duration::from_millis(self.cooldown_ms.load(Ordering::Relaxed))
    }
}

/// Cooldown for the `trips`-th consecutive trip: the base cooldown doubled per
/// trip, capped at [`CIRCUIT_BREAKER_MAX_COOLDOWN`], then scaled by `jitter`.
fn breaker_cooldown(trips: u32, jitter: f64) -> Duration {
    let exponent = trips.saturating_sub(1).min(16);
    let backoff = CIRCUIT_BREAKER_COOLDOWN
        .saturating_mul(1u32 << exponent)
        .min(CIRCUIT_BREAKER_MAX_COOLDOWN);
    backoff.mul_f64(jitter)
}

/// Random jitter factor in `1 ± CIRCUIT_BREAKER_JITTER`, so providers tripped
/// together do not all probe at the same instant.
fn breaker_jitter() -> f64 {
    use rand::Rng;
    rand::thread_rng().gen_range((1.0 - CIRCUIT_BREAKER_JITTER)..=(1.0 + CIRCUIT_BREAKER_JITTER))
}

#[derive(Debug)]
struct PeerState {
    base_url: String,
    instance_id: RwLock<Option<String>>,
    score: AtomicI64,
    consecutive_failures: AtomicU64,
    last_seen_at: RwLock<Option<DateTime<Utc>>>,
    discovered_from: RwLock<HashSet<String>>,
    last_error: RwLock<Option<String>>,
}

impl PeerState {
    fn new(base_url: String, instance_id: Option<String>) -> Self {
        Self {
            base_url,
            instance_id: RwLock::new(instance_id),
            score: AtomicI64::new(PEER_STARTING_SCORE),
            consecutive_failures: AtomicU64::new(0),
            last_seen_at: RwLock::new(None),
            discovered_from: RwLock::new(HashSet::new()),
            last_error: RwLock::new(None),
        }
    }
}

pub struct ProviderRegistry {
    states: RwLock<HashMap<String, Arc<ProviderState>>>,
    peers: RwLock<HashMap<String, Arc<PeerState>>>,
    client: Client,
    instance_id: String,
    public_base_url: Option<String>,
    latency_cursor: AtomicUsize,
    metrics: OnceLock<crate::metrics::AppMetrics>,
}

impl ProviderRegistry {
    pub fn new(providers: Vec<RpcProvider>) -> Arc<Self> {
        Self::new_with_config(providers, RegistryConfig::default())
    }

    pub fn new_with_config(providers: Vec<RpcProvider>, config: RegistryConfig) -> Arc<Self> {
        let mut states = HashMap::new();

        for provider in providers {
            states.insert(
                provider.url.clone(),
                Arc::new(ProviderState::new(
                    provider,
                    "seed",
                    LOCAL_PROVIDER_STARTING_SCORE,
                )),
            );
        }

        let mut peers = HashMap::new();
        for peer in config
            .seed_peers
            .into_iter()
            .map(|peer| normalize_base_url(&peer))
            .filter(|peer| !peer.is_empty())
        {
            if config.public_base_url.as_deref() == Some(peer.as_str()) {
                continue;
            }

            peers.insert(peer.clone(), Arc::new(PeerState::new(peer, None)));
        }

        Arc::new(Self {
            states: RwLock::new(states),
            peers: RwLock::new(peers),
            client: Client::new(),
            instance_id: config.instance_id,
            public_base_url: config.public_base_url.map(|url| normalize_base_url(&url)),
            latency_cursor: AtomicUsize::new(0),
            metrics: OnceLock::new(),
        })
    }

    /// Export circuit breaker transitions through `metrics` (issue #1398).
    /// Only the first call takes effect.
    pub fn attach_metrics(&self, metrics: crate::metrics::AppMetrics) {
        let _ = self.metrics.set(metrics);
    }

    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    pub fn public_base_url(&self) -> Option<&str> {
        self.public_base_url.as_deref()
    }

    /// Return the list of providers that are currently available for requests,
    /// in priority order (skipping tripped providers whose cooldown hasn't elapsed).
    pub async fn healthy_providers(&self) -> Vec<RpcProvider> {
        let mut available = Vec::new();
        let states = self.states.read().await;
        let provider_states = states.values().cloned().collect::<Vec<_>>();
        drop(states);

        for state in provider_states {
            if self.is_available(state.clone()).await {
                available.push(state.provider.read().await.clone());
            }
        }
        available
    }

    pub async fn providers_by_latency(&self) -> Vec<RpcProvider> {
        let states = self.states.read().await;
        let provider_states = states.values().cloned().collect::<Vec<_>>();
        drop(states);

        let mut available = Vec::new();
        for state in provider_states {
            if self.is_available(state.clone()).await {
                available.push(state);
            }
        }

        if available.is_empty() {
            return Vec::new();
        }

        if available.iter().all(|state| state.stats.is_warmed()) {
            available.sort_by_key(|state| state.stats.ema_rtt_us());
        } else {
            let offset = self.latency_cursor.fetch_add(1, Ordering::Relaxed) % available.len();
            available.rotate_left(offset);
        }

        let mut providers = Vec::with_capacity(available.len());
        for state in available {
            providers.push(state.provider.read().await.clone());
        }
        providers
    }

    pub fn record_rtt(&self, url: &str, rtt_us: u64) {
        if let Ok(states) = self.states.try_read() {
            if let Some(state) = states.get(url) {
                state.stats.record(rtt_us);
            }
        }
    }

    pub fn stats_snapshot(&self) -> Vec<ProviderStatsSnapshot> {
        let Ok(states) = self.states.try_read() else {
            return Vec::new();
        };

        states
            .values()
            .filter_map(|state| {
                state
                    .provider
                    .try_read()
                    .ok()
                    .map(|provider| ProviderStatsSnapshot {
                        name: provider.name.clone(),
                        url: provider.url.clone(),
                        ema_rtt_us: state.stats.ema_rtt_us(),
                        sample_count: state.stats.sample_count(),
                    })
            })
            .collect()
    }

    pub async fn provider_reports(&self) -> Vec<ProviderHealthReport> {
        self.collect_provider_reports()
            .await
            .into_iter()
            .map(|(_, report)| report)
            .collect()
    }

    pub async fn peer_reports(&self) -> Vec<PeerHealthReport> {
        let peers = self.peers.read().await;
        let peer_states = peers.values().cloned().collect::<Vec<_>>();
        drop(peers);

        let mut reports = Vec::with_capacity(peer_states.len());
        for peer in peer_states {
            reports.push(self.build_peer_report(peer).await);
        }
        reports
    }

    pub async fn report_success(&self, url: &str) {
        if let Some(state) = self.find_by_url(url).await {
            state.record_outcome(false);
            state.consecutive_failures.store(0, Ordering::Relaxed);
            let recovered_score = clamp_score(
                state
                    .local_score
                    .load(Ordering::Relaxed)
                    .max(DISCOVERED_PROVIDER_STARTING_SCORE)
                    + LOCAL_SUCCESS_BONUS,
            );
            state.local_score.store(recovered_score, Ordering::Relaxed);
            state
                .last_local_observed_at
                .write()
                .await
                .replace(Utc::now());
            self.close_circuit(&state).await;
        }
    }

    pub async fn report_failure(&self, url: &str) {
        if let Some(state) = self.find_by_url(url).await {
            let prev = state.consecutive_failures.fetch_add(1, Ordering::Relaxed);
            let window_tripped = state.record_outcome(true);
            state.local_score.store(
                adjust_score(
                    state.local_score.load(Ordering::Relaxed),
                    -LOCAL_FAILURE_PENALTY,
                ),
                Ordering::Relaxed,
            );
            state
                .last_local_observed_at
                .write()
                .await
                .replace(Utc::now());

            if prev + 1 >= CIRCUIT_BREAKER_THRESHOLD || window_tripped {
                self.trip_circuit(&state, prev + 1).await;
            }
        }
    }

    pub async fn report_rate_limit_failure(&self, url: &str) {
        if let Some(state) = self.find_by_url(url).await {
            let prev = state.consecutive_failures.fetch_add(1, Ordering::Relaxed);
            let window_tripped = state.record_outcome(true);
            state.local_score.store(
                adjust_score(
                    state.local_score.load(Ordering::Relaxed),
                    -(LOCAL_FAILURE_PENALTY * 2),
                ),
                Ordering::Relaxed,
            );
            if prev + 1 >= CIRCUIT_BREAKER_THRESHOLD || window_tripped {
                self.trip_circuit(&state, prev + 1).await;
            }
        }
    }

    pub async fn select_failover_provider(&self, exclude_url: Option<&str>) -> Option<RpcProvider> {
        let healthy = self.healthy_providers().await;
        healthy
            .into_iter()
            .find(|p| exclude_url.map_or(true, |ex| p.url != ex))
    }

    /// Current three-state circuit breaker status for one provider URL.
    ///
    /// Unknown URLs report [`CircuitBreakerState::Closed`]. Once the
    /// cooldown elapses the breaker reports [`CircuitBreakerState::HalfOpen`]
    /// until [`probe_half_open_providers`] runs its single `getHealth` probe.
    ///
    /// [`probe_half_open_providers`]: ProviderRegistry::probe_half_open_providers
    pub async fn circuit_state(&self, url: &str) -> CircuitBreakerState {
        let Some(state) = self.find_by_url(url).await else {
            return CircuitBreakerState::Closed;
        };
        Self::state_of(&state).await
    }

    async fn state_of(state: &ProviderState) -> CircuitBreakerState {
        match *state.tripped_at.read().await {
            None => CircuitBreakerState::Closed,
            Some(tripped) if tripped.elapsed() < state.cooldown() => CircuitBreakerState::Open,
            Some(_) => CircuitBreakerState::HalfOpen,
        }
    }

    /// Open the circuit for `state` with a jittered exponential cooldown.
    ///
    /// A provider that is already open keeps its current cooldown; a
    /// half-open provider (failed probe) re-opens with the next, longer one.
    async fn trip_circuit(&self, state: &ProviderState, failures: u64) {
        if Self::state_of(state).await == CircuitBreakerState::Open {
            return;
        }
        let trips = state.trip_count.fetch_add(1, Ordering::Relaxed) + 1;
        let cooldown = breaker_cooldown(trips, breaker_jitter());
        state
            .cooldown_ms
            .store(cooldown.as_millis() as u64, Ordering::Relaxed);
        *state.tripped_at.write().await = Some(Instant::now());

        let provider = state.provider.read().await;
        tracing::warn!(
            provider = %provider.name,
            url = %provider.url,
            failures,
            trips,
            cooldown_ms = cooldown.as_millis() as u64,
            "Provider circuit breaker tripped"
        );
        self.record_transition(&provider.name, "open");
    }

    /// Close the circuit for `state` and reset its backoff.
    async fn close_circuit(&self, state: &ProviderState) {
        let was_tripped = state.tripped_at.write().await.take().is_some();
        state.trip_count.store(0, Ordering::Relaxed);
        if was_tripped {
            let provider = state.provider.read().await;
            tracing::info!(
                provider = %provider.name,
                url = %provider.url,
                "Provider circuit breaker closed"
            );
            self.record_transition(&provider.name, "closed");
        }
    }

    fn record_transition(&self, provider: &str, to: &str) {
        if let Some(metrics) = self.metrics.get() {
            metrics
                .rpc_circuit_breaker_transitions_total
                .with_label_values(&[provider, to])
                .inc();
        }
    }

    /// Send one `getHealth` probe to every half-open provider (issue #1398).
    ///
    /// A healthy answer closes the circuit and restores traffic; anything
    /// else re-opens it with a longer cooldown.
    pub async fn probe_half_open_providers(&self) {
        let states = self.states.read().await;
        let provider_states = states.values().cloned().collect::<Vec<_>>();
        drop(states);

        for state in provider_states {
            if Self::state_of(&state).await != CircuitBreakerState::HalfOpen {
                continue;
            }
            self.record_transition(&state.provider.read().await.name, "half_open");
            match self.probe_provider(&state).await {
                Ok(ledger) => {
                    state.latest_ledger.store(ledger, Ordering::Relaxed);
                    state.consecutive_failures.store(0, Ordering::Relaxed);
                    state.record_outcome(false);
                    self.close_circuit(&state).await;
                }
                Err(error) => {
                    let failures = state.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
                    state.record_outcome(true);
                    tracing::warn!(error = %error, "Half-open probe failed");
                    self.trip_circuit(&state, failures).await;
                }
            }
        }
    }

    /// Value for the `Retry-After` header when the circuit is open.
    pub fn circuit_open_retry_after_secs() -> u64 {
        CIRCUIT_BREAKER_COOLDOWN.as_secs()
    }

    /// HTTP status + `Retry-After` seconds to return when the circuit is open:
    /// `503 Service Unavailable` so clients back off instead of queueing
    /// behind a degraded upstream.
    pub fn circuit_open_response() -> (u16, u64) {
        (503, Self::circuit_open_retry_after_secs())
    }

    pub fn is_retryable_status(status: u16) -> bool {
        status == 429 || status >= 500
    }

    pub fn spawn_health_checker(
        self: &Arc<Self>,
        interval: Duration,
        mut shutdown: tokio::sync::broadcast::Receiver<()>,
    ) -> tokio::task::JoinHandle<()> {
        let registry = Arc::clone(self);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                tokio::select! {
                    biased;
                    _ = shutdown.recv() => {
                        tracing::info!("RPC health checker shutting down");
                        break;
                    }
                    _ = ticker.tick() => {
                        tokio::select! {
                            biased;
                            _ = shutdown.recv() => {
                                tracing::info!("RPC health checker shutting down");
                                break;
                            }
                            _ = async {
                                registry.run_health_checks().await;
                                registry.probe_half_open_providers().await;
                            } => {}
                        }
                    }
                }
            }
        })
    }

    pub fn spawn_gossip_task(
        self: &Arc<Self>,
        interval: Duration,
        mut shutdown: tokio::sync::broadcast::Receiver<()>,
    ) -> tokio::task::JoinHandle<()> {
        let registry = Arc::clone(self);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                tokio::select! {
                    biased;
                    _ = shutdown.recv() => {
                        tracing::info!("Provider gossip task shutting down");
                        break;
                    }
                    _ = ticker.tick() => {
                        tokio::select! {
                            biased;
                            _ = shutdown.recv() => {
                                tracing::info!("Provider gossip task shutting down");
                                break;
                            }
                            _ = registry.run_gossip_round() => {}
                        }
                    }
                }
            }
        })
    }

    async fn collect_provider_reports(&self) -> Vec<(RpcProvider, ProviderHealthReport)> {
        let states = self.states.read().await;
        let provider_states = states.values().cloned().collect::<Vec<_>>();
        drop(states);

        let mut reports = Vec::with_capacity(provider_states.len());
        for state in provider_states {
            let provider = state.provider.read().await.clone();
            let report = self.build_provider_report(&provider, &state).await;
            reports.push((provider, report));
        }

        reports
    }

    async fn build_provider_report(
        &self,
        provider: &RpcProvider,
        state: &ProviderState,
    ) -> ProviderHealthReport {
        let local_score = state.local_score.load(Ordering::Relaxed);
        let latest_ledger = state.latest_ledger.load(Ordering::Relaxed);
        let consecutive_failures = state.consecutive_failures.load(Ordering::Relaxed);
        let tripped = self.is_provider_tripped(state).await;

        let remote_observations = state.remote_observations.read().await;
        let fresh_observations = remote_observations
            .values()
            .filter(|observation| !is_observation_stale(observation.observed_at))
            .cloned()
            .collect::<Vec<_>>();
        drop(remote_observations);

        let peer_score = if fresh_observations.is_empty() {
            0
        } else {
            fresh_observations.iter().map(|o| o.score).sum::<i64>()
                / fresh_observations.len() as i64
        };

        let remote_healthy = fresh_observations
            .iter()
            .any(|observation| observation.healthy);
        let best_remote_ledger = fresh_observations
            .iter()
            .map(|observation| observation.latest_ledger)
            .max()
            .unwrap_or(0);
        let remote_failure_floor = fresh_observations
            .iter()
            .map(|observation| observation.consecutive_failures)
            .min()
            .unwrap_or(consecutive_failures);

        let effective_score = clamp_score((local_score * 2 + peer_score) / 3);
        let healthy = !tripped
            && (effective_score >= MIN_PROVIDER_SCORE || remote_healthy)
            && (consecutive_failures < CIRCUIT_BREAKER_THRESHOLD || remote_healthy);

        ProviderHealthReport {
            name: provider.name.clone(),
            url: provider.url.clone(),
            effective_score,
            local_score,
            peer_score,
            latest_ledger: latest_ledger.max(best_remote_ledger),
            consecutive_failures: consecutive_failures.min(remote_failure_floor),
            healthy,
            source: state.source.to_string(),
            observation_count: fresh_observations.len(),
        }
    }

    async fn build_peer_report(&self, peer: Arc<PeerState>) -> PeerHealthReport {
        let last_seen_at = *peer.last_seen_at.read().await;
        let discovered_from = peer
            .discovered_from
            .read()
            .await
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        let last_error = peer.last_error.read().await.clone();
        let instance_id = peer.instance_id.read().await.clone();
        let score = peer.score.load(Ordering::Relaxed);
        let consecutive_failures = peer.consecutive_failures.load(Ordering::Relaxed);
        let healthy = score >= (PEER_STARTING_SCORE / 2)
            && last_seen_at
                .map(|seen_at| {
                    Utc::now()
                        .signed_duration_since(seen_at)
                        .to_std()
                        .unwrap_or_default()
                        < PEER_STALE_AFTER
                })
                .unwrap_or(true);

        PeerHealthReport {
            base_url: peer.base_url.clone(),
            instance_id,
            score,
            consecutive_failures,
            healthy,
            last_seen_at,
            discovered_from,
            last_error,
        }
    }

    async fn run_health_checks(&self) {
        let states = self.states.read().await;
        let provider_states = states.values().cloned().collect::<Vec<_>>();
        drop(states);

        for state in provider_states {
            // Tripped providers recover only through the half-open probe.
            if Self::state_of(&state).await != CircuitBreakerState::Closed {
                continue;
            }
            let result = self.probe_provider(&state).await;
            match result {
                Ok(ledger) => {
                    state.latest_ledger.store(ledger, Ordering::Relaxed);
                    state.consecutive_failures.store(0, Ordering::Relaxed);
                    state.local_score.store(
                        adjust_score(
                            state.local_score.load(Ordering::Relaxed),
                            PROBE_SUCCESS_BONUS,
                        ),
                        Ordering::Relaxed,
                    );
                    state
                        .last_local_observed_at
                        .write()
                        .await
                        .replace(Utc::now());
                }
                Err(error) => {
                    let prev = state.consecutive_failures.fetch_add(1, Ordering::Relaxed);
                    state.local_score.store(
                        adjust_score(
                            state.local_score.load(Ordering::Relaxed),
                            -PROBE_FAILURE_PENALTY,
                        ),
                        Ordering::Relaxed,
                    );
                    state
                        .last_local_observed_at
                        .write()
                        .await
                        .replace(Utc::now());

                    {
                        let provider = state.provider.read().await;
                        tracing::warn!(
                            provider = %provider.name,
                            url = %provider.url,
                            consecutive_failures = prev + 1,
                            error = %error,
                            "Provider health check failed"
                        );
                    }

                    if prev + 1 >= CIRCUIT_BREAKER_THRESHOLD {
                        self.trip_circuit(&state, prev + 1).await;
                    }
                }
            }
        }
    }

    async fn run_gossip_round(&self) {
        let peers = self.peers.read().await;
        let peer_states = peers.values().cloned().collect::<Vec<_>>();
        drop(peers);

        if peer_states.is_empty() {
            return;
        }

        let local_snapshot = self.registry_snapshot().await;

        for peer in peer_states {
            let endpoint = format!("{}/registry/gossip", peer.base_url);
            match self
                .client
                .post(&endpoint)
                .json(&local_snapshot)
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    match response.json::<RegistrySnapshot>().await {
                        Ok(snapshot) => {
                            self.merge_snapshot(snapshot).await;
                            self.report_peer_success(&peer.base_url).await;
                        }
                        Err(error) => {
                            self.report_peer_failure(
                                &peer.base_url,
                                format!("invalid gossip payload: {error}"),
                            )
                            .await;
                        }
                    }
                }
                Ok(response) => {
                    self.report_peer_failure(
                        &peer.base_url,
                        format!("HTTP {}", response.status().as_u16()),
                    )
                    .await;
                }
                Err(error) => {
                    self.report_peer_failure(&peer.base_url, error.to_string())
                        .await;
                }
            }
        }
    }

    async fn probe_provider(&self, state: &ProviderState) -> Result<u64, String> {
        let provider = state.provider.read().await.clone();
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getHealth",
            "params": null
        });

        let mut req = self.client.post(&provider.url).json(&body);
        if let (Some(header), Some(value)) = (&provider.auth_header, &provider.auth_value) {
            req = req.header(header.as_str(), value.as_str());
        }

        let start = Instant::now();
        let response = tokio::time::timeout(HEALTH_CHECK_TIMEOUT, req.send())
            .await
            .map_err(|_| "timeout".to_string())?
            .map_err(|error| format!("request error: {error}"))?;

        let rtt_us = start.elapsed().as_micros() as u64;
        state.stats.record(rtt_us);

        if !response.status().is_success() {
            return Err(format!("HTTP {}", response.status().as_u16()));
        }

        if rtt_us > 2_000_000 {
            return Err(format!("RTT latency high: {}ms (> 2000ms)", rtt_us / 1000));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|error| format!("parse error: {error}"))?;

        if let Some(status) = json["result"]["status"].as_str() {
            if status != "healthy" {
                return Err(format!("RPC status unhealthy: {status}"));
            }
        }

        let sequence = json["result"]["sequence"]
            .as_u64()
            .or_else(|| json["result"]["latestLedger"].as_u64())
            .unwrap_or(0);

        Ok(sequence)
    }

    async fn find_by_url(&self, url: &str) -> Option<Arc<ProviderState>> {
        let states = self.states.read().await;
        states.get(url).cloned()
    }

    async fn get_or_insert_provider(
        &self,
        provider: RpcProvider,
        source: &'static str,
        starting_score: i64,
    ) -> Arc<ProviderState> {
        if let Some(existing) = self.find_by_url(&provider.url).await {
            return existing;
        }

        let mut states = self.states.write().await;
        if let Some(existing) = states.get(&provider.url) {
            return existing.clone();
        }

        let state = Arc::new(ProviderState::new(provider.clone(), source, starting_score));
        states.insert(provider.url.clone(), Arc::clone(&state));
        state
    }

    async fn register_peer(
        &self,
        base_url: &str,
        instance_id: Option<String>,
        discovered_from: Option<&str>,
    ) {
        let normalized = normalize_base_url(base_url);
        if normalized.is_empty() || self.public_base_url.as_deref() == Some(normalized.as_str()) {
            return;
        }

        let peer = {
            let mut peers = self.peers.write().await;
            peers
                .entry(normalized.clone())
                .or_insert_with(|| {
                    Arc::new(PeerState::new(normalized.clone(), instance_id.clone()))
                })
                .clone()
        };

        if let Some(instance_id) = instance_id {
            *peer.instance_id.write().await = Some(instance_id);
        }

        if let Some(discovered_from) = discovered_from {
            peer.discovered_from
                .write()
                .await
                .insert(discovered_from.to_string());
        }
    }

    async fn report_peer_success(&self, base_url: &str) {
        let normalized = normalize_base_url(base_url);
        self.register_peer(&normalized, None, None).await;

        if let Some(peer) = self.peers.read().await.get(&normalized).cloned() {
            peer.consecutive_failures.store(0, Ordering::Relaxed);
            peer.score.store(
                adjust_score(peer.score.load(Ordering::Relaxed), PEER_SUCCESS_BONUS),
                Ordering::Relaxed,
            );
            peer.last_seen_at.write().await.replace(Utc::now());
            *peer.last_error.write().await = None;
        }
    }

    async fn report_peer_failure(&self, base_url: &str, error: String) {
        let normalized = normalize_base_url(base_url);
        self.register_peer(&normalized, None, None).await;

        if let Some(peer) = self.peers.read().await.get(&normalized).cloned() {
            peer.consecutive_failures.fetch_add(1, Ordering::Relaxed);
            peer.score.store(
                adjust_score(peer.score.load(Ordering::Relaxed), -PEER_FAILURE_PENALTY),
                Ordering::Relaxed,
            );
            *peer.last_error.write().await = Some(error);
        }
    }

    async fn is_available(&self, state: Arc<ProviderState>) -> bool {
        if self.is_provider_tripped(&state).await {
            return false;
        }

        state.local_score.load(Ordering::Relaxed) >= MIN_PROVIDER_SCORE
            || state.consecutive_failures.load(Ordering::Relaxed) < CIRCUIT_BREAKER_THRESHOLD
    }

    pub async fn registry_snapshot(&self) -> RegistrySnapshot {
        let reports = self.collect_provider_reports().await;
        let providers = reports
            .into_iter()
            .filter(|(provider, report)| provider.should_advertise() && report.healthy)
            .take(MAX_GOSSIP_PROVIDERS)
            .map(|(provider, report)| GossipProviderSnapshot {
                provider: provider.public_provider(),
                score: report.effective_score,
                latest_ledger: Some(report.latest_ledger),
                consecutive_failures: report.consecutive_failures,
                healthy: report.healthy,
                observed_at: Utc::now(),
            })
            .collect();

        let peers = self
            .peer_reports()
            .await
            .into_iter()
            .filter(|peer| peer.healthy)
            .take(MAX_GOSSIP_PEERS)
            .map(|peer| PeerAdvertisement {
                instance_id: peer.instance_id,
                base_url: peer.base_url,
            })
            .collect();

        RegistrySnapshot {
            instance_id: self.instance_id.clone(),
            base_url: self.public_base_url.clone(),
            generated_at: Utc::now(),
            peers,
            providers,
        }
    }

    pub async fn merge_snapshot(&self, snapshot: RegistrySnapshot) {
        if let Some(base_url) = snapshot.base_url.as_deref() {
            self.register_peer(base_url, Some(snapshot.instance_id.clone()), None)
                .await;
        }

        for peer in snapshot.peers {
            self.register_peer(
                &peer.base_url,
                peer.instance_id,
                snapshot.base_url.as_deref(),
            )
            .await;
        }

        for snapshot_provider in snapshot.providers {
            let provider = RpcProvider {
                name: snapshot_provider.provider.name,
                url: snapshot_provider.provider.url,
                auth_header: None,
                auth_value: None,
                advertise: Some(true),
            };
            let state = self
                .get_or_insert_provider(provider, "gossip", DISCOVERED_PROVIDER_STARTING_SCORE)
                .await;
            let source = snapshot
                .base_url
                .clone()
                .unwrap_or_else(|| snapshot.instance_id.clone());
            state.remote_observations.write().await.insert(
                source,
                RemoteProviderObservation {
                    score: snapshot_provider.score,
                    latest_ledger: snapshot_provider.latest_ledger.unwrap_or(0),
                    consecutive_failures: snapshot_provider.consecutive_failures,
                    healthy: snapshot_provider.healthy,
                    observed_at: snapshot_provider.observed_at,
                },
            );
        }
    }

    /// Open and half-open providers both stay out of client traffic; only a
    /// successful half-open probe (or a reported success) closes the circuit.
    async fn is_provider_tripped(&self, state: &ProviderState) -> bool {
        state.tripped_at.read().await.is_some()
    }
}

fn normalize_base_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

fn clamp_score(score: i64) -> i64 {
    score.clamp(MIN_HEALTH_SCORE, MAX_HEALTH_SCORE)
}

fn adjust_score(current: i64, delta: i64) -> i64 {
    clamp_score(current + delta)
}

fn is_observation_stale(observed_at: DateTime<Utc>) -> bool {
    Utc::now()
        .signed_duration_since(observed_at)
        .to_std()
        .unwrap_or_default()
        >= REMOTE_OBSERVATION_TTL
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_provider(name: &str, url: &str) -> RpcProvider {
        RpcProvider {
            name: name.to_string(),
            url: url.to_string(),
            auth_header: None,
            auth_value: None,
            advertise: None,
        }
    }

    #[tokio::test]
    async fn test_all_seed_providers_are_healthy_initially() {
        let registry = ProviderRegistry::new(vec![
            make_provider("a", "http://a.test"),
            make_provider("b", "http://b.test"),
        ]);

        let providers = registry.healthy_providers().await;
        let urls = providers
            .iter()
            .map(|provider| provider.url.as_str())
            .collect::<HashSet<_>>();

        assert_eq!(providers.len(), 2);
        assert!(urls.contains("http://a.test"));
        assert!(urls.contains("http://b.test"));
    }

    #[tokio::test]
    async fn test_circuit_breaker_trips_after_threshold() {
        let registry = ProviderRegistry::new(vec![make_provider("a", "http://a.test")]);

        for _ in 0..CIRCUIT_BREAKER_THRESHOLD {
            registry.report_failure("http://a.test").await;
        }

        assert!(registry.healthy_providers().await.is_empty());
    }

    #[tokio::test]
    async fn test_circuit_breaker_reports_open_and_retry_after() {
        let registry = ProviderRegistry::new(vec![make_provider("a", "http://a.test")]);
        assert_eq!(
            registry.circuit_state("http://a.test").await,
            CircuitBreakerState::Closed
        );

        for _ in 0..CIRCUIT_BREAKER_THRESHOLD {
            registry.report_failure("http://a.test").await;
        }

        assert_eq!(
            registry.circuit_state("http://a.test").await,
            CircuitBreakerState::Open
        );
        assert_eq!(ProviderRegistry::circuit_open_response(), (503, 15));

        registry.report_success("http://a.test").await;
        assert_eq!(
            registry.circuit_state("http://a.test").await,
            CircuitBreakerState::Closed
        );
    }

    #[tokio::test]
    async fn test_success_clears_tripped_provider() {
        let registry = ProviderRegistry::new(vec![make_provider("a", "http://a.test")]);

        for _ in 0..CIRCUIT_BREAKER_THRESHOLD {
            registry.report_failure("http://a.test").await;
        }
        assert!(registry.healthy_providers().await.is_empty());

        registry.report_success("http://a.test").await;
        assert_eq!(registry.healthy_providers().await.len(), 1);
    }

    // ── Adaptive circuit breaker (#1398) ─────────────────────────────────────

    /// A mocked RPC node answering every request with `status` and `body`.
    async fn spawn_rpc_stub(status: u16, body: &'static str) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    let _ = socket.read(&mut buf).await;
                    let response = format!(
                        "HTTP/1.1 {status} Stub\r\nContent-Type: application/json\r\n\
                         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                    let _ = socket.shutdown().await;
                });
            }
        });
        format!("http://{addr}")
    }

    const HEALTHY_BODY: &str =
        r#"{"jsonrpc":"2.0","id":1,"result":{"status":"healthy","latestLedger":100}}"#;

    /// Make the current trip's cooldown elapse without sleeping.
    async fn expire_cooldown(registry: &ProviderRegistry, url: &str) {
        let state = registry.find_by_url(url).await.unwrap();
        state.cooldown_ms.store(0, Ordering::Relaxed);
    }

    #[tokio::test]
    async fn test_rpc_provider_trips_on_error_rate_over_window() {
        let registry = ProviderRegistry::new(vec![make_provider("a", "http://a.test")]);

        // 14 of 20 requests fail, never more than 2 in a row.
        for i in 0..CIRCUIT_BREAKER_WINDOW {
            if i % 3 == 2 {
                registry.report_success("http://a.test").await;
            } else {
                registry.report_failure("http://a.test").await;
            }
        }

        assert_eq!(
            registry.circuit_state("http://a.test").await,
            CircuitBreakerState::Open
        );
        assert!(registry.healthy_providers().await.is_empty());
    }

    #[tokio::test]
    async fn test_rpc_provider_stays_closed_at_or_below_half_errors() {
        let registry = ProviderRegistry::new(vec![make_provider("a", "http://a.test")]);

        // 10 of 20 requests fail, alternating.
        for i in 0..CIRCUIT_BREAKER_WINDOW {
            if i % 2 == 0 {
                registry.report_failure("http://a.test").await;
            } else {
                registry.report_success("http://a.test").await;
            }
        }

        assert_eq!(
            registry.circuit_state("http://a.test").await,
            CircuitBreakerState::Closed
        );
    }

    #[test]
    fn test_rpc_provider_cooldown_backs_off_exponentially() {
        assert_eq!(breaker_cooldown(1, 1.0), Duration::from_secs(15));
        assert_eq!(breaker_cooldown(2, 1.0), Duration::from_secs(30));
        assert_eq!(breaker_cooldown(3, 1.0), Duration::from_secs(60));
        assert_eq!(breaker_cooldown(20, 1.0), CIRCUIT_BREAKER_MAX_COOLDOWN);
        assert_eq!(breaker_cooldown(1, 0.8), Duration::from_secs(12));
    }

    #[test]
    fn test_rpc_provider_jitter_stays_in_bounds() {
        for _ in 0..100 {
            let jitter = breaker_jitter();
            assert!(
                (1.0 - CIRCUIT_BREAKER_JITTER..=1.0 + CIRCUIT_BREAKER_JITTER).contains(&jitter)
            );
        }
    }

    #[tokio::test]
    async fn test_rpc_provider_isolated_with_healthy_fallback() {
        let registry = ProviderRegistry::new(vec![
            make_provider("a", "http://a.test"),
            make_provider("b", "http://b.test"),
        ]);

        for _ in 0..CIRCUIT_BREAKER_THRESHOLD {
            registry.report_failure("http://a.test").await;
        }

        let healthy = registry.healthy_providers().await;
        assert_eq!(healthy.len(), 1);
        assert_eq!(healthy[0].url, "http://b.test");
        assert_eq!(
            registry.select_failover_provider(None).await.unwrap().url,
            "http://b.test"
        );
    }

    #[tokio::test]
    async fn test_rpc_provider_half_open_probe_restores_recovered_endpoint() {
        let url = spawn_rpc_stub(200, HEALTHY_BODY).await;
        let registry = ProviderRegistry::new(vec![make_provider("a", &url)]);
        for _ in 0..CIRCUIT_BREAKER_THRESHOLD {
            registry.report_failure(&url).await;
        }

        expire_cooldown(&registry, &url).await;
        assert_eq!(
            registry.circuit_state(&url).await,
            CircuitBreakerState::HalfOpen
        );
        // Half-open still gets no client traffic before the probe.
        assert!(registry.healthy_providers().await.is_empty());

        registry.probe_half_open_providers().await;

        assert_eq!(
            registry.circuit_state(&url).await,
            CircuitBreakerState::Closed
        );
        assert_eq!(registry.healthy_providers().await.len(), 1);
    }

    #[tokio::test]
    async fn test_rpc_provider_failed_probe_reopens_with_longer_cooldown() {
        let url = spawn_rpc_stub(503, r#"{"error":"unavailable"}"#).await;
        let registry = ProviderRegistry::new(vec![make_provider("a", &url)]);
        for _ in 0..CIRCUIT_BREAKER_THRESHOLD {
            registry.report_failure(&url).await;
        }
        let first_cooldown = registry.find_by_url(&url).await.unwrap().cooldown();

        expire_cooldown(&registry, &url).await;
        registry.probe_half_open_providers().await;

        let state = registry.find_by_url(&url).await.unwrap();
        assert_eq!(
            registry.circuit_state(&url).await,
            CircuitBreakerState::Open
        );
        assert_eq!(state.trip_count.load(Ordering::Relaxed), 2);
        // Second trip: 30 s ± 20% is always longer than the first, 15 s ± 20%.
        assert!(state.cooldown() > first_cooldown);
    }

    #[tokio::test]
    async fn test_rpc_provider_exports_breaker_transitions() {
        let url = spawn_rpc_stub(200, HEALTHY_BODY).await;
        let registry = ProviderRegistry::new(vec![make_provider("a", &url)]);
        let metrics = crate::metrics::AppMetrics::new().unwrap();
        registry.attach_metrics(metrics.clone());

        for _ in 0..CIRCUIT_BREAKER_THRESHOLD {
            registry.report_failure(&url).await;
        }
        expire_cooldown(&registry, &url).await;
        registry.probe_half_open_providers().await;

        let transitions = &metrics.rpc_circuit_breaker_transitions_total;
        assert_eq!(transitions.with_label_values(&["a", "open"]).get(), 1);
        assert_eq!(transitions.with_label_values(&["a", "half_open"]).get(), 1);
        assert_eq!(transitions.with_label_values(&["a", "closed"]).get(), 1);
    }

    #[tokio::test]
    async fn test_gossip_discovers_provider_and_peer() {
        let registry = ProviderRegistry::new_with_config(
            vec![make_provider("seed", "http://seed.test")],
            RegistryConfig {
                instance_id: "node-a".to_string(),
                public_base_url: Some("http://node-a.test".to_string()),
                seed_peers: Vec::new(),
            },
        );

        registry
            .merge_snapshot(RegistrySnapshot {
                instance_id: "node-b".to_string(),
                base_url: Some("http://node-b.test".to_string()),
                generated_at: Utc::now(),
                peers: vec![PeerAdvertisement {
                    instance_id: Some("node-c".to_string()),
                    base_url: "http://node-c.test".to_string(),
                }],
                providers: vec![GossipProviderSnapshot {
                    provider: PublicRpcProvider {
                        name: "shared".to_string(),
                        url: "http://shared.test".to_string(),
                    },
                    score: 90,
                    latest_ledger: Some(123),
                    consecutive_failures: 0,
                    healthy: true,
                    observed_at: Utc::now(),
                }],
            })
            .await;

        let providers = registry.healthy_providers().await;
        assert!(providers
            .iter()
            .any(|provider| provider.url == "http://shared.test"));

        let peers = registry.peer_reports().await;
        assert!(peers
            .iter()
            .any(|peer| peer.base_url == "http://node-b.test"));
        assert!(peers
            .iter()
            .any(|peer| peer.base_url == "http://node-c.test"));
    }

    #[tokio::test]
    async fn test_snapshot_omits_private_provider_credentials() {
        let registry = ProviderRegistry::new(vec![
            make_provider("public", "http://public.test"),
            RpcProvider {
                name: "private".to_string(),
                url: "http://private.test".to_string(),
                auth_header: Some("Authorization".to_string()),
                auth_value: Some("secret".to_string()),
                advertise: None,
            },
        ]);

        let snapshot = registry.registry_snapshot().await;
        assert!(snapshot
            .providers
            .iter()
            .any(|provider| provider.provider.url == "http://public.test"));
        assert!(!snapshot
            .providers
            .iter()
            .any(|provider| provider.provider.url == "http://private.test"));
    }

    #[tokio::test]
    async fn test_peer_failures_lower_peer_score() {
        let registry = ProviderRegistry::new_with_config(
            vec![make_provider("seed", "http://seed.test")],
            RegistryConfig {
                instance_id: "node-a".to_string(),
                public_base_url: Some("http://node-a.test".to_string()),
                seed_peers: vec!["http://node-b.test".to_string()],
            },
        );

        registry
            .report_peer_failure("http://node-b.test", "timeout".to_string())
            .await;

        let report = registry
            .peer_reports()
            .await
            .into_iter()
            .find(|peer| peer.base_url == "http://node-b.test")
            .unwrap();

        assert!(report.score < PEER_STARTING_SCORE);
        assert_eq!(report.last_error.as_deref(), Some("timeout"));
    }

    // ── ProviderStats / latency routing tests ─────────────────────────────

    /// Helper: record `count` identical samples to warm the EMA past the
    /// routing threshold and onto a stable value.
    fn warm_stats(stats: &ProviderStats, rtt_us: u64, count: u64) {
        for _ in 0..count {
            stats.record(rtt_us);
        }
    }

    #[test]
    fn ema_converges_toward_true_value() {
        let stats = ProviderStats::new(20);
        warm_stats(&stats, 1000, 100);
        // With α ≈ 0.095 and 100 identical samples, the EMA sits right
        // on the true value — anything further than 5% off would be a
        // real bug.
        let ema = stats.ema_rtt_us();
        let drift = ema.abs_diff(1000);
        assert!(drift <= 50, "EMA drifted {drift}µs from 1000µs");
        assert_eq!(stats.sample_count(), 100);
    }

    #[test]
    fn ema_first_sample_seeds_exact_value() {
        // The series starts at the first sample instead of climbing from
        // zero — otherwise early routing decisions would penalise brand
        // new providers.
        let stats = ProviderStats::new(20);
        stats.record(777);
        assert_eq!(stats.ema_rtt_us(), 777);
    }

    #[test]
    fn ema_zero_window_is_clamped_safely() {
        // A caller-supplied zero window would have divided by zero; the
        // constructor clamps it to a 1-sample minimum.
        let stats = ProviderStats::new(0);
        stats.record(500);
        stats.record(1000);
        // Just ensure no panic and the EMA moved.
        assert!(stats.ema_rtt_us() > 0);
        assert_eq!(stats.sample_count(), 2);
    }

    #[test]
    fn is_warmed_reflects_sample_count_threshold() {
        let stats = ProviderStats::new(20);
        assert!(!stats.is_warmed());
        for _ in 0..MIN_SAMPLES_FOR_EMA - 1 {
            stats.record(100);
        }
        assert!(
            !stats.is_warmed(),
            "{} samples is below threshold",
            stats.sample_count()
        );
        stats.record(100);
        assert!(
            stats.is_warmed(),
            "{} samples should be warmed",
            stats.sample_count()
        );
    }

    #[tokio::test]
    async fn providers_by_latency_picks_fastest_once_warm() {
        let registry = ProviderRegistry::new(vec![
            make_provider("slow", "http://slow.test"),
            make_provider("fast", "http://fast.test"),
        ]);
        // Warm both with very different latencies.
        for _ in 0..MIN_SAMPLES_FOR_EMA {
            registry.record_rtt("http://slow.test", 500_000);
            registry.record_rtt("http://fast.test", 50_000);
        }
        let ordered = registry.providers_by_latency().await;
        assert_eq!(ordered.len(), 2);
        assert_eq!(ordered[0].name, "fast");
        assert_eq!(ordered[1].name, "slow");
    }

    #[tokio::test]
    async fn providers_by_latency_round_robins_before_warmup() {
        let registry = ProviderRegistry::new(vec![
            make_provider("a", "http://a.test"),
            make_provider("b", "http://b.test"),
        ]);
        // Neither provider has any samples → round-robin fallback.
        let first = registry.providers_by_latency().await;
        let second = registry.providers_by_latency().await;
        assert_eq!(first.len(), 2);
        assert_eq!(second.len(), 2);
        // The cursor rotates by one each call, so the head differs.
        assert_ne!(first[0].name, second[0].name);
    }

    #[tokio::test]
    async fn providers_by_latency_excludes_unhealthy_providers() {
        let registry = ProviderRegistry::new(vec![
            make_provider("fast-but-down", "http://fast-down.test"),
            make_provider("slow-but-up", "http://slow-up.test"),
        ]);
        // Fast provider has the best EMA but we trip its breaker.
        for _ in 0..MIN_SAMPLES_FOR_EMA {
            registry.record_rtt("http://fast-down.test", 10_000);
            registry.record_rtt("http://slow-up.test", 500_000);
        }
        for _ in 0..CIRCUIT_BREAKER_THRESHOLD {
            registry.report_failure("http://fast-down.test").await;
        }
        let ordered = registry.providers_by_latency().await;
        assert_eq!(ordered.len(), 1);
        assert_eq!(ordered[0].name, "slow-but-up");
    }

    #[tokio::test]
    async fn providers_by_latency_empty_when_all_unhealthy() {
        let registry = ProviderRegistry::new(vec![
            make_provider("a", "http://a.test"),
            make_provider("b", "http://b.test"),
        ]);
        for _ in 0..CIRCUIT_BREAKER_THRESHOLD {
            registry.report_failure("http://a.test").await;
            registry.report_failure("http://b.test").await;
        }
        assert!(registry.providers_by_latency().await.is_empty());
    }

    #[test]
    fn record_rtt_for_unknown_url_is_noop() {
        // No panic, no sample recorded.
        let registry = ProviderRegistry::new(vec![make_provider("a", "http://a.test")]);
        registry.record_rtt("http://unknown.test", 100);
        let snap = registry.stats_snapshot();
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].sample_count, 0);
    }

    #[test]
    fn stats_snapshot_reflects_recorded_samples() {
        let registry = ProviderRegistry::new(vec![
            make_provider("a", "http://a.test"),
            make_provider("b", "http://b.test"),
        ]);
        for _ in 0..5 {
            registry.record_rtt("http://a.test", 1_000);
        }
        registry.record_rtt("http://b.test", 10_000);

        let snap = registry.stats_snapshot();
        let a = snap.iter().find(|s| s.name == "a").unwrap();
        let b = snap.iter().find(|s| s.name == "b").unwrap();
        assert_eq!(a.sample_count, 5);
        assert_eq!(b.sample_count, 1);
        // EMA for `a` converged to 1_000; `b` seeded at 10_000.
        assert_eq!(a.ema_rtt_us, 1_000);
        assert_eq!(b.ema_rtt_us, 10_000);
    }

    #[tokio::test]
    async fn health_checker_exits_when_shutdown_is_broadcast() {
        let registry = ProviderRegistry::new(vec![make_provider("a", "http://a.test")]);
        let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
        let handle = registry.spawn_health_checker(Duration::from_millis(10), shutdown_rx);

        shutdown_tx
            .send(())
            .expect("shutdown broadcast should succeed");

        tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .expect("health checker should exit promptly after shutdown")
            .expect("health checker task should not panic");
    }

    #[tokio::test]
    async fn gossip_task_exits_when_shutdown_is_broadcast() {
        let registry = ProviderRegistry::new(vec![make_provider("a", "http://a.test")]);
        let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
        let handle = registry.spawn_gossip_task(Duration::from_millis(10), shutdown_rx);

        shutdown_tx
            .send(())
            .expect("shutdown broadcast should succeed");

        tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .expect("gossip task should exit promptly after shutdown")
            .expect("gossip task should not panic");
    }

    #[tokio::test]
    async fn test_select_failover_provider_and_rate_limit_penalty() {
        let registry = ProviderRegistry::new(vec![
            make_provider("node-1", "http://node1.test"),
            make_provider("node-2", "http://node2.test"),
        ]);

        let initial = registry.select_failover_provider(None).await;
        assert!(initial.is_some());

        let fallback = registry.select_failover_provider(Some("http://node1.test")).await;
        assert_eq!(fallback.unwrap().url, "http://node2.test");

        registry.report_rate_limit_failure("http://node1.test").await;
        let reports = registry.provider_reports().await;
        let n1 = reports.iter().find(|r| r.url == "http://node1.test").unwrap();
        assert!(n1.local_score < LOCAL_PROVIDER_STARTING_SCORE);
    }
}
