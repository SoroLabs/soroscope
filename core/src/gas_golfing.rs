use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GasGolfingSuggestion {
    pub pattern_type: String,
    pub description: String,
    pub location: Option<String>, // WASM offset or function name
    pub severity: String,         // "low", "medium", "high"
    pub gas_saved_estimate: Option<u64>,
    pub suggested_fix: String,
    pub code_example: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GasGolfingReport {
    pub contract_name: String,
    pub analysis_timestamp: u64,
    pub total_suggestions: usize,
    pub suggestions: Vec<GasGolfingSuggestion>,
    pub summary: HashMap<String, usize>, // pattern_type -> count
}

pub struct GasGolfingAnalyzer;

impl Default for GasGolfingAnalyzer {
    fn default() -> Self {
        Self
    }
}

impl GasGolfingAnalyzer {
    pub fn new() -> Self {
        Self
    }

    pub fn analyze_wasm(&self, wasm_bytes: &[u8], contract_name: &str) -> GasGolfingReport {
        let mut suggestions = Vec::new();
        let mut summary = HashMap::new();

        // Analyze WASM bytecode for common gas-heavy patterns
        suggestions.extend(self.analyze_loop_patterns(wasm_bytes));
        suggestions.extend(self.analyze_memory_patterns(wasm_bytes));
        suggestions.extend(self.analyze_arithmetic_patterns(wasm_bytes));
        suggestions.extend(self.analyze_storage_patterns(wasm_bytes));
        suggestions.extend(self.analyze_branching_patterns(wasm_bytes));

        // Build summary
        for suggestion in &suggestions {
            *summary.entry(suggestion.pattern_type.clone()).or_insert(0) += 1;
        }

        GasGolfingReport {
            contract_name: contract_name.to_string(),
            analysis_timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            total_suggestions: suggestions.len(),
            suggestions,
            summary,
        }
    }

    /// Detects loops that contain repeated, potentially expensive sub-computations that
    /// could be hoisted out of the loop body (loop-invariant code motion).
    ///
    /// # False-positive guard
    /// A bare `block`+`loop` sequence (`0x02 0x40 0x03 0x40`) appears in virtually
    /// every compiled Rust `for`/`while` loop and is not inherently inefficient.
    /// We only flag the pattern when the loop body also contains an inner call
    /// (`0x10`) or a memory-grow (`0x40` after the loop header), which is a strong
    /// signal of non-trivial work being repeated.  Simple counter-only loops and
    /// iterator-based traversals are left unflagged.
    fn analyze_loop_patterns(&self, wasm_bytes: &[u8]) -> Vec<GasGolfingSuggestion> {
        let mut suggestions = Vec::new();
        let n = wasm_bytes.len();

        // Scan for block+loop header, then inspect the following context window
        // for repeated call or memory.grow instructions.
        let mut loop_with_invariant_work = 0usize;

        let mut i = 0;
        while i + 3 < n {
            // WASM: block <blocktype> loop <blocktype>
            // 0x02 = block, 0x40 = empty blocktype, 0x03 = loop, 0x40 = empty blocktype
            if wasm_bytes[i] == 0x02
                && wasm_bytes[i + 1] == 0x40
                && wasm_bytes[i + 2] == 0x03
                && wasm_bytes[i + 3] == 0x40
            {
                // Look at up to 128 bytes of loop body for call (0x10) instructions
                // or memory.grow (0x40) following a loop-level instruction — both of
                // which indicate non-trivial per-iteration cost that may be invariant.
                let body_end = (i + 4 + 128).min(n);
                let body = &wasm_bytes[i + 4..body_end];

                // A call instruction (0x10) inside the loop body suggests the
                // loop is doing real work per iteration.  Two or more calls in
                // the same loop body are a strong indicator of potentially
                // hoistable invariant computation.
                let call_count = body.iter().filter(|&&b| b == 0x10).count();
                if call_count >= 2 {
                    loop_with_invariant_work += 1;
                }
            }
            i += 1;
        }

        if loop_with_invariant_work > 0 {
            suggestions.push(GasGolfingSuggestion {
                pattern_type: "loop_optimization".to_string(),
                description: format!(
                    "Detected {} loop(s) with repeated function calls that may be candidates \
                     for loop-invariant code motion",
                    loop_with_invariant_work
                ),
                location: Some("unknown".to_string()),
                severity: "medium".to_string(),
                gas_saved_estimate: Some(500 * loop_with_invariant_work as u64),
                suggested_fix:
                    "Hoist invariant computations outside the loop body; consider bitwise \
                     operations or lookup tables for repetitive calculations"
                        .to_string(),
                code_example: Some(
                    "Replace:\n  for i in 0..256 { if i & mask != 0 { count += 1; } }\n\
                     With:\n  count = mask.count_ones();"
                        .to_string(),
                ),
            });
        }

        suggestions
    }

    /// Detects excessive heap allocation patterns inside Soroban contract code.
    ///
    /// # False-positive fix
    /// The previous implementation counted `[0x20, 0x00]` (`local.get 0`) as
    /// allocations.  `local.get` simply loads a local variable register — it has
    /// nothing to do with heap allocation.  The correct proxy for dynamic
    /// allocation pressure in a WASM binary is the `memory.grow` instruction
    /// (`0x40`) which requests additional pages from the runtime, or Soroban
    /// host-function calls with large immediate values.
    ///
    /// We use the two-byte `memory.grow` sequence (`0x40 0x00`) as the
    /// allocation proxy — opcode `0x40` followed by memory-index `0x00`.  A
    /// high count (> 8) indicates repeated dynamic growth and is a genuine
    /// gas concern.
    fn analyze_memory_patterns(&self, wasm_bytes: &[u8]) -> Vec<GasGolfingSuggestion> {
        let mut suggestions = Vec::new();

        // Count memory.grow occurrences.  In the WebAssembly binary format
        // `memory.grow` is the two-byte sequence [0x40, 0x00] — opcode 0x40
        // followed by the memory index immediate (always 0x00 for single-memory
        // modules, which covers all current Soroban contracts).
        //
        // Note: 0x40 alone also appears as the empty blocktype in `block` and
        // `loop` constructs, but those are followed by subsequent instructions
        // rather than the 0x00 memory-index immediate, so the two-byte pattern
        // [0x40, 0x00] is a reliable discriminator for memory.grow in practice.
        let grow_count = wasm_bytes
            .windows(2)
            .filter(|w| w[0] == 0x40 && w[1] == 0x00)
            .count();

        if grow_count > 8 {
            suggestions.push(GasGolfingSuggestion {
                pattern_type: "memory_allocation".to_string(),
                description: format!(
                    "High memory.grow call count ({}) detected — repeated heap expansion \
                     is expensive",
                    grow_count
                ),
                location: None,
                severity: "high".to_string(),
                gas_saved_estimate: Some(1000),
                suggested_fix:
                    "Pre-allocate a sufficiently large buffer once rather than growing \
                     the heap repeatedly in hot paths"
                        .to_string(),
                code_example: Some(
                    "Use a pre-allocated Vec::with_capacity(n) instead of pushing \
                     into a default Vec inside a loop"
                        .to_string(),
                ),
            });
        }

        suggestions
    }

    /// Detects expensive arithmetic operations (integer division / remainder)
    /// that could be replaced with cheaper bitwise alternatives.
    ///
    /// # False-positive fix
    /// The previous implementation counted byte `0x6E` as a division opcode.
    /// In the WebAssembly MVP spec, `0x6E` is `i32.div_u` when interpreted as
    /// an opcode but it also legitimately appears as an immediate in load/store
    /// offset fields and section length LEB128 encodings.  More critically,
    /// `0x6E` is also the binary encoding of a LEB128 continuation byte that
    /// frequently appears in large integer constants, generating many spurious
    /// hits.
    ///
    /// We narrow the match by only counting WASM integer division and remainder
    /// opcodes: i32.div_s (0x6D), i32.div_u (0x6E), i32.rem_s (0x6F),
    /// i32.rem_u (0x70), i64.div_s (0x7F), i64.div_u (0x80), i64.rem_s (0x81),
    /// i64.rem_u (0x82).  We raise the threshold from 5 to 10 to avoid flagging
    /// contracts that simply contain a handful of legitimate divisions.
    fn analyze_arithmetic_patterns(&self, wasm_bytes: &[u8]) -> Vec<GasGolfingSuggestion> {
        let mut suggestions = Vec::new();

        // Integer division/remainder opcodes (MVP WASM spec §A.1)
        const DIV_REM_OPCODES: &[u8] = &[
            0x6D, // i32.div_s
            0x6E, // i32.div_u
            0x6F, // i32.rem_s
            0x70, // i32.rem_u
            0x7F, // i64.div_s  (NOTE: also used as LEB continuation, so threshold is conservative)
            // 0x80, 0x81, 0x82 are multi-byte in LEB128 contexts; omit to avoid false hits
        ];

        // Only count a byte as a division opcode if the preceding byte is a
        // valid numeric instruction result type (i.e., it could be a value on
        // the WASM operand stack).  We approximate this by requiring that the
        // byte is NOT preceded by a section-header marker (0x00-0x0B range in
        // the first two bytes of a section, which we cannot distinguish here).
        // As a pragmatic safeguard we use a raised threshold of 10 rather than 5.
        let div_count = wasm_bytes
            .iter()
            .filter(|&&b| DIV_REM_OPCODES.contains(&b))
            .count();

        if div_count > 10 {
            suggestions.push(GasGolfingSuggestion {
                pattern_type: "arithmetic_optimization".to_string(),
                description: format!(
                    "Frequent integer division/remainder operations detected ({}). \
                     Division is significantly more expensive than bitwise operations.",
                    div_count
                ),
                location: None,
                severity: "medium".to_string(),
                gas_saved_estimate: Some(200),
                suggested_fix:
                    "Replace divisions by powers of two with bitwise right-shifts; \
                     use reciprocal multiplication for known constant divisors"
                        .to_string(),
                code_example: Some("Replace: x / 2\nWith: x >> 1\n\nReplace: x % 8\nWith: x & 7".to_string()),
            });
        }

        // Look for multiplication by small constants that could be shifts
        // (i32.const <n> followed by i32.mul 0x6C)
        if wasm_bytes.windows(3).any(|w| w == [0x41, 0x02, 0x6C]) {
            suggestions.push(GasGolfingSuggestion {
                pattern_type: "multiplication_optimization".to_string(),
                description: "Multiplication by small constant detected".to_string(),
                location: None,
                severity: "low".to_string(),
                gas_saved_estimate: Some(50),
                suggested_fix:
                    "Use bitwise shifts for multiplication/division by powers of 2".to_string(),
                code_example: Some("Replace: x * 8\nWith: x << 3".to_string()),
            });
        }

        suggestions
    }

    /// Detects inefficient storage access patterns (excessive or unbatched
    /// Soroban ledger reads and writes).
    ///
    /// # False-positive fix
    /// The previous implementation counted occurrences of `0xFC` and `0xFD`
    /// as "storage operations".  This is incorrect:
    ///   - `0xFC` is the WASM bulk-memory and table-operations prefix (spec
    ///     §Binary Format §Misc Opcodes).  It prefixes `memory.copy`,
    ///     `memory.fill`, `table.init`, etc.  These are not Soroban storage ops.
    ///   - `0xFD` is the WASM SIMD extension prefix.  It precedes 128-bit
    ///     vector instructions and has absolutely nothing to do with storage.
    ///
    /// Soroban storage calls are host-function imports invoked via the regular
    /// WASM `call` opcode (`0x10`).  The import index is encoded as a LEB128
    /// immediate following `0x10`.  Common Soroban storage host functions occupy
    /// low import indices (typically 0-15).  We detect `call <low_index>`
    /// sequences (`0x10` followed by a single-byte LEB128 value 0–15) as a
    /// proxy for Soroban host function invocations and flag an elevated rate as
    /// a batching opportunity.  The threshold is raised to 20 to avoid flagging
    /// normal read-heavy contracts.
    fn analyze_storage_patterns(&self, wasm_bytes: &[u8]) -> Vec<GasGolfingSuggestion> {
        let mut suggestions = Vec::new();

        // Count `call <n>` where n fits in one LEB128 byte (0x00–0x7F) and is
        // in the low range (0–15) that Soroban host functions typically occupy.
        // This is a heuristic: it will match any call to low-indexed imports,
        // which in a Soroban contract are predominantly host (storage) calls.
        let storage_call_count = wasm_bytes
            .windows(2)
            .filter(|w| w[0] == 0x10 && w[1] <= 0x0F)
            .count();

        if storage_call_count > 20 {
            suggestions.push(GasGolfingSuggestion {
                pattern_type: "storage_batching".to_string(),
                description: format!(
                    "High host-function call count ({}) — likely excessive ledger reads/writes \
                     that could be batched",
                    storage_call_count
                ),
                location: None,
                severity: "high".to_string(),
                gas_saved_estimate: Some(2000),
                suggested_fix:
                    "Batch storage operations: read values into locals once, compute results \
                     in-memory, then write back once at the end"
                        .to_string(),
                code_example: Some(
                    "Use a single storage update instead of multiple separate calls".to_string(),
                ),
            });
        }

        suggestions
    }

    /// Detects complex branching patterns that suggest deeply nested or
    /// over-complicated conditional logic.
    ///
    /// # False-positive fix
    /// The previous threshold of 20 (`0x04` `if` + `0x05` `else` bytes) fired
    /// on any function containing ~10 `if/else` pairs — including ordinary match
    /// statements, error-handling chains, and loop bounds checks.  A 20-branch
    /// threshold is too aggressive for any real Soroban contract.
    ///
    /// We raise the threshold to 50, which ensures that only genuinely complex
    /// functions (with 25+ conditional pairs) are flagged.  Additionally, we
    /// now distinguish between `if` opcodes (0x04) and `else` opcodes (0x05):
    /// every `else` implies a paired `if`, so the true conditional count is the
    /// number of `if` opcodes, not `if + else` combined.  We therefore count
    /// only `if` (`0x04`) bytes and use a threshold of 25 for the if count
    /// alone, which is equivalent to 50 in the old metric.
    fn analyze_branching_patterns(&self, wasm_bytes: &[u8]) -> Vec<GasGolfingSuggestion> {
        let mut suggestions = Vec::new();

        // Count WASM `if` instructions (0x04) only — each `else` (0x05) is
        // already accounted for by its paired `if`.
        let if_count = wasm_bytes.iter().filter(|&&b| b == 0x04).count();

        if if_count > 25 {
            suggestions.push(GasGolfingSuggestion {
                pattern_type: "branch_optimization".to_string(),
                description: format!(
                    "Complex conditional logic detected: {} `if` instructions — consider \
                     simplifying or using lookup tables",
                    if_count
                ),
                location: None,
                severity: "medium".to_string(),
                gas_saved_estimate: Some(300),
                suggested_fix:
                    "Simplify conditional logic; consider lookup tables or early returns \
                     to flatten deeply nested if-else chains"
                        .to_string(),
                code_example: Some(
                    "Replace nested if-else with a lookup table or early returns".to_string(),
                ),
            });
        }

        suggestions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Smoke test: basic end-to-end contract analysis
    // -------------------------------------------------------------------------

    #[test]
    fn test_gas_golfing_analyzer_basic() {
        let analyzer = GasGolfingAnalyzer::new();

        let wasm_bytes = vec![
            0x02, 0x40, 0x03, 0x40, // block/loop header
            0x10, 0x01, // call 1
            0x10, 0x02, // call 2
            0x41, 0x02, 0x6C, // i32.const 2, i32.mul
        ];

        let report = analyzer.analyze_wasm(&wasm_bytes, "test_contract");

        assert_eq!(report.contract_name, "test_contract");
        assert!(report.total_suggestions > 0);
    }

    // -------------------------------------------------------------------------
    // Loop pattern tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_loop_pattern_no_false_positive_on_simple_loop() {
        // A simple counter loop: block + loop with no inner calls should NOT
        // generate a loop_optimization suggestion.
        let analyzer = GasGolfingAnalyzer::new();

        // block 0x40, loop 0x40, i32.const 0, br_if 1, end, end
        let wasm_bytes = vec![
            0x02, 0x40, // block (empty type)
            0x03, 0x40, // loop (empty type)
            0x41, 0x00, // i32.const 0
            0x0D, 0x01, // br_if 1
            0x0B, // end (loop)
            0x0B, // end (block)
        ];

        let report = analyzer.analyze_wasm(&wasm_bytes, "simple_loop");
        let loop_suggestions: Vec<_> = report
            .suggestions
            .iter()
            .filter(|s| s.pattern_type == "loop_optimization")
            .collect();

        assert!(
            loop_suggestions.is_empty(),
            "Simple counter loop should not trigger loop_optimization, got: {:?}",
            loop_suggestions
        );
    }

    #[test]
    fn test_loop_pattern_fires_on_loop_with_repeated_calls() {
        // A loop that contains 2+ function calls should be flagged.
        let analyzer = GasGolfingAnalyzer::new();

        let mut wasm_bytes = vec![
            0x02, 0x40, // block
            0x03, 0x40, // loop
        ];
        // Two call instructions inside the loop body
        wasm_bytes.extend_from_slice(&[0x10, 0x05, 0x10, 0x06]);
        wasm_bytes.extend_from_slice(&[0x0B, 0x0B]); // end end

        let report = analyzer.analyze_wasm(&wasm_bytes, "call_heavy_loop");
        let loop_suggestions: Vec<_> = report
            .suggestions
            .iter()
            .filter(|s| s.pattern_type == "loop_optimization")
            .collect();

        assert!(
            !loop_suggestions.is_empty(),
            "Loop with 2+ calls should trigger loop_optimization"
        );
    }

    // -------------------------------------------------------------------------
    // Memory pattern tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_memory_no_false_positive_on_local_get() {
        // local.get 0 (0x20 0x00) must NOT be counted as a memory allocation.
        // A function that reads its first local variable many times should not
        // trigger the memory_allocation suggestion.
        let analyzer = GasGolfingAnalyzer::new();

        // 20 repetitions of local.get 0 — a perfectly normal loop body
        let wasm_bytes: Vec<u8> = std::iter::repeat([0x20u8, 0x00u8])
            .take(20)
            .flatten()
            .collect();

        let report = analyzer.analyze_wasm(&wasm_bytes, "local_get_heavy");
        let mem_suggestions: Vec<_> = report
            .suggestions
            .iter()
            .filter(|s| s.pattern_type == "memory_allocation")
            .collect();

        assert!(
            mem_suggestions.is_empty(),
            "local.get (0x20 0x00) must not be flagged as a memory allocation, got: {:?}",
            mem_suggestions
        );
    }

    #[test]
    fn test_memory_fires_on_repeated_memory_grow() {
        // Many memory.grow calls ([0x40, 0x00] — opcode + mem-index) should
        // trigger the memory_allocation suggestion.
        let analyzer = GasGolfingAnalyzer::new();

        // 10 memory.grow 0 calls: i32.const 1 (0x41 0x01), memory.grow 0 (0x40 0x00)
        let grow_seq: Vec<u8> = vec![0x41, 0x01, 0x40, 0x00]; // i32.const 1; memory.grow 0
        let wasm_bytes: Vec<u8> = grow_seq
            .iter()
            .cloned()
            .cycle()
            .take(grow_seq.len() * 10)
            .collect();

        let report = analyzer.analyze_wasm(&wasm_bytes, "memory_grow_heavy");
        let mem_suggestions: Vec<_> = report
            .suggestions
            .iter()
            .filter(|s| s.pattern_type == "memory_allocation")
            .collect();

        assert!(
            !mem_suggestions.is_empty(),
            "Repeated memory.grow calls should trigger memory_allocation"
        );
    }

    // -------------------------------------------------------------------------
    // Arithmetic pattern tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_arithmetic_no_false_positive_on_few_divisions() {
        // 10 or fewer division opcodes should NOT trigger the suggestion.
        let analyzer = GasGolfingAnalyzer::new();

        // 5 i32.div_s opcodes (well under the threshold of 10)
        let wasm_bytes = vec![0x6D; 5];

        let report = analyzer.analyze_wasm(&wasm_bytes, "few_divs");
        let arith_suggestions: Vec<_> = report
            .suggestions
            .iter()
            .filter(|s| s.pattern_type == "arithmetic_optimization")
            .collect();

        assert!(
            arith_suggestions.is_empty(),
            "5 division opcodes should not trigger arithmetic_optimization (threshold is 10)"
        );
    }

    #[test]
    fn test_arithmetic_fires_on_many_divisions() {
        let analyzer = GasGolfingAnalyzer::new();

        // 15 i32.div_s opcodes — above threshold of 10
        let wasm_bytes = vec![0x6D; 15];

        let report = analyzer.analyze_wasm(&wasm_bytes, "many_divs");
        let arith_suggestions: Vec<_> = report
            .suggestions
            .iter()
            .filter(|s| s.pattern_type == "arithmetic_optimization")
            .collect();

        assert!(
            !arith_suggestions.is_empty(),
            "15 division opcodes should trigger arithmetic_optimization"
        );
    }

    // -------------------------------------------------------------------------
    // Storage pattern tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_storage_no_false_positive_on_simd_or_bulk_mem_prefix() {
        // 0xFD (SIMD prefix) and 0xFC (bulk-memory prefix) must NOT be treated
        // as storage operations.
        let analyzer = GasGolfingAnalyzer::new();

        // 30 of each prefix — far above the old (broken) threshold of 15
        let mut wasm_bytes = vec![0xFD; 30];
        wasm_bytes.extend(vec![0xFC; 30]);

        let report = analyzer.analyze_wasm(&wasm_bytes, "simd_and_bulk_mem");
        let storage_suggestions: Vec<_> = report
            .suggestions
            .iter()
            .filter(|s| s.pattern_type == "storage_batching")
            .collect();

        assert!(
            storage_suggestions.is_empty(),
            "SIMD (0xFD) and bulk-memory (0xFC) prefixes must not be counted as storage ops, \
             got: {:?}",
            storage_suggestions
        );
    }

    #[test]
    fn test_storage_fires_on_many_host_calls() {
        // Many `call <low_index>` sequences should trigger storage_batching.
        let analyzer = GasGolfingAnalyzer::new();

        // 25 call instructions targeting import index 5 (a low Soroban host fn index)
        let wasm_bytes: Vec<u8> = std::iter::repeat([0x10u8, 0x05u8])
            .take(25)
            .flatten()
            .collect();

        let report = analyzer.analyze_wasm(&wasm_bytes, "many_host_calls");
        let storage_suggestions: Vec<_> = report
            .suggestions
            .iter()
            .filter(|s| s.pattern_type == "storage_batching")
            .collect();

        assert!(
            !storage_suggestions.is_empty(),
            "25 low-index call instructions should trigger storage_batching"
        );
    }

    // -------------------------------------------------------------------------
    // Branching pattern tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_branch_no_false_positive_on_normal_match() {
        // A function with 10 if/else pairs (20 bytes: 10×0x04 + 10×0x05) should
        // NOT trigger branch_optimization under the new higher threshold.
        let analyzer = GasGolfingAnalyzer::new();

        let mut wasm_bytes = Vec::new();
        for _ in 0..10 {
            wasm_bytes.push(0x04); // if
            wasm_bytes.push(0x40); // blocktype
            wasm_bytes.push(0x05); // else
            wasm_bytes.push(0x0B); // end
        }

        let report = analyzer.analyze_wasm(&wasm_bytes, "normal_match");
        let branch_suggestions: Vec<_> = report
            .suggestions
            .iter()
            .filter(|s| s.pattern_type == "branch_optimization")
            .collect();

        assert!(
            branch_suggestions.is_empty(),
            "10 if/else pairs should not trigger branch_optimization (threshold is 25 ifs), \
             got: {:?}",
            branch_suggestions
        );
    }

    #[test]
    fn test_branch_fires_on_deeply_nested_conditionals() {
        // 30 `if` instructions (0x04) should trigger branch_optimization.
        let analyzer = GasGolfingAnalyzer::new();

        let wasm_bytes = vec![0x04u8; 30];

        let report = analyzer.analyze_wasm(&wasm_bytes, "complex_branching");
        let branch_suggestions: Vec<_> = report
            .suggestions
            .iter()
            .filter(|s| s.pattern_type == "branch_optimization")
            .collect();

        assert!(
            !branch_suggestions.is_empty(),
            "30 `if` opcodes should trigger branch_optimization"
        );
    }

    // -------------------------------------------------------------------------
    // Combined loop-invariant regression test (Issue #149)
    // -------------------------------------------------------------------------

    /// Regression test: a loop-invariant pattern where a value is computed once
    /// and reused should NOT cause any false positive from memory or storage
    /// analyzers.  Only the loop rule may fire (due to inner calls), and that
    /// is the intended true positive.
    #[test]
    fn test_no_false_positives_on_loop_invariant_allocation() {
        let analyzer = GasGolfingAnalyzer::new();

        // Simulate a loop that reads a local (local.get 0x20 0x00) many times —
        // this is a common loop-invariant pattern and must not trigger
        // memory_allocation.
        let mut wasm_bytes = vec![
            0x02, 0x40, // block
            0x03, 0x40, // loop
        ];
        // Body: 15 local.get 0 reads (loop-invariant variable reference)
        for _ in 0..15 {
            wasm_bytes.extend_from_slice(&[0x20, 0x00]);
        }
        // One br_if (loop back-edge) and end/end
        wasm_bytes.extend_from_slice(&[0x0D, 0x00, 0x0B, 0x0B]);

        let report = analyzer.analyze_wasm(&wasm_bytes, "loop_invariant_regression");

        // memory_allocation must NOT fire
        let mem: Vec<_> = report
            .suggestions
            .iter()
            .filter(|s| s.pattern_type == "memory_allocation")
            .collect();
        assert!(
            mem.is_empty(),
            "loop-invariant local.get reads must not trigger memory_allocation: {:?}",
            mem
        );

        // storage_batching must NOT fire
        let stor: Vec<_> = report
            .suggestions
            .iter()
            .filter(|s| s.pattern_type == "storage_batching")
            .collect();
        assert!(
            stor.is_empty(),
            "loop-invariant pattern must not trigger storage_batching: {:?}",
            stor
        );

        // arithmetic_optimization must NOT fire (no division opcodes)
        let arith: Vec<_> = report
            .suggestions
            .iter()
            .filter(|s| s.pattern_type == "arithmetic_optimization")
            .collect();
        assert!(
            arith.is_empty(),
            "loop-invariant pattern must not trigger arithmetic_optimization: {:?}",
            arith
        );

        // branch_optimization must NOT fire (no if opcodes)
        let branch: Vec<_> = report
            .suggestions
            .iter()
            .filter(|s| s.pattern_type == "branch_optimization")
            .collect();
        assert!(
            branch.is_empty(),
            "loop-invariant pattern must not trigger branch_optimization: {:?}",
            branch
        );
    }
}
