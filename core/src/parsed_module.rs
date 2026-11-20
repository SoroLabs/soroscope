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

use std::collections::{HashMap, HashSet};

use stellar_xdr::curr::{Limited, Limits, ReadXdr, ScEnvMetaEntry, ScMetaEntry, ScSpecEntry};
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

// ── Contract custom sections & spec validation (Issue #1406) ─────────────────

/// Custom section holding the Soroban environment interface version.
pub const CONTRACT_ENV_META_SECTION: &str = "contractenvmetav0";
/// Custom section holding the XDR-encoded contract spec.
pub const CONTRACT_SPEC_SECTION: &str = "contractspecv0";
/// Custom section holding free-form contract metadata (key/value pairs).
pub const CONTRACT_META_SECTION: &str = "contractmetav0";

/// A structured compatibility diagnostic for a contract's custom sections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompatibilityWarning {
    /// No `contractenvmetav0` section, so the interface version is unknown.
    MissingEnvMeta,
    /// No `contractspecv0` section, so exports cannot be cross-checked.
    MissingSpec,
    /// A custom section could not be decoded as XDR. An unknown
    /// `ScSpecTypeDef` (or other enum) discriminant lands here too.
    MalformedSection {
        section: String,
        offset: usize,
        reason: String,
    },
    /// Built for an older protocol than the simulator's host.
    OutdatedProtocol {
        contract_protocol: u32,
        host_protocol: u32,
    },
    /// Built for a newer protocol than the simulator's host can run.
    UnsupportedProtocol {
        contract_protocol: u32,
        host_protocol: u32,
    },
    /// Pre-release interface versions must match exactly.
    PreReleaseMismatch {
        contract_pre_release: u32,
        host_pre_release: u32,
    },
    /// The same function name is exported more than once.
    DuplicateExport { name: String },
    /// The same function name appears in more than one spec entry.
    DuplicateSpecFunction { name: String },
    /// An exported function has no `contractspecv0` entry.
    ExportWithoutSpec { name: String },
    /// A spec function is not exported by the module.
    SpecWithoutExport { name: String },
}

/// What was decoded from a contract's custom sections, plus every
/// compatibility warning found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContractMetadataReport {
    /// `(protocol, pre_release)` from `contractenvmetav0`.
    pub interface_version: Option<(u32, u32)>,
    /// Key/value pairs from `contractmetav0`.
    pub meta: Vec<(String, String)>,
    /// Function names declared in `contractspecv0`.
    pub spec_functions: Vec<String>,
    /// Exported function names, in export order.
    pub exported_functions: Vec<String>,
    pub warnings: Vec<CompatibilityWarning>,
}

/// Decode a stream of back-to-back XDR values, returning the byte offset and
/// reason of the first value that fails to decode.
fn decode_xdr_stream<T: ReadXdr>(data: &[u8]) -> Result<Vec<T>, (usize, String)> {
    let mut reader = Limited::new(std::io::Cursor::new(data), Limits::none());
    let mut values = Vec::new();
    while (reader.inner.position() as usize) < data.len() {
        let offset = reader.inner.position() as usize;
        match T::read_xdr(&mut reader) {
            Ok(value) => values.push(value),
            Err(e) => return Err((offset, e.to_string())),
        }
    }
    Ok(values)
}

/// Compare a contract's interface version against the simulator's host.
fn check_interface_version(
    contract_protocol: u32,
    contract_pre_release: u32,
    host_protocol: u32,
    host_pre_release: u32,
) -> Option<CompatibilityWarning> {
    if contract_protocol > host_protocol {
        return Some(CompatibilityWarning::UnsupportedProtocol {
            contract_protocol,
            host_protocol,
        });
    }
    if contract_protocol < host_protocol {
        return Some(CompatibilityWarning::OutdatedProtocol {
            contract_protocol,
            host_protocol,
        });
    }
    // Same protocol: pre-release builds are only compatible with the exact
    // same pre-release.
    if contract_pre_release != host_pre_release {
        return Some(CompatibilityWarning::PreReleaseMismatch {
            contract_pre_release,
            host_pre_release,
        });
    }
    None
}

/// Extract and validate the `contractenvmetav0`, `contractspecv0` and
/// `contractmetav0` custom sections against the active simulator protocol.
///
/// Returns `Err` only when the bytes are not a WASM module at all; every
/// contract-level problem is reported as a [`CompatibilityWarning`].
pub fn validate_contract_metadata(
    wasm_bytes: &[u8],
) -> Result<ContractMetadataReport, ParseFailure> {
    let host = soroban_env_host::meta::INTERFACE_VERSION;
    validate_contract_metadata_against(wasm_bytes, host.protocol, host.pre_release)
}

/// [`validate_contract_metadata`] against an explicit host interface version.
pub fn validate_contract_metadata_against(
    wasm_bytes: &[u8],
    host_protocol: u32,
    host_pre_release: u32,
) -> Result<ContractMetadataReport, ParseFailure> {
    let mut report = ContractMetadataReport::default();
    let mut saw_env_meta = false;
    let mut saw_spec = false;

    for payload in Parser::new(0).parse_all(wasm_bytes) {
        let payload = payload.map_err(|e| ParseFailure(e.to_string()))?;
        match payload {
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export.map_err(|e| ParseFailure(e.to_string()))?;
                    if export.kind == ExternalKind::Func {
                        report.exported_functions.push(export.name.to_string());
                    }
                }
            }
            Payload::CustomSection(custom) => {
                let section = custom.name();
                let offset = custom.data_offset();
                let malformed =
                    |at: usize, reason: String| CompatibilityWarning::MalformedSection {
                        section: section.to_string(),
                        offset: offset + at,
                        reason,
                    };
                if section == CONTRACT_ENV_META_SECTION {
                    saw_env_meta = true;
                    match decode_xdr_stream::<ScEnvMetaEntry>(custom.data()) {
                        Ok(entries) => {
                            for entry in entries {
                                let ScEnvMetaEntry::ScEnvMetaKindInterfaceVersion(v) = entry;
                                report.interface_version = Some((v.protocol, v.pre_release));
                            }
                        }
                        Err((at, reason)) => report.warnings.push(malformed(at, reason)),
                    }
                } else if section == CONTRACT_SPEC_SECTION {
                    saw_spec = true;
                    match decode_xdr_stream::<ScSpecEntry>(custom.data()) {
                        Ok(entries) => {
                            for entry in entries {
                                if let ScSpecEntry::FunctionV0(func) = entry {
                                    report
                                        .spec_functions
                                        .push(func.name.0.to_utf8_string_lossy());
                                }
                            }
                        }
                        Err((at, reason)) => report.warnings.push(malformed(at, reason)),
                    }
                } else if section == CONTRACT_META_SECTION {
                    match decode_xdr_stream::<ScMetaEntry>(custom.data()) {
                        Ok(entries) => {
                            for entry in entries {
                                let ScMetaEntry::ScMetaV0(m) = entry;
                                report.meta.push((
                                    m.key.to_utf8_string_lossy(),
                                    m.val.to_utf8_string_lossy(),
                                ));
                            }
                        }
                        Err((at, reason)) => report.warnings.push(malformed(at, reason)),
                    }
                }
            }
            _ => {}
        }
    }

    match report.interface_version {
        Some((protocol, pre_release)) => {
            if let Some(warning) =
                check_interface_version(protocol, pre_release, host_protocol, host_pre_release)
            {
                report.warnings.push(warning);
            }
        }
        // A present-but-malformed section was already reported above.
        None if !saw_env_meta => report.warnings.push(CompatibilityWarning::MissingEnvMeta),
        None => {}
    }

    let mut seen = HashSet::new();
    for name in &report.exported_functions {
        if !seen.insert(name.as_str()) {
            report
                .warnings
                .push(CompatibilityWarning::DuplicateExport { name: name.clone() });
        }
    }

    let mut seen = HashSet::new();
    for name in &report.spec_functions {
        if !seen.insert(name.as_str()) {
            report
                .warnings
                .push(CompatibilityWarning::DuplicateSpecFunction { name: name.clone() });
        }
    }

    if !saw_spec {
        report.warnings.push(CompatibilityWarning::MissingSpec);
    } else {
        let spec: HashSet<&str> = report.spec_functions.iter().map(String::as_str).collect();
        let exported: HashSet<&str> = report
            .exported_functions
            .iter()
            .map(String::as_str)
            .collect();
        // Names starting with `_` are SDK-internal exports, not contract
        // functions, so they are not expected in the spec.
        for name in &report.exported_functions {
            if !name.starts_with('_') && !spec.contains(name.as_str()) {
                report
                    .warnings
                    .push(CompatibilityWarning::ExportWithoutSpec { name: name.clone() });
            }
        }
        for name in &report.spec_functions {
            if !exported.contains(name.as_str()) {
                report
                    .warnings
                    .push(CompatibilityWarning::SpecWithoutExport { name: name.clone() });
            }
        }
    }

    Ok(report)
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

    // ── Custom section & contract spec validation (Issue #1406) ───────────

    use stellar_xdr::curr::{
        ScEnvMetaEntryInterfaceVersion, ScMetaV0, ScSpecFunctionV0, ScSymbol, VecM, WriteXdr,
    };

    /// Unsigned LEB128, as used for WASM section sizes.
    fn leb128(mut value: usize) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let byte = (value & 0x7f) as u8;
            value >>= 7;
            if value == 0 {
                out.push(byte);
                return out;
            }
            out.push(byte | 0x80);
        }
    }

    /// Encode a WASM custom section.
    fn custom_section(name: &str, data: &[u8]) -> Vec<u8> {
        let mut body = leb128(name.len());
        body.extend_from_slice(name.as_bytes());
        body.extend_from_slice(data);
        let mut section = vec![0x00];
        section.extend(leb128(body.len()));
        section.extend(body);
        section
    }

    /// One `() -> ()` function exported under every name in `names`.
    fn module_exporting(names: &[&str]) -> Vec<u8> {
        let mut wasm = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        wasm.extend_from_slice(&[0x01, 0x04, 0x01, 0x60, 0x00, 0x00]);
        wasm.extend_from_slice(&[0x03, 0x02, 0x01, 0x00]);
        let mut exports = leb128(names.len());
        for name in names {
            exports.extend(leb128(name.len()));
            exports.extend_from_slice(name.as_bytes());
            exports.extend_from_slice(&[0x00, 0x00]);
        }
        wasm.push(0x07);
        wasm.extend(leb128(exports.len()));
        wasm.extend(exports);
        wasm.extend_from_slice(&[0x0a, 0x04, 0x01, 0x02, 0x00, 0x0b]);
        wasm
    }

    fn env_meta(protocol: u32, pre_release: u32) -> Vec<u8> {
        ScEnvMetaEntry::ScEnvMetaKindInterfaceVersion(ScEnvMetaEntryInterfaceVersion {
            protocol,
            pre_release,
        })
        .to_xdr(Limits::none())
        .expect("encode env meta")
    }

    fn spec_function(name: &str) -> Vec<u8> {
        ScSpecEntry::FunctionV0(ScSpecFunctionV0 {
            doc: Default::default(),
            name: ScSymbol(name.try_into().expect("valid symbol")),
            inputs: VecM::default(),
            outputs: VecM::default(),
        })
        .to_xdr(Limits::none())
        .expect("encode spec entry")
    }

    /// A contract exporting `run`, with a matching spec and env meta.
    fn contract(env: Option<Vec<u8>>, spec: Option<Vec<u8>>) -> Vec<u8> {
        let mut wasm = module_exporting(&["run"]);
        if let Some(env) = env {
            wasm.extend(custom_section(CONTRACT_ENV_META_SECTION, &env));
        }
        if let Some(spec) = spec {
            wasm.extend(custom_section(CONTRACT_SPEC_SECTION, &spec));
        }
        wasm
    }

    #[test]
    fn a_valid_contract_has_no_warnings() {
        let wasm = contract(Some(env_meta(22, 0)), Some(spec_function("run")));
        let meta = ScMetaEntry::ScMetaV0(ScMetaV0 {
            key: "rsver".try_into().unwrap(),
            val: "1.81.0".try_into().unwrap(),
        })
        .to_xdr(Limits::none())
        .unwrap();
        let mut wasm = wasm;
        wasm.extend(custom_section(CONTRACT_META_SECTION, &meta));

        let report = validate_contract_metadata_against(&wasm, 22, 0).expect("valid module");

        assert_eq!(report.interface_version, Some((22, 0)));
        assert_eq!(report.spec_functions, vec!["run".to_string()]);
        assert_eq!(
            report.meta,
            vec![("rsver".to_string(), "1.81.0".to_string())]
        );
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    }

    #[test]
    fn missing_env_meta_is_reported() {
        let wasm = contract(None, Some(spec_function("run")));
        let report = validate_contract_metadata_against(&wasm, 22, 0).unwrap();
        assert_eq!(report.warnings, vec![CompatibilityWarning::MissingEnvMeta]);
    }

    #[test]
    fn an_outdated_protocol_is_reported() {
        let wasm = contract(Some(env_meta(20, 0)), Some(spec_function("run")));
        let report = validate_contract_metadata_against(&wasm, 22, 0).unwrap();
        assert_eq!(
            report.warnings,
            vec![CompatibilityWarning::OutdatedProtocol {
                contract_protocol: 20,
                host_protocol: 22
            }]
        );
    }

    #[test]
    fn a_newer_protocol_or_pre_release_mismatch_is_reported() {
        let newer = contract(Some(env_meta(23, 0)), Some(spec_function("run")));
        assert_eq!(
            validate_contract_metadata_against(&newer, 22, 0)
                .unwrap()
                .warnings,
            vec![CompatibilityWarning::UnsupportedProtocol {
                contract_protocol: 23,
                host_protocol: 22
            }]
        );

        let pre_release = contract(Some(env_meta(22, 1)), Some(spec_function("run")));
        assert_eq!(
            validate_contract_metadata_against(&pre_release, 22, 0)
                .unwrap()
                .warnings,
            vec![CompatibilityWarning::PreReleaseMismatch {
                contract_pre_release: 1,
                host_pre_release: 0
            }]
        );
    }

    #[test]
    fn malformed_sections_are_reported_with_their_offset() {
        // Truncated env meta and a spec entry with an unknown discriminant.
        let wasm = contract(Some(vec![0x00, 0x00]), Some(vec![0x00, 0x00, 0x00, 0x7f]));
        let report = validate_contract_metadata_against(&wasm, 22, 0).unwrap();

        let malformed: Vec<&str> = report
            .warnings
            .iter()
            .filter_map(|w| match w {
                CompatibilityWarning::MalformedSection { section, .. } => Some(section.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            malformed,
            vec![CONTRACT_ENV_META_SECTION, CONTRACT_SPEC_SECTION]
        );
        // A present-but-malformed env meta is not also reported as missing.
        assert!(!report
            .warnings
            .contains(&CompatibilityWarning::MissingEnvMeta));
    }

    #[test]
    fn exports_are_cross_checked_against_the_spec() {
        let mut wasm = module_exporting(&["run", "transfer", "_"]);
        wasm.extend(custom_section(CONTRACT_ENV_META_SECTION, &env_meta(22, 0)));
        let mut spec = spec_function("run");
        spec.extend(spec_function("mint"));
        wasm.extend(custom_section(CONTRACT_SPEC_SECTION, &spec));

        let report = validate_contract_metadata_against(&wasm, 22, 0).unwrap();

        assert_eq!(
            report.warnings,
            vec![
                CompatibilityWarning::ExportWithoutSpec {
                    name: "transfer".to_string()
                },
                CompatibilityWarning::SpecWithoutExport {
                    name: "mint".to_string()
                },
            ]
        );
    }

    #[test]
    fn duplicate_exports_and_spec_functions_are_reported() {
        let mut wasm = module_exporting(&["run", "run"]);
        wasm.extend(custom_section(CONTRACT_ENV_META_SECTION, &env_meta(22, 0)));
        let mut spec = spec_function("run");
        spec.extend(spec_function("run"));
        wasm.extend(custom_section(CONTRACT_SPEC_SECTION, &spec));

        let report = validate_contract_metadata_against(&wasm, 22, 0).unwrap();

        assert!(report
            .warnings
            .contains(&CompatibilityWarning::DuplicateExport {
                name: "run".to_string()
            }));
        assert!(report
            .warnings
            .contains(&CompatibilityWarning::DuplicateSpecFunction {
                name: "run".to_string()
            }));
    }

    #[test]
    fn a_missing_spec_is_reported() {
        let wasm = contract(Some(env_meta(22, 0)), None);
        let report = validate_contract_metadata_against(&wasm, 22, 0).unwrap();
        assert_eq!(report.warnings, vec![CompatibilityWarning::MissingSpec]);
    }

    #[test]
    fn validation_uses_the_simulator_host_protocol() {
        let host = soroban_env_host::meta::INTERFACE_VERSION;
        let wasm = contract(
            Some(env_meta(host.protocol, host.pre_release)),
            Some(spec_function("run")),
        );
        let report = validate_contract_metadata(&wasm).unwrap();
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    }

    #[test]
    fn non_wasm_input_is_an_error() {
        assert!(validate_contract_metadata(b"not a wasm module").is_err());
    }
}
