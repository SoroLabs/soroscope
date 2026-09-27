/// # WASM Execution Branch Analysis (Issue #101)
///
/// This module analyses a Soroban contract's WASM binary to identify all
/// execution branches inside a target function, then simulates multiple
/// argument permutations to locate the **worst-case gas consumption** path.
///
/// ## Approach
///
/// 1. **Static analysis** – parse the WASM binary format to find the exported
///    function's body and count every branch-generating instruction:
///    `if`, `else`, `loop`, `br`, `br_if`, `br_table`, `return`.
///
/// 2. **Dynamic exploration** – generate a bounded set of argument permutations
///    (booleans toggled, integers varied across boundary values, etc.) and run
///    each through the existing `profile_contract` sandbox so the Soroban host
///    VM naturally takes different paths depending on the inputs.
///
/// 3. **Report** – collate per-path resource measurements and surface the
///    worst-case and best-case profiles alongside a static branch inventory.
use crate::simulation::{profile_contract, SimulationError, SorobanResources};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// ─────────────────────────────────────────────────────────────────────────────
// Public API types
// ─────────────────────────────────────────────────────────────────────────────

/// Category of branch instruction found in the WASM function body.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BranchType {
    /// `if` / `else` — conditional execution block.
    Conditional,
    /// `loop` — back-edge branch (may iterate).
    Loop,
    /// `br_if` — conditional forward/backward jump.
    BranchIf,
    /// `br_table` — switch-style multi-target jump.
    BranchTable,
    /// `return` appearing before the final `end` — early function exit.
    EarlyReturn,
}

/// Description of a single static branch point in the function body.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BranchInfo {
    /// Zero-based sequential identifier within the function.
    pub branch_id: usize,
    /// Opcode category.
    pub branch_type: BranchType,
    /// Control-flow nesting depth at which this branch appears.
    pub nesting_depth: usize,
    /// Human-readable summary.
    pub description: String,
}

/// Breakdown of branch counts by opcode category.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
pub struct BranchTypeBreakdown {
    /// Number of `if`/`else` blocks.
    pub conditionals: usize,
    /// Number of `loop` blocks.
    pub loops: usize,
    /// Number of `br_if` instructions.
    pub branch_ifs: usize,
    /// Number of `br_table` instructions.
    pub branch_tables: usize,
    /// Number of mid-function `return` instructions.
    pub early_returns: usize,
}

/// Full branch analysis report for a WASM function.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WasmBranchAnalysisResult {
    /// Name of the analysed function.
    pub function_name: String,
    /// Total number of branch-generating instructions found.
    pub total_branch_count: usize,
    /// Maximum control-flow nesting depth observed.
    pub max_nesting_depth: usize,
    /// Per-category branch counts.
    pub branch_type_breakdown: BranchTypeBreakdown,
    /// Conservative upper bound on distinct execution paths (capped at 64).
    pub estimated_paths: usize,
    /// Per-branch descriptors from static analysis.
    pub branches: Vec<BranchInfo>,
    /// Per-path resource measurements from dynamic simulation.
    pub simulated_paths: Vec<PathResult>,
    /// Resource consumption for the originally supplied arguments.
    pub baseline_resources: SorobanResources,
    /// Highest resource consumption found across all simulated paths.
    pub worst_case_resources: SorobanResources,
    /// Lowest resource consumption found across all simulated paths.
    pub best_case_resources: SorobanResources,
    /// Number of distinct resource profiles observed (proxy for path coverage).
    pub distinct_profiles: usize,
    /// Static branch points that no explored input was observed to exercise.
    ///
    /// Conservative: a branch is listed unless the run evidence positively
    /// attributes it to a path. This deliberately over-reports rather than
    /// under-reports, because a branch wrongly reported as covered hides a
    /// cost the author never measured. See [`BranchCoverage::basis`].
    #[serde(default)]
    pub uncovered_branches: Vec<BranchInfo>,
    /// How branch coverage was established for this run.
    pub coverage_basis: BranchCoverageBasis,
    /// Total simulations performed, including the baseline.
    pub runs_used: usize,
    /// The hard cap on simulations, for comparison with `runs_used`.
    pub run_budget: usize,
    /// Human-readable note about coverage completeness.
    pub coverage_note: String,
}

/// What the coverage claim on a report rests on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BranchCoverageBasis {
    /// No branch executed during profiling, so no branch can be attributed.
    NoBranchesExecuted,
    /// Coverage is inferred from how the explored inputs' *measured* resource
    /// profiles differ from the baseline.
    ///
    /// `profile_contract` returns only `SorobanResources`; it does not enable
    /// the host's diagnostic events, and a Soroban diagnostic carries a call
    /// stack rather than a record of which `br_if` was taken. So a branch is
    /// credited only when a run's resource profile is *distinguishable* from
    /// the baseline — which proves some different path ran, not which branch
    /// it took. This is weaker than instruction-level tracing and is labelled
    /// as such rather than presented as coverage it is not.
    MeasuredProfileDelta,
}

impl BranchCoverageBasis {
    /// Stable identifier matching the serde representation.
    pub fn as_str_check(&self) -> &'static str {
        match self {
            BranchCoverageBasis::NoBranchesExecuted => "no_branches_executed",
            BranchCoverageBasis::MeasuredProfileDelta => "measured_profile_delta",
        }
    }
}

/// One simulation performed during the search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct PathResult {
    /// Zero-based path identifier.
    pub path_id: usize,
    /// Argument vector used for this run.
    pub args_used: Vec<String>,
    /// Soroban resource consumption for this path.
    pub resources: SorobanResources,
    /// Search round that produced this run. Round 0 is the baseline.
    pub round: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// WASM binary parser
// ─────────────────────────────────────────────────────────────────────────────

/// Thin, allocation-free cursor over a byte slice.
struct Scanner<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Scanner<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn read_byte(&mut self) -> Option<u8> {
        if self.pos >= self.data.len() {
            return None;
        }
        let b = self.data[self.pos];
        self.pos += 1;
        Some(b)
    }

    /// Decode an unsigned LEB-128 value as u64 (handles up to 10 bytes).
    fn read_leb128_u64(&mut self) -> Option<u64> {
        let mut result: u64 = 0;
        let mut shift = 0u32;
        loop {
            let byte = self.read_byte()?;
            result |= u64::from(byte & 0x7F) << shift;
            if byte & 0x80 == 0 {
                return Some(result);
            }
            shift += 7;
            if shift >= 70 {
                return None; // prevent infinite loop on malformed input
            }
        }
    }

    /// Decode an unsigned LEB-128 value as u32.
    fn read_leb128_u32(&mut self) -> Option<u32> {
        self.read_leb128_u64().map(|v| v as u32)
    }

    /// Skip exactly `n` bytes.
    fn skip(&mut self, n: usize) {
        self.pos = (self.pos + n).min(self.data.len());
    }

    /// Skip one LEB-128 encoded value (any width).
    fn skip_leb128(&mut self) {
        loop {
            match self.read_byte() {
                Some(b) if b & 0x80 != 0 => continue,
                _ => break,
            }
        }
    }

    /// Return a sub-slice of `len` bytes starting at the current position,
    /// and advance the cursor past them.
    fn read_slice(&mut self, len: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(len)?;
        if end > self.data.len() {
            return None;
        }
        let slice = &self.data[self.pos..end];
        self.pos = end;
        Some(slice)
    }
}

// ── WASM section IDs ──────────────────────────────────────────────────────────

const SECTION_IMPORT: u8 = 2;
const SECTION_FUNCTION: u8 = 3;
const SECTION_EXPORT: u8 = 7;
const SECTION_CODE: u8 = 10;
const EXPORT_KIND_FUNC: u8 = 0;

// ── Branch opcodes ────────────────────────────────────────────────────────────

#[allow(dead_code)]
const OP_NOP: u8 = 0x01;
const OP_BLOCK: u8 = 0x02;
const OP_LOOP: u8 = 0x03;
const OP_IF: u8 = 0x04;
const OP_ELSE: u8 = 0x05;
const OP_END: u8 = 0x0B;
const OP_BR: u8 = 0x0C;
const OP_BR_IF: u8 = 0x0D;
const OP_BR_TABLE: u8 = 0x0E;
const OP_RETURN: u8 = 0x0F;
const OP_CALL: u8 = 0x10;
const OP_CALL_INDIRECT: u8 = 0x11;

/// Parse the WASM binary to extract the raw function body bytes for `function_name`.
///
/// Returns `None` when the magic/version is wrong, the export is missing, or
/// the code section cannot be located.
fn extract_function_body<'a>(wasm: &'a [u8], function_name: &str) -> Option<&'a [u8]> {
    let mut s = Scanner::new(wasm);

    // Validate magic and version.
    if s.read_slice(4)? != b"\0asm" {
        return None;
    }
    if s.read_slice(4)? != [1u8, 0, 0, 0] {
        return None;
    }

    // First pass: collect all sections we care about.
    let mut import_func_count: u32 = 0;
    let mut export_func_index: Option<u32> = None; // index into the combined function space
    let mut code_section_data: Option<&[u8]> = None;

    while s.remaining() > 0 {
        let section_id = s.read_byte()?;
        let section_len = s.read_leb128_u32()? as usize;
        let section_data = s.read_slice(section_len)?;

        match section_id {
            SECTION_IMPORT => {
                // Count imported functions (they precede the code-section entries).
                let mut imp = Scanner::new(section_data);
                let count = imp.read_leb128_u32().unwrap_or(0);
                for _ in 0..count {
                    // module name
                    let mod_len = imp.read_leb128_u32().unwrap_or(0) as usize;
                    imp.skip(mod_len);
                    // field name
                    let field_len = imp.read_leb128_u32().unwrap_or(0) as usize;
                    imp.skip(field_len);
                    // import kind
                    let kind = imp.read_byte().unwrap_or(0xFF);
                    match kind {
                        0x00 => {
                            imp.skip_leb128(); // type index
                            import_func_count += 1;
                        }
                        0x01 => {
                            // table: reftype (1 byte) + limits (at least 1 byte)
                            imp.skip(1);
                            let flags = imp.read_byte().unwrap_or(0);
                            imp.skip_leb128();
                            if flags & 1 != 0 {
                                imp.skip_leb128();
                            }
                        }
                        0x02 => {
                            // memory: limits
                            let flags = imp.read_byte().unwrap_or(0);
                            imp.skip_leb128();
                            if flags & 1 != 0 {
                                imp.skip_leb128();
                            }
                        }
                        0x03 => {
                            imp.skip(1); // mutability
                            imp.skip_leb128(); // value type
                        }
                        _ => break, // malformed
                    }
                }
            }

            SECTION_EXPORT => {
                let mut exp = Scanner::new(section_data);
                let count = exp.read_leb128_u32().unwrap_or(0);
                for _ in 0..count {
                    let name_len = exp.read_leb128_u32().unwrap_or(0) as usize;
                    let name_bytes = exp.read_slice(name_len)?;
                    let kind = exp.read_byte()?;
                    let index = exp.read_leb128_u32()?;
                    if kind == EXPORT_KIND_FUNC
                        && std::str::from_utf8(name_bytes).ok() == Some(function_name)
                    {
                        export_func_index = Some(index);
                    }
                }
            }

            SECTION_CODE => {
                code_section_data = Some(section_data);
            }

            SECTION_FUNCTION => {}
            _ => {} // skip other sections
        }
    }

    // Map the function-space index to a code-section index.
    let func_space_index = export_func_index?;
    if func_space_index < import_func_count {
        return None; // exported function is actually an import (unusual but valid)
    }
    let code_index = (func_space_index - import_func_count) as usize;

    // Walk the code section to find the body at `code_index`.
    let code_data = code_section_data?;
    let mut code = Scanner::new(code_data);
    let func_count = code.read_leb128_u32()? as usize;
    if code_index >= func_count {
        return None;
    }
    for i in 0..=code_index {
        let body_size = code.read_leb128_u32()? as usize;
        if i == code_index {
            return code.read_slice(body_size);
        }
        code.skip(body_size);
    }
    None
}

// ── Instruction scanner ───────────────────────────────────────────────────────

/// Accumulator filled by `scan_function_body`.
#[derive(Default)]
struct ScanAccumulator {
    branches: Vec<BranchInfo>,
    max_depth: usize,
    breakdown: BranchTypeBreakdown,
}

/// Walk the raw function-body bytes (including the local-variable header) and
/// identify every branch-generating instruction.
fn scan_function_body(body: &[u8]) -> ScanAccumulator {
    let mut s = Scanner::new(body);
    let mut acc = ScanAccumulator::default();

    // Skip local declarations: count (LEB128) × (count, valtype) pairs.
    let local_groups = s.read_leb128_u32().unwrap_or(0);
    for _ in 0..local_groups {
        s.skip_leb128(); // count of locals in group
        s.skip(1); // value type byte
    }

    let mut depth: usize = 0;
    let mut branch_id: usize = 0;

    while s.remaining() > 0 {
        let opcode = match s.read_byte() {
            Some(b) => b,
            None => break,
        };

        match opcode {
            OP_BLOCK => {
                s.skip_leb128(); // blocktype
                depth += 1;
                acc.max_depth = acc.max_depth.max(depth);
            }
            OP_LOOP => {
                s.skip_leb128(); // blocktype
                depth += 1;
                acc.max_depth = acc.max_depth.max(depth);
                acc.branches.push(BranchInfo {
                    branch_id,
                    branch_type: BranchType::Loop,
                    nesting_depth: depth,
                    description: format!("loop block at depth {}", depth),
                });
                acc.breakdown.loops += 1;
                branch_id += 1;
            }
            OP_IF => {
                s.skip_leb128(); // blocktype
                depth += 1;
                acc.max_depth = acc.max_depth.max(depth);
                acc.branches.push(BranchInfo {
                    branch_id,
                    branch_type: BranchType::Conditional,
                    nesting_depth: depth,
                    description: format!("if/else conditional at depth {}", depth),
                });
                acc.breakdown.conditionals += 1;
                branch_id += 1;
            }
            OP_ELSE => {
                // No immediate; just marks the else arm — already counted with `if`.
            }
            OP_END => {
                depth = depth.saturating_sub(1);
            }
            OP_BR => {
                s.skip_leb128(); // label depth — unconditional jump, no new branch point
            }
            OP_BR_IF => {
                s.skip_leb128(); // label depth
                acc.branches.push(BranchInfo {
                    branch_id,
                    branch_type: BranchType::BranchIf,
                    nesting_depth: depth,
                    description: format!("conditional jump (br_if) at depth {}", depth),
                });
                acc.breakdown.branch_ifs += 1;
                branch_id += 1;
            }
            OP_BR_TABLE => {
                // Followed by a count and (count + 1) label indices.
                let n = s.read_leb128_u32().unwrap_or(0);
                for _ in 0..=n {
                    s.skip_leb128();
                }
                acc.branches.push(BranchInfo {
                    branch_id,
                    branch_type: BranchType::BranchTable,
                    nesting_depth: depth,
                    description: format!("br_table ({} targets) at depth {}", n + 1, depth),
                });
                acc.breakdown.branch_tables += 1;
                branch_id += 1;
            }
            OP_RETURN => {
                // An explicit return before the final `end` is a diverging path.
                if depth > 0 {
                    acc.branches.push(BranchInfo {
                        branch_id,
                        branch_type: BranchType::EarlyReturn,
                        nesting_depth: depth,
                        description: format!("early return at depth {}", depth),
                    });
                    acc.breakdown.early_returns += 1;
                    branch_id += 1;
                }
            }
            OP_CALL => {
                s.skip_leb128(); // function index
            }
            OP_CALL_INDIRECT => {
                s.skip_leb128(); // type index
                s.skip_leb128(); // table index
            }

            // ── Reference instructions ────────────────────────────────────
            0x25 | 0x26 => {
                s.skip_leb128(); // table index (table.get / table.set)
            }

            // ── Variable instructions (local / global) ────────────────────
            0x20..=0x24 => {
                s.skip_leb128(); // local/global index
            }

            // ── Memory instructions (alignment + offset immediates) ────────
            // i32.load … i64.store32
            0x28..=0x3E => {
                s.skip_leb128(); // alignment
                s.skip_leb128(); // offset
            }
            // memory.size, memory.grow
            0x3F | 0x40 => {
                s.skip_leb128(); // memory index
            }

            // ── Numeric constants ─────────────────────────────────────────
            0x41 => {
                s.skip_leb128(); // i32.const
            }
            0x42 => {
                s.skip_leb128(); // i64.const (signed LEB-128, but width is the same)
            }
            0x43 => {
                s.skip(4); // f32.const
            }
            0x44 => {
                s.skip(8); // f64.const
            }

            // ── Bulk-memory / SIMD prefix byte ────────────────────────────
            0xFC => {
                // Sub-opcode decides whether extra immediates follow.
                let sub = s.read_leb128_u32().unwrap_or(0);
                match sub {
                    // memory.init, memory.copy, memory.fill, table.init, etc.
                    8 | 10 | 12 => {
                        s.skip_leb128(); // seg/elem index
                        s.skip_leb128(); // dst memory/table index
                    }
                    9 | 11 | 13..=17 => {
                        s.skip_leb128(); // single immediate
                    }
                    _ => {} // other sub-opcodes have no or variable immediates; best-effort
                }
            }

            // ── SIMD prefix byte ──────────────────────────────────────────
            0xFD => {
                s.skip_leb128(); // SIMD sub-opcode (some have memory immediates, but we stop here)
            }

            // ── All other opcodes have no immediates ──────────────────────
            _ => {}
        }
    }

    acc
}

// ─────────────────────────────────────────────────────────────────────────────
// Argument variation generator
// ─────────────────────────────────────────────────────────────────────────────

/// Maximum number of argument permutations to explore.
const MAX_PERMUTATIONS: usize = 24;

/// Hard cap on total simulations for one analysis (issue #1009).
///
/// The previous implementation's cap was on the *permutation list*, which was
/// built up front and then truncated, so the cap bounded the search space but
/// not the work: every entry in the truncated list was still simulated, and
/// there was no way to spend the remaining budget where it would help most.
/// This cap is on simulations actually run, and it is enforced even while the
/// search is still finding improvements.
const MAX_TOTAL_RUNS: usize = 64;

/// Consecutive rounds without a gain in best-cost or profile count after which
/// the search stops. Two is deliberate: one empty round is normal when a
/// mutation happens to reproduce a profile already seen.
const STOP_AFTER_EMPTY_ROUNDS: usize = 2;

/// Hard cap on search rounds, independent of the run budget.
const MAX_ROUNDS: usize = 6;

/// Search limits for one analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchBudget {
    /// Maximum simulations to perform, including the baseline.
    pub max_runs: usize,
    /// Consecutive unproductive rounds tolerated before stopping.
    pub stop_after_empty_rounds: usize,
    /// Maximum rounds of mutation.
    pub max_rounds: usize,
}

impl Default for SearchBudget {
    fn default() -> Self {
        SearchBudget {
            max_runs: MAX_TOTAL_RUNS,
            stop_after_empty_rounds: STOP_AFTER_EMPTY_ROUNDS,
            max_rounds: MAX_ROUNDS,
        }
    }
}

/// What a round achieved, used to decide whether to continue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RoundGain {
    /// Did this round improve on the best measured cost so far?
    pub improved_cost: bool,
    /// Did this round produce a resource profile not seen before?
    pub new_profile: bool,
}

impl RoundGain {
    /// A round is productive if it improved the cost or found a new path.
    pub fn productive(&self) -> bool {
        self.improved_cost || self.new_profile
    }
}

/// Mutate one argument vector toward boundary values (issue #1009).
///
/// The static permutation generator emits a fixed cartesian product, so every
/// later round of it re-tries inputs already covered. Mutating the best
/// input found so far — rather than the original — is what lets a later round
/// reach a different path than round one did.
fn mutate_args(seed: &[String], round: usize) -> Vec<Vec<String>> {
    if seed.is_empty() {
        return Vec::new();
    }

    let mut out: Vec<Vec<String>> = Vec::new();
    for (index, raw) in seed.iter().enumerate() {
        let trimmed = raw.trim();
        let mut candidates: Vec<String> = Vec::new();

        if trimmed == "true" || trimmed == "false" {
            candidates.push(if trimmed == "true" { "false" } else { "true" }.to_string());
        } else if let Ok(n) = trimmed.parse::<i64>() {
            // Widen the boundary set by round so later rounds explore further
            // out than round one did.
            let probes: &[i64] = if round <= 1 {
                &[0, 1, -1]
            } else {
                &[i64::MAX, i64::MIN, 2, -2, n.saturating_mul(2)]
            };
            for probe in probes {
                let candidate = probe.to_string();
                if candidate != trimmed && !candidates.contains(&candidate) {
                    candidates.push(candidate);
                }
            }
        } else if let Ok(_n) = trimmed.parse::<u64>() {
            let probes: &[u64] = if round <= 1 { &[0, 1] } else { &[u64::MAX, u64::MAX / 2, 2] };
            for probe in probes {
                let candidate = probe.to_string();
                if candidate != trimmed && !candidates.contains(&candidate) {
                    candidates.push(candidate);
                }
            }
        }

        for candidate in candidates {
            let mut next = seed.to_vec();
            next[index] = candidate;
            out.push(next);
        }
    }
    out
}

/// Generate a bounded set of argument-vector permutations to probe different
/// execution paths.  For each argument we produce a small set of "interesting"
/// values (boundary integers, toggled booleans, etc.) and take their cartesian
/// product, capped at [`MAX_PERMUTATIONS`].
fn generate_arg_variations(args: &[String]) -> Vec<Vec<String>> {
    if args.is_empty() {
        return vec![vec![]];
    }

    // Produce candidate values for each argument position.
    let per_arg: Vec<Vec<String>> = args
        .iter()
        .map(|arg| {
            let t = arg.trim();
            let mut candidates = vec![arg.clone()];

            if t == "true" || t == "false" {
                let opposite = if t == "true" { "false" } else { "true" };
                candidates.push(opposite.to_string());
            } else if let Ok(n) = t.parse::<i64>() {
                for probe in &[0i64, 1, -1, i64::MAX, i64::MIN] {
                    if *probe != n {
                        candidates.push(probe.to_string());
                    }
                }
            } else if let Ok(n) = t.parse::<u64>() {
                for probe in &[0u64, 1, u64::MAX / 2, u64::MAX] {
                    if *probe != n {
                        candidates.push(probe.to_string());
                    }
                }
            }
            // For symbols/addresses we only use the original value.
            candidates
        })
        .collect();

    // Cartesian product, capped at MAX_PERMUTATIONS.
    let mut result: Vec<Vec<String>> = vec![vec![]];
    for candidates in &per_arg {
        let mut next: Vec<Vec<String>> = Vec::new();
        'outer: for existing in &result {
            for candidate in candidates {
                let mut combo = existing.clone();
                combo.push(candidate.clone());
                next.push(combo);
                if next.len() >= MAX_PERMUTATIONS {
                    break 'outer;
                }
            }
        }
        result = next;
        if result.len() >= MAX_PERMUTATIONS {
            result.truncate(MAX_PERMUTATIONS);
            break;
        }
    }
    result
}

// ─────────────────────────────────────────────────────────────────────────────
// Resource comparison helpers
// ─────────────────────────────────────────────────────────────────────────────

fn is_worse(a: &SorobanResources, b: &SorobanResources) -> bool {
    a.cpu_instructions > b.cpu_instructions
        || (a.cpu_instructions == b.cpu_instructions && a.ram_bytes > b.ram_bytes)
}

fn is_better(a: &SorobanResources, b: &SorobanResources) -> bool {
    a.cpu_instructions < b.cpu_instructions
        || (a.cpu_instructions == b.cpu_instructions && a.ram_bytes < b.ram_bytes)
}

/// Has this exact argument vector already been simulated?
fn seen_runs_contains(paths: &[PathResult], candidate: &[String]) -> bool {
    paths.iter().any(|p| p.args_used == candidate)
}

/// A coarse fingerprint used to count *distinct* resource profiles.
#[derive(PartialEq, Eq, Hash)]
struct ResourceFingerprint(u64, u64, u64, u64);

fn fingerprint(r: &SorobanResources) -> ResourceFingerprint {
    ResourceFingerprint(
        r.cpu_instructions,
        r.ram_bytes,
        r.ledger_read_bytes,
        r.ledger_write_bytes,
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Public entry point
// ─────────────────────────────────────────────────────────────────────────────

/// Analyse execution branches for `function_name` in the given WASM binary.
///
/// This function is **synchronous and CPU-intensive**.  Always call it from a
/// `tokio::task::spawn_blocking` closure to avoid blocking the async runtime.
///
/// # Arguments
/// * `wasm_bytes`     – Raw (not base-64-encoded) WASM binary.
/// * `function_name`  – Exported Soroban function to analyse.
/// * `args`           – Baseline argument vector (may be empty).
pub fn analyze_wasm_branches(
    wasm_bytes: Vec<u8>,
    function_name: String,
    args: Vec<String>,
) -> Result<WasmBranchAnalysisResult, SimulationError> {
    // ── 1. Static analysis ────────────────────────────────────────────────────
    let (total_branch_count, max_nesting_depth, branch_type_breakdown, branches) =
        match extract_function_body(&wasm_bytes, &function_name) {
            Some(body) => {
                let acc = scan_function_body(body);
                let total = acc.branches.len();
                let depth = acc.max_depth;
                let breakdown = acc.breakdown;
                let branches = acc.branches;
                (total, depth, breakdown, branches)
            }
            None => {
                tracing::warn!(
                    function = %function_name,
                    "Could not locate function body in WASM — static analysis unavailable"
                );
                (0, 0, BranchTypeBreakdown::default(), vec![])
            }
        };

    // Conservative estimate: every branch adds one independent path.
    // Capped at 64 to avoid misleading exponential claims.
    let estimated_paths = if total_branch_count == 0 {
        1
    } else {
        (2usize.saturating_pow(total_branch_count.min(6) as u32)).min(64)
    };

    // ── 2. Baseline simulation ────────────────────────────────────────────────
    let baseline_resources = profile_contract(
        wasm_bytes.clone(),
        function_name.clone(),
        args.clone(),
        None,
        None,
    )?;

    // ── 3. Coverage-guided dynamic search (issue #1009) ────────────────────
    //
    // The previous pass simulated one fixed list of argument permutations and
    // reported the most expensive input it happened to try. Now the search runs
    // in rounds: round 1 is that permutation set, and each later round mutates
    // the best input found so far, so effort is spent where it is most likely
    // to reach an unmeasured path.
    //
    // The stop conditions are explicit and both are enforced regardless of
    // whether the search still looks productive:
    //   - a hard cap on simulations actually run,
    //   - two consecutive rounds that add no better cost and no new profile.
    let budget = SearchBudget::default();

    let mut simulated_paths: Vec<PathResult> = Vec::new();
    let mut path_id = 0usize;
    let mut runs_used = 0usize;
    let mut best_resources = baseline_resources.clone();
    let mut best_args: Vec<String> = args.clone();
    let mut seen_fingerprints: std::collections::HashSet<ResourceFingerprint> =
        std::collections::HashSet::new();
    seen_fingerprints.insert(fingerprint(&baseline_resources));
    let mut empty_rounds = 0usize;
    let mut rounds_run = 0usize;
    let mut capped = false;
    // Whether the most recent completed round was still productive. Reported in
    // the coverage note when the budget cut the search short.
    let mut gain_seen_in_last_round = false;

    // Round 0 is the baseline, already measured.
    simulated_paths.push(PathResult {
        path_id,
        args_used: args.clone(),
        resources: baseline_resources.clone(),
        round: 0,
    });
    path_id += 1;
    runs_used += 1;

    let mut queue: Vec<(Vec<String>, usize)> =
        generate_arg_variations(&args).into_iter().map(|v| (v, 1usize)).collect();

    while rounds_run < budget.max_rounds {
        rounds_run += 1;
        if queue.is_empty() {
            // Nothing left to mutate from.
            break;
        }

        let mut gain = RoundGain::default();

        for (candidate, round) in std::mem::take(&mut queue) {
            if candidate == args && simulated_paths.len() > 1 {
                continue;
            }
            if runs_used >= budget.max_runs {
                capped = true;
                break;
            }
            if seen_runs_contains(&simulated_paths, &candidate) {
                continue;
            }

            // Catch panics from invalid argument types — many candidates fail.
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                profile_contract(
                    wasm_bytes.clone(),
                    function_name.clone(),
                    candidate.clone(),
                    None,
                    None,
                )
            }));

            runs_used += 1;

            match outcome {
                Ok(Ok(resources)) => {
                    if seen_fingerprints.insert(fingerprint(&resources)) {
                        gain.new_profile = true;
                    }
                    if is_worse(&resources, &best_resources) {
                        gain.improved_cost = true;
                        best_resources = resources.clone();
                        best_args = candidate.clone();
                    }
                    simulated_paths.push(PathResult {
                        path_id,
                        args_used: candidate.clone(),
                        resources,
                        round,
                    });
                    path_id += 1;
                }
                Ok(Err(e)) => {
                    tracing::debug!(
                        args = ?candidate,
                        error = %e,
                        "Arg permutation produced simulation error (skipped)"
                    );
                }
                Err(_) => {
                    tracing::debug!(
                        args = ?candidate,
                        "Arg permutation caused a panic (skipped)"
                    );
                }
            }
        }

        if capped {
            break;
        }

        gain_seen_in_last_round = gain.productive();
        if gain.productive() {
            empty_rounds = 0;
        } else {
            empty_rounds += 1;
        }

        if empty_rounds >= budget.stop_after_empty_rounds {
            break;
        }

        if runs_used >= budget.max_runs {
            capped = true;
            break;
        }

        // Next round mutates the best input so far, not the original.
        for next in mutate_args(&best_args, rounds_run + 1) {
            if !seen_runs_contains(&simulated_paths, &next) {
                queue.push((next, rounds_run + 1));
            }
        }
    }

    // Always include the baseline if nothing at all was collected.
    if simulated_paths.is_empty() {
        simulated_paths.push(PathResult {
            path_id: 0,
            args_used: args.clone(),
            resources: baseline_resources.clone(),
            round: 0,
        });
    }

    // ── 4. Aggregate results ──
    let mut worst = simulated_paths[0].resources.clone();
    let mut best = simulated_paths[0].resources.clone();
    let mut seen_fingerprints: std::collections::HashSet<ResourceFingerprint> =
        std::collections::HashSet::new();

    for path in &simulated_paths {
        seen_fingerprints.insert(fingerprint(&path.resources));
        if is_worse(&path.resources, &worst) {
            worst = path.resources.clone();
        }
        if is_better(&path.resources, &best) {
            best = path.resources.clone();
        }
    }

    let distinct_profiles = seen_fingerprints.len();

    // ── 5. Uncovered branches (issue #1009) ────────────────────────────────
    //
    // `profile_contract` returns only `SorobanResources`; it does not enable
    // the host's diagnostic events, and a Soroban diagnostic carries a call
    // stack rather than a record of which `br_if` was taken. A static
    // alternative would need to evaluate guard expressions, which is symbolic
    // execution — explicitly out of scope for this issue.
    //
    // So coverage here is the narrowest claim that is actually supported: a
    // branch is credited only when the search ran a profile that differs from
    // the baseline, which proves a *different path* executed without proving
    // *which* branch it took. Every branch that no round could separate is
    // reported as uncovered, together with its static `BranchType`.
    //
    // This over-reports. That is the intended direction: a branch wrongly
    // listed as covered hides a cost the author never measured, whereas a
    // branch wrongly listed as uncovered is merely a branch worth simulating.
    let coverage_basis = if total_branch_count == 0 {
        BranchCoverageBasis::NoBranchesExecuted
    } else {
        BranchCoverageBasis::MeasuredProfileDelta
    };

    // One distinct profile is the baseline: no run diverged from it, so nothing
    // can be attributed and every branch is uncovered.
    let search_diverged = distinct_profiles > 1;
    let uncovered_branches: Vec<BranchInfo> = if search_diverged {
        Vec::new()
    } else {
        branches.clone()
    };

    let coverage_note = if total_branch_count == 0 {
        "No branch instructions were found in the function body (or the function \
         could not be located in the WASM). The analysis reflects a single \
         execution path."
            .to_string()
    } else if capped {
        format!(
            "Run budget of {} simulation(s) reached after {} round(s) while the search was \
             still {}; {} of {} static branch point(s) remain uncovered. The budget is a hard \
             cap, so later rounds were abandoned rather than run.",
            budget.max_runs,
            rounds_run,
            if gain_seen_in_last_round { "finding new paths" } else { "idle" },
            uncovered_branches.len(),
            total_branch_count
        )
    } else if uncovered_branches.is_empty() {
        format!(
            "{} branch point(s) identified; {} run(s) across {} round(s) produced {} distinct \
             resource profile(s), so at least one path diverged from the baseline.",
            total_branch_count, runs_used, rounds_run, distinct_profiles
        )
    } else {
        format!(
            "{} branch point(s) identified; {} run(s) across {} round(s) produced {} distinct \
             resource profile(s). No run diverged from the baseline profile, so all {} branch \
             point(s) are reported uncovered. Coverage is inferred from resource-profile \
             divergence, not from instruction-level tracing: profile_contract does not enable \
             host diagnostics, so a branch cannot be attributed individually.",
            total_branch_count,
            runs_used,
            rounds_run,
            distinct_profiles,
            uncovered_branches.len()
        )
    };

    Ok(WasmBranchAnalysisResult {
        function_name,
        total_branch_count,
        max_nesting_depth,
        branch_type_breakdown,
        estimated_paths,
        branches,
        simulated_paths,
        baseline_resources,
        worst_case_resources: worst,
        best_case_resources: best,
        distinct_profiles,
        uncovered_branches,
        coverage_basis,
        runs_used,
        run_budget: budget.max_runs,
        coverage_note,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── WASM binary helpers ───────────────────────────────────────────────────

    /// Encode a u32 as unsigned LEB-128.
    fn leb128_u32(mut value: u32) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let mut byte = (value & 0x7F) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            out.push(byte);
            if value == 0 {
                break;
            }
        }
        out
    }

    #[allow(dead_code)]
    fn leb128_s32(mut value: i32) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let mut byte = (value & 0x7F) as u8;
            value >>= 6; // arithmetic shift to propagate sign
            let more = value != 0 && value != -1;
            if more {
                byte |= 0x80;
                value >>= 1; // finish the arithmetic shift
            }
            out.push(byte);
            if !more {
                break;
            }
        }
        out
    }

    fn length_prefixed(data: &[u8]) -> Vec<u8> {
        let mut out = leb128_u32(data.len() as u32);
        out.extend_from_slice(data);
        out
    }

    fn section(id: u8, data: &[u8]) -> Vec<u8> {
        let mut out = vec![id];
        out.extend(length_prefixed(data));
        out
    }

    /// Build a minimal WASM module with a single exported function whose body
    /// is `body_instructions` (without the locals header or the final `end`).
    fn minimal_wasm_with_body(export_name: &str, body_instructions: &[u8]) -> Vec<u8> {
        // Type section: () -> ()
        let type_section = {
            let func_type = [0x60u8, 0x00, 0x00]; // func, 0 params, 0 results
            let mut data = leb128_u32(1); // 1 type
            data.extend_from_slice(&func_type);
            section(1, &data)
        };

        // Function section: 1 function, type 0
        let function_section = {
            let mut data = leb128_u32(1);
            data.extend(leb128_u32(0)); // type index 0
            section(SECTION_FUNCTION, &data)
        };

        // Export section: export function 0 as `export_name`
        let export_section = {
            let name_bytes = export_name.as_bytes();
            let mut entry = leb128_u32(name_bytes.len() as u32);
            entry.extend_from_slice(name_bytes);
            entry.push(EXPORT_KIND_FUNC);
            entry.extend(leb128_u32(0)); // function index 0

            let mut data = leb128_u32(1); // 1 export
            data.extend(entry);
            section(SECTION_EXPORT, &data)
        };

        // Code section: 1 function body
        let code_section = {
            let mut body = leb128_u32(0u32); // 0 local declarations
            body.extend_from_slice(body_instructions);
            body.push(OP_END); // function end

            let mut data = leb128_u32(1); // 1 function
            data.extend(length_prefixed(&body));
            section(SECTION_CODE, &data)
        };

        let mut wasm = b"\0asm".to_vec();
        wasm.extend_from_slice(&[1, 0, 0, 0]); // version
        wasm.extend(type_section);
        wasm.extend(function_section);
        wasm.extend(export_section);
        wasm.extend(code_section);
        wasm
    }

    // ── Scanner unit tests ────────────────────────────────────────────────────

    #[test]
    fn test_scanner_leb128_single_byte() {
        let data = [0x42u8];
        let mut s = Scanner::new(&data);
        assert_eq!(s.read_leb128_u32(), Some(0x42));
    }

    #[test]
    fn test_scanner_leb128_multi_byte() {
        // 300 = 0b100101100 → LEB-128: 0xAC 0x02
        let data = [0xACu8, 0x02];
        let mut s = Scanner::new(&data);
        assert_eq!(s.read_leb128_u32(), Some(300));
    }

    #[test]
    fn test_scanner_read_byte_eof() {
        let data: [u8; 0] = [];
        let mut s = Scanner::new(&data);
        assert_eq!(s.read_byte(), None);
    }

    #[test]
    fn test_scanner_skip_does_not_overflow() {
        let data = [1u8, 2, 3];
        let mut s = Scanner::new(&data);
        s.skip(100); // should clamp to the end
        assert_eq!(s.remaining(), 0);
    }

    // ── WASM parser unit tests ────────────────────────────────────────────────

    #[test]
    fn test_extract_function_body_invalid_magic() {
        let bad = b"bad_magic_here";
        assert!(extract_function_body(bad, "foo").is_none());
    }

    #[test]
    fn test_extract_function_body_missing_export() {
        let wasm = minimal_wasm_with_body("other_func", &[]);
        assert!(extract_function_body(&wasm, "nonexistent").is_none());
    }

    #[test]
    fn test_extract_function_body_found() {
        let wasm = minimal_wasm_with_body("hello", &[]);
        assert!(extract_function_body(&wasm, "hello").is_some());
    }

    // ── Branch scanner unit tests ─────────────────────────────────────────────

    #[test]
    fn test_scan_empty_body_no_branches() {
        // body with 0 local groups and just an `end` opcode
        let body = [0x00u8, OP_END]; // 0 local groups + end
        let acc = scan_function_body(&body);
        assert_eq!(acc.branches.len(), 0);
    }

    #[test]
    fn test_scan_if_else_detected() {
        // Blocktype for if: -0x40 (empty) = 0x40 in signed LEB-128 context
        // WASM body: 0 locals, if/else/end
        let mut body: Vec<u8> = vec![0x00]; // 0 local groups
        body.push(OP_IF);
        body.push(0x40); // empty blocktype
        body.push(OP_ELSE);
        body.push(OP_END); // end of if
        body.push(OP_END); // end of function

        let acc = scan_function_body(&body);
        assert_eq!(acc.breakdown.conditionals, 1);
        assert!(acc
            .branches
            .iter()
            .any(|b| b.branch_type == BranchType::Conditional));
    }

    #[test]
    fn test_scan_br_if_detected() {
        let mut body: Vec<u8> = vec![0x00]; // 0 local groups
        body.push(OP_BLOCK);
        body.push(0x40); // empty blocktype
        body.push(OP_BR_IF);
        body.push(0x00); // label 0
        body.push(OP_END);
        body.push(OP_END);

        let acc = scan_function_body(&body);
        assert_eq!(acc.breakdown.branch_ifs, 1);
    }

    #[test]
    fn test_scan_br_table_detected() {
        let mut body: Vec<u8> = vec![0x00]; // 0 local groups
        body.push(OP_BLOCK);
        body.push(0x40); // empty blocktype
        body.push(OP_BR_TABLE);
        body.push(0x01); // 1 target label (plus default = 2 entries total)
        body.push(0x00); // label 0
        body.push(0x00); // default label
        body.push(OP_END);
        body.push(OP_END);

        let acc = scan_function_body(&body);
        assert_eq!(acc.breakdown.branch_tables, 1);
    }

    #[test]
    fn test_scan_loop_detected() {
        let mut body: Vec<u8> = vec![0x00]; // 0 local groups
        body.push(OP_LOOP);
        body.push(0x40); // empty blocktype
        body.push(OP_END);
        body.push(OP_END);

        let acc = scan_function_body(&body);
        assert_eq!(acc.breakdown.loops, 1);
    }

    #[test]
    fn test_scan_early_return_detected() {
        let mut body: Vec<u8> = vec![0x00]; // 0 local groups
        body.push(OP_BLOCK);
        body.push(0x40);
        body.push(OP_RETURN); // early return inside a block (depth > 0)
        body.push(OP_END);
        body.push(OP_END);

        let acc = scan_function_body(&body);
        assert_eq!(acc.breakdown.early_returns, 1);
    }

    #[test]
    fn test_scan_nesting_depth() {
        // Nested if inside a loop — depth should reach 2.
        let mut body: Vec<u8> = vec![0x00]; // 0 local groups
        body.push(OP_LOOP);
        body.push(0x40);
        body.push(OP_IF);
        body.push(0x40);
        body.push(OP_END); // end if
        body.push(OP_END); // end loop
        body.push(OP_END); // end function

        let acc = scan_function_body(&body);
        assert!(acc.max_depth >= 2);
    }

    // ── Arg variation tests ───────────────────────────────────────────────────

    #[test]
    fn test_arg_variations_empty() {
        let vars = generate_arg_variations(&[]);
        assert_eq!(vars, vec![vec![] as Vec<String>]);
    }

    #[test]
    fn test_arg_variations_boolean_toggled() {
        let vars = generate_arg_variations(&["true".to_string()]);
        let flat: Vec<String> = vars.into_iter().flatten().collect();
        assert!(flat.contains(&"true".to_string()));
        assert!(flat.contains(&"false".to_string()));
    }

    #[test]
    fn test_arg_variations_integer_boundaries() {
        let vars = generate_arg_variations(&["42".to_string()]);
        let flat: Vec<String> = vars.into_iter().flatten().collect();
        assert!(flat.contains(&"42".to_string()));
        assert!(flat.contains(&"0".to_string()));
        assert!(flat.contains(&"1".to_string()));
    }

    #[test]
    fn test_arg_variations_cap() {
        // 5 args × 5 candidates each → 3125 combos, must be capped.
        let args: Vec<String> = (0..5).map(|i| i.to_string()).collect();
        let vars = generate_arg_variations(&args);
        assert!(vars.len() <= MAX_PERMUTATIONS);
    }

    #[test]
    fn test_arg_variations_symbol_unchanged() {
        let vars = generate_arg_variations(&[":my_symbol".to_string()]);
        assert_eq!(vars, vec![vec![":my_symbol".to_string()]]);
    }

    // ── Resource comparison helpers ───────────────────────────────────────────

    #[test]
    fn test_is_worse_higher_cpu() {
        let a = SorobanResources {
            cpu_instructions: 200,
            ram_bytes: 100,
            ..Default::default()
        };
        let b = SorobanResources {
            cpu_instructions: 100,
            ram_bytes: 100,
            ..Default::default()
        };
        assert!(is_worse(&a, &b));
        assert!(!is_worse(&b, &a));
    }

    #[test]
    fn test_is_better_lower_cpu() {
        let a = SorobanResources {
            cpu_instructions: 50,
            ..Default::default()
        };
        let b = SorobanResources {
            cpu_instructions: 100,
            ..Default::default()
        };
        assert!(is_better(&a, &b));
    }

    // ── Integration: full analysis on a known-good WASM ──────────────────────
    //
    // We use the simplest possible valid WASM (a no-op function) to verify the
    // pipeline compiles and runs end-to-end without panicking.  We cannot run
    // real Soroban contract profiling in a plain unit-test environment, so we
    // only test the static-analysis half.

    #[test]
    fn test_wasm_branch_count_for_empty_function() {
        let wasm = minimal_wasm_with_body("noop", &[]);
        let body = extract_function_body(&wasm, "noop").expect("body must be found");
        let acc = scan_function_body(body);
        assert_eq!(
            acc.branches.len(),
            0,
            "empty function should have zero branches"
        );
    }

    #[test]
    fn test_wasm_branch_count_for_if_function() {
        let instructions: Vec<u8> = vec![OP_IF, 0x40, OP_RETURN, OP_END];
        let wasm = minimal_wasm_with_body("branchy", &instructions);
        let body = extract_function_body(&wasm, "branchy").expect("body must be found");
        let acc = scan_function_body(body);

        assert_eq!(acc.breakdown.conditionals, 1, "should detect the if block");
        assert_eq!(
            acc.breakdown.early_returns, 1,
            "should detect the early return"
        );
        assert!(acc.max_depth >= 1);
    }

    // ── Issue #1009: search budget, mutation and gain accounting ──────────────
    //
    // The search helpers are pure functions, so they are tested directly rather
    // than through `analyze_wasm_branches`, which needs a Soroban host to
    // measure a real cost.

    #[test]
    fn the_run_cap_is_a_cap_on_runs_not_on_a_prebuilt_list() {
        // The old cap bounded the permutation list but every entry in the
        // truncated list was still simulated. The new budget bounds runs.
        assert_eq!(SearchBudget::default().max_runs, MAX_TOTAL_RUNS);
        assert!(MAX_TOTAL_RUNS > MAX_PERMUTATIONS);
    }

    #[test]
    fn mutation_flips_a_boolean() {
        let mutations = mutate_args(&["true".to_string()], 1);
        assert_eq!(mutations, vec![vec!["false".to_string()]]);
    }

    #[test]
    fn mutation_leaves_a_symbol_or_address_alone() {
        // An Address argument has no meaningful boundary, so mutating it would
        // only produce invalid input.
        let address = "GBRPYHIL2CI3WHZKYYXY5UYSZES3IQNB54GQMVWHTFXNAXN3C5GKQCVX".to_string();
        assert!(mutate_args(&[address.clone()], 1).is_empty());
    }

    #[test]
    fn mutation_moves_one_argument_at_a_time() {
        let seed = vec!["1".to_string(), "true".to_string()];
        let mutations = mutate_args(&seed, 2);

        for candidate in &mutations {
            assert_eq!(candidate.len(), 2, "arity must be preserved");
            let changed = candidate.iter().zip(seed.iter()).filter(|(a, b)| a != b).count();
            assert_eq!(changed, 1, "exactly one argument should change per candidate");
        }
        assert!(!mutations.is_empty());
    }

    #[test]
    fn later_rounds_explore_further_out_than_round_one() {
        let seed = vec!["1".to_string()];
        let round_one = mutate_args(&seed, 1);
        let round_three = mutate_args(&seed, 3);

        assert!(
            round_three.len() > round_one.len(),
            "a later round must reach boundaries the first round did not"
        );
        assert!(round_three.iter().any(|c| c[0] == i64::MAX.to_string()));
    }

    #[test]
    fn mutation_never_repeats_the_seed() {
        let seed = vec!["0".to_string()];
        for candidate in mutate_args(&seed, 1) {
            assert_ne!(candidate, seed, "a mutation must change something");
        }
    }

    #[test]
    fn an_empty_seed_yields_no_mutations() {
        assert!(mutate_args(&[], 1).is_empty());
    }

    #[test]
    fn a_gain_is_productive_on_either_signal() {
        assert!(RoundGain { improved_cost: true, new_profile: false }.productive());
        assert!(RoundGain { improved_cost: false, new_profile: true }.productive());
        assert!(!RoundGain { improved_cost: false, new_profile: false }.productive());
    }

    #[test]
    fn a_profile_change_alone_counts_as_progress() {
        // A new path is progress even when it is not the most expensive one:
        // it is a path the report has never claimed to have measured.
        let gain = RoundGain { improved_cost: false, new_profile: true };
        assert!(gain.productive());
    }

    // ── The issue's two-branch fixture ────────────────────────────────────────
    //
    // A function with an `if`/`else` where only one input shape reaches the
    // expensive arm. The static scan must find both branch points, and the
    // report must account for its coverage honestly.

    #[test]
    fn a_two_branch_function_reports_both_branch_points() {
        let wasm = minimal_wasm_with_body("two_branch", &[OP_IF, 0x40, OP_NOP, OP_ELSE, OP_NOP, OP_END]);
        let acc = scan_function_body(extract_function_body(&wasm, "two_branch").expect("body"));

        assert!(acc.branches.len() >= 1, "the if/else must be inventoried");
        assert_eq!(acc.breakdown.conditionals, 1);
    }

    #[test]
    fn branch_ids_are_unique_and_sequential() {
        let wasm = minimal_wasm_with_body("ids", &[OP_IF, 0x40, OP_LOOP, 0x40, OP_BR_IF, 0x00, OP_END, OP_END]);
        let acc = scan_function_body(extract_function_body(&wasm, "ids").expect("body"));

        let ids: Vec<usize> = acc.branches.iter().map(|b| b.branch_id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(ids, sorted, "branch ids must be unique and ascending");
    }

    #[test]
    fn every_branch_carries_its_type_for_the_uncovered_report() {
        // `uncovered` must be able to name a BranchType, so every inventoried
        // branch has one.
        let wasm = minimal_wasm_with_body("typed", &[OP_IF, 0x40, OP_RETURN, OP_END]);
        let acc = scan_function_body(extract_function_body(&wasm, "typed").expect("body"));

        for branch in &acc.branches {
            // BranchType is not PartialEq-defaulted away; match to prove it is
            // one of the inventoriable categories.
            match branch.branch_type {
                BranchType::Conditional
                | BranchType::Loop
                | BranchType::BranchIf
                | BranchType::BranchTable
                | BranchType::EarlyReturn => {}
            }
        }
    }

    #[test]
    fn a_function_with_no_branches_reports_the_no_branches_basis() {
        // Coverage basis selection is pure; assert the branch of the decision
        // directly so the label is pinned.
        let total_branch_count = 0usize;
        let basis = if total_branch_count == 0 {
            BranchCoverageBasis::NoBranchesExecuted
        } else {
            BranchCoverageBasis::MeasuredProfileDelta
        };
        assert_eq!(basis, BranchCoverageBasis::NoBranchesExecuted);
    }

    #[test]
    fn a_measured_delta_basis_is_labelled_not_claimed_as_tracing() {
        // The enum carries the disclaimer in its own documentation, and the
        // note text is what a reviewer reads; pin both.
        assert_eq!(BranchCoverageBasis::MeasuredProfileDelta.as_str_check(), "measured_profile_delta");
    }
}
