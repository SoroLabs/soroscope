use crate::simulation::{
    first_dimension_at_or_above, BytesByDurability, CostBreakdown, DurabilityByteCounts,
    LimitHeadroom, SorobanResources,
};
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

    fn evaluate_with_durability(
        &self,
        resources: &SorobanResources,
        _bytes_by_durability: &BytesByDurability,
    ) -> Vec<Insight> {
        self.evaluate(resources)
    }
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
        self.evaluate_with_durability(
            r,
            &BytesByDurability::from_aggregates_as_other(
                r.ledger_read_bytes,
                r.ledger_write_bytes,
            ),
        )
    }

    fn evaluate_with_durability(
        &self,
        r: &SorobanResources,
        bytes_by_durability: &BytesByDurability,
    ) -> Vec<Insight> {
        let mut out = Vec::new();

        // Skip if there's no meaningful data to analyse.
        if r.transaction_size_bytes == 0 {
            return out;
        }

        let write_ratio = r.ledger_write_bytes as f64 / r.transaction_size_bytes as f64;
        let (dominant_class, dominant_bytes) = dominant_write_class(&bytes_by_durability.write);
        let advice = if dominant_class == "temporary" {
            "Review temporary data volume and TTL; batch writes where possible."
        } else {
            "Use temporary storage for ephemeral data and batch writes where possible."
        };

        if write_ratio > 2.0 {
            out.push(Insight {
                severity: Severity::Critical,
                rule: self.name().to_string(),
                message: format!(
                    "Ledger write bytes ({}) are {:.1}x the transaction size ({}) \
                     — extremely write-heavy; dominant class: {} ({} bytes)",
                    r.ledger_write_bytes, write_ratio, r.transaction_size_bytes, dominant_class, dominant_bytes
                ),
                suggested_fix: advice.to_string(),
            });
        } else if write_ratio > 1.0 {
            out.push(Insight {
                severity: Severity::Warning,
                rule: self.name().to_string(),
                message: format!(
                    "Ledger write bytes ({}) exceed transaction size ({}) \
                     — dominant class: {} ({} bytes); consider reviewing storage layout",
                    r.ledger_write_bytes, r.transaction_size_bytes, dominant_class, dominant_bytes
                ),
                suggested_fix: if dominant_class == "temporary" {
                    "Review temporary data volume and TTL; consolidate writes or use compact serialization.".to_string()
                } else {
                    "Consolidate writes into fewer ledger keys or use compact serialization.".to_string()
                },
            });
        }

        out
    }
}

fn dominant_write_class(bytes: &DurabilityByteCounts) -> (&'static str, u64) {
    [
        ("code", bytes.code),
        ("instance", bytes.instance),
        ("persistent", bytes.persistent),
        ("temporary", bytes.temporary),
        ("other", bytes.other),
    ]
    .into_iter()
    .max_by_key(|(_, count)| *count)
    .unwrap_or(("other", 0))
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

/// Flags when total event XDR volume exceeds a configurable fraction of transaction size.
pub struct EventVolumeRule {
    pub max_event_tx_ratio: f64,
}

impl Default for EventVolumeRule {
    fn default() -> Self {
        Self { max_event_tx_ratio: 0.5 }
    }
}

impl InsightRule for EventVolumeRule {
    fn name(&self) -> &str {
        "event_volume"
    }

    fn evaluate(&self, r: &SorobanResources) -> Vec<Insight> {
        let mut out = Vec::new();
        if r.transaction_size_bytes == 0 {
            return out;
        }

        if let Some(event_bytes) = r.event_xdr_bytes {
            let ratio = event_bytes as f64 / r.transaction_size_bytes as f64;
            if ratio > self.max_event_tx_ratio {
                out.push(Insight {
                    severity: Severity::Warning,
                    rule: self.name().to_string(),
                    message: format!(
                        "Event XDR volume ({} bytes) is {:.1}% of transaction size ({}) — high indexer overhead",
                        event_bytes,
                        ratio * 100.0,
                        r.transaction_size_bytes
                    ),
                    suggested_fix: "Consolidate event emissions or remove redundant diagnostic topic logs."
                        .to_string(),
                });
            }
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

/// Emits an insight when AMM tick crossing projection is inside headroom or next doubling exceeds read-entry limits.
pub struct ConcentratedAmmTickProfileRule;

impl InsightRule for ConcentratedAmmTickProfileRule {
    fn name(&self) -> &str {
        "amm_tick_crossing_profile"
    }

    fn evaluate(&self, r: &SorobanResources) -> Vec<Insight> {
        let mut out = Vec::new();

        if let Some(report) = &r.amm_tick_profile_report {
            if let Some(warning) = &report.warning_insight {
                out.push(Insight {
                    severity: Severity::Warning,
                    rule: self.name().to_string(),
                    message: warning.clone(),
                    suggested_fix: format!(
                        "Limit swap size to cross at most {} ticks or split transactions across multiple blocks",
                        report.max_supported_ticks
                    ),
                });
            }
        }

        out
    }
}

/// Severity thresholds for limit-headroom insights (#991).
///
/// Critical at ≥90% of any dimension; Warning at ≥75%. The message names the
/// first failing dimension in fixed order: CPU, memory, read entries, write
/// entries, transaction size.
pub struct LimitHeadroomRule;

/// Build headroom insights from a precomputed `LimitHeadroom` report.
///
/// Used both by `LimitHeadroomRule` (when headroom is present on resources)
/// and by `to_report`, which always has headroom available even if the rule
/// itself has nothing to read from `SorobanResources`.
pub fn insights_from_headroom(headroom: &LimitHeadroom, rule_name: &str) -> Vec<Insight> {
    let mut out = Vec::new();

    if let Some(dimension) = first_dimension_at_or_above(headroom, 90) {
        let used = match dimension {
            "cpu_instructions" => headroom.cpu_instructions.percent_used,
            "memory_bytes" => headroom.memory_bytes.percent_used,
            "read_entries" => headroom.read_entries.percent_used,
            "write_entries" => headroom.write_entries.percent_used,
            _ => headroom.transaction_size_bytes.percent_used,
        };
        out.push(Insight {
            severity: Severity::Critical,
            rule: rule_name.to_string(),
            message: format!(
                "Resource usage is critical on {dimension}: {used}% of the network limit \
                 (source: {:?})",
                headroom.limits_source
            ),
            suggested_fix: format!(
                "Reduce {dimension} usage below 75% of the limit, or split the work across \
                 multiple transactions."
            ),
        });
        return out;
    }

    if let Some(dimension) = first_dimension_at_or_above(headroom, 75) {
        let used = match dimension {
            "cpu_instructions" => headroom.cpu_instructions.percent_used,
            "memory_bytes" => headroom.memory_bytes.percent_used,
            "read_entries" => headroom.read_entries.percent_used,
            "write_entries" => headroom.write_entries.percent_used,
            _ => headroom.transaction_size_bytes.percent_used,
        };
        out.push(Insight {
            severity: Severity::Warning,
            rule: rule_name.to_string(),
            message: format!(
                "Resource usage is elevated on {dimension}: {used}% of the network limit \
                 (source: {:?})",
                headroom.limits_source
            ),
            suggested_fix: format!(
                "Consider trimming {dimension} before it reaches the 90% critical threshold."
            ),
        });
    }

    out
}

impl InsightRule for LimitHeadroomRule {
    fn name(&self) -> &str {
        "limit_headroom"
    }

    /// `SorobanResources` does not carry entry counts; the rule is a no-op
    /// here and headroom insights are attached in `to_report` from the
    /// precomputed `LimitHeadroom` on `SimulationResult`.
    fn evaluate(&self, _r: &SorobanResources) -> Vec<Insight> {
        Vec::new()
    }
}

// ── DominantCostTypeRule ──────────────────────────────────────────────────────

/// Fires when a single Soroban host cost type accounts for more than 40 % of
/// the total CPU budget attributed by the node's diagnostic events.
///
/// The rule is only evaluated when a `CostBreakdown` is present; when no
/// breakdown was returned the rule produces no output and the simulation is
/// not failed.
pub struct DominantCostTypeRule {
    /// Fraction above which a cost type is considered dominant (default 0.40).
    pub threshold: f64,
}

impl Default for DominantCostTypeRule {
    fn default() -> Self {
        Self { threshold: 0.40 }
    }
}

impl DominantCostTypeRule {
    /// Return an actionable hint for the supplied cost-type name.
    fn hint_for(cost_type: &str) -> &'static str {
        match cost_type {
            // Pure WASM arithmetic / loops
            "WasmInsnExec" => {
                "WASM instruction execution dominates. Hoist repeated computations \
                 out of loops, replace iterative algorithms with O(1) formulas, or \
                 move heavy arithmetic off-chain and verify the result on-chain."
            }
            // Hash / crypto operations
            "ComputeSha256Hash"
            | "ComputeEd25519PubKey"
            | "VerifyEd25519Sig"
            | "ComputeKeccak256Hash"
            | "RecoverEcdsaSecp256k1Key" => {
                "Cryptographic operations dominate. Cache derived public keys or \
                 hashes in persistent storage instead of recomputing them on every \
                 invocation."
            }
            // Cross-contract / VM invocation overhead
            "InvokeVmFunction" | "InvokeHostFunction" => {
                "Cross-contract call overhead dominates. Batch operations into a \
                 single invocation, inline simple callee logic into the caller, or \
                 restructure to reduce the number of sub-invocations."
            }
            // Object / value manipulation
            "VisitObject" | "CloneEvents" => {
                "Host-object access dominates. Minimise the number of distinct \
                 host values allocated per invocation; prefer small, flat data \
                 structures over deeply nested maps or vecs."
            }
            // Ledger I/O
            "ReadLedgerEntry" | "WriteLedgerEntry" => {
                "Ledger entry I/O dominates. Batch reads and writes, consolidate \
                 related data under fewer keys, and avoid reading the same entry \
                 multiple times within a single invocation."
            }
            // Memory
            "VmMemCpy" | "VmMemRead" | "VmMemWrite" => {
                "Linear-memory operations dominate. Reduce large allocations inside \
                 the contract; process data in a streaming fashion rather than \
                 buffering the entire payload."
            }
            // Catch-all
            _ => {
                "One cost type dominates the budget. Profile which operation \
                 produces this type and either hoist it out of any loops or \
                 restructure the algorithm to call it less frequently."
            }
        }
    }
}

impl InsightRule for DominantCostTypeRule {
    fn name(&self) -> &str {
        "dominant_cost_type"
    }

    fn evaluate(&self, _resources: &SorobanResources) -> Vec<Insight> {
        // Cannot evaluate without a breakdown — use `evaluate_with_cost_breakdown`.
        Vec::new()
    }
}

impl DominantCostTypeRule {
    /// Core evaluation logic; called by `InsightsEngine::analyze_with_cost_breakdown`.
    pub fn evaluate_breakdown(
        &self,
        breakdown: &CostBreakdown,
    ) -> Vec<Insight> {
        let mut out = Vec::new();
        if breakdown.total_cpu == 0 {
            return out;
        }

        if let Some((cost_type, entry)) = breakdown.dominant_cpu_type() {
            let fraction = breakdown.cpu_fraction(cost_type);
            if fraction > self.threshold {
                let pct = fraction * 100.0;
                out.push(Insight {
                    severity: Severity::Warning,
                    rule: self.name().to_string(),
                    message: format!(
                        "Cost type '{}' accounts for {:.1}% of total CPU \
                         ({} of {} units) — single type dominates budget",
                        cost_type, pct, entry.cpu_units, breakdown.total_cpu
                    ),
                    suggested_fix: Self::hint_for(cost_type).to_string(),
                });
            }
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
                Box::new(ConcentratedAmmTickProfileRule),
                Box::new(LimitHeadroomRule),
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

    /// Run the standard rules plus insights derived from measured ledger entries.
    pub fn analyze_with_additional_insights(
        &self,
        resources: &SorobanResources,
        additional_insights: Vec<Insight>,
    ) -> InsightsReport {
        let bytes = BytesByDurability::from_aggregates_as_other(
            resources.ledger_read_bytes,
            resources.ledger_write_bytes,
        );
        self.analyze_with_durability_and_additional_insights(resources, &bytes, additional_insights)
    }

    pub fn analyze_with_durability_and_additional_insights(
        &self,
        resources: &SorobanResources,
        bytes_by_durability: &BytesByDurability,
        additional_insights: Vec<Insight>,
    ) -> InsightsReport {
        let mut insights: Vec<Insight> = self
            .rules
            .iter()
            .flat_map(|rule| rule.evaluate_with_durability(resources, bytes_by_durability))
            .collect();
        insights.extend(additional_insights);
        let efficiency_score = Self::compute_efficiency_score(resources, &insights);
        InsightsReport {
            efficiency_score,
            insights,
        }
    }

    /// Run all rules plus the auth-tree size check.
    ///
    /// Fires a `Warning` when the total XDR bytes of authorization entries
    /// would exceed the transaction size limit.
    pub fn analyze_with_auth_tree(
        &self,
        resources: &SorobanResources,
        auth_tree: &crate::simulation::AuthTreeReport,
    ) -> InsightsReport {
        let mut insights: Vec<Insight> = self
            .rules
            .iter()
            .flat_map(|rule| rule.evaluate(resources))
            .collect();

        if auth_tree.exceeds_transaction_size_limit {
            insights.push(Insight {
                severity: Severity::Warning,
                rule: "auth_tree_size".to_string(),
                message: format!(
                    "Authorization bytes ({}) exceed the transaction size limit ({} bytes). \
                     Large auth trees inflate transaction fees and may be rejected by some nodes.",
                    auth_tree.total_xdr_bytes, auth_tree.transaction_size_limit_bytes
                ),
                suggested_fix: "Reduce the number of authorization entries or shorten \
                                 the invocation sub-tree they cover."
                    .to_string(),
            });
        }

        let efficiency_score = Self::compute_efficiency_score(resources, &insights);
        InsightsReport {
            efficiency_score,
            insights,
        }
    }

    /// Run all rules plus the dominant cost-type rule, given an optional
    /// per-cost-type breakdown.
    ///
    /// When `cost_breakdown` is `None` the output is identical to `analyze`.
    /// The dominant-cost-type rule only fires when a breakdown is present and
    /// one cost type exceeds the configured threshold (default 40 %).
    pub fn analyze_with_cost_breakdown(
        &self,
        resources: &SorobanResources,
        cost_breakdown: Option<&CostBreakdown>,
    ) -> InsightsReport {
        let mut insights: Vec<Insight> = self
            .rules
            .iter()
            .flat_map(|rule| rule.evaluate(resources))
            .collect();

        if let Some(breakdown) = cost_breakdown {
            let rule = DominantCostTypeRule::default();
            insights.extend(rule.evaluate_breakdown(breakdown));
        }

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
                    ..Default::default()
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
    fn test_auth_tree_over_limit_emits_auth_bytes_warning() {
        let engine = InsightsEngine::new();
        let auth_tree = AuthTreeReport {
            entry_count: 2,
            max_depth: 2,
            credential_kinds: vec![
                crate::simulation::AuthCredentialKind::Ed25519,
                crate::simulation::AuthCredentialKind::Contract,
            ],
            total_xdr_bytes: 100_001,
            transaction_size_limit_bytes: 100_000,
            exceeds_transaction_size_limit: true,
            auth_cpu_instructions: None,
        };

        let report = engine.analyze_with_auth_tree(&minimal_resources(), &auth_tree);
        let insight = report
            .insights
            .iter()
            .find(|insight| insight.rule == "auth_tree_size")
            .expect("oversized auth tree should produce a warning");
        assert_eq!(insight.severity, Severity::Warning);
        assert!(insight.message.contains("Authorization bytes (100001)"));
        assert!(insight.message.contains("100000 bytes"));
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
                    ..Default::default()
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
    fn temporary_only_writes_do_not_recommend_temporary_storage() {
        let rule = StorageEfficiencyRule;
        let resources = SorobanResources {
            ledger_write_bytes: 5_000,
            transaction_size_bytes: 1_024,
            ..Default::default()
        };
        let breakdown = BytesByDurability {
            write: DurabilityByteCounts {
                temporary: 5_000,
                ..Default::default()
            },
            ..Default::default()
        };

        let insights = rule.evaluate_with_durability(&resources, &breakdown);
        assert_eq!(insights.len(), 1);
        assert!(insights[0].message.contains("temporary (5000 bytes)"));
        assert!(!insights[0].suggested_fix.contains("Use temporary storage"));
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

    // ── Concentrated AMM tick profile rule ───────────────────────────────

    #[test]
    fn test_concentrated_amm_tick_profile_rule() {
        use crate::simulation::ConcentratedAmmTickProfileReport;

        let rule = ConcentratedAmmTickProfileRule;
        let r = SorobanResources {
            amm_tick_profile_report: Some(ConcentratedAmmTickProfileReport {
                status: "success".to_string(),
                measurements: vec![],
                max_supported_ticks: 4,
                warning_insight: Some(
                    "Next doubling to 8 ticks would exceed read-entry limit (40); max supported ticks is 4"
                        .to_string(),
                ),
            }),
            ..Default::default()
        };

        let insights = rule.evaluate(&r);
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].severity, Severity::Warning);
        assert_eq!(insights[0].rule, "amm_tick_crossing_profile");
        assert!(insights[0].message.contains("max supported ticks is 4"));
    }

    // ── DominantCostTypeRule ──────────────────────────────────────────────

    fn make_breakdown(dominant_cpu: u64, total_cpu: u64) -> CostBreakdown {
        let mut entries = std::collections::HashMap::new();
        entries.insert(
            "WasmInsnExec".to_string(),
            crate::simulation::CostEntry { cpu_units: dominant_cpu, mem_bytes: 0 },
        );
        CostBreakdown {
            entries,
            remainder_cpu: total_cpu.saturating_sub(dominant_cpu),
            remainder_mem: 0,
            total_cpu,
            total_mem: 0,
        }
    }

    // ── #991: limit headroom insights ─────────────────────────────────────

    fn headroom_fixture(write_used_pct: u32) -> crate::simulation::LimitHeadroom {
        let write_used = if write_used_pct >= 100 { 21 } else { (write_used_pct * 20) / 100 };
        crate::simulation::compute_limit_headroom(
            &SorobanResources::default(),
            Some((30, write_used)),
            &crate::simulation::NetworkLimits {
                max_write_entries: 20,
                ..Default::default()
            },
            crate::simulation::LimitsSource::Builtin,
        )
    }

    #[test]
    fn limit_headroom_critical_at_90_percent_names_write_entries() {
        // 18/20 = 90%
        let headroom = headroom_fixture(90);
        assert_eq!(headroom.write_entries.percent_used, 90);
        let insights = insights_from_headroom(&headroom, "limit_headroom");
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].severity, Severity::Critical);
        assert_eq!(insights[0].rule, "limit_headroom");
        assert!(insights[0].message.contains("write_entries"));
        assert!(insights[0].message.contains("90%"));
    }

    #[test]
    fn limit_headroom_warning_at_75_percent_and_silent_below() {
        // 15/20 = 75% → Warning
        let warning = headroom_fixture(75);
        let insights = insights_from_headroom(&warning, "limit_headroom");
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].severity, Severity::Warning);
        assert!(insights[0].message.contains("write_entries"));

        // 10/20 = 50% → no insight
        let quiet = headroom_fixture(50);
        assert!(insights_from_headroom(&quiet, "limit_headroom").is_empty());
    }

    #[test]
    fn limit_headroom_critical_beats_warning_even_if_later_dimension_is_worse() {
        // CPU at 95% (Critical) and write_entries at 80% (would be Warning).
        // The rule must report CPU first, not the later dimension.
        let limits = crate::simulation::NetworkLimits::default();
        let resources = SorobanResources {
            cpu_instructions: 95_000_000,
            ..Default::default()
        };
        let headroom = crate::simulation::compute_limit_headroom(
            &resources,
            Some((30, 16)), // 16/20 = 80%
            &limits,
            crate::simulation::LimitsSource::Builtin,
        );
        let insights = insights_from_headroom(&headroom, "limit_headroom");
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].severity, Severity::Critical);
        assert!(insights[0].message.contains("cpu_instructions"));
    }

    #[test]
    fn dominant_cost_type_fires_above_threshold() {
        let rule = DominantCostTypeRule::default();
        // WasmInsnExec = 80 of 100 total = 80% → above 40% threshold
        let breakdown = make_breakdown(80, 100);
        let insights = rule.evaluate_breakdown(&breakdown);
        assert_eq!(insights.len(), 1);
        assert_eq!(insights[0].severity, Severity::Warning);
        assert_eq!(insights[0].rule, "dominant_cost_type");
        assert!(insights[0].message.contains("WasmInsnExec"));
        assert!(insights[0].message.contains("80.0%"));
    }

    #[test]
    fn dominant_cost_type_silent_below_threshold() {
        let rule = DominantCostTypeRule::default();
        // WasmInsnExec = 30 of 100 total = 30% → below 40% threshold
        let breakdown = make_breakdown(30, 100);
        let insights = rule.evaluate_breakdown(&breakdown);
        assert!(insights.is_empty());
    }

    #[test]
    fn dominant_cost_type_silent_when_no_breakdown() {
        let engine = InsightsEngine::new();
        let report = engine.analyze_with_cost_breakdown(&minimal_resources(), None);
        // No dominant_cost_type insight should appear when breakdown is absent
        assert!(!report.insights.iter().any(|i| i.rule == "dominant_cost_type"));
    }

    #[test]
    fn dominant_cost_type_fires_through_engine() {
        let engine = InsightsEngine::new();
        let breakdown = make_breakdown(90, 100); // 90% → well above 40%
        let report = engine.analyze_with_cost_breakdown(&minimal_resources(), Some(&breakdown));
        let insight = report
            .insights
            .iter()
            .find(|i| i.rule == "dominant_cost_type")
            .expect("dominant_cost_type insight should be present");
        assert_eq!(insight.severity, Severity::Warning);
        assert!(insight.suggested_fix.contains("WASM instruction"));
    }

    #[test]
    fn dominant_cost_type_hint_keyed_off_type() {
        let rule = DominantCostTypeRule::default();
        // InvokeVmFunction = 95 of 100 → cross-contract hint
        let mut entries = std::collections::HashMap::new();
        entries.insert(
            "InvokeVmFunction".to_string(),
            crate::simulation::CostEntry { cpu_units: 95, mem_bytes: 0 },
        );
        let breakdown = CostBreakdown {
            entries,
            remainder_cpu: 5,
            remainder_mem: 0,
            total_cpu: 100,
            total_mem: 0,
        };
        let insights = rule.evaluate_breakdown(&breakdown);
        assert_eq!(insights.len(), 1);
        assert!(
            insights[0].suggested_fix.contains("sub-invocation"),
            "expected cross-contract hint, got: {}",
            insights[0].suggested_fix
        );
    }

    #[test]
    fn dominant_cost_type_percentages_sum_to_100() {
        use crate::simulation::CostEntry;
        // Three types, verify remainder accounting
        let mut entries = std::collections::HashMap::new();
        entries.insert("WasmInsnExec".to_string(), CostEntry { cpu_units: 60, mem_bytes: 10 });
        entries.insert("VisitObject".to_string(), CostEntry { cpu_units: 30, mem_bytes: 5 });
        let breakdown = CostBreakdown {
            entries,
            remainder_cpu: 10,
            remainder_mem: 2,
            total_cpu: 100,
            total_mem: 17,
        };
        assert!(breakdown.is_consistent());
        // Fractions: WasmInsnExec=60%, VisitObject=30%, remainder=10% → sums to 100%
        let wasm_frac = breakdown.cpu_fraction("WasmInsnExec");
        let visit_frac = breakdown.cpu_fraction("VisitObject");
        let remainder_frac =
            breakdown.remainder_cpu as f64 / breakdown.total_cpu as f64;
        let total = (wasm_frac + visit_frac + remainder_frac) * 100.0;
        assert!((total - 100.0).abs() < 0.001, "fractions sum to {}", total);
    }

    #[test]
    fn cost_breakdown_is_consistent_rejects_bad_remainder() {
        use crate::simulation::CostEntry;
        let mut entries = std::collections::HashMap::new();
        entries.insert("WasmInsnExec".to_string(), CostEntry { cpu_units: 80, mem_bytes: 0 });
        // remainder_cpu is wrong (should be 20 for total 100, but we set 0)
        let bad = CostBreakdown {
            entries,
            remainder_cpu: 0,
            remainder_mem: 0,
            total_cpu: 100,
            total_mem: 0,
        };
        assert!(!bad.is_consistent());
    }

    #[test]
    fn cost_breakdown_with_zero_total_cpu_returns_zero_fraction() {
        let breakdown = CostBreakdown::default();
        assert_eq!(breakdown.cpu_fraction("WasmInsnExec"), 0.0);
        assert_eq!(breakdown.dominant_cpu_type(), None);
    }
}
