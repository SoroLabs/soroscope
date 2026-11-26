//! Structured classification of contract execution failures (issue #1006).
//!
//! `From<HostError> for SimulationError` collapsed every host failure into
//! `ExecutionFailed(format!("{e:?}"))`. A CPU limit, a memory limit, a storage
//! error, an auth rejection and a contract trap all became the same variant
//! carrying the same `Contract execution failed: …` message. The original
//! comment argued the distinction carries no *retry* meaning, which is true —
//! `is_retriable()` is still false for every kind here — but it carries a great
//! deal of *debugging* meaning. "Contract execution failed" does not tell the
//! author whether to raise the instruction limit, shrink a `Vec`, fix an
//! authorisation check, or repair a panic.
//!
//! # Why classification reads the diagnostic text
//!
//! The obvious implementation is a `match` over `HostError`'s variants. This
//! module deliberately does not do that: the exact variant paths live in
//! `soroban-env-host`, they have been renamed between minor versions, and a
//! stale path is a compile error in a crate this workspace depends on but does
//! not own. The rendered `Debug` form is already what the previous code
//! surfaced to callers, so classifying on it changes no wire format and cannot
//! break the build when the host crate is bumped.
//!
//! The cost is that a host upgrade could reword a diagnostic and shift a
//! classification. That is why [`classify`] is deliberately conservative: it
//! only returns a specific kind on tokens that are unambiguous, and everything
//! else lands in [`FailureKind::Other`] rather than being guessed at.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// What kind of failure a local execution hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    /// The CPU / instruction budget was exhausted.
    CpuLimit,
    /// The memory (RAM) budget was exhausted.
    MemLimit,
    /// A storage, footprint, ledger-entry or rent failure.
    Storage,
    /// An authorisation or authentication rejection.
    Auth,
    /// The contract itself trapped: a panic, a `trap`, or an explicit
    /// `contracterror` return.
    ContractTrap,
    /// Anything the classifier could not attribute with confidence.
    Other,
}

impl FailureKind {
    /// The resource whose budget this failure is about, when there is one.
    ///
    /// This is the "cost type that crossed the limit" that gets attached to a
    /// trap context (#1006). `Other` and `Auth` are not budget failures, so
    /// they report `None`.
    pub fn cost_type(&self) -> Option<&'static str> {
        match self {
            FailureKind::CpuLimit => Some("cpu"),
            FailureKind::MemLimit => Some("memory"),
            FailureKind::Storage => Some("storage"),
            FailureKind::Auth | FailureKind::ContractTrap | FailureKind::Other => None,
        }
    }

    /// Stable lowercase identifier, matching the serde representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            FailureKind::CpuLimit => "cpu_limit",
            FailureKind::MemLimit => "mem_limit",
            FailureKind::Storage => "storage",
            FailureKind::Auth => "auth",
            FailureKind::ContractTrap => "contract_trap",
            FailureKind::Other => "other",
        }
    }
}

/// A numeric `contracterror` discriminant, resolved to its name when known.
///
/// Soroban surfaces a contract error as a `u32` in the diagnostic; on its own
/// that is not actionable. `contracts/error_codes` is the canonical table, and
/// this mirrors it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ContractErrorCode {
    pub code: u32,
    /// The `ContractError` variant name, or `None` for a code this table does
    /// not define (a different contract's error space, or a future variant).
    pub name: Option<String>,
}

impl ContractErrorCode {
    /// Resolve a `u32` discriminant against `contracts/error_codes`.
    pub fn from_code(code: u32) -> Self {
        ContractErrorCode { code, name: contract_error_name(code).map(String::from) }
    }
}

/// Mirrors `contracts/error_codes/src/lib.rs` `ContractError` discriminants.
fn contract_error_name(code: u32) -> Option<&'static str> {
    Some(match code {
        1 => "AlreadyInitialized",
        2 => "NotInitialized",
        3 => "Unauthorized",
        4 => "InsufficientBalance",
        5 => "InsufficientLiquidity",
        6 => "InsufficientShares",
        7 => "InsufficientAllowance",
        8 => "SlippageExceeded",
        9 => "InvalidFee",
        10 => "NoPendingFeeUpdate",
        11 => "TimelockNotElapsed",
        12 => "OracleNotConfigured",
        13 => "InvalidOraclePrice",
        14 => "Paused",
        15 => "Overflow",
        16 => "DivisionByZero",
        17 => "InvalidInput",
        18 => "InvalidAmount",
        _ => return None,
    })
}

/// Where a trap happened, when the host's diagnostic carried a stack.
///
/// Every field is optional because a diagnostic only includes a stack for some
/// failure modes. A missing context is normal, not an error.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TrapContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<String>,
    /// `cpu`, `memory`, `storage` — derived from the failure kind when the
    /// caller does not state it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_type: Option<String>,
}

/// A classified execution failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ExecutionFailure {
    pub kind: FailureKind,
    /// The original diagnostic text, preserved verbatim so nothing is lost when
    /// classification is wrong.
    pub detail: String,
    /// Present when the trap payload was a `contracterror` discriminant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract_error: Option<ContractErrorCode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<TrapContext>,
}

impl ExecutionFailure {
    /// Classify a host diagnostic and capture any contract error code in it.
    pub fn from_diagnostic(detail: impl Into<String>) -> Self {
        let detail = detail.into();
        let contract_error = extract_contract_error_code(&detail);
        let kind = classify(&detail, contract_error.is_some());
        ExecutionFailure { kind, detail, contract_error, context: None }
    }

    /// Attach a trap location. The cost type is filled from the failure kind
    /// when the caller does not state one.
    pub fn with_context(mut self, context: TrapContext) -> Self {
        let mut context = context;
        if context.cost_type.is_none() {
            context.cost_type = self.kind.cost_type().map(|c| c.to_string());
        }
        self.context = Some(context);
        self
    }

    /// Attach contract id, function and cost type in one call.
    pub fn at(self, contract_id: Option<String>, function: Option<String>) -> Self {
        self.with_context(TrapContext { contract_id, function, cost_type: None })
    }

    /// Attach the invocation this failure came from.
    ///
    /// The classifier knows *what* went wrong; only the caller knows *where*.
    /// `From<HostError>` has the diagnostic and nothing else, so every failure
    /// it produced had `context: None` and the report could not say which
    /// function to look at. Callers that hold a
    /// [`ContractInvocation`](crate::runner::ContractInvocation) hand the
    /// contract and function name in here.
    pub fn with_invocation(
        mut self,
        contract_id: Option<String>,
        function: Option<&str>,
    ) -> Self {
        // A name in the diagnostic is more precise than the name the caller
        // asked for: a host trap inside a helper reports the helper.
        let function = function_from_diagnostic(&self.detail).or_else(|| function.map(str::to_string));
        let contract_id = contract_id_from_diagnostic(&self.detail).or(contract_id);
        self.with_context(TrapContext { contract_id, function, cost_type: None })
    }

    /// The contract id, function and cost type, for callers that only want to
    /// know whether a location was recovered.
    pub fn context(&self) -> Option<&TrapContext> {
        self.context.as_ref()
    }

    /// Render the contract error with its resolved name when known, so the
    /// message says `contracterror #3 (Unauthorized)` rather than `#3`.
    pub fn describe(&self) -> String {
        match &self.contract_error {
            Some(ContractErrorCode { code, name: Some(name) }) => {
                format!("{} [{}]: contracterror #{} ({})", self.detail, self.kind.as_str(), code, name)
            }
            Some(ContractErrorCode { code, name: None }) => {
                format!("{} [{}]: contracterror #{}", self.detail, self.kind.as_str(), code)
            }
            None => format!("{} [{}]", self.detail, self.kind.as_str()),
        }
    }
}

/// Find a `contracterror` discriminant in a host diagnostic.
///
/// Soroban renders these as `Contract(... #12 …)` in the debug form, and some
/// paths use `Error(Contract, #12)`. Both are matched by scanning for `#`
/// followed by digits, bounded to a plausible `u32`.
fn extract_contract_error_code(detail: &str) -> Option<ContractErrorCode> {
    let bytes = detail.as_bytes();
    let mut idx = 0usize;
    while idx < bytes.len() {
        if bytes[idx] != b'#' {
            idx += 1;
            continue;
        }
        let start = idx + 1;
        let mut end = start;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        if end > start {
            // Guard against absurd digit runs: a real discriminant is small.
            if end - start <= 10 {
                if let Ok(code) = detail[start..end].parse::<u32>() {
                    return Some(ContractErrorCode::from_code(code));
                }
            }
        }
        idx = end.max(start);
    }
    None
}

/// Recover the contract function name from a host diagnostic.
///
/// Soroban renders a trap with the frames it has, in shapes like
/// `HostError: Error(Contract, #3,VmError(...,transfer,...))` or a
/// backtrace mentioning `contract::transfer`. Both a `fn <name>`-style token
/// and a `Contract(...).<name>(` frame are recognised; anything else yields
/// `None` rather than a guess, because a wrong function name sends an author
/// to the wrong place.
pub fn function_from_diagnostic(detail: &str) -> Option<String> {
    // A named frame: `Contract(<id>).<name>(` or `contract::<name>`.
    for (open, close) in [('(', ')'), ('[', ']')] {
        let mut rest = detail;
        while let Some(start) = rest.find(open) {
            let after = &rest[start + 1..];
            if let Some(end) = after.find(close) {
                let frame = &after[..end];
                if let Some(name) = frame.rsplit('.').next() {
                    let name = name.split(&[',', ':'][..]).next().unwrap_or(name).trim();
                    if is_wasm_function_name(name) {
                        return Some(name.to_string());
                    }
                }
            }
            rest = &rest[start + 1..];
        }
    }
    None
}

/// Recover a `C…` contract id (strkey) from a host diagnostic.
///
/// Only the canonical 56-character contract-id form is accepted, so a hex
/// fragment or a bare number cannot be mistaken for one.
pub fn contract_id_from_diagnostic(detail: &str) -> Option<String> {
    detail
        .split(|c: char| !c.is_ascii_uppercase() && !c.is_ascii_digit())
        .filter(|token| token.len() == 56 && token.starts_with('C'))
        .find(|token| token.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()))
        .map(str::to_string)
}

/// Whether a token looks like a WASM export name rather than prose.
fn is_wasm_function_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
        && name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_' || c == '$')
}

/// Classify a host diagnostic.
///
/// `has_contract_error` is passed in rather than recomputed so the caller can
/// tell "there is a `#N` in the text" apart from "the trap was an explicit
/// contract error": a `#` in an unrelated part of a diagnostic should not
/// promote the failure to a contract trap.
pub fn classify(detail: &str, has_contract_error: bool) -> FailureKind {
    let lower = detail.to_ascii_lowercase();

    // Budget failures first: an over-budget diagnostic usually also mentions
    // the contract and the trap, so checking trap first would misattribute it.
    if contains_any(&lower, &["cpu", "instrbudget", "instruction budget", "instructions limit"]) {
        return FailureKind::CpuLimit;
    }
    if contains_any(&lower, &["memory", "memlimit", "memory limit", "alloc"]) {
        return FailureKind::MemLimit;
    }
    if contains_any(&lower, &["storage", "footprint", "ledger entry", "insufficient rent", "ttl"]) {
        return FailureKind::Storage;
    }
    if contains_any(&lower, &["auth", "unauthorized", "forbidden", "not authorized"]) {
        return FailureKind::Auth;
    }
    if has_contract_error {
        return FailureKind::ContractTrap;
    }
    if contains_any(&lower, &["contract", "trap", "panic", "unreachable", "invalid wasm"]) {
        return FailureKind::ContractTrap;
    }
    FailureKind::Other
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    // The host's rendered forms for the two budget cases. Held as fixtures
    // rather than constructed `HostError` values so the classifier can be
    // exercised without the host crate's constructors.
    const CPU_DIAGNOSTIC: &str =
        "HostError: Error(Limits, Error(Contract, #0), Some(Diagnostic { cpu instructions exceeded budget: 1000000000 }))";
    const MEM_DIAGNOSTIC: &str =
        "HostError: Error(Limits, Error(Contract, #0), Some(Diagnostic { memory limit exceeded: 67108864 bytes }))";

    #[test]
    fn cpu_and_memory_limits_are_different_kinds() {
        let cpu = ExecutionFailure::from_diagnostic(CPU_DIAGNOSTIC);
        let mem = ExecutionFailure::from_diagnostic(MEM_DIAGNOSTIC);

        assert_eq!(cpu.kind, FailureKind::CpuLimit);
        assert_eq!(mem.kind, FailureKind::MemLimit);
        assert_ne!(cpu.kind, mem.kind);
    }

    #[test]
    fn cost_type_reflects_the_budget_that_was_crossed() {
        assert_eq!(ExecutionFailure::from_diagnostic(CPU_DIAGNOSTIC).kind.cost_type(), Some("cpu"));
        assert_eq!(ExecutionFailure::from_diagnostic(MEM_DIAGNOSTIC).kind.cost_type(), Some("memory"));
        assert_eq!(FailureKind::Other.cost_type(), None);
    }

    #[test]
    fn a_contract_error_code_is_rendered_as_a_name_not_only_an_integer() {
        // #3 is `Unauthorized` in contracts/error_codes.
        let failure = ExecutionFailure::from_diagnostic(
            "HostError: Error(Contract, #3, Some(ContractError(3)))",
        );

        assert_eq!(failure.kind, FailureKind::ContractTrap);
        let code = failure.contract_error.as_ref().expect("a discriminant was present");
        assert_eq!(code.code, 3);
        assert_eq!(code.name.as_deref(), Some("Unauthorized"));
        assert!(failure.describe().contains("contracterror #3 (Unauthorized)"), "got {}", failure.describe());
    }

    #[test]
    fn every_known_error_code_resolves_to_its_name() {
        for code in 1u32..=18 {
            let resolved = ContractErrorCode::from_code(code);
            assert!(resolved.name.is_some(), "code {} should resolve", code);
        }
    }

    #[test]
    fn an_unknown_error_code_keeps_the_integer_and_has_no_name() {
        let failure = ExecutionFailure::from_diagnostic("HostError: Error(Contract, #9999)");
        let code = failure.contract_error.as_ref().expect("code present");
        assert_eq!(code.code, 9999);
        assert_eq!(code.name, None);
        assert!(failure.describe().contains("contracterror #9999"));
    }

    #[test]
    fn storage_auth_and_other_classify_independently() {
        assert_eq!(
            ExecutionFailure::from_diagnostic("Error(Storage, footprint exceeded)").kind,
            FailureKind::Storage
        );
        assert_eq!(
            ExecutionFailure::from_diagnostic("Error(Auth, not authorized)").kind,
            FailureKind::Auth
        );
        assert_eq!(ExecutionFailure::from_diagnostic("something unrecognised").kind, FailureKind::Other);
    }

    #[test]
    fn an_unattributable_failure_is_other_rather_than_a_guess() {
        // A limit word inside an unrelated word must not promote the kind.
        assert_eq!(classify("current block height changed", false), FailureKind::Other);
    }

    #[test]
    fn trap_context_carries_contract_function_and_cost_type() {
        let failure = ExecutionFailure::from_diagnostic(CPU_DIAGNOSTIC).at(
            Some("CBQHNAX3CFZWBUF2J4C6QEBGB2FEHZPXN2O3KILYZQ2X5XNBEHXHDW5TK".into()),
            Some("transfer".into()),
        );

        let context = failure.context.expect("context attached");
        assert_eq!(context.contract_id.as_deref(), Some("CBQHNAX3CFZWBUF2J4C6QEBGB2FEHZPXN2O3KILYZQ2X5XNBEHXHDW5TK"));
        assert_eq!(context.function.as_deref(), Some("transfer"));
        assert_eq!(context.cost_type.as_deref(), Some("cpu"));
    }

    #[test]
    fn an_explicit_cost_type_is_not_overwritten() {
        let failure = ExecutionFailure::from_diagnostic(CPU_DIAGNOSTIC).with_context(TrapContext {
            contract_id: None,
            function: None,
            cost_type: Some("custom".into()),
        });
        assert_eq!(failure.context.unwrap().cost_type.as_deref(), Some("custom"));
    }

    #[test]
    fn a_function_named_in_the_diagnostic_is_attached() {
        let failure = ExecutionFailure::from_diagnostic(
            "HostError: Error(Contract, #3, VmError(Ok, Error(Contract, transfer)))",
        )
        .with_invocation(None, Some("withdraw"));

        let context = failure.context().expect("context attached");
        // The name inside the diagnostic wins: it is the frame that trapped.
        assert_eq!(context.function.as_deref(), Some("transfer"));
        assert_eq!(context.cost_type.as_deref(), Some("cpu"));
    }

    #[test]
    fn the_invocation_name_is_used_when_the_diagnostic_names_no_frame() {
        let failure =
            ExecutionFailure::from_diagnostic(CPU_DIAGNOSTIC).with_invocation(None, Some("withdraw"));
        assert_eq!(failure.context().unwrap().function.as_deref(), Some("withdraw"));
    }

    #[test]
    fn a_contract_id_strkey_in_the_diagnostic_is_attached() {
        let id = "CBQHNAX3CFZWBUF2J4C6QEBGB2FEHZPXN2O3KILYZQ2X5XNBEHXHDW5TK";
        let failure = ExecutionFailure::from_diagnostic(&format!(
            "HostError: Error(Contract, #1, Some(Diagnostic {{ contract: {id}, cpu instructions exceeded budget }}))"
        ));

        assert_eq!(contract_id_from_diagnostic(&failure.detail).as_deref(), Some(id));
    }

    #[test]
    fn a_caller_supplied_contract_id_is_used_when_the_diagnostic_has_none() {
        let hash = "ab".repeat(32);
        let failure =
            ExecutionFailure::from_diagnostic(MEM_DIAGNOSTIC).with_invocation(Some(hash.clone()), None);
        assert_eq!(failure.context().unwrap().contract_id.as_deref(), Some(hash.as_str()));
    }

    #[test]
    fn prose_is_not_mistaken_for_a_function_name() {
        assert_eq!(function_from_diagnostic("something unrecognised"), None);
        assert_eq!(contract_id_from_diagnostic("deadbeef is not a contract id"), None);
    }

    #[test]
    fn an_exported_style_name_is_recognised() {
        assert_eq!(
            function_from_diagnostic("backtrace: contract::set_config").as_deref(),
            Some("set_config")
        );
    }

    #[test]
    fn kind_serialises_in_snake_case() {
        let json = serde_json::to_string(&FailureKind::CpuLimit).expect("serialises");
        assert_eq!(json, "\"cpu_limit\"");
        assert_eq!(serde_json::to_string(&FailureKind::MemLimit).unwrap(), "\"mem_limit\"");
    }
}
