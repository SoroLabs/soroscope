//! Host system resource alarms for issue #592.
//!
//! Background tasks that compile and analyse Soroban contracts spike CPU
//! and memory unpredictably. Without prior warning a saturation event can
//! OOM-kill the hosting node before anyone notices. This module spawns an
//! internal tokio task that periodically samples the host's CPU and RAM
//! usage and triggers a webhook + structured log when either metric
//! crosses a configured threshold (default 85 %).
//!
//! Design notes:
//!
//! * **Edge-triggered** — alerts re-fire only when a resource has *cleared*
//!   the threshold and re-crossed it. Steady-state saturation produces one
//!   alert, not a flood, so on-call isn't paged into silence.
//! * **Pure-state evaluator** — the threshold/hysteresis logic lives in
//!   [`BreachDetector`] and [`SysAlarmEvaluator`], both of which are
//!   allocation-free and exercised by the `#[cfg(test)]` block below
//!   without any tokio runtime, network I/O, or `sysinfo` access.
//! * **Same webhook contract as [`crate::simulation_service`]\u00a0\u00b7 \u00a0`emit_alert`** \u2014 POST a
//!   JSON body and degrade gracefully to log-only when the URL is unset.
//! * **Prometheus-friendly** — the latest sample is exposed via
//!   `host_cpu_usage_percent`, `host_memory_usage_percent`, and
//!   `process_memory_bytes` gauges that are owned by the consumer's
//!   [`AppMetrics`] instance.
//!
//! Configuration is read from environment variables via
//! [`SysAlarmConfig::from_env`]:
//!
//! | Env var                                | Default | Purpose                                |
//! |----------------------------------------|---------|----------------------------------------|
//! | `SOROSCOPE_ALARM_THRESHOLD_PERCENT`    | `85.0`  | Trip threshold (0–100).                 |
//! | `SOROSCOPE_ALARM_INTERVAL_SECS`        | `10`    | Sampling cadence.                      |
//! | `SOROSCOPE_ALARM_WEBHOOK_URL`          | unset   | POST target for breach/recovery events.|
//! | `SOROSCOPE_ALARM_DISABLE`              | `false` | Set `1`/`true` to disable entirely.    |

use crate::AppMetrics;
use reqwest::Client;
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use sysinfo::{Pid, System};
use tracing::{info, warn};

/// Default alarm threshold, matching the spec from issue #592.
pub const DEFAULT_THRESHOLD_PERCENT: f64 = 85.0;
/// Default sampling interval in seconds.
pub const DEFAULT_INTERVAL_SECS: u64 = 10;

/// Maximum HTTP time for a single webhook POST before we abandon it.
const WEBHOOK_TIMEOUT: Duration = Duration::from_secs(5);

/// Environment variable names (kept in one place to make typos visible
/// only at one site).
pub const ENV_THRESHOLD: &str = "SOROSCOPE_ALARM_THRESHOLD_PERCENT";
pub const ENV_INTERVAL: &str = "SOROSCOPE_ALARM_INTERVAL_SECS";
pub const ENV_WEBHOOK: &str = "SOROSCOPE_ALARM_WEBHOOK_URL";
pub const ENV_DISABLE: &str = "SOROSCOPE_ALARM_DISABLE";
pub const ENV_INSTANCE_ID: &str = "SOROSCOPE_INSTANCE_ID";

/// Confguration for the alarm monitor.
#[derive(Debug, Clone)]
pub struct SysAlarmConfig {
    /// Percentage (0–100) that triggers an alarm.
    pub threshold_percent: f64,
    /// How often to sample CPU and RAM usage.
    pub interval: Duration,
    /// Webhook URL to POST to on breach / recovery. `None` = log only.
    pub webhook_url: Option<String>,
    /// Master switch — `false` short-circuits [`SysAlarmMonitor::spawn`].
    pub enabled: bool,
}

impl Default for SysAlarmConfig {
    fn default() -> Self {
        Self {
            threshold_percent: DEFAULT_THRESHOLD_PERCENT,
            interval: Duration::from_secs(DEFAULT_INTERVAL_SECS),
            webhook_url: None,
            enabled: true,
        }
    }
}

impl SysAlarmConfig {
    /// Construct from defaults and overlay environment variables.
    /// Out-of-range or unparseable env values are logged and ignored,
    /// keeping current defaults reachable from misconfigured
    /// deployments.
    pub fn from_env() -> Self {
        let mut cfg = Self::default();
        cfg.apply_env_overrides();
        cfg
    }

    /// Mutate in place, reading from `std::env`. Behaves identically to
    /// [`Self::from_env`] but does not allocate a new struct.
    pub fn apply_env_overrides(&mut self) {
        if let Ok(raw) = std::env::var(ENV_THRESHOLD) {
            self.set_threshold_from_str(&raw);
        }
        if let Ok(raw) = std::env::var(ENV_INTERVAL) {
            self.set_interval_secs_from_str(&raw);
        }
        if let Ok(raw) = std::env::var(ENV_WEBHOOK) {
            self.webhook_url = if raw.trim().is_empty() {
                None
            } else {
                Some(raw)
            };
        }
        if let Ok(raw) = std::env::var(ENV_DISABLE) {
            let disabled = matches!(
                raw.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            );
            self.enabled = !disabled;
        }
    }

    fn set_threshold_from_str(&mut self, raw: &str) {
        match raw.trim().parse::<f64>() {
            Ok(value) if (0.0..=100.0).contains(&value) => {
                self.threshold_percent = value;
            }
            Ok(_) => warn!(
                value = raw,
                env = %ENV_THRESHOLD,
                "Ignoring threshold: outside [0, 100] range"
            ),
            Err(_) => warn!(
                value = raw,
                env = %ENV_THRESHOLD,
                "Ignoring threshold: not a valid floating point number"
            ),
        }
    }

    fn set_interval_secs_from_str(&mut self, raw: &str) {
        match raw.trim().parse::<u64>() {
            Ok(value) if value > 0 => {
                self.interval = Duration::from_secs(value);
            }
            _ => warn!(
                value = raw,
                env = %ENV_INTERVAL,
                "Ignoring interval: must be a positive integer number of seconds"
            ),
        }
    }
}

/// State for a single resource. We could track entropy across multiple
/// resources, but each only cares whether the last sample was above or
/// below the threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreachState {
    Below,
    Above,
}

impl Default for BreachState {
    fn default() -> Self {
        Self::Below
    }
}

/// Edge transition observed by a [`BreachDetector`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    /// Reading crossed `threshold` upward since the previous sample.
    Triggered,
    /// Reading dropped below `threshold` since the previous sample.
    Resolved,
    /// Reading did not cross `threshold` — no event should be emitted.
    NoChange,
}

/// Per-resource stateful threshold detector. Maintains the previous
/// breach state so identical samples do not produce duplicate
/// notifications.
#[derive(Debug, Clone, Default)]
pub struct BreachDetector {
    state: BreachState,
}

impl BreachDetector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn currently_above(&self) -> bool {
        matches!(self.state, BreachState::Above)
    }

    /// Edge-triggered: returns `Triggered` the sample the value first
    /// crosses `threshold` from below, and `Resolved` when the value
    /// drops back below. The spec in issue #592 says "exceeds 85 %", so
    /// `value >= threshold` is treated as a breach — including the
    /// boundary sample itself.
    pub fn observe(&mut self, value: f64, threshold: f64) -> Transition {
        let now_above = value >= threshold;
        let new_state = if now_above {
            BreachState::Above
        } else {
            BreachState::Below
        };
        match (self.state, new_state) {
            (BreachState::Below, BreachState::Above) => {
                self.state = BreachState::Above;
                Transition::Triggered
            }
            (BreachState::Above, BreachState::Below) => {
                self.state = BreachState::Below;
                Transition::Resolved
            }
            _ => Transition::NoChange,
        }
    }
}

/// Combined evaluator for CPU and memory. Used in production by
/// [`SysAlarmMonitor`] and exercised directly by unit tests.
#[derive(Debug, Clone, Default)]
pub struct SysAlarmEvaluator {
    cpu: BreachDetector,
    memory: BreachDetector,
}

impl SysAlarmEvaluator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn observe_cpu(&mut self, cpu_percent: f64, threshold: f64) -> Transition {
        self.cpu.observe(cpu_percent, threshold)
    }

    pub fn observe_memory(&mut self, mem_percent: f64, threshold: f64) -> Transition {
        self.memory.observe(mem_percent, threshold)
    }

    pub fn is_cpu_above(&self) -> bool {
        self.cpu.currently_above()
    }

    pub fn is_memory_above(&self) -> bool {
        self.memory.currently_above()
    }
}

/// JSON payload posted to the webhook on threshold events. Kept stable
/// for downstream scrapers / pagers.
#[derive(Debug, Clone, Serialize)]
pub struct SysAlarmEvent {
    /// Either `"threshold_breach"` or `"threshold_recovered"`.
    pub event: &'static str,
    /// Either `"cpu"` or `"memory"`.
    pub resource: &'static str,
    pub value_percent: f64,
    pub threshold_percent: f64,
    pub process_memory_bytes: u64,
    pub total_memory_bytes: u64,
    pub pid: u32,
    pub node_id: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Background tokio task that owns the `sysinfo::System` handle. Spawn
/// it once at startup and forget about it — it exits only when the
/// tokio runtime shuts down.
pub struct SysAlarmMonitor {
    config: SysAlarmConfig,
    metrics: Option<Arc<AppMetrics>>,
}

impl SysAlarmMonitor {
    pub fn new(config: SysAlarmConfig) -> Self {
        Self {
            config,
            metrics: None,
        }
    }

    /// Hook the monitor into the shared Prometheus registry so external
    /// scrapers can see host-level samples alongside application
    /// metrics.
    pub fn with_metrics(mut self, metrics: Arc<AppMetrics>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Spawn the background loop. Returns `None` if the monitor is
    /// disabled, which removes the branch from the parent startup code
    /// entirely.
    pub fn spawn(self) -> Option<tokio::task::JoinHandle<()>> {
        if !self.config.enabled {
            info!(
                threshold_percent = self.config.threshold_percent,
                "SysAlarmMonitor disabled via {env}",
                env = ENV_DISABLE
            );
            return None;
        }

        info!(
            threshold_percent = self.config.threshold_percent,
            interval_secs = self.config.interval.as_secs(),
            webhook_enabled = self.config.webhook_url.is_some(),
            pid = std::process::id(),
            "SysAlarmMonitor starting"
        );

        let monitor = self;
        Some(tokio::spawn(async move {
            monitor.run_loop().await;
        }))
    }

    async fn run_loop(&self) {
        // Use `new_all` so process lookups work without an extra
        // refresh later. The first refresh below seeds the baseline
        // sample that `global_cpu_usage` needs to compute its delta.
        let mut system = System::new_all();
        system.refresh_cpu_usage();

        let pid = Pid::from_u32(std::process::id());
        let node_id = std::env::var(ENV_INSTANCE_ID)
            .ok()
            .or_else(|| std::env::var("HOSTNAME").ok())
            .filter(|s| !s.trim().is_empty());

        let mut evaluator = SysAlarmEvaluator::new();
        let mut interval = tokio::time::interval(self.config.interval);
        // `tokio::time::interval` fires its first tick immediately
        // which leaves no time for sysinfo to compute a CPU usage
        // delta. Wait once so the *first* reading in the loop already
        // reflects elapsed CPU activity rather than reading 0 %.
        tokio::time::sleep(self.config.interval).await;
        system.refresh_cpu_usage();
        system.refresh_memory();

        // Steady-state loop. We deliberately ignore errors here:
        // `System::refresh_*` is infallible on sysinfo's stable
        // versions, but if a future release panics we don't want to
        // crash the host process — just keep sampling on the next
        // tick.
        loop {
            system.refresh_cpu_usage();
            system.refresh_memory();

            let cpu_percent = system.global_cpu_usage() as f64;
            let total_memory = system.total_memory();
            let used_memory = system.used_memory();
            let mem_percent = if total_memory > 0 {
                (used_memory as f64 / total_memory as f64) * 100.0
            } else {
                0.0
            };
            let process_memory_bytes = system.process(pid).map(|p| p.memory()).unwrap_or(0);

            if let Some(metrics) = self.metrics.as_ref() {
                metrics
                    .host_cpu_usage_percent
                    .with_label_values(&["local"])
                    .set(cpu_percent);
                metrics
                    .host_memory_usage_percent
                    .with_label_values(&["local"])
                    .set(mem_percent);
                metrics
                    .process_memory_bytes
                    .with_label_values(&["soroscope-core"])
                    .set(process_memory_bytes as f64);
            } else {
                // Even without a metrics sink we still want operators
                // to see periodic readings in `tracing` output, but at
                // debug level to avoid flooding info-level logs.
                tracing::debug!(
                    cpu_percent,
                    mem_percent,
                    process_memory_bytes,
                    total_memory,
                    "sysalarm sample"
                );
            }

            if evaluator.observe_cpu(cpu_percent, self.config.threshold_percent)
                != Transition::NoChange
            {
                self.fire_alert(
                    "cpu",
                    cpu_percent,
                    evaluator.is_cpu_above(),
                    process_memory_bytes,
                    total_memory,
                    node_id.as_deref(),
                )
                .await;
            }

            if evaluator.observe_memory(mem_percent, self.config.threshold_percent)
                != Transition::NoChange
            {
                self.fire_alert(
                    "memory",
                    mem_percent,
                    evaluator.is_memory_above(),
                    process_memory_bytes,
                    total_memory,
                    node_id.as_deref(),
                )
                .await;
            }

            interval.tick().await;
        }
    }

    async fn fire_alert(
        &self,
        resource: &'static str,
        value_percent: f64,
        currently_above: bool,
        process_memory_bytes: u64,
        total_memory_bytes: u64,
        node_id: Option<&str>,
    ) {
        let event = if currently_above {
            "threshold_breach"
        } else {
            "threshold_recovered"
        };

        if currently_above {
            warn!(
                resource,
                event,
                value_percent,
                threshold_percent = self.config.threshold_percent,
                process_memory_bytes,
                total_memory_bytes,
                "Host resource usage breached alarm threshold"
            );
        } else {
            info!(
                resource,
                event,
                value_percent,
                threshold_percent = self.config.threshold_percent,
                "Host resource usage recovered below alarm threshold"
            );
        }

        let payload = SysAlarmEvent {
            event,
            resource,
            value_percent,
            threshold_percent: self.config.threshold_percent,
            process_memory_bytes,
            total_memory_bytes,
            pid: std::process::id(),
            node_id: node_id.map(|s| s.to_string()),
            timestamp: chrono::Utc::now(),
        };

        let Some(url) = self.config.webhook_url.as_deref() else {
            return;
        };

        let client = match Client::builder().timeout(WEBHOOK_TIMEOUT).build() {
            Ok(client) => client,
            Err(err) => {
                warn!(error = %err, "Failed to construct HTTP client for sysalarm webhook");
                return;
            }
        };

        match client.post(url).json(&payload).send().await {
            Ok(response) if response.status().is_success() => {}
            Ok(response) => {
                warn!(
                    status = response.status().as_u16(),
                    url, "Sysalarm webhook returned non-success status"
                );
            }
            Err(err) => {
                warn!(error = %err, url, "Failed to POST sysalarm webhook");
            }
        }
    }
}

/// JSON payload for a fee-spike alert.
#[derive(Debug, Clone, Serialize)]
pub struct FeeSpikeAlert {
    pub event: &'static str,
    pub current_fee: u64,
    pub spike_threshold: u64,
    pub ledger: u32,
}

/// Pure check: true when `current_fee` meets or exceeds the threshold.
pub fn is_fee_spike(current_fee: u64, spike_threshold: u64) -> bool {
    current_fee >= spike_threshold
}

/// Evaluate a fee sample for the cron job. Returns `Some(alert)` when the
/// fee spikes at or above `spike_threshold`.
pub fn check_fee_spike(current_fee: u64, spike_threshold: u64, ledger: u32) -> Option<FeeSpikeAlert> {
    if is_fee_spike(current_fee, spike_threshold) {
        Some(FeeSpikeAlert {
            event: "fee_spike",
            current_fee,
            spike_threshold,
            ledger,
        })
    } else {
        None
    }
}

/// Dispatch a fee-spike alert to `webhook_url` via POST. Log-only when the
/// URL is unset; never panics.
pub async fn dispatch_fee_spike_webhook(
    client: &Client,
    webhook_url: Option<&str>,
    alert: &FeeSpikeAlert,
) {
    let Some(url) = webhook_url else {
        warn!(?alert, "Fee spike detected (no webhook configured)");
        return;
    };
    match client.post(url).json(alert).send().await {
        Ok(response) if response.status().is_success() => {}
        Ok(response) => {
            warn!(
                status = response.status().as_u16(),
                url, "Fee-spike webhook returned non-success status"
            );
        }
        Err(err) => {
            warn!(error = %err, url, "Failed to POST fee-spike webhook");
        }
    }
}

// ── Resource exhaustion alarms (issue #1411) ────────────────────────────────
//
// Proactive thresholds for process RSS memory, open file descriptors and free
// space on the data volume, so the server degrades gracefully (pauses
// non-essential jobs, fails `/readyz`) before an OOM kill or disk-full panic.

/// Usage percentage at which a resource enters [`AlarmSeverity::Warning`].
pub const WARNING_THRESHOLD_PERCENT: f64 = 80.0;
/// Usage percentage at which a resource enters [`AlarmSeverity::Critical`].
pub const CRITICAL_THRESHOLD_PERCENT: f64 = 92.0;
/// Volume whose free space is watched; defaults to the working directory.
pub const ENV_DATA_DIR: &str = "SOROSCOPE_DATA_DIR";

/// Alarm level for one resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AlarmSeverity {
    Normal,
    Warning,
    Critical,
}

impl AlarmSeverity {
    /// Severity for a usage percentage.
    pub fn from_percent(percent: f64) -> Self {
        if percent >= CRITICAL_THRESHOLD_PERCENT {
            AlarmSeverity::Critical
        } else if percent >= WARNING_THRESHOLD_PERCENT {
            AlarmSeverity::Warning
        } else {
            AlarmSeverity::Normal
        }
    }

    fn as_u8(self) -> u8 {
        self as u8
    }

    fn from_u8(raw: u8) -> Self {
        match raw {
            2 => AlarmSeverity::Critical,
            1 => AlarmSeverity::Warning,
            _ => AlarmSeverity::Normal,
        }
    }
}

/// The resources watched for exhaustion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AlarmResource {
    Memory,
    FileDescriptors,
    Disk,
}

impl AlarmResource {
    pub const ALL: [AlarmResource; 3] =
        [AlarmResource::Memory, AlarmResource::FileDescriptors, AlarmResource::Disk];

    pub fn as_str(self) -> &'static str {
        match self {
            AlarmResource::Memory => "memory",
            AlarmResource::FileDescriptors => "file_descriptors",
            AlarmResource::Disk => "disk",
        }
    }
}

/// One sample of the watched resources. `None` means the platform could not
/// report that resource; it is then treated as [`AlarmSeverity::Normal`].
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ResourceStats {
    pub process_rss_bytes: u64,
    pub total_memory_bytes: u64,
    pub open_fds: Option<u64>,
    pub fd_limit: Option<u64>,
    pub disk_free_bytes: Option<u64>,
    pub disk_total_bytes: Option<u64>,
}

impl ResourceStats {
    /// Usage percentage (0–100) for `resource`, if it can be computed.
    pub fn usage_percent(&self, resource: AlarmResource) -> Option<f64> {
        let ratio = |used: u64, total: u64| {
            (total > 0).then(|| (used as f64 / total as f64) * 100.0)
        };
        match resource {
            AlarmResource::Memory => ratio(self.process_rss_bytes, self.total_memory_bytes),
            AlarmResource::FileDescriptors => ratio(self.open_fds?, self.fd_limit?),
            AlarmResource::Disk => {
                let total = self.disk_total_bytes?;
                ratio(total.saturating_sub(self.disk_free_bytes?), total)
            }
        }
    }
}

/// Source of [`ResourceStats`]; injectable so tests can supply mock stats.
pub trait SystemStatsSource: Send {
    fn sample(&mut self) -> ResourceStats;
}

/// Reads real process and host stats via `sysinfo` (and `/proc` on Linux for
/// file descriptors).
pub struct HostStatsSource {
    system: System,
    pid: Pid,
    data_dir: std::path::PathBuf,
}

impl HostStatsSource {
    pub fn new() -> Self {
        let data_dir = std::env::var(ENV_DATA_DIR)
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
        Self {
            system: System::new_all(),
            pid: Pid::from_u32(std::process::id()),
            data_dir: std::fs::canonicalize(&data_dir).unwrap_or(data_dir),
        }
    }

    fn file_descriptors() -> (Option<u64>, Option<u64>) {
        let open = std::fs::read_dir("/proc/self/fd").ok().map(|d| d.count() as u64);
        let limit = std::fs::read_to_string("/proc/self/limits").ok().and_then(|limits| {
            limits
                .lines()
                .find(|l| l.starts_with("Max open files"))
                .and_then(|l| l.split_whitespace().nth(3))
                .and_then(|soft| soft.parse().ok())
        });
        (open, limit)
    }

    fn disk_space(&self) -> (Option<u64>, Option<u64>) {
        let disks = sysinfo::Disks::new_with_refreshed_list();
        disks
            .list()
            .iter()
            .filter(|d| self.data_dir.starts_with(d.mount_point()))
            .max_by_key(|d| d.mount_point().as_os_str().len())
            .map(|d| (Some(d.available_space()), Some(d.total_space())))
            .unwrap_or((None, None))
    }
}

impl Default for HostStatsSource {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemStatsSource for HostStatsSource {
    fn sample(&mut self) -> ResourceStats {
        self.system.refresh_memory();
        self.system
            .refresh_processes(sysinfo::ProcessesToUpdate::Some(&[self.pid]), true);
        let (open_fds, fd_limit) = Self::file_descriptors();
        let (disk_free_bytes, disk_total_bytes) = self.disk_space();
        ResourceStats {
            process_rss_bytes: self.system.process(self.pid).map(|p| p.memory()).unwrap_or(0),
            total_memory_bytes: self.system.total_memory(),
            open_fds,
            fd_limit,
            disk_free_bytes,
            disk_total_bytes,
        }
    }
}

/// Latest alarm severity per resource, shared with `/readyz` and the
/// background jobs.
#[derive(Debug, Default)]
pub struct ResourceAlarmState {
    memory: std::sync::atomic::AtomicU8,
    file_descriptors: std::sync::atomic::AtomicU8,
    disk: std::sync::atomic::AtomicU8,
}

impl ResourceAlarmState {
    fn slot(&self, resource: AlarmResource) -> &std::sync::atomic::AtomicU8 {
        match resource {
            AlarmResource::Memory => &self.memory,
            AlarmResource::FileDescriptors => &self.file_descriptors,
            AlarmResource::Disk => &self.disk,
        }
    }

    pub fn severity(&self, resource: AlarmResource) -> AlarmSeverity {
        AlarmSeverity::from_u8(self.slot(resource).load(std::sync::atomic::Ordering::Relaxed))
    }

    /// Store `severity`, returning the previous value.
    pub fn set(&self, resource: AlarmResource, severity: AlarmSeverity) -> AlarmSeverity {
        AlarmSeverity::from_u8(
            self.slot(resource)
                .swap(severity.as_u8(), std::sync::atomic::Ordering::Relaxed),
        )
    }

    /// Resources currently in [`AlarmSeverity::Critical`].
    pub fn critical_resources(&self) -> Vec<AlarmResource> {
        AlarmResource::ALL
            .into_iter()
            .filter(|r| self.severity(*r) == AlarmSeverity::Critical)
            .collect()
    }

    /// True while a critical memory or disk alarm is active; non-essential
    /// background jobs skip their work until it clears.
    pub fn should_pause_background_jobs(&self) -> bool {
        self.severity(AlarmResource::Memory) == AlarmSeverity::Critical
            || self.severity(AlarmResource::Disk) == AlarmSeverity::Critical
    }

    /// `{"memory": "normal", ...}` for `/readyz`.
    pub fn snapshot(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for resource in AlarmResource::ALL {
            map.insert(
                resource.as_str().to_string(),
                serde_json::to_value(self.severity(resource)).unwrap_or_default(),
            );
        }
        serde_json::Value::Object(map)
    }
}

static RESOURCE_ALARMS: std::sync::OnceLock<ResourceAlarmState> = std::sync::OnceLock::new();

/// Process-wide alarm state updated by [`ResourceAlarmMonitor`].
pub fn resource_alarm_state() -> &'static ResourceAlarmState {
    RESOURCE_ALARMS.get_or_init(ResourceAlarmState::default)
}

/// Polls a [`SystemStatsSource`], updates a [`ResourceAlarmState`], logs
/// severity transitions and publishes the `resource_alarm_severity` gauge.
pub struct ResourceAlarmMonitor<S: SystemStatsSource> {
    source: S,
    state: &'static ResourceAlarmState,
    interval: Duration,
    gauge: Option<prometheus::IntGaugeVec>,
}

impl<S: SystemStatsSource + 'static> ResourceAlarmMonitor<S> {
    pub fn new(source: S, state: &'static ResourceAlarmState, interval: Duration) -> Self {
        Self { source, state, interval, gauge: None }
    }

    /// Publish severity (0 normal, 1 warning, 2 critical) per resource.
    pub fn with_gauge(mut self, gauge: prometheus::IntGaugeVec) -> Self {
        self.gauge = Some(gauge);
        self
    }

    /// Take one sample and update the alarm state.
    pub fn poll_once(&mut self) -> ResourceStats {
        let stats = self.source.sample();
        for resource in AlarmResource::ALL {
            let percent = stats.usage_percent(resource);
            let severity = percent.map(AlarmSeverity::from_percent).unwrap_or(AlarmSeverity::Normal);
            let previous = self.state.set(resource, severity);

            if let Some(gauge) = &self.gauge {
                gauge.with_label_values(&[resource.as_str()]).set(severity.as_u8() as i64);
            }

            if severity != previous {
                if severity > AlarmSeverity::Normal {
                    warn!(
                        resource = resource.as_str(),
                        severity = ?severity,
                        previous = ?previous,
                        usage_percent = percent.unwrap_or_default(),
                        "resource alarm raised"
                    );
                } else {
                    info!(
                        resource = resource.as_str(),
                        previous = ?previous,
                        usage_percent = percent.unwrap_or_default(),
                        "resource alarm cleared"
                    );
                }
            }
        }
        stats
    }

    /// Poll every `interval` on a background task.
    pub fn spawn(mut self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(self.interval);
            loop {
                interval.tick().await;
                self.poll_once();
            }
        })
    }
}

#[cfg(test)]
#[allow(deprecated)] // `std::env::set_var`/`remove_var` are deprecated in 1.74+; tests serialise via ENV_LOCK.
mod tests {
    use super::*;

    #[test]
    fn default_config_uses_issue_592_values() {
        let cfg = SysAlarmConfig::default();
        assert!((cfg.threshold_percent - DEFAULT_THRESHOLD_PERCENT).abs() < f64::EPSILON);
        assert_eq!(cfg.interval, Duration::from_secs(DEFAULT_INTERVAL_SECS));
        assert!(cfg.enabled);
        assert!(cfg.webhook_url.is_none());
    }

    #[test]
    fn breach_detector_starts_below() {
        let detector = BreachDetector::new();
        assert!(!detector.currently_above());
    }

    #[test]
    fn breach_detector_emits_triggered_on_first_cross_up() {
        let mut detector = BreachDetector::new();
        assert_eq!(detector.observe(85.001, 85.0), Transition::Triggered);
        assert!(detector.currently_above());
    }

    #[test]
    fn breach_detector_emits_no_change_on_repeated_high_samples() {
        let mut detector = BreachDetector::new();
        assert_eq!(detector.observe(86.0, 85.0), Transition::Triggered);
        for value in [86.0, 90.0, 99.99, 100.0, 250.0] {
            assert_eq!(detector.observe(value, 85.0), Transition::NoChange);
            assert!(
                detector.currently_above(),
                "detector flipped to below at value={value}"
            );
        }
    }

    #[test]
    fn breach_detector_emits_resolved_on_cross_down() {
        let mut detector = BreachDetector::new();
        detector.observe(86.0, 85.0);
        assert_eq!(detector.observe(84.999, 85.0), Transition::Resolved);
        assert!(!detector.currently_above());
    }

    #[test]
    fn breach_detector_boundary_value_counts_as_breach() {
        // Spec wording is "exceeds 85 %". Triggering on `>= threshold`
        // matches both phrasings and gives operators a heads-up half a
        // sample earlier than a strict `>` interpretation would.
        let mut detector = BreachDetector::new();
        assert_eq!(detector.observe(85.0, 85.0), Transition::Triggered);
    }

    #[test]
    fn breach_detector_requires_drop_then_recross_to_re_alert() {
        let mut detector = BreachDetector::new();
        detector.observe(90.0, 85.0);
        detector.observe(40.0, 85.0);
        assert_eq!(detector.observe(92.0, 85.0), Transition::Triggered);
        // Same-side samples stay quiet.
        assert_eq!(detector.observe(95.0, 85.0), Transition::NoChange);
        // Drop.
        assert_eq!(detector.observe(20.0, 85.0), Transition::Resolved);
        // Recross.
        assert_eq!(detector.observe(96.0, 85.0), Transition::Triggered);
    }

    #[test]
    fn evaluator_tracks_cpu_and_memory_independently() {
        let mut evaluator = SysAlarmEvaluator::new();
        assert_eq!(evaluator.observe_cpu(95.0, 85.0), Transition::Triggered);
        assert_eq!(evaluator.observe_memory(95.0, 85.0), Transition::Triggered);
        assert!(evaluator.is_cpu_above());
        assert!(evaluator.is_memory_above());

        assert_eq!(evaluator.observe_cpu(40.0, 85.0), Transition::Resolved);
        assert!(!evaluator.is_cpu_above());
        // Memory still saturated — no second spike.
        assert_eq!(evaluator.observe_memory(96.0, 85.0), Transition::NoChange);
        assert!(evaluator.is_memory_above());
    }

    #[test]
    fn evaluator_classifies_steady_high_load_as_no_change() {
        let mut evaluator = SysAlarmEvaluator::new();
        evaluator.observe_cpu(86.0, 85.0);
        for _ in 0..100 {
            assert_eq!(evaluator.observe_cpu(87.0, 85.0), Transition::NoChange);
        }
        assert_eq!(evaluator.observe_cpu(50.0, 85.0), Transition::Resolved);
    }

    #[test]
    fn alarm_event_serializes_with_expected_keys() {
        let event = SysAlarmEvent {
            event: "threshold_breach",
            resource: "cpu",
            value_percent: 92.5,
            threshold_percent: 85.0,
            process_memory_bytes: 123_456_789,
            total_memory_bytes: 16_000_000_000,
            pid: 4242,
            node_id: Some("node-a".to_string()),
            timestamp: chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
        };
        let json = serde_json::to_string(&event).expect("serialize");
        assert!(json.contains("\"event\":\"threshold_breach\""));
        assert!(json.contains("\"resource\":\"cpu\""));
        assert!(json.contains("\"value_percent\":92.5"));
        assert!(json.contains("\"pid\":4242"));
        assert!(json.contains("\"node_id\":\"node-a\""));
    }

    // ── Config-from-env tests ──────────────────────────────────────────
    //
    // Modifying process-global env from a parallel test process is
    // unsafe in Rust 1.74+ so we serialise access via a static Mutex.
    // The lock is leaked on purpose — the unit tests in this binary run
    // in the same allocator and cleanup is unnecessary.

    use std::sync::Mutex;
    // Use OnceLock-free approach: a `static` `Mutex` initialised at
    // compile time is available since Rust 1.63.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// RAII guard that restores every `SOROSCOPE_ALARM_*` env value on
    /// drop so a panic mid-test can't pollute later suites.
    struct EnvRestore {
        prev: Vec<(&'static str, Option<String>)>,
    }

    impl EnvRestore {
        fn capture(names: &'static [&'static str]) -> Self {
            let prev = names
                .iter()
                .map(|name| (*name, std::env::var(name).ok()))
                .collect::<Vec<_>>();
            Self { prev }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for (name, value) in self.prev.drain(..) {
                match value {
                    Some(v) => std::env::set_var(name, v),
                    None => std::env::remove_var(name),
                }
            }
        }
    }

    #[test]
    fn config_from_env_reads_threshold_interval_and_webhook() {
        let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _restore =
            EnvRestore::capture(&[ENV_THRESHOLD, ENV_INTERVAL, ENV_WEBHOOK, ENV_DISABLE]);

        std::env::set_var(ENV_THRESHOLD, "92.5");
        std::env::set_var(ENV_INTERVAL, "15");
        std::env::set_var(ENV_WEBHOOK, "https://example.test/hook");
        std::env::remove_var(ENV_DISABLE);

        let cfg = SysAlarmConfig::from_env();
        assert!((cfg.threshold_percent - 92.5).abs() < f64::EPSILON);
        assert_eq!(cfg.interval, Duration::from_secs(15));
        assert_eq!(
            cfg.webhook_url.as_deref(),
            Some("https://example.test/hook")
        );
        assert!(cfg.enabled);
    }

    #[test]
    fn config_from_env_disables_when_env_says_so() {
        let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _restore = EnvRestore::capture(&[ENV_DISABLE]);

        std::env::set_var(ENV_DISABLE, "1");
        let cfg = SysAlarmConfig::from_env();
        assert!(!cfg.enabled);

        std::env::set_var(ENV_DISABLE, "true");
        let cfg = SysAlarmConfig::from_env();
        assert!(!cfg.enabled);

        std::env::set_var(ENV_DISABLE, "no");
        let cfg = SysAlarmConfig::from_env();
        assert!(cfg.enabled);
    }

    #[test]
    fn config_from_env_rejects_out_of_range_threshold() {
        let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _restore = EnvRestore::capture(&[ENV_THRESHOLD]);

        std::env::set_var(ENV_THRESHOLD, "150");
        let cfg = SysAlarmConfig::from_env();
        assert!((cfg.threshold_percent - DEFAULT_THRESHOLD_PERCENT).abs() < f64::EPSILON);

        std::env::set_var(ENV_THRESHOLD, "-5");
        let cfg = SysAlarmConfig::from_env();
        assert!((cfg.threshold_percent - DEFAULT_THRESHOLD_PERCENT).abs() < f64::EPSILON);

        std::env::set_var(ENV_THRESHOLD, "garbage");
        let cfg = SysAlarmConfig::from_env();
        assert!((cfg.threshold_percent - DEFAULT_THRESHOLD_PERCENT).abs() < f64::EPSILON);
    }

    #[test]
    fn config_from_env_rejects_zero_or_garbage_interval() {
        let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _restore = EnvRestore::capture(&[ENV_INTERVAL]);

        std::env::set_var(ENV_INTERVAL, "0");
        let cfg = SysAlarmConfig::from_env();
        assert_eq!(cfg.interval, Duration::from_secs(DEFAULT_INTERVAL_SECS));

        std::env::set_var(ENV_INTERVAL, "garbage");
        let cfg = SysAlarmConfig::from_env();
        assert_eq!(cfg.interval, Duration::from_secs(DEFAULT_INTERVAL_SECS));

        std::env::set_var(ENV_INTERVAL, "30");
        let cfg = SysAlarmConfig::from_env();
        assert_eq!(cfg.interval, Duration::from_secs(30));
    }

    #[test]
    fn config_from_env_treats_empty_webhook_as_none() {
        let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _restore = EnvRestore::capture(&[ENV_WEBHOOK]);

        std::env::set_var(ENV_WEBHOOK, "");
        let cfg = SysAlarmConfig::from_env();
        assert!(cfg.webhook_url.is_none());

        std::env::set_var(ENV_WEBHOOK, "    ");
        let cfg = SysAlarmConfig::from_env();
        assert!(cfg.webhook_url.is_none());

        std::env::remove_var(ENV_WEBHOOK);
        let cfg = SysAlarmConfig::from_env();
        assert!(cfg.webhook_url.is_none());
    }


    // ── Resource exhaustion alarms (issue #1411) ────────────────────────────

    struct MockStats(Vec<ResourceStats>);

    impl SystemStatsSource for MockStats {
        fn sample(&mut self) -> ResourceStats {
            if self.0.len() > 1 {
                self.0.remove(0)
            } else {
                self.0[0]
            }
        }
    }

    fn stats(memory_pct: u64, fd_pct: u64, disk_used_pct: u64) -> ResourceStats {
        ResourceStats {
            process_rss_bytes: memory_pct,
            total_memory_bytes: 100,
            open_fds: Some(fd_pct),
            fd_limit: Some(100),
            disk_free_bytes: Some(100 - disk_used_pct),
            disk_total_bytes: Some(100),
        }
    }

    fn leaked_state() -> &'static ResourceAlarmState {
        Box::leak(Box::new(ResourceAlarmState::default()))
    }

    #[test]
    fn sys_alarms_severity_thresholds_are_80_and_92_percent() {
        assert_eq!(AlarmSeverity::from_percent(79.9), AlarmSeverity::Normal);
        assert_eq!(AlarmSeverity::from_percent(80.0), AlarmSeverity::Warning);
        assert_eq!(AlarmSeverity::from_percent(91.9), AlarmSeverity::Warning);
        assert_eq!(AlarmSeverity::from_percent(92.0), AlarmSeverity::Critical);
    }

    #[test]
    fn sys_alarms_mock_stats_drive_each_resource_severity() {
        let state = leaked_state();
        let mut monitor =
            ResourceAlarmMonitor::new(MockStats(vec![stats(50, 85, 95)]), state, Duration::from_secs(1));

        monitor.poll_once();

        assert_eq!(state.severity(AlarmResource::Memory), AlarmSeverity::Normal);
        assert_eq!(state.severity(AlarmResource::FileDescriptors), AlarmSeverity::Warning);
        assert_eq!(state.severity(AlarmResource::Disk), AlarmSeverity::Critical);
        assert_eq!(state.critical_resources(), vec![AlarmResource::Disk]);
        assert!(state.should_pause_background_jobs());
        assert_eq!(state.snapshot()["disk"], "critical");
    }

    #[test]
    fn sys_alarms_critical_fds_alone_do_not_pause_background_jobs() {
        let state = leaked_state();
        let mut monitor =
            ResourceAlarmMonitor::new(MockStats(vec![stats(10, 99, 10)]), state, Duration::from_secs(1));

        monitor.poll_once();

        assert_eq!(state.critical_resources(), vec![AlarmResource::FileDescriptors]);
        assert!(!state.should_pause_background_jobs());
    }

    #[test]
    fn sys_alarms_clear_when_usage_recovers() {
        let state = leaked_state();
        let mut monitor = ResourceAlarmMonitor::new(
            MockStats(vec![stats(95, 10, 10), stats(40, 10, 10)]),
            state,
            Duration::from_secs(1),
        );

        monitor.poll_once();
        assert!(state.should_pause_background_jobs());

        monitor.poll_once();
        assert_eq!(state.severity(AlarmResource::Memory), AlarmSeverity::Normal);
        assert!(state.critical_resources().is_empty());
    }

    #[test]
    fn sys_alarms_unreported_resources_stay_normal() {
        let state = leaked_state();
        let unknown = ResourceStats {
            process_rss_bytes: 10,
            total_memory_bytes: 100,
            ..ResourceStats::default()
        };
        let mut monitor = ResourceAlarmMonitor::new(MockStats(vec![unknown]), state, Duration::from_secs(1));

        monitor.poll_once();

        assert_eq!(state.severity(AlarmResource::FileDescriptors), AlarmSeverity::Normal);
        assert_eq!(state.severity(AlarmResource::Disk), AlarmSeverity::Normal);
    }

    #[test]
    fn sys_alarms_publish_severity_gauges() {
        let gauge = crate::metrics::resource_alarm_severity_gauge().expect("gauge");
        let mut monitor = ResourceAlarmMonitor::new(
            MockStats(vec![stats(85, 95, 10)]),
            leaked_state(),
            Duration::from_secs(1),
        )
        .with_gauge(gauge.clone());

        monitor.poll_once();

        assert_eq!(gauge.with_label_values(&["memory"]).get(), 1);
        assert_eq!(gauge.with_label_values(&["file_descriptors"]).get(), 2);
        assert_eq!(gauge.with_label_values(&["disk"]).get(), 0);
    }
}
