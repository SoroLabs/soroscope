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

/// Resource measurements observed for one simulated execution path.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PathResult {
    /// Zero-based path identifier.
    pub path_id: usize,
    /// Argument vector used for this run.
    pub args_used: Vec<String>,
    /// Soroban resource consumption for this path.
    pub resources: SorobanResources,
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
    /// Human-readable note about coverage completeness.
    pub coverage_note: String,
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

    // ── 3. Multi-path dynamic exploration ────────────────────────────────────
    let variations = generate_arg_variations(&args);
    let total_variations = variations.len();

    let mut simulated_paths: Vec<PathResult> = Vec::new();
    let mut path_id = 0usize;

    for variant_args in &variations {
        // Skip if this permutation is identical to the baseline we already have.
        if *variant_args == args && !simulated_paths.is_empty() {
            continue;
        }

        // Catch panics from invalid argument types — many permutations will fail.
        let resources = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            profile_contract(
                wasm_bytes.clone(),
                function_name.clone(),
                variant_args.clone(),
                None,
                None,
            )
        }));

        match resources {
            Ok(Ok(r)) => {
                simulated_paths.push(PathResult {
                    path_id,
                    args_used: variant_args.clone(),
                    resources: r,
                });
                path_id += 1;
            }
            Ok(Err(e)) => {
                tracing::debug!(
                    args = ?variant_args,
                    error = %e,
                    "Arg permutation produced simulation error (skipped)"
                );
            }
            Err(_) => {
                tracing::debug!(
                    args = ?variant_args,
                    "Arg permutation caused a panic (skipped)"
                );
            }
        }
    }

    // Always include the baseline if no paths were collected.
    if simulated_paths.is_empty() {
        simulated_paths.push(PathResult {
            path_id: 0,
            args_used: args.clone(),
            resources: baseline_resources.clone(),
        });
    }

    // ── 4. Aggregate results ──────────────────────────────────────────────────
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

    let coverage_note = if total_variations >= MAX_PERMUTATIONS {
        format!(
            "Argument exploration was capped at {} permutations. \
             {} branches were identified statically; some execution paths may \
             not have been exercised. Consider supplying targeted test arguments \
             to improve coverage.",
            MAX_PERMUTATIONS, total_branch_count
        )
    } else if total_branch_count == 0 {
        "No branch instructions were found in the function body (or the function \
         could not be located in the WASM). The analysis reflects a single \
         execution path."
            .to_string()
    } else {
        format!(
            "{} branch point(s) identified; {} permutation(s) explored; \
             {} distinct resource profile(s) observed.",
            total_branch_count,
            simulated_paths.len(),
            distinct_profiles
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
}
