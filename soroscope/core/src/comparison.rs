use crate::simulation::{SimulationEngine, SimulationError, SorobanResources};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use utoipa::ToSchema;

// ── Regression threshold ─────────────────────────────────────────────────────

/// Any resource that increases by more than this percentage is flagged.
const REGRESSION_THRESHOLD: f64 = 10.0;

// ── Types ────────────────────────────────────────────────────────────────────

/// How the two contract versions are provided for comparison.
#[derive(Debug, Clone)]
pub enum CompareMode {
    /// Two local WASM files (current = new version, base = reference version).
    LocalVsLocal {
        current_wasm: PathBuf,
        base_wasm: PathBuf,
    },
    /// A local WASM file compared against a deployed contract on the network.
    LocalVsDeployed {
        current_wasm: PathBuf,
        contract_id: String,
        function_name: String,
        args: Vec<String>,
    },
}

/// Percentage change for each tracked resource metric.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ResourceDelta {
    /// CPU instruction change (e.g. +15.4 means 15.4% increase)
    #[schema(example = json!(15.4))]
    pub cpu_instructions: f64,
    /// RAM byte change
    #[schema(example = json!(-2.1))]
    pub ram_bytes: f64,
    /// Ledger read bytes change
    #[schema(example = json!(0.0))]
    pub ledger_read_bytes: f64,
    /// Ledger write bytes change
    #[schema(example = json!(5.3))]
    pub ledger_write_bytes: f64,
    /// Transaction size bytes change
    #[schema(example = json!(1.0))]
    pub transaction_size_bytes: f64,
}

/// A single regression alert for a resource that exceeds the threshold.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RegressionFlag {
    /// The resource that regressed (e.g. "cpu_instructions")
    pub resource: String,
    /// Percentage change
    pub change_percent: f64,
    /// "high" if >10%, "critical" if >25%
    pub severity: String,
}

/// Full comparison report returned by the API and CLI.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RegressionReport {
    /// Resource metrics for the new (current) version
    pub current: SorobanResources,
    /// Resource metrics for the reference (base) version
    pub base: SorobanResources,
    /// Percentage change per resource metric
    pub deltas: ResourceDelta,
    /// Alerts for any resource that increased by more than the threshold
    pub regression_flags: Vec<RegressionFlag>,
    /// Human-readable summary of the comparison
    pub summary: String,
}

// ── Core logic ───────────────────────────────────────────────────────────────

/// Run a comparison between two contract versions.
///
/// Both simulations are executed concurrently via `tokio::join!` using the same
/// `SimulationEngine` (and therefore the same ledger state / RPC node) for
/// consistency.
pub async fn run_comparison(
    engine: &SimulationEngine,
    mode: CompareMode,
) -> Result<RegressionReport, SimulationError> {
    let (current_resources, base_resources) = match mode {
        CompareMode::LocalVsLocal {
            current_wasm,
            base_wasm,
        } => {
            // For LocalVsLocal, use the file paths as contract identifiers.
            // The SimulationEngine.simulate_from_contract_id expects a C…
            // contract address, so we extract the stem and pass as identifier.
            // In a real deployment these would be contract IDs after upload.
            let current_id = current_wasm.to_string_lossy().to_string();
            let base_id = base_wasm.to_string_lossy().to_string();

            let (current_result, base_result) = tokio::join!(
                engine.simulate_from_contract_id(&current_id, "compare", vec![], None, None, None),
                engine.simulate_from_contract_id(&base_id, "compare", vec![], None, None, None)
            );

            (current_result?.resources, base_result?.resources)
        }
        CompareMode::LocalVsDeployed {
            current_wasm,
            contract_id,
            function_name,
            args,
        } => {
            let current_id = current_wasm.to_string_lossy().to_string();

            let (current_result, base_result) = tokio::join!(
                engine.simulate_from_contract_id(
                    &current_id,
                    &function_name,
                    args.clone(),
                    None,
                    None,
                    None,
                ),
                engine.simulate_from_contract_id(
                    &contract_id,
                    &function_name,
                    args,
                    None,
                    None,
                    None
                )
            );

            (current_result?.resources, base_result?.resources)
        }
    };

    Ok(build_report(current_resources, base_resources))
}

/// Build a `RegressionReport` from two sets of resource metrics.
/// This is also useful for testing and for cases where metrics are already
/// available (e.g. from cached simulation results).
pub fn build_report(current: SorobanResources, base: SorobanResources) -> RegressionReport {
    let deltas = calculate_deltas(&current, &base);
    let regression_flags = detect_regressions(&deltas, REGRESSION_THRESHOLD);

    let summary = if regression_flags.is_empty() {
        "No significant regressions detected. All resource changes are within acceptable limits."
            .to_string()
    } else {
        format!(
            "⚠ {} regression(s) detected: {}",
            regression_flags.len(),
            regression_flags
                .iter()
                .map(|f| format!("{} ({:+.1}%)", f.resource, f.change_percent))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    RegressionReport {
        current,
        base,
        deltas,
        regression_flags,
        summary,
    }
}

/// Compute percentage change for each resource metric.
///
/// Formula: `((current - base) / base) * 100.0`
///
/// If `base` is zero for a metric, the delta is reported as `0.0` to avoid
/// division-by-zero (a change from 0 to any value is informational, not a
/// percentage).
pub fn calculate_deltas(current: &SorobanResources, base: &SorobanResources) -> ResourceDelta {
    ResourceDelta {
        cpu_instructions: pct_change(current.cpu_instructions, base.cpu_instructions),
        ram_bytes: pct_change(current.ram_bytes, base.ram_bytes),
        ledger_read_bytes: pct_change(current.ledger_read_bytes, base.ledger_read_bytes),
        ledger_write_bytes: pct_change(current.ledger_write_bytes, base.ledger_write_bytes),
        transaction_size_bytes: pct_change(
            current.transaction_size_bytes,
            base.transaction_size_bytes,
        ),
    }
}

/// Identify metrics whose **increase** exceeds `threshold` percent.
///
/// Negative deltas (improvements) are never flagged.
pub fn detect_regressions(deltas: &ResourceDelta, threshold: f64) -> Vec<RegressionFlag> {
    let metrics: Vec<(&str, f64)> = vec![
        ("cpu_instructions", deltas.cpu_instructions),
        ("ram_bytes", deltas.ram_bytes),
        ("ledger_read_bytes", deltas.ledger_read_bytes),
        ("ledger_write_bytes", deltas.ledger_write_bytes),
        ("transaction_size_bytes", deltas.transaction_size_bytes),
    ];

    metrics
        .into_iter()
        .filter(|(_, change)| *change > threshold)
        .map(|(resource, change)| RegressionFlag {
            resource: resource.to_string(),
            change_percent: change,
            severity: if change > 25.0 {
                "critical".to_string()
            } else {
                "high".to_string()
            },
        })
        .collect()
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn pct_change(current: u64, base: u64) -> f64 {
    if base == 0 {
        return 0.0;
    }
    ((current as f64 - base as f64) / base as f64) * 100.0
}

/// Pretty-print a `RegressionReport` to stdout (used by the CLI).
pub fn print_report(report: &RegressionReport) {
    println!("\n{}", "=".repeat(60));
    println!("  SoroScope — Contract Regression Report");
    println!("{}\n", "=".repeat(60));

    println!(
        "  {:<25} {:>12} {:>12} {:>10}",
        "Metric", "Current", "Base", "Delta"
    );
    println!("  {}", "-".repeat(59));

    print_metric_row(
        "CPU Instructions",
        report.current.cpu_instructions,
        report.base.cpu_instructions,
        report.deltas.cpu_instructions,
    );
    print_metric_row(
        "RAM Bytes",
        report.current.ram_bytes,
        report.base.ram_bytes,
        report.deltas.ram_bytes,
    );
    print_metric_row(
        "Ledger Read Bytes",
        report.current.ledger_read_bytes,
        report.base.ledger_read_bytes,
        report.deltas.ledger_read_bytes,
    );
    print_metric_row(
        "Ledger Write Bytes",
        report.current.ledger_write_bytes,
        report.base.ledger_write_bytes,
        report.deltas.ledger_write_bytes,
    );
    print_metric_row(
        "Transaction Size",
        report.current.transaction_size_bytes,
        report.base.transaction_size_bytes,
        report.deltas.transaction_size_bytes,
    );

    println!();

    if report.regression_flags.is_empty() {
        println!("  ✓ No regressions detected.");
    } else {
        println!(
            "  ⚠ {} REGRESSION(S) DETECTED:\n",
            report.regression_flags.len()
        );
        for flag in &report.regression_flags {
            println!(
                "    [{:>8}] {} — {:+.1}%",
                flag.severity.to_uppercase(),
                flag.resource,
                flag.change_percent,
            );
        }
    }

    println!("\n  Summary: {}", report.summary);
    println!("{}\n", "=".repeat(60));
}

fn print_metric_row(label: &str, current: u64, base: u64, delta: f64) {
    let arrow = if delta > 0.0 {
        "▲"
    } else if delta < 0.0 {
        "▼"
    } else {
        "="
    };
    println!(
        "  {:<25} {:>12} {:>12} {:>+8.1}% {}",
        label, current, base, delta, arrow,
    );
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_resources(cpu: u64, ram: u64, lr: u64, lw: u64, tx: u64) -> SorobanResources {
        SorobanResources {
            cpu_instructions: cpu,
            ram_bytes: ram,
            ledger_read_bytes: lr,
            ledger_write_bytes: lw,
            transaction_size_bytes: tx,
        }
    }

    #[test]
    fn test_calculate_deltas_basic() {
        let current = make_resources(1150, 2000, 500, 300, 100);
        let base = make_resources(1000, 2000, 400, 300, 100);

        let deltas = calculate_deltas(&current, &base);

        assert!((deltas.cpu_instructions - 15.0).abs() < 0.001);
        assert!((deltas.ram_bytes - 0.0).abs() < 0.001);
        assert!((deltas.ledger_read_bytes - 25.0).abs() < 0.001);
        assert!((deltas.ledger_write_bytes - 0.0).abs() < 0.001);
        assert!((deltas.transaction_size_bytes - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_deltas_zero_base() {
        let current = make_resources(500, 0, 0, 0, 0);
        let base = make_resources(0, 0, 0, 0, 0);

        let deltas = calculate_deltas(&current, &base);

        // When base is zero, delta should be 0.0 (no meaningful percentage)
        assert!((deltas.cpu_instructions - 0.0).abs() < 0.001);
        assert!((deltas.ram_bytes - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_deltas_no_change() {
        let resources = make_resources(1000, 2000, 300, 400, 500);

        let deltas = calculate_deltas(&resources, &resources);

        assert!((deltas.cpu_instructions).abs() < 0.001);
        assert!((deltas.ram_bytes).abs() < 0.001);
        assert!((deltas.ledger_read_bytes).abs() < 0.001);
        assert!((deltas.ledger_write_bytes).abs() < 0.001);
        assert!((deltas.transaction_size_bytes).abs() < 0.001);
    }

    #[test]
    fn test_detect_regressions_above_threshold() {
        let deltas = ResourceDelta {
            cpu_instructions: 15.4,
            ram_bytes: 5.0,
            ledger_read_bytes: 30.0,
            ledger_write_bytes: 0.0,
            transaction_size_bytes: 11.0,
        };

        let flags = detect_regressions(&deltas, 10.0);

        assert_eq!(flags.len(), 3);
        assert!(flags.iter().any(|f| f.resource == "cpu_instructions"));
        assert!(flags.iter().any(|f| f.resource == "ledger_read_bytes"));
        assert!(flags.iter().any(|f| f.resource == "transaction_size_bytes"));
        // ram_bytes (5.0) and ledger_write_bytes (0.0) are under threshold
    }

    #[test]
    fn test_detect_regressions_below_threshold() {
        let deltas = ResourceDelta {
            cpu_instructions: 5.0,
            ram_bytes: 3.0,
            ledger_read_bytes: 9.9,
            ledger_write_bytes: 0.0,
            transaction_size_bytes: -5.0,
        };

        let flags = detect_regressions(&deltas, 10.0);

        assert!(flags.is_empty());
    }

    #[test]
    fn test_detect_regressions_exact_threshold() {
        let deltas = ResourceDelta {
            cpu_instructions: 10.0,
            ram_bytes: 10.0,
            ledger_read_bytes: 10.0,
            ledger_write_bytes: 10.0,
            transaction_size_bytes: 10.0,
        };

        // Exactly at threshold should NOT flag (must be strictly greater)
        let flags = detect_regressions(&deltas, 10.0);
        assert!(flags.is_empty());
    }

    #[test]
    fn test_detect_regressions_improvements_ignored() {
        let deltas = ResourceDelta {
            cpu_instructions: -20.0,
            ram_bytes: -50.0,
            ledger_read_bytes: -5.0,
            ledger_write_bytes: -100.0,
            transaction_size_bytes: -0.1,
        };

        let flags = detect_regressions(&deltas, 10.0);
        assert!(flags.is_empty());
    }

    #[test]
    fn test_regression_report_serialization() {
        let report = build_report(
            make_resources(1150, 2000, 500, 300, 100),
            make_resources(1000, 2000, 400, 300, 100),
        );

        let json = serde_json::to_string(&report).expect("should serialize");
        let deserialized: RegressionReport =
            serde_json::from_str(&json).expect("should deserialize");

        assert_eq!(deserialized.current.cpu_instructions, 1150);
        assert_eq!(deserialized.base.cpu_instructions, 1000);
        assert!((deserialized.deltas.cpu_instructions - 15.0).abs() < 0.001);
    }

    #[test]
    fn test_regression_severity_levels() {
        let deltas = ResourceDelta {
            cpu_instructions: 12.0, // high
            ram_bytes: 30.0,        // critical (>25%)
            ledger_read_bytes: 0.0,
            ledger_write_bytes: 0.0,
            transaction_size_bytes: 0.0,
        };

        let flags = detect_regressions(&deltas, 10.0);

        let cpu_flag = flags
            .iter()
            .find(|f| f.resource == "cpu_instructions")
            .unwrap();
        assert_eq!(cpu_flag.severity, "high");

        let ram_flag = flags.iter().find(|f| f.resource == "ram_bytes").unwrap();
        assert_eq!(ram_flag.severity, "critical");
    }

    #[test]
    fn test_compare_mode_variants() {
        let local = CompareMode::LocalVsLocal {
            current_wasm: PathBuf::from("v2.wasm"),
            base_wasm: PathBuf::from("v1.wasm"),
        };
        assert!(matches!(local, CompareMode::LocalVsLocal { .. }));

        let deployed = CompareMode::LocalVsDeployed {
            current_wasm: PathBuf::from("v2.wasm"),
            contract_id: "CABC123".to_string(),
            function_name: "hello".to_string(),
            args: vec![],
        };
        assert!(matches!(deployed, CompareMode::LocalVsDeployed { .. }));
    }

    #[test]
    fn test_build_report_no_regressions() {
        let report = build_report(
            make_resources(1000, 2000, 300, 400, 500),
            make_resources(1000, 2000, 300, 400, 500),
        );

        assert!(report.regression_flags.is_empty());
        assert!(report.summary.contains("No significant regressions"));
    }

    #[test]
    fn test_build_report_with_regressions() {
        let report = build_report(
            make_resources(1500, 2000, 300, 400, 500),
            make_resources(1000, 2000, 300, 400, 500),
        );

        assert_eq!(report.regression_flags.len(), 1);
        assert_eq!(report.regression_flags[0].resource, "cpu_instructions");
        assert!(report.summary.contains("regression(s) detected"));
    }
}
