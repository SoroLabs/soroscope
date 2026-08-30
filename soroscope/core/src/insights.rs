use crate::simulation::SorobanResources;
use serde::{Deserialize, Serialize};

// ── Types ─────────────────────────────────────────────────────────────────────

/// Severity level for an optimisation insight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

/// A single actionable insight produced by the analysis engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Insight {
    pub severity: Severity,
    pub rule: String,
    pub message: String,
    pub suggested_fix: String,
}

/// Complete insights report returned alongside resource metrics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InsightsReport {
    /// Weighted efficiency score in the range 0–100.
    pub efficiency_score: u32,
    /// Individual insights (may be empty when the contract is well-optimised).
    pub insights: Vec<Insight>,
}

// ── Rule trait ─────────────────────────────────────────────────────────────────

/// Extensible trait — implement this to add new heuristic rules without
/// touching existing code.
pub trait InsightRule: Send + Sync {
    /// Unique identifier for this rule (e.g. `"storage_efficiency"`).
    fn name(&self) -> &str;

    /// Evaluate the rule against a resource footprint and return zero or more
    /// insights.
    fn evaluate(&self, resources: &SorobanResources) -> Vec<Insight>;
}

// ── Built-in rules ────────────────────────────────────────────────────────────

/// Flags disproportionately high ledger write bytes relative to overall
/// transaction data, suggesting inefficient storage patterns.
pub struct StorageEfficiencyRule;

impl InsightRule for StorageEfficiencyRule {
    fn name(&self) -> &str {
        "storage_efficiency"
    }

    fn evaluate(&self, r: &SorobanResources) -> Vec<Insight> {
        let mut out = Vec::new();

        // Skip if there's no meaningful data to analyse.
        if r.transaction_size_bytes == 0 {
            return out;
        }

        let write_ratio = r.ledger_write_bytes as f64 / r.transaction_size_bytes as f64;

        if write_ratio > 2.0 {
            out.push(Insight {
                severity: Severity::Critical,
                rule: self.name().to_string(),
                message: format!(
                    "Ledger write bytes ({}) are {:.1}x the transaction size ({}) \
                     — extremely write-heavy",
                    r.ledger_write_bytes, write_ratio, r.transaction_size_bytes
                ),
                suggested_fix: "Use temporary storage (TTL entries) for ephemeral data \
                                and batch writes where possible."
                    .to_string(),
            });
        } else if write_ratio > 1.0 {
            out.push(Insight {
                severity: Severity::Warning,
                rule: self.name().to_string(),
                message: format!(
                    "Ledger write bytes ({}) exceed transaction size ({}) \
                     — consider reviewing storage layout",
                    r.ledger_write_bytes, r.transaction_size_bytes
                ),
                suggested_fix:
                    "Consolidate writes into fewer ledger keys or use compact serialization."
                        .to_string(),
            });
        }

        out
    }
}

/// Detects high CPU usage with relatively low ledger activity, indicating
/// computation-heavy logic that may benefit from off-chain pre-computation.
pub struct InstructionDensityRule;

impl InsightRule for InstructionDensityRule {
    fn name(&self) -> &str {
        "instruction_density"
    }

    fn evaluate(&self, r: &SorobanResources) -> Vec<Insight> {
        let mut out = Vec::new();

        let total_ledger = r.ledger_read_bytes + r.ledger_write_bytes;

        // High CPU with low ledger I/O → pure compute workload.
        if r.cpu_instructions > 50_000_000 && total_ledger < 1_024 {
            out.push(Insight {
                severity: Severity::Critical,
                rule: self.name().to_string(),
                message: format!(
                    "Very high CPU ({} instructions) with minimal ledger I/O ({} bytes) \
                     — heavy computation detected",
                    r.cpu_instructions, total_ledger
                ),
                suggested_fix: "Cache intermediate results in persistent storage or move \
                                complex calculations off-chain with on-chain verification."
                    .to_string(),
            });
        } else if r.cpu_instructions > 10_000_000 && total_ledger < 2_048 {
            out.push(Insight {
                severity: Severity::Warning,
                rule: self.name().to_string(),
                message: format!(
                    "High CPU ({} instructions) relative to ledger activity ({} bytes) \
                     — consider optimising hot loops",
                    r.cpu_instructions, total_ledger
                ),
                suggested_fix:
                    "Profile the contract to identify hot loops; consider lookup tables \
                     or pre-computed values."
                        .to_string(),
            });
        }

        out
    }
}

/// Flags transactions with a large footprint (many ledger keys), which
/// increases base fees and contention risk.
pub struct FootprintBloatRule;

impl InsightRule for FootprintBloatRule {
    fn name(&self) -> &str {
        "footprint_bloat"
    }

    fn evaluate(&self, r: &SorobanResources) -> Vec<Insight> {
        let mut out = Vec::new();

        // Heuristic: average ledger key ≈ 40–80 bytes.  We estimate the key
        // count from the total footprint size.
        let estimated_keys = (r.ledger_read_bytes + r.ledger_write_bytes) / 60;

        if estimated_keys > 20 {
            out.push(Insight {
                severity: Severity::Critical,
                rule: self.name().to_string(),
                message: format!(
                    "Estimated footprint contains ~{} ledger keys — very large transaction",
                    estimated_keys
                ),
                suggested_fix: "Split the operation into smaller batches or reduce the \
                                number of distinct storage keys accessed per invocation."
                    .to_string(),
            });
        } else if estimated_keys > 10 {
            out.push(Insight {
                severity: Severity::Warning,
                rule: self.name().to_string(),
                message: format!(
                    "Estimated footprint contains ~{} ledger keys — above recommended threshold",
                    estimated_keys
                ),
                suggested_fix: "Consider consolidating related data into fewer keys \
                                (e.g., a single Map entry instead of many individual keys)."
                    .to_string(),
            });
        }

        out
    }
}

/// Flags high RAM usage which may push against per-transaction memory limits.
pub struct MemoryPressureRule;

impl InsightRule for MemoryPressureRule {
    fn name(&self) -> &str {
        "memory_pressure"
    }

    fn evaluate(&self, r: &SorobanResources) -> Vec<Insight> {
        let mut out = Vec::new();

        if r.ram_bytes > 20 * 1024 * 1024 {
            out.push(Insight {
                severity: Severity::Critical,
                rule: self.name().to_string(),
                message: format!(
                    "RAM usage ({} bytes / {:.1} MiB) is very high — \
                     approaching protocol memory limits",
                    r.ram_bytes,
                    r.ram_bytes as f64 / (1024.0 * 1024.0)
                ),
                suggested_fix: "Reduce in-memory data structures; process data in \
                                streaming fashion rather than loading everything at once."
                    .to_string(),
            });
        } else if r.ram_bytes > 5 * 1024 * 1024 {
            out.push(Insight {
                severity: Severity::Warning,
                rule: self.name().to_string(),
                message: format!(
                    "RAM usage ({} bytes / {:.1} MiB) is elevated",
                    r.ram_bytes,
                    r.ram_bytes as f64 / (1024.0 * 1024.0)
                ),
                suggested_fix:
                    "Review large allocations; consider lazy initialization or smaller buffers."
                        .to_string(),
            });
        }

        out
    }
}

// ── Engine ────────────────────────────────────────────────────────────────────

/// The insights engine holds a set of rules and evaluates them against resource
/// metrics to produce an `InsightsReport`.
pub struct InsightsEngine {
    rules: Vec<Box<dyn InsightRule>>,
}

impl Clone for InsightsEngine {
    fn clone(&self) -> Self {
        Self::new()
    }
}

impl InsightsEngine {
    /// Create an engine pre-loaded with all built-in rules.
    pub fn new() -> Self {
        Self {
            rules: vec![
                Box::new(StorageEfficiencyRule),
                Box::new(InstructionDensityRule),
                Box::new(FootprintBloatRule),
                Box::new(MemoryPressureRule),
            ],
        }
    }

    /// Add a custom rule at runtime.
    #[allow(dead_code)]
    pub fn add_rule(&mut self, rule: Box<dyn InsightRule>) {
        self.rules.push(rule);
    }

    /// Run all rules and compute the efficiency score.
    pub fn analyze(&self, resources: &SorobanResources) -> InsightsReport {
        let insights: Vec<Insight> = self
            .rules
            .iter()
            .flat_map(|rule| rule.evaluate(resources))
            .collect();

        let efficiency_score = Self::compute_efficiency_score(resources, &insights);

        InsightsReport {
            efficiency_score,
            insights,
        }
    }

    /// Weighted efficiency score (0–100).
    ///
    /// Starts at 100 and deducts points for:
    /// - Each Critical insight: −20
    /// - Each Warning insight: −10
    /// - Each Info insight: −3
    /// - High absolute resource usage (graduated penalties)
    fn compute_efficiency_score(resources: &SorobanResources, insights: &[Insight]) -> u32 {
        let mut score: i32 = 100;

        // Deduct for insight severity.
        for insight in insights {
            match insight.severity {
                Severity::Critical => score -= 20,
                Severity::Warning => score -= 10,
                Severity::Info => score -= 3,
            }
        }

        // Graduated penalties for absolute resource consumption.

        // CPU: mild penalty above 10M, heavier above 50M.
        if resources.cpu_instructions > 50_000_000 {
            score -= 10;
        } else if resources.cpu_instructions > 10_000_000 {
            score -= 5;
        }

        // RAM: penalty above 5 MiB.
        if resources.ram_bytes > 20 * 1024 * 1024 {
            score -= 10;
        } else if resources.ram_bytes > 5 * 1024 * 1024 {
            score -= 5;
        }

        // Ledger I/O: penalty for heavy readers/writers.
        let total_ledger = resources.ledger_read_bytes + resources.ledger_write_bytes;
        if total_ledger > 100 * 1024 {
            score -= 10;
        } else if total_ledger > 50 * 1024 {
            score -= 5;
        }

        score.clamp(0, 100) as u32
    }
}

impl Default for InsightsEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_resources() -> SorobanResources {
        SorobanResources {
            cpu_instructions: 100_000,
            ram_bytes: 1_024,
            ledger_read_bytes: 256,
            ledger_write_bytes: 128,
            transaction_size_bytes: 512,
        }
    }

    // ── Efficiency score ──────────────────────────────────────────────────

    #[test]
    fn test_perfect_score_for_minimal_resources() {
        let engine = InsightsEngine::new();
        let report = engine.analyze(&minimal_resources());
        assert_eq!(report.efficiency_score, 100);
        assert!(report.insights.is_empty());
    }

    #[test]
    fn test_score_never_below_zero() {
        let engine = InsightsEngine::new();
        let r = SorobanResources {
            cpu_instructions: 500_000_000,
            ram_bytes: 50 * 1024 * 1024,
            ledger_read_bytes: 200 * 1024,
            ledger_write_bytes: 200 * 1024,
            transaction_size_bytes: 1_024,
        };
        let report = engine.analyze(&r);
        assert!(report.efficiency_score <= 100);
    }

    #[test]
    fn test_score_capped_at_100() {
        let engine = InsightsEngine::new();
        let report = engine.analyze(&SorobanResources::default());
        assert!(report.efficiency_score <= 100);
    }

    // ── Storage efficiency rule ───────────────────────────────────────────

    #[test]
    fn test_storage_efficiency_no_warning_when_balanced() {
        let rule = StorageEfficiencyRule;
        let r = SorobanResources {
            ledger_write_bytes: 400,
            transaction_size_bytes: 1_024,
            ..Default::default()
        };
        assert!(rule.evaluate(&r).is_empty());
    }

    #[test]
    fn test_storage_efficiency_warning_when_writes_exceed_tx_size() {
        let rule = StorageEfficiencyRule;
        let r = SorobanResources {
            ledger_write_bytes: 2_000,
            transaction_size_bytes: 1_024,
            ..Default::default()
        };
        let insights = rule.evaluate(&r);
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].severity, Severity::Warning);
        assert_eq!(insights[0].rule, "storage_efficiency");
    }

    #[test]
    fn test_storage_efficiency_critical_when_writes_double_tx_size() {
        let rule = StorageEfficiencyRule;
        let r = SorobanResources {
            ledger_write_bytes: 5_000,
            transaction_size_bytes: 1_024,
            ..Default::default()
        };
        let insights = rule.evaluate(&r);
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].severity, Severity::Critical);
    }

    #[test]
    fn test_storage_efficiency_skips_zero_tx_size() {
        let rule = StorageEfficiencyRule;
        let r = SorobanResources {
            ledger_write_bytes: 5_000,
            transaction_size_bytes: 0,
            ..Default::default()
        };
        assert!(rule.evaluate(&r).is_empty());
    }

    // ── Instruction density rule ──────────────────────────────────────────

    #[test]
    fn test_instruction_density_no_warning_when_balanced() {
        let rule = InstructionDensityRule;
        let r = SorobanResources {
            cpu_instructions: 5_000_000,
            ledger_read_bytes: 4_096,
            ledger_write_bytes: 2_048,
            ..Default::default()
        };
        assert!(rule.evaluate(&r).is_empty());
    }

    #[test]
    fn test_instruction_density_warning_high_cpu_low_ledger() {
        let rule = InstructionDensityRule;
        let r = SorobanResources {
            cpu_instructions: 15_000_000,
            ledger_read_bytes: 512,
            ledger_write_bytes: 256,
            ..Default::default()
        };
        let insights = rule.evaluate(&r);
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].severity, Severity::Warning);
        assert_eq!(insights[0].rule, "instruction_density");
    }

    #[test]
    fn test_instruction_density_critical_very_high_cpu() {
        let rule = InstructionDensityRule;
        let r = SorobanResources {
            cpu_instructions: 80_000_000,
            ledger_read_bytes: 256,
            ledger_write_bytes: 128,
            ..Default::default()
        };
        let insights = rule.evaluate(&r);
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].severity, Severity::Critical);
    }

    // ── Footprint bloat rule ──────────────────────────────────────────────

    #[test]
    fn test_footprint_bloat_no_warning_few_keys() {
        let rule = FootprintBloatRule;
        let r = SorobanResources {
            ledger_read_bytes: 256,
            ledger_write_bytes: 128,
            ..Default::default()
        };
        assert!(rule.evaluate(&r).is_empty());
    }

    #[test]
    fn test_footprint_bloat_warning_above_10_keys() {
        let rule = FootprintBloatRule;
        // ~11 estimated keys: (11 * 60) = 660 bytes
        let r = SorobanResources {
            ledger_read_bytes: 400,
            ledger_write_bytes: 300,
            ..Default::default()
        };
        let insights = rule.evaluate(&r);
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].severity, Severity::Warning);
        assert_eq!(insights[0].rule, "footprint_bloat");
    }

    #[test]
    fn test_footprint_bloat_critical_above_20_keys() {
        let rule = FootprintBloatRule;
        // ~25 estimated keys: 25 * 60 = 1500 bytes
        let r = SorobanResources {
            ledger_read_bytes: 1_000,
            ledger_write_bytes: 500,
            ..Default::default()
        };
        let insights = rule.evaluate(&r);
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].severity, Severity::Critical);
    }

    // ── Memory pressure rule ──────────────────────────────────────────────

    #[test]
    fn test_memory_pressure_no_warning_low_ram() {
        let rule = MemoryPressureRule;
        let r = SorobanResources {
            ram_bytes: 1_024 * 1_024,
            ..Default::default()
        };
        assert!(rule.evaluate(&r).is_empty());
    }

    #[test]
    fn test_memory_pressure_warning_elevated_ram() {
        let rule = MemoryPressureRule;
        let r = SorobanResources {
            ram_bytes: 10 * 1024 * 1024,
            ..Default::default()
        };
        let insights = rule.evaluate(&r);
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].severity, Severity::Warning);
        assert_eq!(insights[0].rule, "memory_pressure");
    }

    #[test]
    fn test_memory_pressure_critical_very_high_ram() {
        let rule = MemoryPressureRule;
        let r = SorobanResources {
            ram_bytes: 30 * 1024 * 1024,
            ..Default::default()
        };
        let insights = rule.evaluate(&r);
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].severity, Severity::Critical);
    }

    // ── Custom rule extensibility ─────────────────────────────────────────

    struct AlwaysWarnRule;

    impl InsightRule for AlwaysWarnRule {
        fn name(&self) -> &str {
            "always_warn"
        }

        fn evaluate(&self, _resources: &SorobanResources) -> Vec<Insight> {
            vec![Insight {
                severity: Severity::Info,
                rule: self.name().to_string(),
                message: "Custom rule triggered".to_string(),
                suggested_fix: "No action needed".to_string(),
            }]
        }
    }

    #[test]
    fn test_custom_rule_added_and_evaluated() {
        let mut engine = InsightsEngine::new();
        engine.add_rule(Box::new(AlwaysWarnRule));
        let report = engine.analyze(&minimal_resources());
        assert!(report.insights.iter().any(|i| i.rule == "always_warn"));
    }

    // ── Serialization ─────────────────────────────────────────────────────

    #[test]
    fn test_insights_report_serialization() {
        let report = InsightsReport {
            efficiency_score: 85,
            insights: vec![Insight {
                severity: Severity::Warning,
                rule: "test_rule".to_string(),
                message: "Test message".to_string(),
                suggested_fix: "Test fix".to_string(),
            }],
        };
        let json = serde_json::to_string(&report).unwrap();
        let deserialized: InsightsReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report, deserialized);
    }
}
