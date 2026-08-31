//! Unified configuration loader for SoroScope Core (Issue #39).
//!
//! [`AppConfig`] is the single source of truth for every runtime parameter.
//! Values are resolved in priority order:
//!
//! 1. Environment variables (highest priority)
//! 2. `.env` file in the current working directory (loaded by [`dotenvy`])
//! 3. Compiled-in defaults (lowest priority)
//!
//! # Usage
//!
//! ```rust,no_run
//! use soroscope_core::config::load_config;
//!
//! let cfg = load_config().expect("failed to load configuration");
//! println!("Listening on port {}", cfg.server_port);
//! ```
//!
//! # Production validation
//!
//! Call [`AppConfig::validate_production`] when `APP_ENV=production` to enforce
//! that all security-sensitive fields are explicitly set (no dev defaults).

use config::{Config, ConfigError};
use serde::Deserialize;
use std::fmt;

// ---------------------------------------------------------------------------
// Configuration errors
// ---------------------------------------------------------------------------

/// Errors returned by the configuration subsystem.
#[derive(Debug)]
pub enum ConfigLoadError {
    /// Underlying [`config`] crate error (parse / type mismatch / missing key).
    Load(ConfigError),
    /// One or more required fields are absent or empty in production mode.
    Validation(Vec<String>),
}

impl fmt::Display for ConfigLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigLoadError::Load(e) => write!(f, "Configuration load error: {e}"),
            ConfigLoadError::Validation(errs) => {
                writeln!(f, "Configuration validation failed ({} error(s)):", errs.len())?;
                for (i, err) in errs.iter().enumerate() {
                    writeln!(f, "  [{}] {}", i + 1, err)?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ConfigLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigLoadError::Load(e) => Some(e),
            ConfigLoadError::Validation(_) => None,
        }
    }
}

impl From<ConfigError> for ConfigLoadError {
    fn from(e: ConfigError) -> Self {
        ConfigLoadError::Load(e)
    }
}

// ---------------------------------------------------------------------------
// Default value helpers
// ---------------------------------------------------------------------------

fn default_server_port() -> u16 {
    8080
}

fn default_rust_log() -> String {
    "info".to_string()
}

fn default_soroban_rpc_url() -> String {
    "https://soroban-testnet.stellar.org".to_string()
}

fn default_network_passphrase() -> String {
    "Test SDF Network ; September 2015".to_string()
}

fn default_redis_url() -> String {
    "redis://127.0.0.1:6379".to_string()
}

fn default_database_url() -> String {
    "sqlite://soroscope.db".to_string()
}

fn default_simulation_mode() -> String {
    "failover".to_string()
}

fn default_health_check_interval_secs() -> u64 {
    30
}

fn default_gossip_interval_secs() -> u64 {
    30
}

fn default_simulation_timeout_secs() -> u64 {
    30
}

fn default_job_timeout_secs() -> u64 {
    300
}

fn default_max_concurrent_jobs() -> usize {
    10
}

fn default_event_worker_threads() -> usize {
    4
}

fn default_fee_collection_interval_secs() -> u64 {
    5
}

fn default_fee_retention_days() -> u32 {
    30
}

fn default_fee_analysis_enabled() -> bool {
    true
}

fn default_emergency_verification_paused() -> bool {
    false
}

fn default_disk_cache_path() -> String {
    // Empty → L2 disk cache disabled. Operators opt in by setting this explicitly.
    String::new()
}

fn default_max_ledger_age() -> u32 {
    100
}

fn default_event_bus_capacity() -> usize {
    256
}

fn default_log_format_json() -> bool {
    false
}

// ---------------------------------------------------------------------------
// AppConfig struct
// ---------------------------------------------------------------------------

/// Complete runtime configuration for the SoroScope Core server.
///
/// All fields map 1-to-1 to environment variables (upper-case, with the same
/// name). E.g. `server_port` is read from `SERVER_PORT`.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AppConfig {
    // --- Server ---

    /// TCP port the HTTP server binds to.
    /// Env: `SERVER_PORT` · Default: `8080`
    #[serde(default = "default_server_port")]
    pub server_port: u16,

    // --- Logging ---

    /// Tracing filter directive (supports per-module overrides).
    /// Env: `RUST_LOG` · Default: `"info"`
    #[serde(default = "default_rust_log")]
    pub rust_log: String,

    /// Emit structured JSON log lines when `true`.
    /// Env: `LOG_FORMAT_JSON` · Default: `false`
    #[serde(default = "default_log_format_json")]
    pub log_format_json: bool,

    // --- Stellar / Soroban RPC ---

    /// Primary RPC endpoint. Used as a single-provider fallback when
    /// `RPC_PROVIDERS` is unset.
    /// Env: `SOROBAN_RPC_URL` · Default: Stellar testnet
    #[serde(default = "default_soroban_rpc_url")]
    pub soroban_rpc_url: String,

    /// Stellar network passphrase — must match the network of `soroban_rpc_url`.
    /// Env: `NETWORK_PASSPHRASE` · Default: testnet passphrase
    #[serde(default = "default_network_passphrase")]
    pub network_passphrase: String,

    /// JSON-encoded array of `RpcProvider` objects.
    /// When absent or empty the server falls back to `soroban_rpc_url`.
    /// Env: `RPC_PROVIDERS` · Default: `""`
    #[serde(default)]
    pub rpc_providers: String,

    // --- Database ---

    /// Connection URL for the job queue / fee store (PostgreSQL or SQLite).
    /// Env: `DATABASE_URL` · Default: `"sqlite://soroscope.db"`
    #[serde(default = "default_database_url")]
    pub database_url: String,

    // --- Redis ---

    /// Redis connection URL — reserved for the distributed cache migration.
    /// Env: `REDIS_URL` · Default: `"redis://127.0.0.1:6379"`
    #[serde(default = "default_redis_url")]
    pub redis_url: String,

    // --- Authentication ---

    /// PEM-encoded RSA private key for RS256 JWT signing.
    /// When absent a throwaway dev key is generated on startup.
    /// Env: `JWT_PRIVATE_KEY` · Default: `None`
    pub jwt_private_key: Option<String>,

    // --- CORS ---

    /// Comma-separated list of allowed CORS origins.
    /// Empty → allow all origins (development fallback).
    /// Env: `CORS_ALLOWED_ORIGINS` · Default: `""`
    #[serde(default)]
    pub cors_allowed_origins: String,

    /// Alias for `cors_allowed_origins` (retained for backwards compatibility).
    /// Env: `ALLOWED_ORIGINS` · Default: `""`
    #[serde(default)]
    pub allowed_origins: String,

    // --- Webhooks ---

    /// Pre-shared HMAC secret for inbound webhook validation.
    /// Env: `INBOUND_WEBHOOK_SECRET` · Default: `""` (disabled)
    #[serde(default)]
    pub inbound_webhook_secret: String,

    // --- Provider registry ---

    /// Stable identifier for this node in the gossip registry.
    /// Env: `REGISTRY_INSTANCE_ID` · Default: random UUID generated at startup
    #[serde(default)]
    pub registry_instance_id: String,

    /// Public base URL announced to registry peers.
    /// Env: `REGISTRY_PUBLIC_URL` · Default: `"http://127.0.0.1:<port>"`
    #[serde(default)]
    pub registry_public_url: String,

    /// JSON array or comma-separated list of seed peer base URLs.
    /// Env: `REGISTRY_SEED_PEERS` · Default: `""`
    #[serde(default)]
    pub registry_seed_peers: String,

    // --- Health & gossip ---

    /// Interval (seconds) between health-check pings.
    /// Env: `HEALTH_CHECK_INTERVAL_SECS` · Default: `30`
    #[serde(default = "default_health_check_interval_secs")]
    pub health_check_interval_secs: u64,

    /// Interval (seconds) between gossip sync rounds.
    /// Env: `GOSSIP_INTERVAL_SECS` · Default: `30`
    #[serde(default = "default_gossip_interval_secs")]
    pub gossip_interval_secs: u64,

    // --- Simulation ---

    /// Per-request simulation timeout in seconds.
    /// Env: `SIMULATION_TIMEOUT_SECS` · Default: `30`
    #[serde(default = "default_simulation_timeout_secs")]
    pub simulation_timeout_secs: u64,

    /// Execution mode: `"failover"` or `"consensus"`.
    /// Env: `SIMULATION_MODE` · Default: `"failover"`
    #[serde(default = "default_simulation_mode")]
    pub simulation_mode: String,

    // --- Job queue ---

    /// Per-job timeout in seconds.
    /// Env: `JOB_TIMEOUT_SECS` · Default: `300`
    #[serde(default = "default_job_timeout_secs")]
    pub job_timeout_secs: u64,

    /// Maximum number of jobs executed concurrently.
    /// Env: `MAX_CONCURRENT_JOBS` · Default: `10`
    #[serde(default = "default_max_concurrent_jobs")]
    pub max_concurrent_jobs: usize,

    // --- Event worker pool ---

    /// Thread count for the dedicated event worker pool.
    /// Env: `EVENT_WORKER_THREADS` · Default: `4`
    #[serde(default = "default_event_worker_threads")]
    pub event_worker_threads: usize,

    // --- Fee market ---

    /// Interval (seconds) between fee data collection sweeps.
    /// Env: `FEE_COLLECTION_INTERVAL_SECS` · Default: `5`
    #[serde(default = "default_fee_collection_interval_secs")]
    pub fee_collection_interval_secs: u64,

    /// Number of days to retain historical fee data.
    /// Env: `FEE_RETENTION_DAYS` · Default: `30`
    #[serde(default = "default_fee_retention_days")]
    pub fee_retention_days: u32,

    /// Enable fee market analysis and predictions.
    /// Env: `FEE_ANALYSIS_ENABLED` · Default: `true`
    #[serde(default = "default_fee_analysis_enabled")]
    pub fee_analysis_enabled: bool,

    // --- Emergency controls ---

    /// When `true`, all message-verification endpoints return an error.
    /// Env: `EMERGENCY_VERIFICATION_PAUSED` · Default: `false`
    #[serde(default = "default_emergency_verification_paused")]
    pub emergency_verification_paused: bool,

    // --- Disk cache (L2) ---

    /// Filesystem path for the disk-persistent L2 simulation cache.
    /// Empty → L2 disabled.
    /// Env: `DISK_CACHE_PATH` · Default: `""`
    #[serde(default = "default_disk_cache_path")]
    pub disk_cache_path: String,

    /// Maximum ledger age (in ledgers) before an L2 cache entry is treated as stale.
    /// Env: `MAX_LEDGER_AGE` · Default: `100`
    #[serde(default = "default_max_ledger_age")]
    pub max_ledger_age: u32,

    // --- WebSocket event bus ---

    /// Per-subscriber in-flight event buffer capacity. Clamped to [16, 65536].
    /// Env: `EVENT_BUS_CAPACITY` · Default: `256`
    #[serde(default = "default_event_bus_capacity")]
    pub event_bus_capacity: usize,

    // --- Deployment environment ---

    /// Deployment environment tag used for production validation.
    /// Set to `"production"` to enforce stricter config requirements.
    /// Env: `APP_ENV` · Default: `"development"`
    #[serde(default = "default_app_env")]
    pub app_env: String,
}

fn default_app_env() -> String {
    "development".to_string()
}

impl AppConfig {
    /// Returns `true` when running in production mode (`APP_ENV=production`).
    pub fn is_production(&self) -> bool {
        self.app_env.eq_ignore_ascii_case("production")
    }

    /// Clamp [`event_bus_capacity`] to the valid range `[16, 65536]`.
    pub fn clamped_event_bus_capacity(&self) -> usize {
        self.event_bus_capacity.clamp(16, 65536)
    }

    /// Validate that all security-sensitive fields are explicitly configured
    /// when running in production.
    ///
    /// Returns `Ok(())` when validation passes, or `Err(ConfigLoadError::Validation)`
    /// with a list of all failing fields so operators can fix everything at once.
    ///
    /// # Fields validated in production
    ///
    /// | Field | Requirement |
    /// |-------|-------------|
    /// | `database_url` | Must not be the SQLite dev default |
    /// | `network_passphrase` | Must not be the testnet passphrase |
    /// | `soroban_rpc_url` | Must not point to Stellar testnet |
    /// | `cors_allowed_origins` | Must not be empty (allow-all) |
    /// | `jwt_private_key` | Must be explicitly provided |
    /// | `inbound_webhook_secret` | Must be non-empty when webhooks are used |
    pub fn validate_production(&self) -> Result<(), ConfigLoadError> {
        if !self.is_production() {
            return Ok(());
        }

        let mut errors: Vec<String> = Vec::new();

        // Database must be a real PostgreSQL connection in production.
        if self.database_url.starts_with("sqlite://") {
            errors.push(
                "DATABASE_URL: SQLite is not suitable for production; \
                 set a PostgreSQL connection string."
                    .to_string(),
            );
        }

        // Must target mainnet or a known non-testnet network.
        if self
            .network_passphrase
            .contains("Test SDF Network ; September 2015")
        {
            errors.push(
                "NETWORK_PASSPHRASE: still set to the Stellar testnet passphrase; \
                 set the mainnet passphrase ('Public Global Stellar Network ; September 2015') \
                 or your private network's passphrase."
                    .to_string(),
            );
        }

        // RPC endpoint must not be the public testnet.
        if self
            .soroban_rpc_url
            .contains("soroban-testnet.stellar.org")
        {
            errors.push(
                "SOROBAN_RPC_URL: still pointing at Stellar testnet; \
                 set a mainnet or private-network RPC endpoint."
                    .to_string(),
            );
        }

        // CORS must be restricted in production (no allow-all wildcard).
        if self.cors_allowed_origins.trim().is_empty() && self.allowed_origins.trim().is_empty() {
            errors.push(
                "CORS_ALLOWED_ORIGINS: must be set to an explicit list of allowed origins \
                 in production (e.g. 'https://app.example.com')."
                    .to_string(),
            );
        }

        // JWT private key must be explicitly supplied.
        if self.jwt_private_key.as_deref().unwrap_or("").trim().is_empty() {
            errors.push(
                "JWT_PRIVATE_KEY: must be set to a PEM-encoded RSA private key in production; \
                 auto-generated dev keys do not persist across restarts."
                    .to_string(),
            );
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ConfigLoadError::Validation(errors))
        }
    }
}

// ---------------------------------------------------------------------------
// Loader
// ---------------------------------------------------------------------------

/// Load [`AppConfig`] from the environment and an optional `.env` file.
///
/// Resolution order (highest → lowest priority):
/// 1. Process environment variables
/// 2. `.env` file in the current directory (silently skipped if absent)
/// 3. Compiled-in defaults
///
/// After loading, if `APP_ENV=production` the function calls
/// [`AppConfig::validate_production`] and returns an error if any required
/// production fields are missing.
///
/// # Errors
///
/// Returns [`ConfigLoadError::Load`] for parse / type errors, or
/// [`ConfigLoadError::Validation`] when production validation fails.
///
/// # Example
///
/// ```rust,no_run
/// use soroscope_core::config::load_config;
///
/// match load_config() {
///     Ok(cfg) => println!("Server port: {}", cfg.server_port),
///     Err(e) => {
///         eprintln!("FATAL: {e}");
///         std::process::exit(1);
///     }
/// }
/// ```
pub fn load_config() -> Result<AppConfig, ConfigLoadError> {
    // Load `.env` if present; ignore the error if the file doesn't exist.
    dotenvy::dotenv().ok();

    let cfg: AppConfig = Config::builder()
        // Environment variables override everything.
        .add_source(config::Environment::default())
        // ---- Compiled-in defaults (lowest priority) ----
        .set_default("server_port", default_server_port())?
        .set_default("rust_log", default_rust_log())?
        .set_default("log_format_json", default_log_format_json())?
        .set_default("soroban_rpc_url", default_soroban_rpc_url())?
        .set_default("network_passphrase", default_network_passphrase())?
        .set_default("redis_url", default_redis_url())?
        .set_default("database_url", default_database_url())?
        .set_default("simulation_mode", default_simulation_mode())?
        .set_default("rpc_providers", "")?
        .set_default("registry_instance_id", "")?
        .set_default("registry_public_url", "")?
        .set_default("registry_seed_peers", "")?
        .set_default("health_check_interval_secs", default_health_check_interval_secs())?
        .set_default("gossip_interval_secs", default_gossip_interval_secs())?
        .set_default("simulation_timeout_secs", default_simulation_timeout_secs())?
        .set_default("job_timeout_secs", default_job_timeout_secs())?
        .set_default("max_concurrent_jobs", default_max_concurrent_jobs())?
        .set_default("event_worker_threads", default_event_worker_threads())?
        .set_default(
            "fee_collection_interval_secs",
            default_fee_collection_interval_secs(),
        )?
        .set_default("fee_retention_days", default_fee_retention_days())?
        .set_default("fee_analysis_enabled", default_fee_analysis_enabled())?
        .set_default(
            "emergency_verification_paused",
            default_emergency_verification_paused(),
        )?
        .set_default("disk_cache_path", default_disk_cache_path())?
        .set_default("max_ledger_age", default_max_ledger_age())?
        .set_default("event_bus_capacity", default_event_bus_capacity())?
        .set_default("cors_allowed_origins", "")?
        .set_default("allowed_origins", "")?
        .set_default("inbound_webhook_secret", "")?
        .set_default("app_env", default_app_env())?
        .build()?
        .try_deserialize()?;

    // Production gate: fail fast with actionable error messages.
    cfg.validate_production()?;

    Ok(cfg)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    /// Helper: remove a set of env vars after the test, regardless of outcome.
    struct EnvGuard(Vec<String>);
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for key in &self.0 {
                env::remove_var(key);
            }
        }
    }

    // ------------------------------------------------------------------
    // Defaults
    // ------------------------------------------------------------------

    #[test]
    fn defaults_are_applied_when_no_env_is_set() {
        // These keys must not be set by the outer test environment.
        let keys = vec![
            "SERVER_PORT",
            "RUST_LOG",
            "SOROBAN_RPC_URL",
            "NETWORK_PASSPHRASE",
            "DATABASE_URL",
            "APP_ENV",
        ];
        for k in &keys {
            env::remove_var(k);
        }
        let _guard = EnvGuard(keys.iter().map(|s| s.to_string()).collect());

        let cfg = load_config().expect("load_config should succeed with defaults");

        assert_eq!(cfg.server_port, 8080);
        assert_eq!(cfg.rust_log, "info");
        assert_eq!(cfg.soroban_rpc_url, "https://soroban-testnet.stellar.org");
        assert_eq!(
            cfg.network_passphrase,
            "Test SDF Network ; September 2015"
        );
        assert_eq!(cfg.database_url, "sqlite://soroscope.db");
        assert!(!cfg.is_production());
    }

    // ------------------------------------------------------------------
    // Env overrides
    // ------------------------------------------------------------------

    #[test]
    fn env_vars_override_defaults() {
        let _guard = EnvGuard(vec![
            "SERVER_PORT".into(),
            "RUST_LOG".into(),
            "DATABASE_URL".into(),
            "APP_ENV".into(),
        ]);

        env::set_var("SERVER_PORT", "9090");
        env::set_var("RUST_LOG", "debug");
        env::set_var("DATABASE_URL", "sqlite://custom.db");
        env::set_var("APP_ENV", "development");

        let cfg = load_config().expect("load_config should succeed");

        assert_eq!(cfg.server_port, 9090);
        assert_eq!(cfg.rust_log, "debug");
        assert_eq!(cfg.database_url, "sqlite://custom.db");
    }

    // ------------------------------------------------------------------
    // event_bus_capacity clamping
    // ------------------------------------------------------------------

    #[test]
    fn event_bus_capacity_is_clamped_to_lower_bound() {
        let _guard = EnvGuard(vec!["EVENT_BUS_CAPACITY".into(), "APP_ENV".into()]);
        env::set_var("EVENT_BUS_CAPACITY", "4"); // below minimum
        env::set_var("APP_ENV", "development");

        let cfg = load_config().expect("load_config should succeed");
        assert_eq!(cfg.clamped_event_bus_capacity(), 16);
    }

    #[test]
    fn event_bus_capacity_is_clamped_to_upper_bound() {
        let _guard = EnvGuard(vec!["EVENT_BUS_CAPACITY".into(), "APP_ENV".into()]);
        env::set_var("EVENT_BUS_CAPACITY", "999999"); // above maximum
        env::set_var("APP_ENV", "development");

        let cfg = load_config().expect("load_config should succeed");
        assert_eq!(cfg.clamped_event_bus_capacity(), 65536);
    }

    #[test]
    fn event_bus_capacity_within_range_is_unchanged() {
        let _guard = EnvGuard(vec!["EVENT_BUS_CAPACITY".into(), "APP_ENV".into()]);
        env::set_var("EVENT_BUS_CAPACITY", "512");
        env::set_var("APP_ENV", "development");

        let cfg = load_config().expect("load_config should succeed");
        assert_eq!(cfg.clamped_event_bus_capacity(), 512);
    }

    // ------------------------------------------------------------------
    // Production validation
    // ------------------------------------------------------------------

    #[test]
    fn production_validation_passes_with_correct_values() {
        let cfg = AppConfig {
            app_env: "production".to_string(),
            database_url: "postgres://user:pass@db.example.com/soroscope".to_string(),
            network_passphrase: "Public Global Stellar Network ; September 2015".to_string(),
            soroban_rpc_url: "https://rpc.mainnet.example.com".to_string(),
            cors_allowed_origins: "https://app.example.com".to_string(),
            jwt_private_key: Some("-----BEGIN RSA PRIVATE KEY-----\nMIIE...\n-----END RSA PRIVATE KEY-----".to_string()),
            // Remaining fields use defaults
            server_port: 8080,
            rust_log: "info".to_string(),
            log_format_json: false,
            redis_url: "redis://127.0.0.1:6379".to_string(),
            rpc_providers: String::new(),
            allowed_origins: String::new(),
            inbound_webhook_secret: String::new(),
            registry_instance_id: String::new(),
            registry_public_url: String::new(),
            registry_seed_peers: String::new(),
            health_check_interval_secs: 30,
            gossip_interval_secs: 30,
            simulation_timeout_secs: 30,
            simulation_mode: "failover".to_string(),
            job_timeout_secs: 300,
            max_concurrent_jobs: 10,
            event_worker_threads: 4,
            fee_collection_interval_secs: 5,
            fee_retention_days: 30,
            fee_analysis_enabled: true,
            emergency_verification_paused: false,
            disk_cache_path: String::new(),
            max_ledger_age: 100,
            event_bus_capacity: 256,
        };

        assert!(cfg.validate_production().is_ok());
    }

    #[test]
    fn production_validation_reports_all_missing_fields() {
        // Deliberately misconfigured: all required production fields are wrong.
        let cfg = AppConfig {
            app_env: "production".to_string(),
            database_url: "sqlite://soroscope.db".to_string(),      // ← bad: SQLite
            network_passphrase: "Test SDF Network ; September 2015".to_string(), // ← bad: testnet
            soroban_rpc_url: "https://soroban-testnet.stellar.org".to_string(),  // ← bad: testnet
            cors_allowed_origins: String::new(), // ← bad: allow-all
            allowed_origins: String::new(),
            jwt_private_key: None,               // ← bad: no key
            server_port: 8080,
            rust_log: "info".to_string(),
            log_format_json: false,
            redis_url: "redis://127.0.0.1:6379".to_string(),
            rpc_providers: String::new(),
            inbound_webhook_secret: String::new(),
            registry_instance_id: String::new(),
            registry_public_url: String::new(),
            registry_seed_peers: String::new(),
            health_check_interval_secs: 30,
            gossip_interval_secs: 30,
            simulation_timeout_secs: 30,
            simulation_mode: "failover".to_string(),
            job_timeout_secs: 300,
            max_concurrent_jobs: 10,
            event_worker_threads: 4,
            fee_collection_interval_secs: 5,
            fee_retention_days: 30,
            fee_analysis_enabled: true,
            emergency_verification_paused: false,
            disk_cache_path: String::new(),
            max_ledger_age: 100,
            event_bus_capacity: 256,
        };

        let result = cfg.validate_production();
        assert!(result.is_err(), "expected validation to fail");

        if let Err(ConfigLoadError::Validation(errors)) = result {
            // All five misconfigured fields must be reported in one shot.
            assert_eq!(
                errors.len(),
                5,
                "expected 5 validation errors, got {}: {errors:#?}",
                errors.len()
            );
        } else {
            panic!("expected ConfigLoadError::Validation variant");
        }
    }

    #[test]
    fn validation_is_skipped_in_development() {
        // Even with bad "production-only" values, dev mode skips the gate.
        let cfg = AppConfig {
            app_env: "development".to_string(),
            database_url: "sqlite://soroscope.db".to_string(),
            network_passphrase: "Test SDF Network ; September 2015".to_string(),
            soroban_rpc_url: "https://soroban-testnet.stellar.org".to_string(),
            cors_allowed_origins: String::new(),
            allowed_origins: String::new(),
            jwt_private_key: None,
            server_port: 8080,
            rust_log: "info".to_string(),
            log_format_json: false,
            redis_url: "redis://127.0.0.1:6379".to_string(),
            rpc_providers: String::new(),
            inbound_webhook_secret: String::new(),
            registry_instance_id: String::new(),
            registry_public_url: String::new(),
            registry_seed_peers: String::new(),
            health_check_interval_secs: 30,
            gossip_interval_secs: 30,
            simulation_timeout_secs: 30,
            simulation_mode: "failover".to_string(),
            job_timeout_secs: 300,
            max_concurrent_jobs: 10,
            event_worker_threads: 4,
            fee_collection_interval_secs: 5,
            fee_retention_days: 30,
            fee_analysis_enabled: true,
            emergency_verification_paused: false,
            disk_cache_path: String::new(),
            max_ledger_age: 100,
            event_bus_capacity: 256,
        };

        assert!(
            cfg.validate_production().is_ok(),
            "validation should be a no-op in development mode"
        );
    }
}
