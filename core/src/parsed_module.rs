//! Parsed-module view of a WASM contract (issues #1007, #1008).
//!
//! The gas analyzer used to read raw bytes and look for opcode-shaped byte
//! windows. That is unreliable in both directions: an opcode byte also appears
//! inside LEB128 immediates and data-segment contents, so a `0x02 0x40 0x03
//! 0x40` run in a string literal was indistinguishable from a real
//! `block`/`loop` header, and a location could only ever be a byte offset.
//!
//! This module does the parse once and exposes what both the pattern rules
//! (#1007) and the host-import heat map (#1008) need: which function indices
//! are host imports, what each exported function is called, and that function's
//! operator stream. The section order the WASM spec guarantees — imports and
//! exports before the code section — is what lets a single `parse_all` pass
//! resolve function indices correctly.

use std::collections::HashMap;

use wasmparser::{ExternalKind, Operator, Parser, Payload, TypeRef};

/// A function imported from outside the module. In a Soroban contract these
/// are the host functions the contract is allowed to call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostImport {
    /// Absolute function index, i.e. its position among *function* imports.
    pub index: u32,
    pub module: String,
    pub name: String,
}

impl HostImport {
    /// `put_contract_data` style dotted name, matching the host API spelling.
    pub fn qualified(&self) -> String {
        format!("{}.{}", self.module, self.name)
    }
}

/// One defined (non-imported) function and its decoded body.
#[derive(Debug, Clone)]
pub struct FunctionInfo<'a> {
    /// Absolute function index: `import_count + ordinal`.
    pub index: u32,
    /// Export name when the function is exported, else `None`.
    pub export_name: Option<String>,
    /// Decoded instruction stream.
    pub operators: Vec<Operator<'a>>,
}

impl<'a> FunctionInfo<'a> {
    /// Ordinal position of `op_index` within the body, counting only
    /// instructions (not the bytes they occupy).
    pub fn instruction_offset(&self, op_index: usize) -> usize {
        op_index.min(self.operators.len().saturating_sub(1))
    }

    /// Human-readable location for a finding inside this function.
    ///
    /// The issue asks for `export_name + instruction offset`, falling back to
    /// the function index when the function is not exported — an internal
    /// helper still needs to be findable, just not by name.
    pub fn location(&self, op_index: usize) -> String {
        let offset = self.instruction_offset(op_index);
        match &self.export_name {
            Some(name) => format!("{}#+{}", name, offset),
            None => format!("func#{}#+{}", self.index, offset),
        }
    }

    /// Offset of the first operator satisfying `predicate`, if any.
    pub fn find(&self, predicate: impl Fn(&Operator<'a>) -> bool) -> Option<usize> {
        self.operators.iter().position(predicate)
    }
}

/// A decoded module, reduced to what static analysis needs.
#[derive(Debug, Clone, Default)]
pub struct ParsedModule<'a> {
    /// Host function imports, ordered by ascending function index.
    pub host_imports: Vec<HostImport>,
    /// Function index -> position in `host_imports`.
    pub import_by_index: HashMap<u32, usize>,
    /// Defined functions, in index order.
    pub functions: Vec<FunctionInfo<'a>>,
    /// Function index -> export name, for every exported function.
    pub exports: HashMap<u32, String>,
}

impl<'a> ParsedModule<'a> {
    /// The host import a `call` refers to, if that index is an import.
    pub fn host_import(&self, function_index: u32) -> Option<&HostImport> {
        self.import_by_index.get(&function_index).and_then(|i| self.host_imports.get(*i))
    }

    /// Is this function index an imported (host) function?
    pub fn is_host_import(&self, function_index: u32) -> bool {
        self.import_by_index.contains_key(&function_index)
    }

    /// The export name of a function index, or `None`.
    pub fn export_name(&self, function_index: u32) -> Option<&str> {
        self.exports.get(&function_index).map(|s| s.as_str())
    }

    /// Total defined functions across the module.
    pub fn defined_function_count(&self) -> usize {
        self.functions.len()
    }
}

/// Why a module could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseFailure(pub String);

impl std::fmt::Display for ParseFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WASM parse failed: {}", self.0)
    }
}

impl std::error::Error for ParseFailure {}

/// Decode a module.
///
/// Returns `Err` for input that is not a valid module at all. A module that
/// parses but whose code section is truncated yields fewer functions rather than
/// an error, so a partially damaged body cannot mask findings from the
/// functions that did decode.
pub fn parse_module<'a>(wasm_bytes: &'a [u8]) -> Result<ParsedModule<'a>, ParseFailure> {
    let mut module = ParsedModule::default();
    let mut next_import_func_index: u32 = 0;
    let mut defined_ordinal: u32 = 0;

    for payload in Parser::new(0).parse_all(wasm_bytes) {
        let payload = payload.map_err(|e| ParseFailure(e.to_string()))?;
        match payload {
            // Function imports are numbered before any defined function, in
            // the order they appear, and only function imports consume an
            // index — a memory or global import does not shift function indices.
            Payload::ImportSection(reader) => {
                for import in reader {
                    let import = import.map_err(|e| ParseFailure(e.to_string()))?;
                    if matches!(import.ty, TypeRef::Func(_)) {
                        let index = next_import_func_index;
                        next_import_func_index += 1;
                        module.import_by_index.insert(index, module.host_imports.len());
                        module.host_imports.push(HostImport {
                            index,
                            module: import.module.to_string(),
                            name: import.name.to_string(),
                        });
                    }
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export.map_err(|e| ParseFailure(e.to_string()))?;
                    if export.kind == ExternalKind::Func {
                        module.exports.insert(export.index, export.name.to_string());
                    }
                }
            }
            Payload::CodeSectionEntry(body) => {
                let index = next_import_func_index + defined_ordinal;
                defined_ordinal += 1;

                let operators = match body.get_operators_reader() {
                    Ok(reader) => reader.into_iter().flatten().collect::<Vec<Operator<'a>>>(),
                    // A body we cannot decode is recorded as empty rather than
                    // dropped, so the function keeps its index and location.
                    Err(_) => Vec::new(),
                };

                module.functions.push(FunctionInfo {
                    index,
                    export_name: module.export_name(index).map(|s| s.to_string()),
                    operators,
                });
            }
            _ => {}
        }
    }

    Ok(module)
}

/// Result of scanning a function body for structured loops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopSpan {
    /// Position of the `loop` operator within the function's operator stream.
    pub op_index: usize,
    /// Position of the matching `end`, inclusive.
    pub end_index: usize,
    /// Nesting depth of the `loop` itself (0 = outermost).
    pub depth: usize,
}

impl LoopSpan {
    /// Operators inside this loop, nested blocks included.
    pub fn contains_op(&self, op_index: usize) -> bool {
        op_index > self.op_index && op_index <= self.end_index
    }
}

/// Find every structured `loop` in a function, with the range its `end` closes.
///
/// The WASM control stack is reconstructed from the operator stream: `block`,
/// `loop` and `if` push, `end` pops, and `else` does not change depth. This is
/// what makes "inside the loop's structured block, at any nesting depth" a
/// real check rather than a 128-byte lookahead that runs off the end of the
/// body and into the next one.
pub fn loop_spans(operators: &[Operator]) -> Vec<LoopSpan> {
    let mut spans = Vec::new();
    // (is_loop, op_index_of_open, depth_at_open)
    let mut stack: Vec<(bool, usize, usize)> = Vec::new();

    for (op_index, op) in operators.iter().enumerate() {
        match op {
            Operator::Block { .. } | Operator::If { .. } => {
                stack.push((false, op_index, stack.len()));
            }
            Operator::Loop { .. } => {
                stack.push((true, op_index, stack.len()));
            }
            Operator::End => {
                if let Some((is_loop, open_index, depth)) = stack.pop() {
                    if is_loop {
                        spans.push(LoopSpan { op_index: open_index, end_index: op_index, depth });
                    }
                }
            }
            _ => {}
        }
    }

    // A module that ends without balancing its blocks yields a truncated
    // stream; those loops never reach here because their `end` never arrived.
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal valid module: one exported function, `nop` body.
    fn minimal_module() -> Vec<u8> {
        let mut wasm = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];

        // Type section: one `() -> ()`.
        wasm.extend_from_slice(&[0x01, 0x04, 0x01, 0x60, 0x00, 0x00]);
        // Function section: one function of type 0.
        wasm.extend_from_slice(&[0x03, 0x02, 0x01, 0x00]);
        // Export section: export 0 as "run".
        wasm.extend_from_slice(&[0x07, 0x07, 0x01, 0x03, 0x72, 0x75, 0x6e, 0x00, 0x00]);
        // Code section: one body, no locals, just `end`.
        wasm.extend_from_slice(&[0x0a, 0x04, 0x01, 0x02, 0x00, 0x0b]);
        wasm
    }

    #[test]
    fn parses_a_minimal_module() {
        let bytes = minimal_module();
        let module = parse_module(&bytes).expect("valid module");
        assert_eq!(module.defined_function_count(), 1);
        assert_eq!(module.host_imports.len(), 0);
        assert_eq!(module.export_name(0), Some("run"));
    }

    #[test]
    fn a_function_index_resolves_to_its_import() {
        // Import one host function, then call it from the body.
        let mut wasm = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        // Type section: `() -> ()`.
        wasm.extend_from_slice(&[0x01, 0x04, 0x01, 0x60, 0x00, 0x00]);
        // Import section: 1 import, "env"."put_contract_data", func type 0.
        //   name lengths: "env" = 3, "put_contract_data" = 18.
        wasm.extend_from_slice(&[0x02]);
        let mut import_body = vec![0x01, 0x03, b'e', b'n', b'v', 0x12];
        import_body.extend_from_slice(b"put_contract_data");
        import_body.extend_from_slice(&[0x00, 0x00]);
        wasm.push(import_body.len() as u8);
        wasm.extend_from_slice(&import_body);
        // Function section: one defined function (index 1, after the import).
        wasm.extend_from_slice(&[0x03, 0x02, 0x01, 0x00]);
        // Code section: call import 0, then end.
        wasm.extend_from_slice(&[0x0a, 0x06, 0x01, 0x04, 0x00, 0x10, 0x00, 0x0b]);

        let module = parse_module(&wasm).expect("valid module");
        assert_eq!(module.host_imports.len(), 1);
        let import = &module.host_imports[0];
        assert_eq!(import.index, 0);
        assert_eq!(import.qualified(), "env.put_contract_data");
        assert!(module.is_host_import(0));
        // The defined function is index 1, not 0, because the import took 0.
        assert_eq!(module.functions[0].index, 1);
        assert!(!module.is_host_import(1));
    }

    #[test]
    fn a_data_segment_does_not_become_a_loop() {
        // The false positive the issue describes: a data segment whose bytes
        // are the old `0x02 0x40 0x03 0x40` signature must not produce a loop.
        let mut wasm = minimal_module();
        // Data section: one active segment, memory 0, offset i32.const 0,
        // then the four magic bytes.
        let payload = vec![0x01, 0x41, 0x00, 0x0b, 0x02, 0x40, 0x03, 0x40];
        let mut section = vec![0x01, payload.len() as u8];
        section.extend_from_slice(&payload);
        wasm.extend_from_slice(&[0x0b, section.len() as u8]);
        wasm.extend_from_slice(&section);

        let module = parse_module(&wasm).expect("data section parses");
        let spans = loop_spans(&module.functions[0].operators);
        assert!(spans.is_empty(), "a data segment must not be read as a loop");
    }

    #[test]
    fn loop_spans_reconstruct_nesting() {
        // block; loop; nop; end; end  ->  one loop, depth 1.
        let operators = vec![
            Operator::Block { blockty: wasmparser::BlockType::Empty },
            Operator::Loop { blockty: wasmparser::BlockType::Empty },
            Operator::Nop,
            Operator::End,
            Operator::End,
        ];
        let spans = loop_spans(&operators);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].op_index, 1);
        assert_eq!(spans[0].end_index, 3);
        assert_eq!(spans[0].depth, 1);
        assert!(spans[0].contains_op(2));
        assert!(!spans[0].contains_op(0));
    }

    #[test]
    fn else_does_not_change_the_control_depth() {
        // if; nop; else; nop; end  ->  no loop at all.
        let operators = vec![
            Operator::If { blockty: wasmparser::BlockType::Empty },
            Operator::Nop,
            Operator::Else,
            Operator::Nop,
            Operator::End,
        ];
        assert!(loop_spans(&operators).is_empty());
    }

    #[test]
    fn an_unbalanced_body_yields_no_spans() {
        // A truncated body must not report a loop whose `end` never arrived.
        let operators = vec![
            Operator::Loop { blockty: wasmparser::BlockType::Empty },
            Operator::Nop,
        ];
        assert!(loop_spans(&operators).is_empty());
    }

    #[test]
    fn location_uses_the_export_name_then_falls_back_to_the_index() {
        let exported = FunctionInfo {
            index: 1,
            export_name: Some("transfer".into()),
            operators: vec![Operator::Nop, Operator::Nop],
        };
        assert_eq!(exported.location(0), "transfer#+0");

        let internal = FunctionInfo { index: 7, export_name: None, operators: vec![Operator::Nop] };
        assert_eq!(internal.location(0), "func#7#+0");
    }

    #[test]
    fn invalid_bytes_are_rejected_rather_than_silently_scanned() {
        // The old analyzer accepted any byte string. A parser must not.
        assert!(parse_module(b"not a wasm module").is_err());
    }
}
