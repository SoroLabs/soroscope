use serde::{Deserialize, Serialize};
use serde_json::Value;
use soroban_sdk::xdr::{
    Hash, Limits, ScAddress, ScMap, ScMapEntry, ScString, ScSymbol, ScVal, ScVec, StringM, Uint256,
    VecM, WriteXdr,
};
use stellar_strkey::Strkey;
use thiserror::Error;

#[derive(Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum ParserError {
    #[error("Invalid JSON type at {location}: expected {expected}, found {found}")]
    InvalidType {
        location: String,
        expected: String,
        found: String,
    },

    #[error("Invalid symbol at {location}: {details}")]
    InvalidSymbol { location: String, details: String },

    #[error("Invalid hex bytes at {location}: {details}")]
    InvalidHex { location: String, details: String },
}

pub struct ArgParser;

impl ArgParser {
    /// Parse a JSON string into an ScVal
    pub fn parse(json: &str) -> Result<ScVal, ParserError> {
        let value: Value = serde_json::from_str(json).map_err(|e| ParserError::InvalidType {
            location: "$".to_string(),
            expected: "valid JSON".to_string(),
            found: e.to_string(),
        })?;
        Self::parse_value(&value, "$")
    }

    /// Parse a serde_json::Value into an ScVal recursively
    pub fn parse_value(value: &Value, path: &str) -> Result<ScVal, ParserError> {
        match value {
            Value::Null => Ok(ScVal::Void),
            Value::Bool(b) => Ok(ScVal::Bool(*b)),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(ScVal::I64(i))
                } else if let Some(u) = n.as_u64() {
                    Ok(ScVal::U64(u))
                } else {
                    Err(ParserError::InvalidType {
                        location: path.to_string(),
                        expected: "integer".to_string(),
                        found: format!("number {}", n),
                    })
                }
            }
            Value::String(s) => {
                // Address detection
                if (s.starts_with('G') || s.starts_with('C')) && s.len() == 56 {
                    if let Ok(addr) = Self::parse_address(s) {
                        return Ok(ScVal::Address(addr));
                    }
                }

                // Symbol detection (prefixed with :)
                if let Some(sym_str) = s.strip_prefix(':') {
                    let sym: ScSymbol =
                        sym_str.try_into().map_err(|_| ParserError::InvalidSymbol {
                            location: path.to_string(),
                            details: "Symbol must be 1-32 characters".to_string(),
                        })?;
                    return Ok(ScVal::Symbol(sym));
                }

                // Hex bytes detection (prefixed with 0x)
                if let Some(hex_str) = s.strip_prefix("0x") {
                    let bytes = hex::decode(hex_str).map_err(|e| ParserError::InvalidHex {
                        location: path.to_string(),
                        details: e.to_string(),
                    })?;
                    return Ok(ScVal::Bytes(bytes.try_into().map_err(|_| {
                        ParserError::InvalidHex {
                            location: path.to_string(),
                            details: "Bytes exceed maximum allowed size".to_string(),
                        }
                    })?));
                }

                // Default: Treat as String
                let string_m: StringM =
                    s.as_bytes()
                        .to_vec()
                        .try_into()
                        .map_err(|_| ParserError::InvalidType {
                            location: path.to_string(),
                            expected: "shorter string".to_string(),
                            found: "string length exceeds limit".to_string(),
                        })?;
                Ok(ScVal::String(ScString(string_m)))
            }
            Value::Array(arr) => {
                let mut vec = Vec::new();
                for (i, v) in arr.iter().enumerate() {
                    vec.push(Self::parse_value(v, &format!("{}[{}]", path, i))?);
                }
                let vec_m: VecM<ScVal> = vec.try_into().map_err(|_| ParserError::InvalidType {
                    location: path.to_string(),
                    expected: "shorter vector".to_string(),
                    found: "vector size exceeds limit".to_string(),
                })?;
                Ok(ScVal::Vec(Some(ScVec(vec_m))))
            }
            Value::Object(obj) => {
                let mut entries = Vec::new();
                for (k, v) in obj {
                    let key_sym: ScSymbol =
                        k.as_str()
                            .try_into()
                            .map_err(|_| ParserError::InvalidSymbol {
                                location: format!("{}.{}", path, k),
                                details: "Key name too long for symbol".to_string(),
                            })?;
                    let key = ScVal::Symbol(key_sym);
                    let val = Self::parse_value(v, &format!("{}.{}", path, k))?;
                    entries.push(ScMapEntry { key, val });
                }

                entries.sort_by(|a, b| {
                    let a_bytes = a.key.to_xdr(Limits::none()).unwrap_or_default();
                    let b_bytes = b.key.to_xdr(Limits::none()).unwrap_or_default();
                    a_bytes.cmp(&b_bytes)
                });

                let map_m: VecM<ScMapEntry> =
                    entries.try_into().map_err(|_| ParserError::InvalidType {
                        location: path.to_string(),
                        expected: "smaller map".to_string(),
                        found: "map size exceeds limit".to_string(),
                    })?;
                Ok(ScVal::Map(Some(ScMap(map_m))))
            }
        }
    }

    fn parse_address(address: &str) -> Result<ScAddress, String> {
        let strkey = Strkey::from_string(address).map_err(|e| e.to_string())?;

        match strkey {
            Strkey::Contract(contract) => Ok(ScAddress::Contract(Hash(contract.0))),
            Strkey::PublicKeyEd25519(pubkey) => {
                Ok(ScAddress::Account(soroban_sdk::xdr::AccountId(
                    soroban_sdk::xdr::PublicKey::PublicKeyTypeEd25519(Uint256(pubkey.0)),
                )))
            }
            _ => Err("Unsupported address type".to_string()),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Contract Bytecode Security Scanner (Issue #36)
// ─────────────────────────────────────────────────────────────────────────────

/// How serious a scanner finding is.
///
/// Only [`Severity::Critical`] findings cause [`SecurityReport::verified`] to
/// be `false`; everything else is advisory so operators can triage a contract
/// without being blocked from deploying it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Informational only — no action required.
    Info,
    /// Suspicious but legal; worth a human look.
    Warning,
    /// Verification fails: the module is malformed or unsafe to execute.
    Critical,
}

/// Machine-readable classification for a [`SecurityFinding`].
///
/// Kept separate from the human-readable `detail` string so callers can filter
/// or alert on a stable identifier rather than matching on prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    /// The binary is not a valid WebAssembly module.
    MalformedBinary,
    /// A linear memory declares no maximum, so it can grow unbounded.
    UnboundedMemoryGrowth,
    /// A declared memory maximum exceeds the configured page budget.
    ExcessiveMemoryLimit,
    /// `memory.grow` is reachable in a function body.
    DynamicMemoryGrowth,
    /// Floating-point instructions are present (non-deterministic across hosts).
    FloatingPointOperation,
    /// The module exports no functions at all.
    MissingExports,
    /// An exported name does not look like a Soroban contract entry point.
    NonStandardEntryPoint,
    /// A `start` section runs code at instantiation time.
    StartSectionPresent,
    /// The module imports host functions outside the Soroban environment.
    NonStandardImport,
    /// The module declares a mutable global, i.e. cross-invocation state.
    MutableGlobal,
}

/// A single issue discovered while scanning a WASM binary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityFinding {
    /// Stable classification of the issue.
    pub kind: FindingKind,
    /// How serious the issue is.
    pub severity: Severity,
    /// Human-readable explanation, including offending names/values.
    pub detail: String,
}

/// Result of scanning a contract's WASM bytecode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityReport {
    /// `false` when at least one [`Severity::Critical`] finding was recorded.
    pub verified: bool,
    /// Every finding, ordered most severe first.
    pub findings: Vec<SecurityFinding>,
    /// Names of every exported function, in declaration order.
    pub exported_functions: Vec<String>,
    /// Total number of linear memories declared or imported.
    pub memory_count: usize,
}

impl SecurityReport {
    /// Highest severity across all findings, or `None` for a clean report.
    pub fn max_severity(&self) -> Option<Severity> {
        self.findings.iter().map(|f| f.severity).max()
    }

    /// Every finding matching `kind`.
    pub fn findings_of(&self, kind: FindingKind) -> impl Iterator<Item = &SecurityFinding> {
        self.findings.iter().filter(move |f| f.kind == kind)
    }
}

/// Tunable thresholds for [`WasmSecurityScanner`].
#[derive(Debug, Clone)]
pub struct ScannerConfig {
    /// Largest acceptable declared memory maximum, in 64 KiB WASM pages.
    ///
    /// Soroban contracts are expected to stay well inside this; the default of
    /// 512 pages (32 MiB) is generous while still catching absurd requests.
    pub max_memory_pages: u64,
    /// Treat floating-point instructions as a finding.
    ///
    /// Float results can differ across hosts, so deterministic execution
    /// environments generally want this on.
    pub flag_floating_point: bool,
    /// Import module names considered standard for Soroban contracts.
    pub allowed_import_modules: Vec<String>,
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            max_memory_pages: 512,
            flag_floating_point: true,
            // Soroban host functions are grouped into single-letter modules
            // ("x" for context, "b" for buf, "m" for map, and so on); "env"
            // is the catch-all used by the SDK's generated glue.
            allowed_import_modules: [
                "env", "x", "b", "m", "v", "i", "c", "a", "l", "d", "t",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        }
    }
}

/// Static safety checker for Soroban contract WASM binaries.
///
/// The scanner is deliberately conservative: it performs a structural pass over
/// the module (validating it first, then walking sections and function bodies)
/// and reports anything that would make execution non-deterministic, unbounded,
/// or otherwise surprising. It never executes the module.
///
/// # Example
///
/// ```
/// use soroscope_core::parser::WasmSecurityScanner;
///
/// let wasm_bytes: &[u8] = &[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
/// let report = WasmSecurityScanner::new().scan(wasm_bytes);
/// if !report.verified {
///     eprintln!("rejected: {:?}", report.findings);
/// }
/// ```
pub struct WasmSecurityScanner {
    config: ScannerConfig,
}

impl Default for WasmSecurityScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmSecurityScanner {
    /// Scanner with [`ScannerConfig::default`] thresholds.
    pub fn new() -> Self {
        Self {
            config: ScannerConfig::default(),
        }
    }

    /// Scanner with caller-supplied thresholds.
    pub fn with_config(config: ScannerConfig) -> Self {
        Self { config }
    }

    /// Scan `wasm_bytes` and return a [`SecurityReport`].
    ///
    /// A malformed binary short-circuits: validation failure is reported as a
    /// single [`FindingKind::MalformedBinary`] critical finding, because no
    /// further structural claim about the module would be trustworthy.
    pub fn scan(&self, wasm_bytes: &[u8]) -> SecurityReport {
        // Validate first. Everything downstream assumes a well-formed module,
        // and wasmparser's streaming reader would otherwise happily report
        // sections from a truncated or corrupt binary.
        let mut validator = wasmparser::Validator::new();
        if let Err(e) = validator.validate_all(wasm_bytes) {
            return SecurityReport {
                verified: false,
                findings: vec![SecurityFinding {
                    kind: FindingKind::MalformedBinary,
                    severity: Severity::Critical,
                    detail: format!("WASM validation failed: {e}"),
                }],
                exported_functions: Vec::new(),
                memory_count: 0,
            };
        }

        let mut findings: Vec<SecurityFinding> = Vec::new();
        let mut exported_functions: Vec<String> = Vec::new();
        let mut memory_count = 0usize;
        // Float and memory.grow findings are aggregated so a contract with a
        // thousand float ops yields one finding, not a thousand.
        let mut float_ops = 0usize;
        let mut memory_grow_sites = 0usize;

        for payload in wasmparser::Parser::new(0).parse_all(wasm_bytes) {
            // Validation already succeeded, so a parse error here would be a
            // wasmparser inconsistency rather than bad input; treat it as
            // malformed anyway rather than silently continuing.
            let payload = match payload {
                Ok(p) => p,
                Err(e) => {
                    findings.push(SecurityFinding {
                        kind: FindingKind::MalformedBinary,
                        severity: Severity::Critical,
                        detail: format!("WASM parse error after validation: {e}"),
                    });
                    break;
                }
            };

            match payload {
                wasmparser::Payload::MemorySection(reader) => {
                    for mem in reader.into_iter().flatten() {
                        memory_count += 1;
                        self.check_memory(&mem, &mut findings);
                    }
                }
                wasmparser::Payload::ImportSection(reader) => {
                    for import in reader.into_iter().flatten() {
                        if let wasmparser::TypeRef::Memory(mem) = import.ty {
                            memory_count += 1;
                            self.check_memory(&mem, &mut findings);
                        }
                        if !self
                            .config
                            .allowed_import_modules
                            .iter()
                            .any(|m| m == import.module)
                        {
                            findings.push(SecurityFinding {
                                kind: FindingKind::NonStandardImport,
                                severity: Severity::Warning,
                                detail: format!(
                                    "Import `{}::{}` is outside the standard Soroban host modules",
                                    import.module, import.name
                                ),
                            });
                        }
                    }
                }
                wasmparser::Payload::ExportSection(reader) => {
                    for export in reader.into_iter().flatten() {
                        if export.kind == wasmparser::ExternalKind::Func {
                            exported_functions.push(export.name.to_string());
                        }
                    }
                }
                wasmparser::Payload::GlobalSection(reader) => {
                    for global in reader.into_iter().flatten() {
                        if global.ty.mutable {
                            findings.push(SecurityFinding {
                                kind: FindingKind::MutableGlobal,
                                severity: Severity::Info,
                                detail:
                                    "Module declares a mutable global; state is not persisted \
                                     across invocations and may indicate a porting mistake"
                                        .to_string(),
                            });
                        }
                    }
                }
                wasmparser::Payload::StartSection { func, .. } => {
                    findings.push(SecurityFinding {
                        kind: FindingKind::StartSectionPresent,
                        severity: Severity::Warning,
                        detail: format!(
                            "Module declares a start section (function {func}) that runs \
                             automatically at instantiation"
                        ),
                    });
                }
                wasmparser::Payload::CodeSectionEntry(body) => {
                    self.scan_body(&body, &mut float_ops, &mut memory_grow_sites);
                }
                _ => {}
            }
        }

        if self.config.flag_floating_point && float_ops > 0 {
            findings.push(SecurityFinding {
                kind: FindingKind::FloatingPointOperation,
                severity: Severity::Critical,
                detail: format!(
                    "Module contains {float_ops} floating-point instruction(s); float results \
                     are not guaranteed deterministic across hosts and Soroban rejects them"
                ),
            });
        }

        if memory_grow_sites > 0 {
            findings.push(SecurityFinding {
                kind: FindingKind::DynamicMemoryGrowth,
                severity: Severity::Warning,
                detail: format!(
                    "Module calls `memory.grow` at {memory_grow_sites} site(s); memory use \
                     is input-dependent and may exhaust the host budget"
                ),
            });
        }

        self.check_exports(&exported_functions, &mut findings);

        // Most severe first so callers can render or truncate by priority.
        findings.sort_by(|a, b| b.severity.cmp(&a.severity));

        SecurityReport {
            verified: !findings.iter().any(|f| f.severity == Severity::Critical),
            findings,
            exported_functions,
            memory_count,
        }
    }

    /// Flag a memory whose maximum is absent or larger than the budget.
    fn check_memory(&self, mem: &wasmparser::MemoryType, findings: &mut Vec<SecurityFinding>) {
        match mem.maximum {
            None => findings.push(SecurityFinding {
                kind: FindingKind::UnboundedMemoryGrowth,
                severity: Severity::Critical,
                detail: format!(
                    "Linear memory declares no maximum (initial {} pages); it can grow \
                     until the host budget is exhausted",
                    mem.initial
                ),
            }),
            Some(max) if max > self.config.max_memory_pages => {
                findings.push(SecurityFinding {
                    kind: FindingKind::ExcessiveMemoryLimit,
                    severity: Severity::Warning,
                    detail: format!(
                        "Linear memory maximum of {} pages exceeds the {}-page budget",
                        max, self.config.max_memory_pages
                    ),
                });
            }
            Some(_) => {}
        }
    }

    /// Count float instructions and `memory.grow` sites in one function body.
    ///
    /// Errors while reading the body are ignored: validation has already
    /// passed, so a failure here cannot indicate malformed input, and skipping
    /// a body only makes the scan more permissive rather than unsound.
    fn scan_body(
        &self,
        body: &wasmparser::FunctionBody<'_>,
        float_ops: &mut usize,
        memory_grow_sites: &mut usize,
    ) {
        let reader = match body.get_operators_reader() {
            Ok(r) => r,
            Err(_) => return,
        };

        for op in reader.into_iter().flatten() {
            if matches!(op, wasmparser::Operator::MemoryGrow { .. }) {
                *memory_grow_sites += 1;
            }
            if self.config.flag_floating_point && Self::is_float_op(&op) {
                *float_ops += 1;
            }
        }
    }

    /// Whether `op` is a floating-point instruction.
    ///
    /// Matching on the debug name keeps this robust across wasmparser releases
    /// that add new float opcodes; every float instruction in the spec is
    /// prefixed `F32`/`F64` (or `F32x4`/`F64x2` for SIMD).
    fn is_float_op(op: &wasmparser::Operator<'_>) -> bool {
        let name = format!("{op:?}");
        name.starts_with("F32") || name.starts_with("F64")
    }

    /// Flag missing exports and names that are not valid Soroban entry points.
    fn check_exports(&self, exports: &[String], findings: &mut Vec<SecurityFinding>) {
        if exports.is_empty() {
            findings.push(SecurityFinding {
                kind: FindingKind::MissingExports,
                severity: Severity::Critical,
                detail: "Module exports no functions; it has no callable contract entry point"
                    .to_string(),
            });
            return;
        }

        for name in exports {
            if !Self::is_standard_entry_point(name) {
                findings.push(SecurityFinding {
                    kind: FindingKind::NonStandardEntryPoint,
                    severity: Severity::Warning,
                    detail: format!(
                        "Exported function `{name}` is not a valid Soroban entry point name \
                         (expected a 1-32 character symbol of [a-zA-Z0-9_] not starting with \
                         a digit)"
                    ),
                });
            }
        }
    }

    /// Whether `name` is a legal Soroban contract function symbol.
    ///
    /// Soroban identifiers are `ScSymbol`s: 1–32 characters drawn from
    /// `[a-zA-Z0-9_]` and never leading with a digit. Reserved underscore-
    /// prefixed names emitted by the toolchain (`_start`, `__data_end`) are
    /// allowed through so ordinary SDK output does not trip the check.
    fn is_standard_entry_point(name: &str) -> bool {
        if name.starts_with('_') {
            return true;
        }
        if name.is_empty() || name.len() > 32 {
            return false;
        }
        if name.starts_with(|c: char| c.is_ascii_digit()) {
            return false;
        }
        name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::xdr::ScVal;

    #[test]
    fn test_parse_primitives() {
        assert!(matches!(ArgParser::parse("null").unwrap(), ScVal::Void));
        assert!(matches!(
            ArgParser::parse("true").unwrap(),
            ScVal::Bool(true)
        ));
        assert!(matches!(
            ArgParser::parse("false").unwrap(),
            ScVal::Bool(false)
        ));
        assert!(matches!(ArgParser::parse("123").unwrap(), ScVal::I64(123)));
        assert!(matches!(
            ArgParser::parse("-456").unwrap(),
            ScVal::I64(-456)
        ));
    }

    #[test]
    fn test_parse_string_and_symbol() {
        let s = ArgParser::parse("\"hello\"").unwrap();
        match s {
            ScVal::String(st) => {
                let bytes: Vec<u8> = st.0.into();
                assert_eq!(String::from_utf8(bytes).unwrap(), "hello");
            }
            _ => panic!("Expected String variant"),
        }

        let sym = ArgParser::parse("\":my_sym\"").unwrap();
        match sym {
            ScVal::Symbol(s) => {
                let bytes: Vec<u8> = s.0.into();
                assert_eq!(String::from_utf8(bytes).unwrap(), "my_sym");
            }
            _ => panic!("Expected Symbol variant"),
        }
    }

    #[test]
    fn test_parse_address() {
        // Valid strkeys from snapshots
        let account = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAGO6V";
        let contract = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM";

        let result = ArgParser::parse(&format!("\"{}\"", account)).unwrap();
        assert!(matches!(result, ScVal::Address(ScAddress::Account(_))));

        let result = ArgParser::parse(&format!("\"{}\"", contract)).unwrap();
        assert!(matches!(result, ScVal::Address(ScAddress::Contract(_))));
    }

    #[test]
    fn test_parse_complex_nested() {
        let json = r#"{
            "admin": "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAGO6V",
            "config": {
                "threshold": 3,
                "active": true
            },
            "tags": [":tag1", ":tag2"]
        }"#;

        let result = ArgParser::parse(json).unwrap();
        if let ScVal::Map(Some(map)) = result {
            assert_eq!(map.0.len(), 3);
        } else {
            panic!("Expected Map");
        }
    }

    #[test]
    fn test_error_path() {
        let json = r#"{"a": {"b": [1, 1.5]}}"#;
        let result = ArgParser::parse(json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("$.a.b[1]"));
        assert!(err.contains("expected integer, found number 1.5"));
    }

    // ─────────────────────────────────────────────────────────────────────
    // Security scanner (Issue #36)
    // ─────────────────────────────────────────────────────────────────────

    /// Assemble a minimal module from a WAT-like byte builder.
    ///
    /// Hand-rolling the binary keeps these tests free of a WAT dependency
    /// while still exercising real section encoding.
    fn wasm_module(sections: &[(u8, Vec<u8>)]) -> Vec<u8> {
        let mut out = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        for (id, body) in sections {
            out.push(*id);
            leb128(body.len() as u64, &mut out);
            out.extend_from_slice(body);
        }
        out
    }

    fn leb128(mut v: u64, out: &mut Vec<u8>) {
        loop {
            let mut byte = (v & 0x7f) as u8;
            v >>= 7;
            if v != 0 {
                byte |= 0x80;
            }
            out.push(byte);
            if v == 0 {
                break;
            }
        }
    }

    /// Type section with a single `() -> ()` function type.
    fn type_section() -> (u8, Vec<u8>) {
        (1, vec![0x01, 0x60, 0x00, 0x00])
    }

    /// Function section declaring one function of type 0.
    fn func_section() -> (u8, Vec<u8>) {
        (3, vec![0x01, 0x00])
    }

    /// Export section exporting function 0 under `name`.
    fn export_section(name: &str) -> (u8, Vec<u8>) {
        let mut body = vec![0x01];
        body.push(name.len() as u8);
        body.extend_from_slice(name.as_bytes());
        body.push(0x00); // kind: func
        body.push(0x00); // index 0
        (7, body)
    }

    /// Code section with one body containing `ops` then `end`.
    fn code_section(ops: &[u8]) -> (u8, Vec<u8>) {
        let mut func = vec![0x00]; // zero local declarations
        func.extend_from_slice(ops);
        func.push(0x0b); // end
        let mut body = vec![0x01];
        leb128(func.len() as u64, &mut body);
        body.extend_from_slice(&func);
        (10, body)
    }

    /// Memory section: `limits` flag 0x00 = min only, 0x01 = min+max.
    fn memory_section(min: u64, max: Option<u64>) -> (u8, Vec<u8>) {
        let mut body = vec![0x01];
        match max {
            None => {
                body.push(0x00);
                leb128(min, &mut body);
            }
            Some(m) => {
                body.push(0x01);
                leb128(min, &mut body);
                leb128(m, &mut body);
            }
        }
        (5, body)
    }

    /// A well-formed contract: bounded memory, integer-only, one good export.
    fn clean_module() -> Vec<u8> {
        wasm_module(&[
            type_section(),
            func_section(),
            memory_section(1, Some(16)),
            export_section("transfer"),
            code_section(&[]),
        ])
    }

    #[test]
    fn scanner_accepts_clean_module() {
        let report = WasmSecurityScanner::new().scan(&clean_module());
        assert!(
            report.verified,
            "clean module should verify, findings: {:?}",
            report.findings
        );
        assert_eq!(report.exported_functions, vec!["transfer".to_string()]);
        assert_eq!(report.memory_count, 1);
    }

    #[test]
    fn scanner_rejects_malformed_binary() {
        // Valid magic, truncated body.
        let bad = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0xff];
        let report = WasmSecurityScanner::new().scan(&bad);
        assert!(!report.verified);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].kind, FindingKind::MalformedBinary);
        assert_eq!(report.findings[0].severity, Severity::Critical);
    }

    #[test]
    fn scanner_rejects_non_wasm_bytes() {
        let report = WasmSecurityScanner::new().scan(b"this is not wasm at all");
        assert!(!report.verified);
        assert_eq!(report.findings[0].kind, FindingKind::MalformedBinary);
    }

    #[test]
    fn scanner_rejects_empty_input() {
        let report = WasmSecurityScanner::new().scan(&[]);
        assert!(!report.verified);
        assert_eq!(report.findings[0].kind, FindingKind::MalformedBinary);
    }

    #[test]
    fn scanner_flags_unbounded_memory() {
        let wasm = wasm_module(&[
            type_section(),
            func_section(),
            memory_section(1, None),
            export_section("go"),
            code_section(&[]),
        ]);
        let report = WasmSecurityScanner::new().scan(&wasm);
        assert!(!report.verified, "unbounded memory must fail verification");
        assert_eq!(
            report
                .findings_of(FindingKind::UnboundedMemoryGrowth)
                .count(),
            1
        );
    }

    #[test]
    fn scanner_warns_on_excessive_memory_maximum() {
        let wasm = wasm_module(&[
            type_section(),
            func_section(),
            memory_section(1, Some(4096)),
            export_section("go"),
            code_section(&[]),
        ]);
        let report = WasmSecurityScanner::new().scan(&wasm);
        // Warning only: the module is still bounded, so it verifies.
        assert!(report.verified);
        assert_eq!(
            report.findings_of(FindingKind::ExcessiveMemoryLimit).count(),
            1
        );
    }

    #[test]
    fn scanner_flags_floating_point_operations() {
        // f64.const 1.0; drop
        let mut ops = vec![0x44];
        ops.extend_from_slice(&1.0f64.to_le_bytes());
        ops.push(0x1a); // drop
        let wasm = wasm_module(&[
            type_section(),
            func_section(),
            memory_section(1, Some(16)),
            export_section("calc"),
            code_section(&ops),
        ]);
        let report = WasmSecurityScanner::new().scan(&wasm);
        assert!(!report.verified, "float ops must fail verification");
        assert_eq!(
            report
                .findings_of(FindingKind::FloatingPointOperation)
                .count(),
            1
        );
    }

    #[test]
    fn scanner_allows_floats_when_disabled() {
        let mut ops = vec![0x44];
        ops.extend_from_slice(&1.0f64.to_le_bytes());
        ops.push(0x1a);
        let wasm = wasm_module(&[
            type_section(),
            func_section(),
            memory_section(1, Some(16)),
            export_section("calc"),
            code_section(&ops),
        ]);
        let config = ScannerConfig {
            flag_floating_point: false,
            ..ScannerConfig::default()
        };
        let report = WasmSecurityScanner::with_config(config).scan(&wasm);
        assert!(report.verified);
        assert_eq!(
            report
                .findings_of(FindingKind::FloatingPointOperation)
                .count(),
            0
        );
    }

    #[test]
    fn scanner_warns_on_dynamic_memory_growth() {
        // i32.const 1; memory.grow 0; drop
        let ops = vec![0x41, 0x01, 0x40, 0x00, 0x1a];
        let wasm = wasm_module(&[
            type_section(),
            func_section(),
            memory_section(1, Some(16)),
            export_section("grow"),
            code_section(&ops),
        ]);
        let report = WasmSecurityScanner::new().scan(&wasm);
        // memory.grow alone is a warning, not a hard failure.
        assert!(report.verified);
        assert_eq!(
            report.findings_of(FindingKind::DynamicMemoryGrowth).count(),
            1
        );
    }

    #[test]
    fn scanner_flags_missing_exports() {
        let wasm = wasm_module(&[
            type_section(),
            func_section(),
            memory_section(1, Some(16)),
            code_section(&[]),
        ]);
        let report = WasmSecurityScanner::new().scan(&wasm);
        assert!(!report.verified, "a module with no exports must fail");
        assert_eq!(report.findings_of(FindingKind::MissingExports).count(), 1);
    }

    #[test]
    fn scanner_warns_on_non_standard_entry_point() {
        let wasm = wasm_module(&[
            type_section(),
            func_section(),
            memory_section(1, Some(16)),
            export_section("bad-name!"),
            code_section(&[]),
        ]);
        let report = WasmSecurityScanner::new().scan(&wasm);
        assert!(report.verified, "a bad name is advisory, not fatal");
        assert_eq!(
            report
                .findings_of(FindingKind::NonStandardEntryPoint)
                .count(),
            1
        );
    }

    #[test]
    fn scanner_allows_toolchain_reserved_exports() {
        // Underscore-prefixed names come from the toolchain, not the contract
        // author, so they must not be reported as non-standard.
        assert!(WasmSecurityScanner::is_standard_entry_point("_start"));
        assert!(WasmSecurityScanner::is_standard_entry_point("__data_end"));
        assert!(WasmSecurityScanner::is_standard_entry_point("transfer"));
        assert!(WasmSecurityScanner::is_standard_entry_point("set_admin"));

        assert!(!WasmSecurityScanner::is_standard_entry_point("has space"));
        assert!(!WasmSecurityScanner::is_standard_entry_point("1leading"));
        assert!(!WasmSecurityScanner::is_standard_entry_point(""));
        assert!(!WasmSecurityScanner::is_standard_entry_point(
            &"x".repeat(33)
        ));
        assert!(WasmSecurityScanner::is_standard_entry_point(&"x".repeat(32)));
    }

    #[test]
    fn findings_are_sorted_most_severe_first() {
        // Unbounded memory (critical) + odd export name (warning).
        let wasm = wasm_module(&[
            type_section(),
            func_section(),
            memory_section(1, None),
            export_section("bad-name!"),
            code_section(&[]),
        ]);
        let report = WasmSecurityScanner::new().scan(&wasm);
        assert!(report.findings.len() >= 2);
        for pair in report.findings.windows(2) {
            assert!(
                pair[0].severity >= pair[1].severity,
                "findings must be ordered most severe first"
            );
        }
        assert_eq!(report.max_severity(), Some(Severity::Critical));
    }

    #[test]
    fn report_serialises_to_json() {
        let report = WasmSecurityScanner::new().scan(&clean_module());
        let json = serde_json::to_string(&report).expect("report should serialise");
        assert!(json.contains("\"verified\":true"));
    }
}
