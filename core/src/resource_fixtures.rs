//! Versioned, network-free Soroban resource regression fixtures.

use crate::simulation::{SimulationError, SorobanResources};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use soroban_sdk::testutils::{Ledger as _, Snapshot};
use soroban_sdk::xdr::{Hash, LedgerEntry, LedgerKey, Limits, ReadXdr, ScAddress, ScVal};
use soroban_sdk::{Address, Env, Symbol, TryFromVal, Val};
use std::path::Path;
use stellar_strkey::Strkey;
use thiserror::Error;

pub const RESOURCE_FIXTURE_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceFixture {
    pub format_version: u32,
    pub name: String,
    /// Path to the WASM, relative to the fixture document.
    pub wasm_path: String,
    pub wasm_sha256: String,
    pub contract_id: String,
    pub function: String,
    /// One base64-encoded XDR `ScVal` per argument.
    pub args_xdr: Vec<String>,
    pub ledger_entries: Vec<LedgerEntryFixture>,
    pub protocol_snapshot: ProtocolSnapshot,
    pub expected: SorobanResources,
    pub tolerance: ResourceTolerance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LedgerEntryFixture {
    pub key_xdr: String,
    pub entry_xdr: String,
    #[serde(default)]
    pub live_until_ledger: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolSnapshot {
    pub protocol_version: u32,
    pub sequence_number: u32,
    pub timestamp: u64,
    /// 32-byte network ID, lowercase hexadecimal.
    pub network_id: String,
    pub base_reserve: u32,
    pub min_persistent_entry_ttl: u32,
    pub min_temp_entry_ttl: u32,
    pub max_entry_ttl: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceTolerance {
    /// Relative CPU tolerance in basis points (100 = 1%).
    pub cpu_relative_error_bps: u32,
    /// Relative RAM tolerance in basis points (100 = 1%).
    pub ram_relative_error_bps: u32,
}

#[derive(Debug, Clone)]
pub struct FixtureCheck {
    pub expected: SorobanResources,
    pub actual: SorobanResources,
}

#[derive(Debug, Error)]
pub enum FixtureError {
    #[error("fixture I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("fixture JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("fixture simulation failed: {0}")]
    Simulation(String),
    #[error("unsupported resource fixture format version {0}")]
    UnsupportedFormat(u32),
    #[error("fixture WASM SHA-256 mismatch: expected {expected}, actual {actual}")]
    WasmHashMismatch { expected: String, actual: String },
    #[error("fixture meter {meter} mismatch: expected {expected}, actual {actual}{tolerance}")]
    MeterMismatch {
        meter: &'static str,
        expected: u64,
        actual: u64,
        tolerance: String,
    },
}

/// Re-execute one fixture locally after validating its WASM digest.
///
/// The digest check intentionally happens before XDR parsing or host creation.
pub fn check_fixture(
    fixture: &ResourceFixture,
    wasm_bytes: &[u8],
) -> Result<FixtureCheck, FixtureError> {
    let expected = fixture.expected.clone();
    let actual = run_fixture_locally(fixture, wasm_bytes)?;
    check_fixture_resources(fixture, &expected, &actual)?;
    Ok(FixtureCheck { expected, actual })
}

/// Execute a fixture once locally after checking its format and WASM digest.
pub fn run_fixture_locally(
    fixture: &ResourceFixture,
    wasm_bytes: &[u8],
) -> Result<SorobanResources, FixtureError> {
    validate_format(fixture)?;
    let actual_hash = sha256_hex(wasm_bytes);
    if !fixture.wasm_sha256.eq_ignore_ascii_case(&actual_hash) {
        return Err(FixtureError::WasmHashMismatch {
            expected: fixture.wasm_sha256.clone(),
            actual: actual_hash,
        });
    }
    run_local(fixture, wasm_bytes)
}

/// Compare the recorded meters against one local execution.
pub fn check_fixture_resources(
    fixture: &ResourceFixture,
    expected: &SorobanResources,
    actual: &SorobanResources,
) -> Result<(), FixtureError> {
    compare_resources(expected, actual, &fixture.tolerance)
}

/// Record expected resources by executing locally; this function never contacts RPC.
pub fn record_fixture(
    fixture: &mut ResourceFixture,
    wasm_bytes: &[u8],
) -> Result<(), FixtureError> {
    validate_format(fixture)?;
    fixture.wasm_sha256 = sha256_hex(wasm_bytes);
    fixture.expected = run_local(fixture, wasm_bytes)?;
    Ok(())
}

/// Load and check a fixture whose `wasm_path` is relative to the fixture file.
pub fn check_fixture_file(path: impl AsRef<Path>) -> Result<FixtureCheck, FixtureError> {
    let path = path.as_ref();
    let fixture: ResourceFixture = serde_json::from_slice(&std::fs::read(path)?)?;
    let wasm_path = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(&fixture.wasm_path);
    let wasm = std::fs::read(wasm_path)?;
    check_fixture(&fixture, &wasm)
}

/// Record resources and write the fixture document as pretty JSON.
pub fn record_fixture_file(
    fixture: &mut ResourceFixture,
    wasm_bytes: &[u8],
    path: impl AsRef<Path>,
) -> Result<(), FixtureError> {
    record_fixture(fixture, wasm_bytes)?;
    std::fs::write(path, serde_json::to_vec_pretty(fixture)?)?;
    Ok(())
}

fn validate_format(fixture: &ResourceFixture) -> Result<(), FixtureError> {
    if fixture.format_version != RESOURCE_FIXTURE_FORMAT_VERSION {
        return Err(FixtureError::UnsupportedFormat(fixture.format_version));
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn compare_resources(
    expected: &SorobanResources,
    actual: &SorobanResources,
    tolerance: &ResourceTolerance,
) -> Result<(), FixtureError> {
    compare_meter(
        "cpu_instructions",
        expected.cpu_instructions,
        actual.cpu_instructions,
        Some(tolerance.cpu_relative_error_bps),
    )?;
    compare_meter(
        "ram_bytes",
        expected.ram_bytes,
        actual.ram_bytes,
        Some(tolerance.ram_relative_error_bps),
    )?;
    compare_meter(
        "ledger_read_bytes",
        expected.ledger_read_bytes,
        actual.ledger_read_bytes,
        None,
    )?;
    compare_meter(
        "ledger_write_bytes",
        expected.ledger_write_bytes,
        actual.ledger_write_bytes,
        None,
    )?;
    compare_meter(
        "transaction_size_bytes",
        expected.transaction_size_bytes,
        actual.transaction_size_bytes,
        None,
    )
}

fn compare_meter(
    meter: &'static str,
    expected: u64,
    actual: u64,
    tolerance_bps: Option<u32>,
) -> Result<(), FixtureError> {
    let tolerance = match tolerance_bps {
        Some(bps) if relative_error_bps(expected, actual) <= u64::from(bps) => return Ok(()),
        Some(bps) => format!(" (tolerance {bps} bps)"),
        None if expected == actual => return Ok(()),
        None => String::new(),
    };
    Err(FixtureError::MeterMismatch {
        meter,
        expected,
        actual,
        tolerance,
    })
}

fn relative_error_bps(expected: u64, actual: u64) -> u64 {
    if expected == 0 {
        return if actual == 0 { 0 } else { u64::MAX };
    }
    (u128::from(expected.abs_diff(actual)) * 10_000 / u128::from(expected)) as u64
}

fn run_local(
    fixture: &ResourceFixture,
    wasm_bytes: &[u8],
) -> Result<SorobanResources, FixtureError> {
    let mut snapshot = Snapshot::default();
    snapshot.ledger.protocol_version = fixture.protocol_snapshot.protocol_version;
    snapshot.ledger.sequence_number = fixture.protocol_snapshot.sequence_number;
    snapshot.ledger.timestamp = fixture.protocol_snapshot.timestamp;
    snapshot.ledger.network_id = decode_network_id(&fixture.protocol_snapshot.network_id)?;
    snapshot.ledger.base_reserve = fixture.protocol_snapshot.base_reserve;
    snapshot.ledger.min_persistent_entry_ttl = fixture.protocol_snapshot.min_persistent_entry_ttl;
    snapshot.ledger.min_temp_entry_ttl = fixture.protocol_snapshot.min_temp_entry_ttl;
    snapshot.ledger.max_entry_ttl = fixture.protocol_snapshot.max_entry_ttl;

    for entry in &fixture.ledger_entries {
        let key_bytes = BASE64
            .decode(&entry.key_xdr)
            .map_err(|error| FixtureError::Simulation(error.to_string()))?;
        let key = LedgerKey::from_xdr(&key_bytes, Limits::none())
            .map_err(|error| FixtureError::Simulation(error.to_string()))?;
        let entry_bytes = BASE64
            .decode(&entry.entry_xdr)
            .map_err(|error| FixtureError::Simulation(error.to_string()))?;
        let ledger_entry = LedgerEntry::from_xdr(&entry_bytes, Limits::none())
            .map_err(|error| FixtureError::Simulation(error.to_string()))?;
        snapshot.ledger.ledger_entries.push((
            Box::new(key),
            (Box::new(ledger_entry), entry.live_until_ledger),
        ));
    }

    let env = Env::from_snapshot(snapshot);
    env.ledger()
        .set_protocol_version(fixture.protocol_snapshot.protocol_version);
    env.mock_all_auths();

    let contract_hash = match Strkey::from_string(&fixture.contract_id)
        .map_err(|error| FixtureError::Simulation(error.to_string()))?
    {
        Strkey::Contract(contract) => contract.0,
        _ => {
            return Err(FixtureError::Simulation(
                "fixture contract_id must be a C... strkey".to_string(),
            ))
        }
    };
    let contract_address = Address::try_from_val(
        &env,
        &ScVal::Address(ScAddress::Contract(Hash(contract_hash))),
    )
    .map_err(|error| FixtureError::Simulation(format!("invalid contract address: {error:?}")))?;
    env.register_at(&contract_address, wasm_bytes, ());

    let mut args = soroban_sdk::Vec::<Val>::new(&env);
    for arg_xdr in &fixture.args_xdr {
        let bytes = BASE64
            .decode(arg_xdr)
            .map_err(|error| FixtureError::Simulation(error.to_string()))?;
        let scval = ScVal::from_xdr(&bytes, Limits::none())
            .map_err(|error| FixtureError::Simulation(error.to_string()))?;
        args.push_back(Val::try_from_val(&env, &scval).map_err(|error| {
            FixtureError::Simulation(format!("invalid ScVal argument: {error:?}"))
        })?);
    }

    let function = Symbol::try_from(fixture.function.as_str())
        .map_err(|error| FixtureError::Simulation(format!("invalid function symbol: {error:?}")))?;
    env.cost_estimate().budget().reset_unlimited();
    let start_cpu = env.cost_estimate().budget().cpu_instruction_cost();
    let start_ram = env.cost_estimate().budget().memory_bytes_cost();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        env.invoke_contract::<Val>(&contract_address, &function, args)
    }));
    let cpu = env.cost_estimate().budget().cpu_instruction_cost();
    let ram = env.cost_estimate().budget().memory_bytes_cost();
    if result.is_err() {
        return Err(FixtureError::Simulation(format!(
            "{}::{} invocation failed",
            fixture.name, fixture.function
        )));
    }

    Ok(SorobanResources {
        cpu_instructions: cpu.saturating_sub(start_cpu),
        ram_bytes: ram.saturating_sub(start_ram),
        ledger_read_bytes: 0,
        ledger_write_bytes: 0,
        transaction_size_bytes: wasm_bytes.len() as u64,
        amm_tick_profile_report: None,
    })
}

fn decode_network_id(value: &str) -> Result<[u8; 32], FixtureError> {
    let bytes = hex::decode(value).map_err(|error| FixtureError::Simulation(error.to_string()))?;
    bytes.try_into().map_err(|bytes: Vec<u8>| {
        FixtureError::Simulation(format!(
            "protocol snapshot network_id must be 32 bytes, got {}",
            bytes.len()
        ))
    })
}

/// Record a fixture from values suitable for JSON serialization.
pub fn write_fixture(
    path: impl AsRef<Path>,
    fixture: &ResourceFixture,
) -> Result<(), FixtureError> {
    std::fs::write(path, serde_json::to_vec_pretty(fixture)?)?;
    Ok(())
}
