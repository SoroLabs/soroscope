//! Property-based fuzz tests for the soroscope **simulation engine**.
//!
//! These tests exercise internal functions (parsing, XDR handling, cost
//! calculation, TTL analysis, cache, call-graph) with randomly generated
//! inputs to surface panics, overflows, or inconsistent results.
//!
//! Closes #116.

#[cfg(test)]
mod tests {
    use crate::cache::SimulationCache;
    use crate::simulation::{
        CallGraph, CallNode, SimulationEngine, SimulationResult, SimulationStateSnapshot,
        SorobanResources, TtlEntryReport,
    };
    use proptest::prelude::*;
    use std::collections::HashMap;

    // ── Strategies ──────────────────────────────────────────────────────

    /// Arbitrary printable string up to 128 chars.
    fn arb_short_string() -> impl Strategy<Value = String> {
        "[[:print:]]{0,128}"
    }

    /// Arbitrary short ASCII-only identifier.
    /// Arbitrary `SorobanResources`.
    fn arb_resources() -> impl Strategy<Value = SorobanResources> {
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

    /// Arbitrary `TtlEntryReport`.
    fn arb_ttl_report() -> impl Strategy<Value = TtlEntryReport> {
        ("[a-zA-Z0-9]{1,16}", any::<u32>(), -500_000i64..500_000i64).prop_map(|(key, live, rem)| {
            TtlEntryReport {
                key,
                live_until_ledger: live,
                remaining_ledgers: rem,
            }
        })
    }

    /// Arbitrary `CallNode` tree with bounded depth.
    fn arb_call_node() -> impl Strategy<Value = CallNode> {
        let leaf = ("[a-zA-Z0-9]{4,8}", "[a-zA-Z]{3,10}").prop_map(|(cid, func)| CallNode {
            contract_id: cid,
            function: func,
            children: vec![],
        });

        leaf.prop_recursive(3, 15, 4, |inner| {
            (
                "[a-zA-Z0-9]{4,8}",
                "[a-zA-Z]{3,10}",
                proptest::collection::vec(inner, 0..4),
            )
                .prop_map(|(cid, func, kids)| CallNode {
                    contract_id: cid,
                    function: func,
                    children: kids,
                })
        })
    }

    /// Arbitrary `SimulationResult`.
    fn arb_simulation_result() -> impl Strategy<Value = SimulationResult> {
        (
            arb_resources(),
            any::<u64>(),
            any::<u64>(),
            "[a-zA-Z0-9+/=]{0,64}",
        )
            .prop_map(|(res, ledger, cost, td)| SimulationResult {
                resources: res,
                transaction_hash: None,
                latest_ledger: ledger,
                cost_stroops: cost,
                state_dependency: None,
                ttl_analysis: None,
                transaction_data: td,
                call_graph: None,
                state_snapshot: None,
                protocol_version: 0,
            })
    }

    // ── Fuzz tests ──────────────────────────────────────────────────────

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1000))]

        // ─── 1. parse_sc_val_arg ────────────────────────────────────────

        /// Must never panic on arbitrary string input.
        #[test]
        fn fuzz_parse_sc_val_arg_no_panic(input in arb_short_string()) {
            let engine = SimulationEngine::new("https://test.com".into());
            let _ = engine.parse_sc_val_arg(&input);
        }

        /// Boolean literals always parse successfully.
        #[test]
        fn fuzz_parse_sc_val_arg_booleans(b in prop::bool::ANY) {
            let engine = SimulationEngine::new("https://test.com".into());
            let input = if b { "true" } else { "false" };
            let result = engine.parse_sc_val_arg(input);
            prop_assert!(result.is_ok());
        }

        /// Void literals always parse successfully.
        #[test]
        fn fuzz_parse_sc_val_arg_void(input in prop::sample::select(vec!["void", "()"])) {
            let engine = SimulationEngine::new("https://test.com".into());
            let result = engine.parse_sc_val_arg(input);
            prop_assert!(result.is_ok());
        }

        /// Integer strings always parse successfully.
        #[test]
        fn fuzz_parse_sc_val_arg_integers(n in any::<i64>()) {
            let engine = SimulationEngine::new("https://test.com".into());
            let result = engine.parse_sc_val_arg(&n.to_string());
            prop_assert!(result.is_ok());
        }

        /// Hex byte strings (0x...) must not panic.
        #[test]
        fn fuzz_parse_sc_val_arg_hex(hex in "[0-9a-fA-F]{0,64}") {
            let engine = SimulationEngine::new("https://test.com".into());
            let input = format!("0x{}", hex);
            let _ = engine.parse_sc_val_arg(&input);
        }

        /// Symbol-prefixed strings must not panic.
        #[test]
        fn fuzz_parse_sc_val_arg_symbols(sym in "[a-zA-Z_][a-zA-Z0-9_]{0,30}") {
            let engine = SimulationEngine::new("https://test.com".into());
            let input = format!(":{}", sym);
            let _ = engine.parse_sc_val_arg(&input);
        }

        /// JSON objects/arrays must not panic.
        #[test]
        fn fuzz_parse_sc_val_arg_json_objects(
            k in "[a-zA-Z]{1,8}",
            v in any::<i64>(),
        ) {
            let engine = SimulationEngine::new("https://test.com".into());
            let json = format!(r#"{{"{}": {}}}"#, k, v);
            let _ = engine.parse_sc_val_arg(&json);
        }

        // ─── 2. parse_contract_id ───────────────────────────────────────

        /// Must never panic on arbitrary strings.
        #[test]
        fn fuzz_parse_contract_id_no_panic(input in arb_short_string()) {
            let engine = SimulationEngine::new("https://test.com".into());
            let _ = engine.parse_contract_id(&input);
        }

        /// Valid C... contract IDs must return Ok with 32-byte result.
        #[test]
        fn fuzz_parse_contract_id_valid_returns_32_bytes(
            _dummy in Just(())
        ) {
            let engine = SimulationEngine::new("https://test.com".into());
            let result = engine.parse_contract_id(
                "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC",
            );
            prop_assert!(result.is_ok());
            prop_assert_eq!(result.unwrap().len(), 32);
        }

        /// G... addresses must return Err (not a contract).
        #[test]
        fn fuzz_parse_contract_id_rejects_account_ids(_dummy in Just(())) {
            let engine = SimulationEngine::new("https://test.com".into());
            // This is the standard zero-account G address
            let result = engine.parse_contract_id(
                "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
            );
            prop_assert!(result.is_err());
        }

        // ─── 3. create_invoke_transaction ───────────────────────────────

        /// Valid contract ID + valid function name must produce base64 output.
        #[test]
        fn fuzz_create_invoke_transaction_valid(
            func in "[a-zA-Z_][a-zA-Z0-9_]{0,15}",
        ) {
            let engine = SimulationEngine::new("https://test.com".into());
            let result = engine.create_invoke_transaction(
                "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC",
                &func,
                vec![],
            );
            prop_assert!(result.is_ok());
            let b64 = result.unwrap();
            prop_assert!(!b64.is_empty());
        }

        /// With boolean/integer args, transaction building must not panic.
        #[test]
        fn fuzz_create_invoke_transaction_with_args(
            func in "[a-zA-Z]{1,10}",
            arg_count in 0usize..5,
        ) {
            let engine = SimulationEngine::new("https://test.com".into());
            let args: Vec<String> = (0..arg_count)
                .map(|i| if i % 2 == 0 { "true".into() } else { "42".into() })
                .collect();
            let _ = engine.create_invoke_transaction(
                "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC",
                &func,
                args,
            );
        }

        // ─── 4. extract_footprint_from_xdr ──────────────────────────────

        /// Must never panic on any string input; always returns (u64, u64).
        #[test]
        fn fuzz_extract_footprint_no_panic(input in arb_short_string()) {
            let engine = SimulationEngine::new("https://test.com".into());
            let _ = engine.extract_footprint_from_xdr(&input);
        }

        /// Empty input must return (0, 0).
        #[test]
        fn fuzz_extract_footprint_empty(_dummy in Just(())) {
            let engine = SimulationEngine::new("https://test.com".into());
            let result = engine.extract_footprint_from_xdr("");
            prop_assert_eq!(result, (0, 0));
        }

        // ─── 5. extract_touched_ledger_keys ─────────────────────────────

        /// Must never panic on arbitrary base64-like input.
        #[test]
        fn fuzz_extract_touched_ledger_keys_no_panic(
            input in "[a-zA-Z0-9+/=]{0,128}"
        ) {
            let engine = SimulationEngine::new("https://test.com".into());
            let _ = engine.extract_touched_ledger_keys(&input);
        }

        /// Empty input returns empty Vec.
        #[test]
        fn fuzz_extract_touched_ledger_keys_empty(_dummy in Just(())) {
            let engine = SimulationEngine::new("https://test.com".into());
            let keys = engine.extract_touched_ledger_keys("");
            prop_assert!(keys.is_empty());
        }

        // ─── 6. estimate_scval_size ─────────────────────────────────────

        /// Primitive ScVal variants must never panic.
        #[test]
        fn fuzz_estimate_scval_size_primitives(
            u32val in any::<u32>(),
            i32val in any::<i32>(),
            u64val in any::<u64>(),
            i64val in any::<i64>(),
        ) {
            use soroban_sdk::xdr::ScVal;
            let engine = SimulationEngine::new("https://test.com".into());

            prop_assert_eq!(engine.estimate_scval_size(&ScVal::Bool(true)), 1);
            prop_assert_eq!(engine.estimate_scval_size(&ScVal::Void), 0);
            prop_assert_eq!(engine.estimate_scval_size(&ScVal::U32(u32val)), 4);
            prop_assert_eq!(engine.estimate_scval_size(&ScVal::I32(i32val)), 4);
            prop_assert_eq!(engine.estimate_scval_size(&ScVal::U64(u64val)), 8);
            prop_assert_eq!(engine.estimate_scval_size(&ScVal::I64(i64val)), 8);
        }

        /// Address ScVal must return 32.
        #[test]
        fn fuzz_estimate_scval_address(_dummy in Just(())) {
            use soroban_sdk::xdr::{ScVal, ScAddress, Hash};
            let engine = SimulationEngine::new("https://test.com".into());
            let addr = ScVal::Address(ScAddress::Contract(Hash([0u8; 32])));
            prop_assert_eq!(engine.estimate_scval_size(&addr), 32);
        }

        /// None-Vec and None-Map must return 4.
        #[test]
        fn fuzz_estimate_scval_empty_containers(_dummy in Just(())) {
            use soroban_sdk::xdr::ScVal;
            let engine = SimulationEngine::new("https://test.com".into());
            prop_assert_eq!(engine.estimate_scval_size(&ScVal::Vec(None)), 4);
            prop_assert_eq!(engine.estimate_scval_size(&ScVal::Map(None)), 4);
        }

        // ─── 7. calculate_cost ──────────────────────────────────────────

        /// Must never panic or overflow on any u64 resource values.
        #[test]
        fn fuzz_calculate_cost_no_panic(resources in arb_resources()) {
            let engine = SimulationEngine::new("https://test.com".into());
            let _cost = engine.calculate_cost(&resources);
            // Cost is computed via integer division — always finite.
        }

        /// Deterministic: same inputs → same cost.
        #[test]
        fn fuzz_calculate_cost_deterministic(resources in arb_resources()) {
            let engine = SimulationEngine::new("https://test.com".into());
            let c1 = engine.calculate_cost(&resources);
            let c2 = engine.calculate_cost(&resources);
            prop_assert_eq!(c1, c2);
        }

        /// Zero resources → zero cost.
        #[test]
        fn fuzz_calculate_cost_zero(_dummy in Just(())) {
            let engine = SimulationEngine::new("https://test.com".into());
            let zero = SorobanResources::default();
            prop_assert_eq!(engine.calculate_cost(&zero), 0);
        }

        // ─── 8. build_extend_ttl_suggestions ────────────────────────────

        /// Must never panic on arbitrary TTL entries and ledger values.
        #[test]
        fn fuzz_build_ttl_suggestions_no_panic(
            entries in proptest::collection::vec(arb_ttl_report(), 0..20),
            latest_ledger in any::<u64>(),
        ) {
            let suggestions =
                SimulationEngine::build_extend_ttl_suggestions(&entries, latest_ledger);
            // Every suggestion's extend_to_ledger >= current_live_until_ledger.
            for s in &suggestions {
                prop_assert!(
                    s.extend_to_ledger >= s.current_live_until_ledger,
                    "extend_to_ledger {} < current {}",
                    s.extend_to_ledger,
                    s.current_live_until_ledger
                );
            }
        }

        /// Entries with high remaining_ledgers should never be suggested.
        #[test]
        fn fuzz_build_ttl_suggestions_high_ttl_not_flagged(
            key in "[a-z]{4,8}",
            live_until in 500_000u32..u32::MAX,
        ) {
            let entries = vec![TtlEntryReport {
                key,
                live_until_ledger: live_until,
                remaining_ledgers: 200_000, // > 120_000 threshold
            }];
            let suggestions =
                SimulationEngine::build_extend_ttl_suggestions(&entries, 100);
            prop_assert!(suggestions.is_empty());
        }

        // ─── 9. SimulationCache::generate_key ───────────────────────────

        /// Must never panic; always returns 64-char hex.
        #[test]
        fn fuzz_cache_key_format(
            cid in arb_short_string(),
            func in arb_short_string(),
            args in proptest::collection::vec(arb_short_string(), 0..5),
        ) {
            let arg_refs: Vec<String> = args;
            let key = SimulationCache::generate_key(&cid, &func, &arg_refs);
            prop_assert_eq!(key.len(), 64);
            prop_assert!(key.chars().all(|c| c.is_ascii_hexdigit()));
        }

        /// Deterministic: same inputs → same key.
        #[test]
        fn fuzz_cache_key_deterministic(
            cid in arb_short_string(),
            func in arb_short_string(),
        ) {
            let k1 = SimulationCache::generate_key(&cid, &func, &[]);
            let k2 = SimulationCache::generate_key(&cid, &func, &[]);
            prop_assert_eq!(k1, k2);
        }

        /// Different inputs → different keys (collision resistance).
        #[test]
        fn fuzz_cache_key_collision_resistance(
            cid1 in "[a-z]{4,8}",
            cid2 in "[a-z]{4,8}",
            func in "[a-z]{4,8}",
        ) {
            prop_assume!(cid1 != cid2);
            let k1 = SimulationCache::generate_key(&cid1, &func, &[]);
            let k2 = SimulationCache::generate_key(&cid2, &func, &[]);
            prop_assert_ne!(k1, k2);
        }

        // ─── 10. CallGraph::to_mermaid ──────────────────────────────────

        /// Must never panic on arbitrary call trees; output starts correctly.
        #[test]
        fn fuzz_call_graph_to_mermaid(root in arb_call_node()) {
            let graph = CallGraph { root };
            let mermaid = graph.to_mermaid();
            prop_assert!(mermaid.starts_with("graph TD\n"));
            prop_assert!(!mermaid.is_empty());
        }

        // ─── 11. SimulationResult serialization ─────────────────────────

        /// Must serialize to JSON without panic.
        #[test]
        fn fuzz_simulation_result_serializes(result in arb_simulation_result()) {
            let json = serde_json::to_string(&result);
            prop_assert!(json.is_ok(), "Serialization failed: {:?}", json.err());
        }

        /// Key fields must survive JSON round-trip.
        #[test]
        fn fuzz_simulation_result_roundtrip(result in arb_simulation_result()) {
            let json = serde_json::to_string(&result).unwrap();
            let deser: SimulationResult = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(deser.latest_ledger, result.latest_ledger);
            prop_assert_eq!(deser.cost_stroops, result.cost_stroops);
            prop_assert_eq!(
                deser.resources.cpu_instructions,
                result.resources.cpu_instructions
            );
            prop_assert_eq!(deser.resources.ram_bytes, result.resources.ram_bytes);
        }

        // ─── 12. SimulationStateSnapshot serialization ──────────────────

        /// Must round-trip through JSON without data loss.
        #[test]
        fn fuzz_state_snapshot_roundtrip(
            ledger in any::<u64>(),
            key in "[a-zA-Z0-9]{4,16}",
            val in "[a-zA-Z0-9]{4,16}",
            ttl in any::<u32>(),
        ) {
            let mut le = HashMap::new();
            let mut te = HashMap::new();
            le.insert(key.clone(), val);
            te.insert(key, ttl);
            let snap = SimulationStateSnapshot {
                ledger_entries: le,
                ttl_entries: te,
                latest_ledger: ledger,
            };
            let json = serde_json::to_string(&snap).unwrap();
            let deser: SimulationStateSnapshot = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(deser.latest_ledger, snap.latest_ledger);
            prop_assert_eq!(deser.ledger_entries.len(), snap.ledger_entries.len());
            prop_assert_eq!(deser.ttl_entries.len(), snap.ttl_entries.len());
        }
    }
}
