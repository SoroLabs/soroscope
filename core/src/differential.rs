//! Differential resource checks for local Soroban execution versus RPC simulation.

use crate::fee_quote::SorobanFeeConfig;
use crate::runner::{ContractInvocation, LocalRunner};
use crate::simulation::{SimulationEngine, SimulationError, SimulationResult, SorobanResources};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const TOLERANCES_JSON: &str = include_str!("../config/differential-tolerances.json");

#[derive(Debug, Clone, Deserialize)]
struct ToleranceFile {
    protocol_snapshots: std::collections::HashMap<String, MeterTolerances>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MeterTolerances {
    /// Relative CPU tolerance in basis points (1000 = 10%).
    pub cpu_relative_error_bps: u32,
    /// Relative memory tolerance in basis points (1000 = 10%).
    pub memory_relative_error_bps: u32,
}

impl MeterTolerances {
    pub fn for_protocol(protocol_version: u32) -> Result<Self, SimulationError> {
        let fixtures: ToleranceFile =
            serde_json::from_str(TOLERANCES_JSON).map_err(SimulationError::SerializationError)?;
        fixtures
            .protocol_snapshots
            .get(&protocol_version.to_string())
            .cloned()
            .ok_or_else(|| {
                SimulationError::InvalidContract(format!(
                    "No differential tolerances checked in for protocol {protocol_version}"
                ))
            })
    }
}

#[derive(Debug, Clone)]
pub struct DifferentialInput {
    pub wasm: Vec<u8>,
    pub contract_id: String,
    pub function_name: String,
    pub args: Vec<String>,
    pub protocol_version: u32,
    /// Set only when both executions are known to have used the exact same ledger entries.
    pub same_ledger_entries_confirmed: bool,
}

#[derive(Debug, Clone)]
pub struct DifferentialSide {
    pub resources: Option<SorobanResources>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DifferentialReport {
    pub protocol_version: u32,
    pub local: DifferentialSide,
    pub rpc: DifferentialSide,
    pub cpu_relative_error_bps: Option<u64>,
    pub memory_relative_error_bps: Option<u64>,
    pub ledger_read_bytes_match: Option<bool>,
    pub ledger_write_bytes_match: Option<bool>,
    pub passed: bool,
    pub failures: Vec<String>,
}

/// Runs both simulation paths independently; neither path is used as a fallback for the other.
pub struct DifferentialRunner {
    local: Arc<LocalRunner>,
    rpc: SimulationEngine,
}

impl DifferentialRunner {
    pub fn new(rpc_url: String, local: Arc<LocalRunner>) -> Self {
        Self {
            local,
            rpc: SimulationEngine::new(rpc_url),
        }
    }

    pub async fn run(
        &self,
        input: DifferentialInput,
    ) -> Result<DifferentialReport, SimulationError> {
        let (tolerances, tolerance_error) =
            match MeterTolerances::for_protocol(input.protocol_version) {
                Ok(tolerances) => (tolerances, None),
                Err(error) => (
                    MeterTolerances {
                        cpu_relative_error_bps: 0,
                        memory_relative_error_bps: 0,
                    },
                    Some(error.to_string()),
                ),
            };
        let local_result = self
            .run_local(&input)
            .await
            .map(|result| result.resources)
            .map_err(|error| error.to_string());

        // Always attempt RPC, even when local execution failed, so the report retains both sides.
        let rpc_result = self
            .rpc
            .simulate_from_contract_id(
                &input.contract_id,
                &input.function_name,
                input.args.clone(),
                None,
                Some(input.protocol_version),
                None,
            )
            .await
            .map(|result| result.resources)
            .map_err(|error| error.to_string());

        let mut report = compare_results(
            input.protocol_version,
            tolerances,
            input.same_ledger_entries_confirmed,
            local_result,
            rpc_result,
        );
        if let Some(error) = tolerance_error {
            report
                .failures
                .push(format!("protocol tolerance unavailable: {error}"));
            report.passed = false;
        }
        Ok(report)
    }

    /// Replay one saved invocation locally under the baseline and the candidate
    /// protocol, then compare the two executions cost type by cost type.
    pub async fn replay_protocol_upgrade(
        &self,
        input: &DifferentialInput,
        baseline: &ProtocolConfig,
        candidate: &ProtocolConfig,
    ) -> Result<ProtocolUpgradeReport, SimulationError> {
        let contract_hash = self.rpc.parse_contract_id(&input.contract_id)?;
        self.local
            .load_wasm(contract_hash, input.wasm.clone())
            .await;
        let invocation = ContractInvocation::new(
            contract_hash,
            input.function_name.clone(),
            input.args.clone(),
        );

        let baseline_result = self
            .local
            .simulate_with_protocol(&invocation, Some(baseline.protocol_version))
            .await?;
        let candidate_result = self
            .local
            .simulate_with_protocol(&invocation, Some(candidate.protocol_version))
            .await?;

        Ok(compare_protocol_configs(
            &input.function_name,
            baseline,
            &InvocationSample::from(&baseline_result),
            candidate,
            &InvocationSample::from(&candidate_result),
        ))
    }

    async fn run_local(
        &self,
        input: &DifferentialInput,
    ) -> Result<SimulationResult, SimulationError> {
        let contract_hash = self.rpc.parse_contract_id(&input.contract_id)?;
        self.local
            .load_wasm(contract_hash, input.wasm.clone())
            .await;
        let invocation = ContractInvocation::new(
            contract_hash,
            input.function_name.clone(),
            input.args.clone(),
        );
        self.local
            .simulate_with_protocol(&invocation, Some(input.protocol_version))
            .await
    }
}

fn compare_results(
    protocol_version: u32,
    tolerances: MeterTolerances,
    same_ledger_entries_confirmed: bool,
    local_result: Result<SorobanResources, String>,
    rpc_result: Result<SorobanResources, String>,
) -> DifferentialReport {
    let local = side(local_result);
    let rpc = side(rpc_result);
    let mut failures = Vec::new();

    if let Some(error) = &local.error {
        failures.push(format!("local simulation failed: {error}"));
    }
    if let Some(error) = &rpc.error {
        failures.push(format!("RPC simulation failed: {error}"));
    }

    let (
        cpu_relative_error_bps,
        memory_relative_error_bps,
        ledger_read_bytes_match,
        ledger_write_bytes_match,
    ) = match (&local.resources, &rpc.resources) {
        (Some(local_resources), Some(rpc_resources)) => {
            let cpu_error = relative_error_bps(
                local_resources.cpu_instructions,
                rpc_resources.cpu_instructions,
            );
            let memory_error =
                relative_error_bps(local_resources.ram_bytes, rpc_resources.ram_bytes);

            if cpu_error > u64::from(tolerances.cpu_relative_error_bps) {
                failures.push(format!(
                        "CPU relative error {cpu_error} bps exceeds protocol {protocol_version} tolerance {} bps",
                        tolerances.cpu_relative_error_bps
                    ));
            }
            if memory_error > u64::from(tolerances.memory_relative_error_bps) {
                failures.push(format!(
                        "memory relative error {memory_error} bps exceeds protocol {protocol_version} tolerance {} bps",
                        tolerances.memory_relative_error_bps
                    ));
            }

            let read_match = same_ledger_entries_confirmed
                .then_some(local_resources.ledger_read_bytes == rpc_resources.ledger_read_bytes);
            let write_match = same_ledger_entries_confirmed
                .then_some(local_resources.ledger_write_bytes == rpc_resources.ledger_write_bytes);
            if read_match == Some(false) {
                failures
                    .push("ledger_read_bytes must match for identical ledger entries".to_string());
            }
            if write_match == Some(false) {
                failures
                    .push("ledger_write_bytes must match for identical ledger entries".to_string());
            }

            (Some(cpu_error), Some(memory_error), read_match, write_match)
        }
        _ => (None, None, None, None),
    };

    DifferentialReport {
        protocol_version,
        local,
        rpc,
        cpu_relative_error_bps,
        memory_relative_error_bps,
        ledger_read_bytes_match,
        ledger_write_bytes_match,
        passed: failures.is_empty(),
        failures,
    }
}

fn side(result: Result<SorobanResources, String>) -> DifferentialSide {
    match result {
        Ok(resources) => DifferentialSide {
            resources: Some(resources),
            error: None,
        },
        Err(error) => DifferentialSide {
            resources: None,
            error: Some(error),
        },
    }
}

fn relative_error_bps(local: u64, rpc: u64) -> u64 {
    if rpc == 0 {
        return if local == 0 { 0 } else { u64::MAX };
    }
    let difference = local.abs_diff(rpc) as u128;
    ((difference * 10_000) / u128::from(rpc)) as u64
}

// ─────────────────────────────────────────────────────────────────────────────
// Protocol upgrade regression analysis (Issue #1402)
// ─────────────────────────────────────────────────────────────────────────────

/// Candidate usage above this share of a network limit (80%, in basis points)
/// is flagged as a budget risk.
pub const BUDGET_SAFETY_THRESHOLD_BPS: u64 = 8_000;

/// Per-transaction network limits under one protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolLimits {
    pub max_cpu_instructions: u64,
    pub max_memory_bytes: u64,
    pub max_ledger_read_bytes: u64,
    pub max_ledger_write_bytes: u64,
}

impl Default for ProtocolLimits {
    /// Pubnet protocol 22 transaction limits.
    fn default() -> Self {
        Self {
            max_cpu_instructions: 100_000_000,
            max_memory_bytes: 40 * 1024 * 1024,
            max_ledger_read_bytes: 200_000,
            max_ledger_write_bytes: 132_096,
        }
    }
}

/// A protocol snapshot to replay invocations against: its version, network
/// limits and fee (rent) parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProtocolConfig {
    pub protocol_version: u32,
    pub limits: ProtocolLimits,
    pub fee_config: SorobanFeeConfig,
}

impl ProtocolConfig {
    /// The checked-in fee table and default limits, labelled with its protocol.
    pub fn checked_in() -> Self {
        let fee_config = SorobanFeeConfig::checked_in().clone();
        Self {
            protocol_version: fee_config.protocol,
            limits: ProtocolLimits::default(),
            fee_config,
        }
    }

    /// Rent in stroops for `rent_bytes`, priced at this protocol's persistent rate.
    fn rent_stroops(&self, rent_bytes: u64) -> u64 {
        self.fee_config
            .estimate_entry_rent_stroops(rent_bytes, 1, false)
    }
}

/// The measured cost of one invocation under one protocol.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InvocationSample {
    pub resources: SorobanResources,
    /// `cost.rentBytes`, when the execution reported it.
    pub rent_bytes: Option<u64>,
}

impl From<&SimulationResult> for InvocationSample {
    fn from(result: &SimulationResult) -> Self {
        Self {
            resources: result.resources.clone(),
            rent_bytes: result.rent_bytes,
        }
    }
}

/// The Soroban cost types compared across protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostType {
    CpuInstructions,
    MemoryBytes,
    LedgerReadBytes,
    LedgerWriteBytes,
    RentStroops,
}

/// Baseline vs candidate for one cost type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostTypeDelta {
    pub cost_type: CostType,
    pub baseline: u64,
    pub candidate: u64,
    /// `candidate - baseline`.
    pub absolute_delta: i128,
    /// Change relative to the baseline, in percent. `None` when the baseline is
    /// zero and the candidate is not (the change is unbounded).
    pub percent_delta: Option<f64>,
    /// The candidate protocol's network limit for this cost type, if it has one.
    pub candidate_limit: Option<u64>,
    /// Candidate usage as a share of `candidate_limit`, in basis points.
    pub candidate_utilization_bps: Option<u64>,
    /// Whether candidate usage exceeds [`BUDGET_SAFETY_THRESHOLD_BPS`].
    pub exceeds_safety_threshold: bool,
}

/// Differential report for one invocation replayed under two protocols.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProtocolUpgradeReport {
    pub invocation: String,
    pub baseline_protocol: u32,
    pub candidate_protocol: u32,
    pub deltas: Vec<CostTypeDelta>,
    /// One message per cost type over the safety threshold.
    pub warnings: Vec<String>,
    pub crosses_safety_threshold: bool,
}

impl ProtocolUpgradeReport {
    /// The delta for `cost_type`.
    pub fn delta(&self, cost_type: CostType) -> Option<&CostTypeDelta> {
        self.deltas.iter().find(|d| d.cost_type == cost_type)
    }
}

/// Compare one invocation's cost under a baseline and a candidate protocol.
pub fn compare_protocol_configs(
    invocation: &str,
    baseline_config: &ProtocolConfig,
    baseline: &InvocationSample,
    candidate_config: &ProtocolConfig,
    candidate: &InvocationSample,
) -> ProtocolUpgradeReport {
    let limits = candidate_config.limits;
    let (base, cand) = (&baseline.resources, &candidate.resources);

    let deltas = vec![
        cost_type_delta(
            CostType::CpuInstructions,
            base.cpu_instructions,
            cand.cpu_instructions,
            Some(limits.max_cpu_instructions),
        ),
        cost_type_delta(
            CostType::MemoryBytes,
            base.ram_bytes,
            cand.ram_bytes,
            Some(limits.max_memory_bytes),
        ),
        cost_type_delta(
            CostType::LedgerReadBytes,
            base.ledger_read_bytes,
            cand.ledger_read_bytes,
            Some(limits.max_ledger_read_bytes),
        ),
        cost_type_delta(
            CostType::LedgerWriteBytes,
            base.ledger_write_bytes,
            cand.ledger_write_bytes,
            Some(limits.max_ledger_write_bytes),
        ),
        cost_type_delta(
            CostType::RentStroops,
            baseline_config.rent_stroops(baseline.rent_bytes.unwrap_or(0)),
            candidate_config.rent_stroops(candidate.rent_bytes.unwrap_or(0)),
            None,
        ),
    ];

    let warnings: Vec<String> = deltas
        .iter()
        .filter(|d| d.exceeds_safety_threshold)
        .map(|d| {
            format!(
                "{invocation}: {:?} uses {} of {} ({} bps) under protocol {}, above the {} bps safety threshold",
                d.cost_type,
                d.candidate,
                d.candidate_limit.unwrap_or(0),
                d.candidate_utilization_bps.unwrap_or(0),
                candidate_config.protocol_version,
                BUDGET_SAFETY_THRESHOLD_BPS,
            )
        })
        .collect();

    ProtocolUpgradeReport {
        invocation: invocation.to_string(),
        baseline_protocol: baseline_config.protocol_version,
        candidate_protocol: candidate_config.protocol_version,
        crosses_safety_threshold: !warnings.is_empty(),
        warnings,
        deltas,
    }
}

fn cost_type_delta(
    cost_type: CostType,
    baseline: u64,
    candidate: u64,
    candidate_limit: Option<u64>,
) -> CostTypeDelta {
    let percent_delta = match (baseline, candidate) {
        (0, 0) => Some(0.0),
        (0, _) => None,
        _ => Some((candidate as f64 - baseline as f64) / baseline as f64 * 100.0),
    };
    let candidate_utilization_bps = candidate_limit
        .filter(|limit| *limit > 0)
        .map(|limit| (u128::from(candidate) * 10_000 / u128::from(limit)) as u64);

    CostTypeDelta {
        cost_type,
        baseline,
        candidate,
        absolute_delta: i128::from(candidate) - i128::from(baseline),
        percent_delta,
        candidate_limit,
        candidate_utilization_bps,
        exceeds_safety_threshold: candidate_utilization_bps
            .is_some_and(|bps| bps > BUDGET_SAFETY_THRESHOLD_BPS),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resources(cpu: u64, memory: u64) -> SorobanResources {
        SorobanResources {
            cpu_instructions: cpu,
            ram_bytes: memory,
            ..SorobanResources::default()
        }
    }

    #[test]
    fn mock_rpc_cpu_mismatch_over_tolerance_names_the_meter() {
        let report = compare_results(
            22,
            MeterTolerances {
                cpu_relative_error_bps: 1000,
                memory_relative_error_bps: 1000,
            },
            false,
            Ok(resources(115, 100)),
            Ok(resources(100, 100)),
        );

        assert!(!report.passed);
        assert_eq!(report.cpu_relative_error_bps, Some(1500));
        assert!(report
            .failures
            .iter()
            .any(|failure| failure.contains("CPU")));
        assert!(report.local.resources.is_some());
        assert!(report.rpc.resources.is_some());
    }

    #[test]
    fn identical_mock_fixtures_pass_and_require_exact_ledger_bytes() {
        let mut local = resources(100, 200);
        local.ledger_read_bytes = 64;
        local.ledger_write_bytes = 32;
        let mut rpc = local.clone();
        rpc.transaction_size_bytes = 999;

        let report = compare_results(
            22,
            MeterTolerances {
                cpu_relative_error_bps: 1000,
                memory_relative_error_bps: 1000,
            },
            true,
            Ok(local),
            Ok(rpc),
        );

        assert!(report.passed, "{:?}", report.failures);
        assert_eq!(report.ledger_read_bytes_match, Some(true));
        assert_eq!(report.ledger_write_bytes_match, Some(true));
    }

    #[test]
    fn both_side_results_are_kept_when_local_execution_fails() {
        let report = compare_results(
            22,
            MeterTolerances {
                cpu_relative_error_bps: 1000,
                memory_relative_error_bps: 1000,
            },
            false,
            Err("local unavailable".to_string()),
            Ok(resources(100, 200)),
        );

        assert!(!report.passed);
        assert!(report.local.error.is_some());
        assert!(report.rpc.resources.is_some());
    }

    // ── Protocol upgrade regression analysis (Issue #1402) ──────────────────

    /// Protocol 22 as checked in, and a hypothetical protocol 23 that makes
    /// ledger writes (and therefore rent) 50% more expensive.
    fn protocol_pair() -> (ProtocolConfig, ProtocolConfig) {
        let baseline = ProtocolConfig::checked_in();
        let mut candidate = baseline.clone();
        candidate.protocol_version = 23;
        candidate.fee_config.protocol = 23;
        candidate.fee_config.fee_per_write_1kb = baseline.fee_config.fee_per_write_1kb * 3 / 2;
        (baseline, candidate)
    }

    fn sample(cpu: u64, memory: u64, read: u64, write: u64, rent: u64) -> InvocationSample {
        InvocationSample {
            resources: SorobanResources {
                cpu_instructions: cpu,
                ram_bytes: memory,
                ledger_read_bytes: read,
                ledger_write_bytes: write,
                ..SorobanResources::default()
            },
            rent_bytes: Some(rent),
        }
    }

    fn assert_percent(delta: &CostTypeDelta, expected: f64) {
        let actual = delta.percent_delta.expect("percent delta is defined");
        assert!(
            (actual - expected).abs() < 1e-9,
            "{:?}: expected {expected}%, got {actual}%",
            delta.cost_type
        );
    }

    #[test]
    fn protocol_upgrade_reports_exact_and_percentage_deltas_per_cost_type() {
        let (baseline_config, candidate_config) = protocol_pair();
        // 1024 * 2103 rent bytes cost exactly `fee_per_write_1kb` stroops.
        let rent_bytes = 1024 * 2103;
        let baseline = sample(1_000_000, 2_000_000, 1_000, 0, rent_bytes);
        let candidate = sample(1_150_000, 1_900_000, 1_000, 512, rent_bytes);

        let report = compare_protocol_configs(
            "transfer",
            &baseline_config,
            &baseline,
            &candidate_config,
            &candidate,
        );

        assert_eq!(report.baseline_protocol, 22);
        assert_eq!(report.candidate_protocol, 23);
        assert_eq!(report.deltas.len(), 5);

        let cpu = report.delta(CostType::CpuInstructions).unwrap();
        assert_eq!(
            (cpu.baseline, cpu.candidate, cpu.absolute_delta),
            (1_000_000, 1_150_000, 150_000)
        );
        assert_percent(cpu, 15.0);
        assert_eq!(cpu.candidate_utilization_bps, Some(115));

        let memory = report.delta(CostType::MemoryBytes).unwrap();
        assert_eq!(memory.absolute_delta, -100_000);
        assert_percent(memory, -5.0);

        let read = report.delta(CostType::LedgerReadBytes).unwrap();
        assert_eq!(read.absolute_delta, 0);
        assert_percent(read, 0.0);

        let write = report.delta(CostType::LedgerWriteBytes).unwrap();
        assert_eq!(write.absolute_delta, 512);
        assert_eq!(write.percent_delta, None, "growth from zero is unbounded");

        let rent = report.delta(CostType::RentStroops).unwrap();
        assert_eq!((rent.baseline, rent.candidate), (12_000, 18_000));
        assert_eq!(rent.absolute_delta, 6_000);
        assert_percent(rent, 50.0);
        assert_eq!(rent.candidate_limit, None);

        assert!(!report.crosses_safety_threshold);
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn protocol_upgrade_warns_when_candidate_usage_exceeds_80_percent_of_limits() {
        let (baseline_config, candidate_config) = protocol_pair();
        let baseline = sample(70_000_000, 1_000_000, 1_000, 1_000, 0);
        // 85% of the CPU limit and 90% of the write-byte limit.
        let candidate = sample(85_000_000, 1_000_000, 1_000, 118_886, 0);

        let report = compare_protocol_configs(
            "batch_settle",
            &baseline_config,
            &baseline,
            &candidate_config,
            &candidate,
        );

        assert!(report.crosses_safety_threshold);
        let cpu = report.delta(CostType::CpuInstructions).unwrap();
        assert_eq!(cpu.candidate_utilization_bps, Some(8_500));
        assert!(cpu.exceeds_safety_threshold);
        let write = report.delta(CostType::LedgerWriteBytes).unwrap();
        assert_eq!(write.candidate_utilization_bps, Some(8_999));
        assert!(write.exceeds_safety_threshold);
        assert!(
            !report
                .delta(CostType::MemoryBytes)
                .unwrap()
                .exceeds_safety_threshold
        );

        assert_eq!(report.warnings.len(), 2);
        assert!(report.warnings[0].contains("CpuInstructions"));
        assert!(report.warnings[0].contains("protocol 23"));
    }

    #[test]
    fn protocol_upgrade_threshold_is_strictly_above_80_percent() {
        let (baseline_config, candidate_config) = protocol_pair();
        let at_threshold = sample(80_000_000, 0, 0, 0, 0);

        let report = compare_protocol_configs(
            "edge",
            &baseline_config,
            &at_threshold,
            &candidate_config,
            &at_threshold,
        );

        let cpu = report.delta(CostType::CpuInstructions).unwrap();
        assert_eq!(cpu.candidate_utilization_bps, Some(8_000));
        assert!(!cpu.exceeds_safety_threshold);
        assert!(!report.crosses_safety_threshold);
    }
}
