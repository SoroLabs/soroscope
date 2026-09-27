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

        // Issue #1007: decode the module once, then run every rule over the
        // operator streams. Input that is not a valid module produces no
        // suggestions rather than byte-pattern false positives, and a parse
        // failure is recorded in the report instead of being silent.
        match crate::parsed_module::parse_module(wasm_bytes) {
            Ok(module) => {
                suggestions.extend(self.analyze_loop_patterns(&module));
                suggestions.extend(self.analyze_memory_patterns(&module));
                suggestions.extend(self.analyze_arithmetic_patterns(&module));
                suggestions.extend(self.analyze_storage_patterns(&module));
                suggestions.extend(self.analyze_branching_patterns(&module));
            }
            Err(err) => {
                suggestions.push(GasGolfingSuggestion {
                    pattern_type: "parse_error".to_string(),
                    description: format!(
                        "input is not a valid WASM module, so no pattern rules were run: {err}"
                    ),
                    location: None,
                    severity: "high".to_string(),
                    gas_saved_estimate: None,
                    suggested_fix:
                        "Supply a compiled contract WASM artifact; the byte-pattern scanner that \
                         previously ran here would have reported suggestions for arbitrary input"
                            .to_string(),
                    code_example: None,
                });
            }
        }

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

    // ── Parsed-module rule pass (issue #1007) ─────────────────────────────────
    //
    // Every rule below runs on decoded instructions from
    // `parsed_module::parse_module`, not on a byte window. The previous
    // implementation scanned for opcode-shaped byte sequences, which cannot
    // distinguish a real instruction from the same byte appearing inside a
    // LEB128 immediate or a data segment, and could only report a byte offset
    // as a location. A parser makes the false-positive guard the docs already
    // claimed to provide actually true.
    //
    // `gas_saved_estimate` is intentionally `None` here. Pricing an import
    // needs a cost-parameter table, which is issue #1008's job; emitting a
    // hand-picked number per rule is what made the field untrustworthy.

    /// Imports whose host name denotes a ledger read or write.
    fn is_storage_import(module: &crate::parsed_module::ParsedModule, op: &wasmparser::Operator) -> bool {
        let wasmparser::Operator::Call { function_index } = op else { return false };
        let Some(import) = module.host_import(*function_index) else { return false };
        let name = import.name.as_str();
        name.contains("contract_data") || name.contains("contract_code") || name == "extend_contract_data_ttl"
    }

    fn is_division(op: &wasmparser::Operator) -> bool {
        matches!(
            op,
            wasmparser::Operator::I32DivS
                | wasmparser::Operator::I32DivU
                | wasmparser::Operator::I32RemS
                | wasmparser::Operator::I32RemU
                | wasmparser::Operator::I64DivS
                | wasmparser::Operator::I64DivU
                | wasmparser::Operator::I64RemS
                | wasmparser::Operator::I64RemU
        )
    }

    /// Detects loops that call a host import or grow memory inside the loop's
    /// own structured block.
    ///
    /// The previous version looked 128 bytes past a `block`+`loop` byte
    /// signature, which matched the same four bytes inside a data segment and
    /// ran past the end of one function into the next. Here the control stack
    /// gives the loop's real extent, and a loop is flagged only when the body
    /// actually does per-iteration work: a call to a host import, an indirect
    /// call, or a `memory.grow`. A counter loop does none of those and is
    /// correctly left alone.
    fn analyze_loop_patterns(&self, module: &crate::parsed_module::ParsedModule) -> Vec<GasGolfingSuggestion> {
        let mut suggestions = Vec::new();
        let mut _flagged = 0usize;

        for function in &module.functions {
            let spans = crate::parsed_module::loop_spans(&function.operators);

            for span in &spans {
                let body = &function.operators[(span.op_index + 1)..=span.end_index.min(function.operators.len() - 1)];

                let host_calls = body
                    .iter()
                    .filter(|op| matches!(op, wasmparser::Operator::Call { function_index } if module.is_host_import(*function_index)))
                    .count();
                let indirect_calls = body
                    .iter()
                    .filter(|op| matches!(op, wasmparser::Operator::CallIndirect { .. }))
                    .count();
                let memory_grows = body
                    .iter()
                    .filter(|op| matches!(op, wasmparser::Operator::MemoryGrow { .. }))
                    .count();

                if host_calls == 0 && indirect_calls == 0 && memory_grows == 0 {
                    // Nothing per-iteration that could be hoisted.
                    continue;
                }

                _flagged += 1;
                let mut detail = format!(
                    "loop at {} performs per-iteration work: {} host call(s), {} indirect call(s), \
                     {} memory.grow — candidates for loop-invariant code motion",
                    function.location(span.op_index),
                    host_calls,
                    indirect_calls,
                    memory_grows
                );
                if span.depth > 0 {
                    detail.push_str(&format!(" (nested at depth {})", span.depth));
                }
                suggestions.push(GasGolfingSuggestion {
                    pattern_type: "loop_optimization".to_string(),
                    description: detail,
                    location: Some(function.location(span.op_index)),
                    severity: if host_calls + indirect_calls >= 2 { "high" } else { "medium" }.to_string(),
                    gas_saved_estimate: None,
                    suggested_fix:
                        "Hoist invariant computations and host reads out of the loop body; the \
                         call and memory.grow inside a loop are repeated every iteration"
                            .to_string(),
                    code_example: Some(
                        "Read the value into a local before the loop, then compute inside it:\
                         \n\n  for i in 0..n { let v = storage::get(k); total += v * f(i); }\
                         \n  -> \n  let base = storage::get(k); for i in 0..n { total += base * f(i); }"
                            .to_string(),
                    ),
                });
            }
        }

        suggestions
    }

    /// Detects repeated `memory.grow`, the allocation-pressure proxy.
    ///
    /// The previous version matched the two-byte sequence `[0x40, 0x00]`, which
    /// is `memory.grow` with a zero memory index — but `0x40` is also the empty
    /// block type, and `[0x40, 0x00]` occurs incidentally in encoded
    /// immediates. Counting the decoded instruction removes both false hits.
    fn analyze_memory_patterns(&self, module: &crate::parsed_module::ParsedModule) -> Vec<GasGolfingSuggestion> {
        const GROW_THRESHOLD: usize = 8;
        let mut suggestions = Vec::new();

        let mut grow_count = 0usize;
        let mut first_site: Option<String> = None;
        for function in &module.functions {
            for (op_index, op) in function.operators.iter().enumerate() {
                if matches!(op, wasmparser::Operator::MemoryGrow { .. }) {
                    grow_count += 1;
                    if first_site.is_none() {
                        first_site = Some(function.location(op_index));
                    }
                }
            }
        }

        if grow_count > GROW_THRESHOLD {
            suggestions.push(GasGolfingSuggestion {
                pattern_type: "memory_allocation".to_string(),
                description: format!(
                    "High memory.grow call count ({}) — repeated heap expansion is expensive",
                    grow_count
                ),
                location: first_site,
                severity: "high".to_string(),
                gas_saved_estimate: None,
                suggested_fix:
                    "Pre-allocate with Vec::with_capacity(n) rather than growing the heap \
                     repeatedly in a hot path"
                        .to_string(),
                code_example: Some("Vec::with_capacity(n) instead of pushing into Vec::new() in a loop".to_string()),
            });
        }

        suggestions
    }

    /// Detects expensive division/remainder and multiplication by a small
    /// constant, both of which have cheaper bitwise forms.
    ///
    /// The previous version counted raw bytes `0x6D..0x70`, which are equally
    /// likely to be LEB128 continuation bytes or immediate values, and matched
    /// `[0x41, 0x02, 0x6C]` for `i32.const 2; i32.mul` anywhere in the file.
    fn analyze_arithmetic_patterns(&self, module: &crate::parsed_module::ParsedModule) -> Vec<GasGolfingSuggestion> {
        const DIV_THRESHOLD: usize = 10;
        let mut suggestions = Vec::new();

        let mut div_count = 0usize;
        let mut div_site: Option<String> = None;
        let mut mul_const_site: Option<String> = None;

        for function in &module.functions {
            for (op_index, op) in function.operators.iter().enumerate() {
                if Self::is_division(op) {
                    div_count += 1;
                    if div_site.is_none() {
                        div_site = Some(function.location(op_index));
                    }
                }
                // i32.const <power of two>; i32.mul  ->  a shift.
                if let wasmparser::Operator::I32Mul = op {
                    if op_index > 0 {
                        if let wasmparser::Operator::I32Const { value } =
                            &function.operators[op_index - 1]
                        {
                            if *value > 1 && (*value as u32).is_power_of_two() {
                                if mul_const_site.is_none() {
                                    mul_const_site = Some(function.location(op_index - 1));
                                }
                            }
                        }
                    }
                }
            }
        }

        if div_count > DIV_THRESHOLD {
            suggestions.push(GasGolfingSuggestion {
                pattern_type: "arithmetic_optimization".to_string(),
                description: format!(
                    "Frequent integer division/remainder operations ({}) — division is far more \
                     expensive than bitwise operations",
                    div_count
                ),
                location: div_site,
                severity: "medium".to_string(),
                gas_saved_estimate: None,
                suggested_fix:
                    "Replace division by powers of two with a right shift; use reciprocal \
                     multiplication for known constant divisors"
                        .to_string(),
                code_example: Some("x / 2 -> x >> 1;  x % 8 -> x & 7".to_string()),
            });
        }

        if let Some(site) = mul_const_site {
            suggestions.push(GasGolfingSuggestion {
                pattern_type: "multiplication_optimization".to_string(),
                description: format!("Multiplication by a power-of-two constant at {site}", site = site),
                location: Some(site),
                severity: "low".to_string(),
                gas_saved_estimate: None,
                suggested_fix: "Use a shift for multiplication by a power of two".to_string(),
                code_example: Some("x * 8 -> x << 3".to_string()),
            });
        }

        suggestions
    }

    /// Detects unbatched ledger access, now by identifying the *actual* host
    /// import rather than guessing from a low call index.
    ///
    /// The previous version treated `call <index 0..=15>` as a storage
    /// operation. That conflated every low-indexed import with a ledger call and
    /// missed any storage import the linker placed higher. Matching the import's
    /// host name is exact, and it is the same import table issue #1008 prices.
    fn analyze_storage_patterns(&self, module: &crate::parsed_module::ParsedModule) -> Vec<GasGolfingSuggestion> {
        const STORAGE_THRESHOLD: usize = 20;
        let mut suggestions = Vec::new();

        let mut storage_calls = 0usize;
        let mut first_site: Option<String> = None;
        for function in &module.functions {
            for (op_index, op) in function.operators.iter().enumerate() {
                if Self::is_storage_import(module, op) {
                    storage_calls += 1;
                    if first_site.is_none() {
                        first_site = Some(function.location(op_index));
                    }
                }
            }
        }

        if storage_calls > STORAGE_THRESHOLD {
            suggestions.push(GasGolfingSuggestion {
                pattern_type: "storage_batching".to_string(),
                description: format!(
                    "High ledger access count ({}) — storage reads/writes could be batched",
                    storage_calls
                ),
                location: first_site,
                severity: "high".to_string(),
                gas_saved_estimate: None,
                suggested_fix:
                    "Read values into locals once, compute in memory, then write back once at \
                     the end of the function"
                        .to_string(),
                code_example: Some("Replace per-iteration storage reads with one read outside the loop".to_string()),
            });
        }

        suggestions
    }

    /// Detects deeply nested conditional logic.
    ///
    /// The previous version counted raw `0x04` bytes, so an `if` opcode in an
    /// immediate was counted as a branch. Counting decoded `if` operators is
    /// exact, and the threshold is on the count itself rather than on a doubled
    /// `if`+`else` proxy.
    fn analyze_branching_patterns(&self, module: &crate::parsed_module::ParsedModule) -> Vec<GasGolfingSuggestion> {
        const IF_THRESHOLD: usize = 25;
        let mut suggestions = Vec::new();

        let mut if_count = 0usize;
        let mut first_site: Option<String> = None;
        for function in &module.functions {
            for (op_index, op) in function.operators.iter().enumerate() {
                if matches!(op, wasmparser::Operator::If { .. }) {
                    if_count += 1;
                    if first_site.is_none() {
                        first_site = Some(function.location(op_index));
                    }
                }
            }
        }

        if if_count > IF_THRESHOLD {
            suggestions.push(GasGolfingSuggestion {
                pattern_type: "branch_optimization".to_string(),
                description: format!(
                    "Complex conditional logic: {} `if` instructions — consider a lookup table or \
                     early returns",
                    if_count
                ),
                location: first_site,
                severity: "medium".to_string(),
                gas_saved_estimate: None,
                suggested_fix: "Flatten nested if-else chains with early returns or a lookup table".to_string(),
                code_example: Some("Flatten nested conditionals into early returns".to_string()),
            });
        }

        suggestions
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsed_module::parse_module;

    // -------------------------------------------------------------------------
    // WASM fixture builder (issue #1007)
    //
    // The rules run on decoded instructions, so the fixtures have to be real
    // modules. They are assembled byte by byte here rather than pulled from a
    // compiled artifact so each test states exactly which instruction it is
    // about, and so the "data segment containing the old byte signature" case
    // can be constructed precisely — a real compiler would never emit that.
    // -------------------------------------------------------------------------

    struct Fixture {
        /// `() -> ()`
        body: Vec<u8>,
        /// Number of function imports that precede the defined function.
        import_count: u32,
        /// `put_contract_data` host import, added to the module.
        with_host_import: bool,
        /// Bytes appended as an active data segment at memory offset 0.
        data: Option<Vec<u8>>,
        /// Export name for the defined function, or `None` to leave it internal.
        export: Option<&'static str>,
    }

    impl Fixture {
        fn new() -> Self {
            Fixture {
                body: vec![0x0b], // end
                import_count: 0,
                with_host_import: false,
                data: None,
                export: Some("run"),
            }
        }

        fn body(mut self, ops: &[u8]) -> Self {
            let mut body = ops.to_vec();
            body.push(0x0b); // end
            self.body = body;
            self
        }

        fn with_host_import(mut self) -> Self {
            self.with_host_import = true;
            self.import_count = 1;
            self
        }

        fn with_data(mut self, data: Vec<u8>) -> Self {
            self.data = Some(data);
            self
        }

        fn internal(mut self) -> Self {
            self.export = None;
            self
        }

        fn build(&self) -> Vec<u8> {
            let mut wasm = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];

            // Type section (1): one `() -> ()`.
            wasm.extend_from_slice(&[0x01, 0x04, 0x01, 0x60, 0x00, 0x00]);

            // Import section (2), when the fixture calls a host function.
            if self.with_host_import {
                let mut import = vec![0x01, 0x03, b'e', b'n', b'v', 0x12];
                import.extend_from_slice(b"put_contract_data");
                import.extend_from_slice(&[0x00, 0x00]);
                wasm.push(0x02);
                wasm.push(import.len() as u8);
                wasm.extend_from_slice(&import);
            }

            // Function section (3): one defined function of type 0.
            wasm.extend_from_slice(&[0x03, 0x02, 0x01, 0x00]);

            // Export section (7).
            if let Some(name) = self.export {
                let mut section = vec![0x01, name.len() as u8];
                section.extend_from_slice(name.as_bytes());
                section.push(0x00);
                section.push((self.import_count).to_le_bytes()[0]);
                wasm.push(0x07);
                wasm.push(section.len() as u8);
                wasm.extend_from_slice(&section);
            }

            // Code section (10): one body, no locals.
            let mut body = vec![0x00]; // zero local declarations
            body.extend_from_slice(&self.body);
            let mut section = vec![0x01, body.len() as u8];
            section.extend_from_slice(&body);
            wasm.push(0x0a);
            wasm.push(section.len() as u8);
            wasm.extend_from_slice(&section);

            // Data section (11): one active segment at offset i32.const 0.
            if let Some(data) = &self.data {
                let mut payload = vec![0x00, 0x41, 0x00, 0x0b, data.len() as u8];
                payload.extend_from_slice(data);
                wasm.push(0x0b);
                wasm.push(payload.len() as u8);
                wasm.extend_from_slice(&payload);
            }

            wasm
        }
    }

    // opcodes used by the fixtures
    const NOP: u8 = 0x01;
    const LOOP: u8 = 0x03; // followed by 0x40 (empty block type)
    const BLOCK: u8 = 0x02; // followed by 0x40
    const END: u8 = 0x0b;
    const LOCAL_GET: u8 = 0x20;
    const LOCAL_SET: u8 = 0x21;
    const I32_ADD: u8 = 0x6a;
    const I32_CONST: u8 = 0x41;
    const I32_MUL: u8 = 0x6c;
    const I32_DIV_S: u8 = 0x6d;
    const MEMORY_GROW: u8 = 0x40; // followed by 0x00 memory index
    const CALL: u8 = 0x10;

    /// A data segment holding the old `block`+`loop` byte signature. This is
    /// the false positive the issue names: a byte scanner cannot tell these
    /// bytes from a real loop header, a parser can.
    #[test]
    fn a_data_segment_containing_the_old_byte_pattern_is_not_a_loop() {
        let wasm = Fixture::new()
            .with_data(vec![0x02, 0x40, 0x03, 0x40, 0x10, 0x10, 0x10, 0x10])
            .build();

        let module = parse_module(&wasm).expect("fixture parses");
        assert!(
            crate::parsed_module::loop_spans(&module.functions[0].operators).is_empty(),
            "data-segment bytes must not be decoded as a loop"
        );

        let report = GasGolfingAnalyzer::new().analyze_wasm(&wasm, "data_segment_fixture");
        assert_eq!(
            report.summary.get("loop_optimization").copied().unwrap_or(0),
            0,
            "the data segment must produce zero loop suggestions"
        );
    }

    /// A real `loop` whose body calls a host import, at nesting depth 1.
    #[test]
    fn a_loop_calling_a_host_import_is_flagged_and_located() {
        let wasm = Fixture::new()
            .with_host_import()
            .internal()
            .body(&[
                BLOCK, 0x40, // block
                LOOP, 0x40, // loop
                CALL, 0x00, // call the host import (index 0)
                LOCAL_GET, 0x00, // counter
                I32_CONST, 0x01,
                I32_ADD,
                LOCAL_SET, 0x00,
                END, END,
            ])
            .build();

        let report = GasGolfingAnalyzer::new().analyze_wasm(&wasm, "host_call_loop");

        assert_eq!(report.summary.get("loop_optimization").copied().unwrap_or(0), 1);

        let suggestion = report
            .suggestions
            .iter()
            .find(|s| s.pattern_type == "loop_optimization")
            .expect("a loop suggestion");
        // The function is internal, so the location names the function index.
        let location = suggestion.location.as_deref().expect("a location is set");
        assert!(
            location.starts_with("func#"),
            "expected a function-indexed location, got {location}"
        );
        assert!(suggestion.description.contains("1 host call(s)"), "got {}", suggestion.description);
    }

    /// An exported loop that calls a host import: the location names the export.
    #[test]
    fn an_exported_loop_location_names_the_function() {
        let wasm = Fixture::new()
            .with_host_import()
            .body(&[LOOP, 0x40, CALL, 0x00, END])
            .build();

        let report = GasGolfingAnalyzer::new().analyze_wasm(&wasm, "exported_loop");
        let suggestion = report
            .suggestions
            .iter()
            .find(|s| s.pattern_type == "loop_optimization")
            .expect("a loop suggestion");
        let location = suggestion.location.as_deref().expect("a location is set");
        assert!(location.starts_with("run#+"), "expected an export-named location, got {location}");
    }

    /// A counter loop does no per-iteration host work and must not be flagged.
    #[test]
    fn a_counter_loop_is_not_flagged() {
        let wasm = Fixture::new()
            .body(&[
                LOOP, 0x40,
                LOCAL_GET, 0x00,
                I32_CONST, 0x01,
                I32_ADD,
                LOCAL_SET, 0x00,
                END,
            ])
            .build();

        let report = GasGolfingAnalyzer::new().analyze_wasm(&wasm, "counter_loop");

        assert_eq!(
            report.summary.get("loop_optimization").copied().unwrap_or(0),
            0,
            "a counter loop has nothing to hoist"
        );
    }

    /// A loop whose only cost is `memory.grow` is still hoistable work.
    #[test]
    fn a_loop_that_grows_memory_is_flagged() {
        let wasm = Fixture::new().body(&[LOOP, 0x40, MEMORY_GROW, 0x00, END]).build();
        let report = GasGolfingAnalyzer::new().analyze_wasm(&wasm, "grow_loop");
        assert_eq!(report.summary.get("loop_optimization").copied().unwrap_or(0), 1);
    }

    /// The old byte scanner reported suggestions for arbitrary input. A parser
    /// must report the input as unparseable instead.
    #[test]
    fn non_wasm_input_reports_a_parse_error_instead_of_suggestions() {
        let report = GasGolfingAnalyzer::new().analyze_wasm(b"definitely not wasm", "garbage");

        assert_eq!(report.total_suggestions, 1);
        assert_eq!(report.suggestions[0].pattern_type, "parse_error");
        assert!(report.suggestions[0].description.contains("not a valid WASM module"));
    }

    /// A valid module with nothing notable produces no suggestions.
    #[test]
    fn a_clean_module_produces_no_suggestions() {
        let wasm = Fixture::new().body(&[NOP, NOP, NOP]).build();
        let report = GasGolfingAnalyzer::new().analyze_wasm(&wasm, "clean");

        assert_eq!(report.contract_name, "clean");
        assert_eq!(report.total_suggestions, 0);
    }

    /// Storage detection must key on the import's host name, not on a guessed
    /// low call index.
    #[test]
    fn storage_detection_keys_on_the_import_name() {
        // A loop calling a host import named put_contract_data.
        let wasm = Fixture::new()
            .with_host_import()
            .body(&[LOOP, 0x40, CALL, 0x00, END])
            .build();

        let module = parse_module(&wasm).expect("fixture parses");
        assert_eq!(module.host_imports[0].qualified(), "env.put_contract_data");

        // One call is far below the batching threshold, so no suggestion — but
        // the import is recognised, which is what the old index heuristic could
        // not guarantee.
        let report = GasGolfingAnalyzer::new().analyze_wasm(&wasm, "one_storage_call");
        assert_eq!(report.summary.get("storage_batching").copied().unwrap_or(0), 0);
    }

    #[test]
    fn a_single_division_below_threshold_produces_no_suggestion() {
        let wasm = Fixture::new().body(&[LOCAL_GET, 0x00, I32_CONST, 0x02, I32_DIV_S]).build();
        let report = GasGolfingAnalyzer::new().analyze_wasm(&wasm, "one_division");
        assert_eq!(report.summary.get("arithmetic_optimization").copied().unwrap_or(0), 0);
    }

    #[test]
    fn multiplication_by_a_power_of_two_constant_is_flagged_with_a_location() {
        let wasm = Fixture::new().body(&[LOCAL_GET, 0x00, I32_CONST, 0x08, I32_MUL]).build();
        let report = GasGolfingAnalyzer::new().analyze_wasm(&wasm, "mul_by_eight");

        assert_eq!(report.summary.get("multiplication_optimization").copied().unwrap_or(0), 1);
        let suggestion = report
            .suggestions
            .iter()
            .find(|s| s.pattern_type == "multiplication_optimization")
            .expect("a multiplication suggestion");
        assert!(suggestion.location.as_deref().unwrap_or_default().starts_with("run#+"));
    }

    /// Multiplication by 2 is not worth a shift, and 0/1 are not multipliers.
    #[test]
    fn multiplication_by_one_is_not_flagged() {
        let wasm = Fixture::new().body(&[LOCAL_GET, 0x00, I32_CONST, 0x01, I32_MUL]).build();
        let report = GasGolfingAnalyzer::new().analyze_wasm(&wasm, "mul_by_one");
        assert_eq!(report.summary.get("multiplication_optimization").copied().unwrap_or(0), 0);
    }

    /// `gas_saved_estimate` is not emitted until it can be priced from a cost
    /// table; an unpriced guess is worse than an absent number.
    #[test]
    fn no_suggestion_carries_an_unpriced_estimate() {
        let wasm = Fixture::new()
            .with_host_import()
            .body(&[LOOP, 0x40, CALL, 0x00, END])
            .build();
        let report = GasGolfingAnalyzer::new().analyze_wasm(&wasm, "unpriced");

        for suggestion in &report.suggestions {
            assert_eq!(
                suggestion.gas_saved_estimate, None,
                "{} still carries an unpriced estimate",
                suggestion.pattern_type
            );
        }
    }

    /// The report's JSON shape is part of the API contract.
    #[test]
    fn report_field_names_are_unchanged() {
        let wasm = Fixture::new().body(&[NOP]).build();
        let report = GasGolfingAnalyzer::new().analyze_wasm(&wasm, "shape");
        let json = serde_json::to_string(&report).expect("report serialises");

        for field in [
            "contract_name",
            "analysis_timestamp",
            "total_suggestions",
            "suggestions",
            "summary",
        ] {
            assert!(json.contains(field), "missing {field} in {json}");
        }
    }
}
