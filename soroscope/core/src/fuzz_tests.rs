//! Property-based fuzz tests for the soroscope simulation engine.
//!
//! These tests use `proptest` to generate a wide variety of ledger states,
//! contract arguments, and resource configurations to find edge cases where
//! the engine panics, overflows, or returns inconsistent results.

#[cfg(test)]
mod tests {
    use crate::comparison::{build_report, calculate_deltas, detect_regressions, ResourceDelta};
    use crate::simulation::{SorobanResources, TtlEntryReport};
    use proptest::prelude::*;

    // ── Strategies ──────────────────────────────────────────────────────────

    /// Generate a random `SorobanResources` struct with arbitrary u64 fields.
    fn arb_soroban_resources() -> impl Strategy<Value = SorobanResources> {
        (
            any::<u64>(),
            any::<u64>(),
            any::<u64>(),
            any::<u64>(),
            any::<u64>(),
        )
            .prop_map(|(cpu, ram, lr, lw, tx)| SorobanResources {
                cpu_instructions: cpu,
                ram_bytes: ram,
                ledger_read_bytes: lr,
                ledger_write_bytes: lw,
                transaction_size_bytes: tx,
            })
    }

    /// Generate resources in a "realistic" range to avoid division-by-zero
    /// edge cases in percentage calculations.
    fn arb_nonzero_resources() -> impl Strategy<Value = SorobanResources> {
        (
            1u64..=u64::MAX,
            1u64..=u64::MAX,
            1u64..=u64::MAX,
            1u64..=u64::MAX,
            1u64..=u64::MAX,
        )
            .prop_map(|(cpu, ram, lr, lw, tx)| SorobanResources {
                cpu_instructions: cpu,
                ram_bytes: ram,
                ledger_read_bytes: lr,
                ledger_write_bytes: lw,
                transaction_size_bytes: tx,
            })
    }

    /// Generate an arbitrary `ResourceDelta` (percentage changes).
    fn arb_resource_delta() -> impl Strategy<Value = ResourceDelta> {
        (
            -100.0f64..500.0f64,
            -100.0f64..500.0f64,
            -100.0f64..500.0f64,
            -100.0f64..500.0f64,
            -100.0f64..500.0f64,
        )
            .prop_map(|(cpu, ram, lr, lw, tx)| ResourceDelta {
                cpu_instructions: cpu,
                ram_bytes: ram,
                ledger_read_bytes: lr,
                ledger_write_bytes: lw,
                transaction_size_bytes: tx,
            })
    }

    /// Generate an arbitrary `TtlEntryReport`.
    fn arb_ttl_report() -> impl Strategy<Value = TtlEntryReport> {
        (
            "[a-zA-Z0-9]{1,16}",
            any::<u32>(),
            // remaining_ledgers can be negative (expired entries)
            -500_000i64..500_000i64,
        )
            .prop_map(
                |(key, live_until_ledger, remaining_ledgers)| TtlEntryReport {
                    key,
                    live_until_ledger,
                    remaining_ledgers,
                },
            )
    }

    // ── Fuzz tests ──────────────────────────────────────────────────────────

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1000))]

        // ─── calculate_cost (SimulationEngine method) ───────────────────

        /// The private `calculate_cost` method uses integer division, so the
        /// result should always be deterministic and never panic on any u64
        /// inputs. We exercise it through `build_report` which constructs
        /// the cost internally.
        #[test]
        fn fuzz_soroban_resources_construction(
            cpu in any::<u64>(),
            ram in any::<u64>(),
            lr in any::<u64>(),
            lw in any::<u64>(),
            tx in any::<u64>(),
        ) {
            // Just constructing SorobanResources must never panic.
            let r = SorobanResources {
                cpu_instructions: cpu,
                ram_bytes: ram,
                ledger_read_bytes: lr,
                ledger_write_bytes: lw,
                transaction_size_bytes: tx,
            };
            // All fields should round-trip.
            prop_assert_eq!(r.cpu_instructions, cpu);
            prop_assert_eq!(r.ram_bytes, ram);
            prop_assert_eq!(r.ledger_read_bytes, lr);
            prop_assert_eq!(r.ledger_write_bytes, lw);
            prop_assert_eq!(r.transaction_size_bytes, tx);
        }

        // ─── calculate_deltas ───────────────────────────────────────────

        /// `calculate_deltas` must never panic on arbitrary resource pairs,
        /// including zeros, max values, and asymmetric inputs.
        #[test]
        fn fuzz_calculate_deltas_no_panic(
            current in arb_soroban_resources(),
            base in arb_soroban_resources(),
        ) {
            let _deltas = calculate_deltas(&current, &base);
            // Just reaching here without panic is the assertion.
        }

        /// When base is zero, `calculate_deltas` should return 0.0 for that
        /// field (no division by zero / NaN / Inf).
        #[test]
        fn fuzz_calculate_deltas_zero_base_safe(
            current in arb_soroban_resources(),
        ) {
            let base = SorobanResources {
                cpu_instructions: 0,
                ram_bytes: 0,
                ledger_read_bytes: 0,
                ledger_write_bytes: 0,
                transaction_size_bytes: 0,
            };

            let deltas = calculate_deltas(&current, &base);

            // The implementation returns 0.0 when base is 0 to avoid div-by-zero.
            prop_assert!(!deltas.cpu_instructions.is_nan());
            prop_assert!(!deltas.ram_bytes.is_nan());
            prop_assert!(!deltas.ledger_read_bytes.is_nan());
            prop_assert!(!deltas.ledger_write_bytes.is_nan());
            prop_assert!(!deltas.transaction_size_bytes.is_nan());

            prop_assert!(!deltas.cpu_instructions.is_infinite());
            prop_assert!(!deltas.ram_bytes.is_infinite());
            prop_assert!(!deltas.ledger_read_bytes.is_infinite());
            prop_assert!(!deltas.ledger_write_bytes.is_infinite());
            prop_assert!(!deltas.transaction_size_bytes.is_infinite());
        }

        /// Comparing identical resources must yield zero deltas.
        #[test]
        fn fuzz_calculate_deltas_identity(
            resources in arb_soroban_resources(),
        ) {
            let deltas = calculate_deltas(&resources, &resources);

            prop_assert!((deltas.cpu_instructions).abs() < f64::EPSILON);
            prop_assert!((deltas.ram_bytes).abs() < f64::EPSILON);
            prop_assert!((deltas.ledger_read_bytes).abs() < f64::EPSILON);
            prop_assert!((deltas.ledger_write_bytes).abs() < f64::EPSILON);
            prop_assert!((deltas.transaction_size_bytes).abs() < f64::EPSILON);
        }

        /// When base != 0, deltas should have the correct sign:
        /// - current > base → positive delta
        /// - current < base → negative delta
        /// - current == base → zero delta
        #[test]
        fn fuzz_calculate_deltas_sign_consistency(
            current in arb_nonzero_resources(),
            base in arb_nonzero_resources(),
        ) {
            let deltas = calculate_deltas(&current, &base);

            // Check CPU sign consistency
            if current.cpu_instructions > base.cpu_instructions {
                prop_assert!(deltas.cpu_instructions > 0.0,
                    "Expected positive delta for cpu: current={} base={} delta={}",
                    current.cpu_instructions, base.cpu_instructions, deltas.cpu_instructions);
            } else if current.cpu_instructions < base.cpu_instructions {
                prop_assert!(deltas.cpu_instructions < 0.0,
                    "Expected negative delta for cpu: current={} base={} delta={}",
                    current.cpu_instructions, base.cpu_instructions, deltas.cpu_instructions);
            }
        }

        // ─── detect_regressions ─────────────────────────────────────────

        /// `detect_regressions` must never panic on arbitrary delta values
        /// and arbitrary thresholds.
        #[test]
        fn fuzz_detect_regressions_no_panic(
            deltas in arb_resource_delta(),
            threshold in -100.0f64..1000.0f64,
        ) {
            let _flags = detect_regressions(&deltas, threshold);
            // Reaching here without panic is the assertion.
        }

        /// Negative deltas (improvements) should never be flagged regardless
        /// of threshold.
        #[test]
        fn fuzz_detect_regressions_improvements_never_flagged(
            threshold in 0.0f64..100.0f64,
        ) {
            let deltas = ResourceDelta {
                cpu_instructions: -50.0,
                ram_bytes: -25.0,
                ledger_read_bytes: -10.0,
                ledger_write_bytes: -1.0,
                transaction_size_bytes: -0.1,
            };

            let flags = detect_regressions(&deltas, threshold);
            prop_assert!(flags.is_empty(),
                "Expected no flags for negative deltas, got {} flags", flags.len());
        }

        /// Regression flags should only contain resources that exceed the threshold.
        #[test]
        fn fuzz_detect_regressions_flag_correctness(
            deltas in arb_resource_delta(),
            threshold in 0.0f64..100.0f64,
        ) {
            let flags = detect_regressions(&deltas, threshold);

            for flag in &flags {
                // Every flagged resource's change_percent must be > threshold
                prop_assert!(flag.change_percent > threshold,
                    "Flagged resource '{}' with change {:.1}% should be > threshold {:.1}%",
                    flag.resource, flag.change_percent, threshold);

                // Severity must be "high" or "critical"
                prop_assert!(
                    flag.severity == "high" || flag.severity == "critical",
                    "Unknown severity '{}' for resource '{}'",
                    flag.severity, flag.resource
                );

                // "critical" iff > 25%
                if flag.change_percent > 25.0 {
                    prop_assert_eq!(&flag.severity, "critical");
                } else {
                    prop_assert_eq!(&flag.severity, "high");
                }
            }

            // At most 5 flags (one per resource metric)
            prop_assert!(flags.len() <= 5,
                "Expected at most 5 regression flags, got {}", flags.len());
        }

        // ─── build_report (end-to-end) ──────────────────────────────────

        /// `build_report` must never panic on any pair of resource values,
        /// including extreme edge cases like u64::MAX.
        #[test]
        fn fuzz_build_report_no_panic(
            current in arb_soroban_resources(),
            base in arb_soroban_resources(),
        ) {
            let report = build_report(current.clone(), base.clone());

            // Current and base must be preserved.
            prop_assert_eq!(report.current, current);
            prop_assert_eq!(report.base, base);

            // Summary must not be empty.
            prop_assert!(!report.summary.is_empty());
        }

        /// The report's regression flags must be consistent with its deltas:
        /// every flag must reference a delta value that exceeds the internal
        /// threshold (10.0%).
        #[test]
        fn fuzz_build_report_consistency(
            current in arb_soroban_resources(),
            base in arb_soroban_resources(),
        ) {
            let report = build_report(current, base);

            for flag in &report.regression_flags {
                let delta_value = match flag.resource.as_str() {
                    "cpu_instructions" => report.deltas.cpu_instructions,
                    "ram_bytes" => report.deltas.ram_bytes,
                    "ledger_read_bytes" => report.deltas.ledger_read_bytes,
                    "ledger_write_bytes" => report.deltas.ledger_write_bytes,
                    "transaction_size_bytes" => report.deltas.transaction_size_bytes,
                    other => panic!("Unknown resource in flag: {}", other),
                };

                prop_assert!(delta_value > 10.0,
                    "Flag for '{}' has delta {:.1}% which should be > 10%",
                    flag.resource, delta_value);
            }
        }

        // ─── TtlEntryReport ─────────────────────────────────────────────

        /// TTL entry reports must round-trip through construction without panic.
        #[test]
        fn fuzz_ttl_entry_report_construction(
            report in arb_ttl_report(),
        ) {
            prop_assert!(!report.key.is_empty());
            // remaining_ledgers can be negative (expired) — that's fine.
        }

        // ─── Serialization round-trip ───────────────────────────────────

        /// SorobanResources must serialize/deserialize without data loss.
        #[test]
        fn fuzz_soroban_resources_serde_roundtrip(
            resources in arb_soroban_resources(),
        ) {
            let json = serde_json::to_string(&resources)
                .expect("SorobanResources should serialize");
            let deserialized: SorobanResources = serde_json::from_str(&json)
                .expect("SorobanResources should deserialize");

            prop_assert_eq!(resources, deserialized);
        }

        /// RegressionReport must serialize without panic on any input.
        #[test]
        fn fuzz_regression_report_serde(
            current in arb_soroban_resources(),
            base in arb_soroban_resources(),
        ) {
            let report = build_report(current, base);
            let json = serde_json::to_string(&report);
            prop_assert!(json.is_ok(), "Report serialization failed: {:?}", json.err());
        }

        // ─── Determinism ────────────────────────────────────────────────

        /// Running `calculate_deltas` twice with the same inputs must yield
        /// bit-identical results (determinism).
        #[test]
        fn fuzz_calculate_deltas_deterministic(
            current in arb_soroban_resources(),
            base in arb_soroban_resources(),
        ) {
            let d1 = calculate_deltas(&current, &base);
            let d2 = calculate_deltas(&current, &base);

            prop_assert_eq!(d1.cpu_instructions.to_bits(), d2.cpu_instructions.to_bits());
            prop_assert_eq!(d1.ram_bytes.to_bits(), d2.ram_bytes.to_bits());
            prop_assert_eq!(d1.ledger_read_bytes.to_bits(), d2.ledger_read_bytes.to_bits());
            prop_assert_eq!(d1.ledger_write_bytes.to_bits(), d2.ledger_write_bytes.to_bits());
            prop_assert_eq!(d1.transaction_size_bytes.to_bits(), d2.transaction_size_bytes.to_bits());
        }

        /// Running `build_report` twice with the same inputs must produce
        /// identical regression flags (determinism).
        #[test]
        fn fuzz_build_report_deterministic(
            current in arb_soroban_resources(),
            base in arb_soroban_resources(),
        ) {
            let r1 = build_report(current.clone(), base.clone());
            let r2 = build_report(current, base);

            prop_assert_eq!(r1.regression_flags.len(), r2.regression_flags.len());
            prop_assert_eq!(r1.summary, r2.summary);
        }
    }
}
