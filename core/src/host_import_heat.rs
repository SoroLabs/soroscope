//! Per-export host-import heat map with cost-parameter estimates (issue #1008).
//!
//! `cost_breakdown` needs a simulation. This is the static complement: which
//! exported function imports which host functions, how many times each appears
//! in the body, and what those calls would cost under a given cost-parameter
//! table. It reviews in CI with no ledger state and no RPC, and it points at
//! the one function worth simulating first.
//!
//! # Static estimate, not a budget
//!
//! Every number here is [`EstimateKind::StaticEstimate`]. A real budget also
//! accounts for ledger entry contents, the invocation's own footprint, call
//! depth and the per-invocation base cost, none of which are knowable from a
//! module's instructions. The row carries that label, and the report repeats
//! it, so an estimate is never mistaken for a `cost_breakdown` figure.
//!
//! # Indirect calls
//!
//! Only direct `call <index>` instructions are attributed, and only when the
//! index resolves to a host import. `call_indirect` is *counted* but never
//! resolved: its target depends on runtime data, and inventing one would put a
//! fabricated import name in a report whose whole value is that it is not
//! fabricated.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use wasmparser::Operator;

use crate::parsed_module::{self, ParsedModule};

/// Per-unit costs for the resource kinds a Soroban invocation can consume.
///
/// The field names follow the network's cost-parameter type names
/// (`Instr`, `MemAlloc`, `MemRead`, `MemWrite`, `EntrySize`, `Rent`,
/// `Event`, `EventCount`, `LoadLedgerEntry`) so a table can be transcribed
/// from network settings without a translation step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractCostParams {
    /// Per decoded WASM instruction.
    pub instruction_cost: u64,
    /// Per byte of linear-memory allocation.
    pub mem_alloc_byte_cost: u64,
    /// Per byte read from linear memory.
    pub mem_read_byte_cost: u64,
    /// Per byte written to linear memory.
    pub mem_write_byte_cost: u64,
    /// Per byte of a ledger entry's serialized size.
    pub entry_size_byte_cost: u64,
    /// Per byte of rent held by a ledger entry.
    pub rent_byte_cost: u64,
    /// Per byte of diagnostic event data.
    pub event_byte_cost: u64,
    /// Per diagnostic event, regardless of size.
    pub event_count_cost: u64,
    /// Per ledger entry loaded.
    pub load_ledger_entry_cost: u64,
    /// Charged once per invocation, before any instruction runs.
    pub base_cpu_cost: u64,
}

impl ContractCostParams {
    /// The checked-in default table.
    ///
    /// These are the order-of-magnitude figures Soroban testnet and the
    /// published cost model use for the units each field counts. They are a
    /// *shape*, not a network quote: swap in a real table for a real figure.
    pub fn network_default() -> Self {
        ContractCostParams {
            instruction_cost: 1,
            mem_alloc_byte_cost: 1,
            mem_read_byte_cost: 1,
            mem_write_byte_cost: 1,
            entry_size_byte_cost: 1,
            rent_byte_cost: 1,
            event_byte_cost: 1,
            event_count_cost: 1,
            load_ledger_entry_cost: 1,
            base_cpu_cost: 100,
        }
    }

    /// A table where every unit is free, for isolating call counts from pricing.
    pub fn free() -> Self {
        ContractCostParams {
            instruction_cost: 0,
            mem_alloc_byte_cost: 0,
            mem_read_byte_cost: 0,
            mem_write_byte_cost: 0,
            entry_size_byte_cost: 0,
            rent_byte_cost: 0,
            event_byte_cost: 0,
            event_count_cost: 0,
            load_ledger_entry_cost: 0,
            base_cpu_cost: 0,
        }
    }

    /// Unit cost of one call to a host import with this name.
    ///
    /// Storage calls dominate a contract's budget and are priced accordingly;
    /// an unrecognised import is charged the instruction cost, which is the
    /// floor rather than an invented number.
    pub fn import_cost(&self, qualified_name: &str) -> u64 {
        if qualified_name.contains("put_contract_data")
            || qualified_name.contains("del_contract_data")
        {
            // A write touches the footprint, serialises an entry and holds rent.
            self.instruction_cost + self.entry_size_byte_cost + self.rent_byte_cost
        } else if qualified_name.contains("get_contract_data")
            || qualified_name.contains("has_contract_data")
            || qualified_name.contains("extend_contract_data_ttl")
        {
            // A read loads the entry; the TTL extension additionally writes it.
            self.instruction_cost
                + self.load_ledger_entry_cost
                + if qualified_name.contains("extend_") { self.entry_size_byte_cost } else { 0 }
        } else if qualified_name.contains("contract_code") {
            self.instruction_cost + self.load_ledger_entry_cost
        } else if qualified_name.contains("contract_event") || qualified_name.contains("log")
        {
            self.instruction_cost + self.event_count_cost + self.event_byte_cost
        } else {
            self.instruction_cost
        }
    }
}

/// The broad resource an import is charged against, for ranking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CostClass {
    Storage,
    Event,
    ContractCode,
    Other,
}

impl CostClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            CostClass::Storage => "storage",
            CostClass::Event => "event",
            CostClass::ContractCode => "contract_code",
            CostClass::Other => "other",
        }
    }
}

/// Classify an import by its host name.
pub fn classify_import(qualified_name: &str) -> CostClass {
    if qualified_name.contains("contract_data") || qualified_name.contains("contract_code") {
        if qualified_name.contains("contract_code") {
            CostClass::ContractCode
        } else {
            CostClass::Storage
        }
    } else if qualified_name.contains("contract_event") || qualified_name.contains("log") {
        CostClass::Event
    } else {
        CostClass::Other
    }
}

/// Marks every number in the report as a static estimate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EstimateKind {
    StaticEstimate,
}

impl EstimateKind {
    pub fn as_str(&self) -> &'static str {
        "static_estimate"
    }
}

/// One import's contribution to one exported function.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ImportHeat {
    /// `module.name` as declared in the import section.
    pub import: String,
    pub cost_class: CostClass,
    /// Direct `call` instructions targeting this import.
    pub call_count: u64,
    /// `call_count * import_cost`, under the supplied table.
    pub estimated_cost: u64,
    /// Number of bytes in the import's encoded name, the only size signal a
    /// static pass has. Exposed so the estimate is inspectable rather than a
    /// bare total.
    pub estimated_bytes: u64,
}

/// One row per exported function.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct HostImportHeatRow {
    /// Export name. `None` for a function that is not exported, which the
    /// report still lists so an unattributed cost is not invisible.
    pub export_name: Option<String>,
    pub function_index: u32,
    /// Every instruction in the body, the base the estimate is built on.
    pub instruction_count: u64,
    /// Direct calls to host imports.
    pub host_call_count: u64,
    /// `call_indirect` instructions. Counted, never resolved to a target.
    pub indirect_call_count: u64,
    /// Direct calls to *defined* functions, which cost dispatch but no host work.
    pub internal_call_count: u64,
    /// Static CPU estimate for this function.
    pub estimated_cpu: u64,
    pub estimate_kind: EstimateKind,
    /// Full per-import breakdown.
    pub imports: Vec<ImportHeat>,
    /// The three imports with the highest `call_count`, ties broken by cost.
    pub top_imports: Vec<ImportHeat>,
}

/// The whole heat map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct HostImportHeatMap {
    pub contract_name: String,
    pub estimate_kind: EstimateKind,
    /// Present on every report: an estimate is not a budget.
    pub caveat: String,
    pub total_host_imports: usize,
    pub rows: Vec<HostImportHeatRow>,
}

impl HostImportHeatMap {
    /// The row for an export, if present.
    pub fn row(&self, export_name: &str) -> Option<&HostImportHeatRow> {
        self.rows.iter().find(|r| r.export_name.as_deref() == Some(export_name))
    }
}

fn import_estimated_bytes(qualified_name: &str) -> u64 {
    qualified_name.len() as u64
}

/// Build the heat map for a module under a cost-parameter table.
pub fn build_host_import_heat_map(
    wasm_bytes: &[u8],
    contract_name: &str,
    params: &ContractCostParams,
) -> Result<HostImportHeatMap, parsed_module::ParseFailure> {
    let module = parsed_module::parse_module(wasm_bytes)?;
    Ok(heat_map_from_parsed(&module, contract_name, params))
}

/// Build the heat map from an already-parsed module.
pub fn heat_map_from_parsed(
    module: &ParsedModule,
    contract_name: &str,
    params: &ContractCostParams,
) -> HostImportHeatMap {
    let mut rows = Vec::new();

    for function in &module.functions {
        // import index -> (calls, class, qualified name)
        let mut per_import: HashMap<u32, (u64, u64)> = HashMap::new();
        let mut indirect_call_count: u64 = 0;
        let mut internal_call_count: u64 = 0;

        for op in &function.operators {
            match op {
                Operator::Call { function_index } => match module.host_import(*function_index) {
                    Some(import) => {
                        let entry = per_import.entry(import.index).or_insert((0, 0));
                        entry.0 += 1;
                    }
                    None => internal_call_count += 1,
                },
                Operator::CallIndirect { .. } => indirect_call_count += 1,
                _ => {}
            }
        }

        if per_import.is_empty() && indirect_call_count == 0 && internal_call_count == 0 {
            // Nothing call-shaped: not a row worth reporting.
            continue;
        }

        let mut imports: Vec<ImportHeat> = per_import
            .iter()
            .filter_map(|(import_index, (call_count, _))| {
                let import = module.host_imports.get(*import_index as usize)?;
                let qualified = import.qualified();
                Some(ImportHeat {
                    import: qualified.clone(),
                    cost_class: classify_import(&qualified),
                    call_count: *call_count,
                    estimated_cost: *call_count * params.import_cost(&qualified),
                    estimated_bytes: import_estimated_bytes(&qualified),
                })
            })
            .collect();

        // Rank by call count, then by estimated cost, then by name for
        // determinism: a report that reorders between runs is not reviewable.
        imports.sort_by(|a, b| {
            b.call_count
                .cmp(&a.call_count)
                .then_with(|| b.estimated_cost.cmp(&a.estimated_cost))
                .then_with(|| a.import.cmp(&b.import))
        });

        let host_call_count: u64 = imports.iter().map(|i| i.call_count).sum();
        let host_estimate: u64 = imports.iter().map(|i| i.estimated_cost).sum();
        let instruction_count = function.operators.len() as u64;
        // Internal calls and indirect calls still consume instructions; charge
        // them the instruction cost rather than pretending they are free.
        let internal_estimate =
            (internal_call_count + indirect_call_count) * params.instruction_cost;
        let instruction_estimate = instruction_count * params.instruction_cost;

        rows.push(HostImportHeatRow {
            export_name: function.export_name.clone(),
            function_index: function.index,
            instruction_count,
            host_call_count,
            indirect_call_count,
            internal_call_count,
            estimated_cpu: params.base_cpu_cost + host_estimate + instruction_estimate + internal_estimate,
            estimate_kind: EstimateKind::StaticEstimate,
            top_imports: imports.iter().take(3).cloned().collect(),
            imports,
        });
    }

    // Stable ordering: exported functions first by name, then internal by index.
    rows.sort_by(|a, b| match (&a.export_name, &b.export_name) {
        (Some(x), Some(y)) => x.cmp(y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.function_index.cmp(&b.function_index),
    });

    HostImportHeatMap {
        contract_name: contract_name.to_string(),
        estimate_kind: EstimateKind::StaticEstimate,
        caveat: "static_estimate: derived from decoded instructions and the supplied cost-parameter \
                 table. Not a budget and not a substitute for cost_breakdown, which requires a \
                 simulation."
            .to_string(),
        total_host_imports: module.host_imports.len(),
        rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A module with two host imports and one export that calls each twice and
    // once respectively. Built byte by byte so the counts are unambiguous.
    fn two_import_module() -> Vec<u8> {
        let mut wasm = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        // Type: () -> ()
        wasm.extend_from_slice(&[0x01, 0x04, 0x01, 0x60, 0x00, 0x00]);
        // Two function imports, indices 0 and 1.
        let mut imports = vec![0x02, 0x03, b'e', b'n', b'v'];
        for name in ["put_contract_data", "get_contract_data"] {
            imports.push(name.len() as u8);
            imports.extend_from_slice(name.as_bytes());
            imports.extend_from_slice(&[0x00, 0x00]);
        }
        wasm.push(0x02);
        wasm.push(imports.len() as u8);
        wasm.extend_from_slice(&imports);
        // One defined function (index 2).
        wasm.extend_from_slice(&[0x03, 0x02, 0x01, 0x00]);
        // Export it as "work".
        let mut export = vec![0x01, 0x04];
        export.extend_from_slice(b"work");
        export.extend_from_slice(&[0x00, 0x02]);
        wasm.push(0x07);
        wasm.push(export.len() as u8);
        wasm.extend_from_slice(&export);
        // Body: call 0, call 0, call 1, end.
        let body = vec![0x00, 0x10, 0x00, 0x10, 0x00, 0x10, 0x01, 0x0b];
        let mut section = vec![0x01, body.len() as u8];
        section.extend_from_slice(&body);
        wasm.push(0x0a);
        wasm.push(section.len() as u8);
        wasm.extend_from_slice(&section);
        wasm
    }

    #[test]
    fn counts_direct_calls_per_export() {
        let map = build_host_import_heat_map(&two_import_module(), "two_imports", &ContractCostParams::free())
            .expect("module parses");

        assert_eq!(map.total_host_imports, 2);
        let row = map.row("work").expect("a row for the export");
        assert_eq!(row.host_call_count, 3);
        assert_eq!(row.indirect_call_count, 0);
        assert_eq!(row.internal_call_count, 0);
        assert_eq!(row.imports.len(), 2);
    }

    #[test]
    fn ranks_the_most_called_import_first_and_caps_at_three() {
        let map = build_host_import_heat_map(&two_import_module(), "two_imports", &ContractCostParams::free())
            .expect("module parses");
        let row = map.row("work").expect("a row");
        assert_eq!(row.top_imports[0].import, "env.put_contract_data");
        assert_eq!(row.top_imports[0].call_count, 2);
        assert!(row.top_imports.len() <= 3);
    }

    // "storage_heavy ranks a storage import above arithmetic" — expressed
    // against a module whose heaviest import is a storage write.
    #[test]
    fn a_storage_import_outranks_an_arithmetic_only_function() {
        let params = ContractCostParams::network_default();
        let map = build_host_import_heat_map(&two_import_module(), "storage_heavy_like", &params)
            .expect("module parses");
        let row = map.row("work").expect("a row");

        assert_eq!(row.top_imports[0].cost_class, CostClass::Storage);
        assert!(
            row.top_imports[0].estimated_cost >= row.top_imports[1].estimated_cost,
            "a storage write must not be priced below the read beside it"
        );
    }

    // The issue requires estimates to react to the table, not to be constants.
    #[test]
    fn estimates_change_when_the_cost_table_is_swapped() {
        let wasm = two_import_module();
        let free = build_host_import_heat_map(&wasm, "c", &ContractCostParams::free()).expect("parses");
        let priced =
            build_host_import_heat_map(&wasm, "c", &ContractCostParams::network_default()).expect("parses");

        let free_cpu = free.row("work").expect("row").estimated_cpu;
        let priced_cpu = priced.row("work").expect("row").estimated_cpu;
        assert_ne!(free_cpu, priced_cpu, "swapping the table must move the estimate");
        assert!(priced_cpu > free_cpu, "a real table must cost more than a free one");
    }

    #[test]
    fn a_free_table_prices_everything_at_zero() {
        let map = build_host_import_heat_map(&two_import_module(), "free", &ContractCostParams::free())
            .expect("parses");
        let row = map.row("work").expect("row");
        assert_eq!(row.estimated_cpu, 0);
        assert!(row.imports.iter().all(|i| i.estimated_cost == 0));
    }

    #[test]
    fn every_row_is_labelled_a_static_estimate() {
        let map = build_host_import_heat_map(&two_import_module(), "labelled", &ContractCostParams::network_default())
            .expect("parses");

        assert_eq!(map.estimate_kind, EstimateKind::StaticEstimate);
        assert_eq!(map.estimate_kind.as_str(), "static_estimate");
        assert!(map.rows.iter().all(|r| r.estimate_kind == EstimateKind::StaticEstimate));
    }

    // "The report says estimates are not a substitute for cost_breakdown."
    #[test]
    fn the_report_states_it_is_not_a_cost_breakdown() {
        let map = build_host_import_heat_map(&two_import_module(), "caveat", &ContractCostParams::network_default())
            .expect("parses");

        assert!(map.caveat.contains("static_estimate"));
        assert!(map.caveat.contains("cost_breakdown"));
        assert!(map.caveat.contains("requires a simulation"));
    }

    #[test]
    fn indirect_calls_are_counted_but_never_resolved_to_a_target() {
        // One import, one direct call, then a call_indirect.
        let mut wasm = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        wasm.extend_from_slice(&[0x01, 0x04, 0x01, 0x60, 0x00, 0x00]);
        let mut import = vec![0x01, 0x03, b'e', b'n', b'v', 0x0b];
        import.extend_from_slice(b"log_topic");
        import.extend_from_slice(&[0x00, 0x00]);
        wasm.push(0x02);
        wasm.push(import.len() as u8);
        wasm.extend_from_slice(&import);
        wasm.extend_from_slice(&[0x03, 0x02, 0x01, 0x00]);
        let mut export = vec![0x01, 0x03];
        export.extend_from_slice(b"run");
        export.extend_from_slice(&[0x00, 0x01]);
        wasm.push(0x07);
        wasm.push(export.len() as u8);
        wasm.extend_from_slice(&export);
        // call 0; call_indirect { type 0, table 0 } = 0x11 0x00 0x00; end
        let body = vec![0x00, 0x10, 0x00, 0x11, 0x00, 0x00, 0x0b];
        let mut section = vec![0x01, body.len() as u8];
        section.extend_from_slice(&body);
        wasm.push(0x0a);
        wasm.push(section.len() as u8);
        wasm.extend_from_slice(&section);

        let map = build_host_import_heat_map(&wasm, "indirect", &ContractCostParams::network_default())
            .expect("parses");
        let row = map.row("run").expect("a row");

        assert_eq!(row.indirect_call_count, 1, "the indirect call must be counted");
        assert_eq!(row.host_call_count, 1, "only the direct call is a host call");
        // The indirect call must not have been attributed to any import.
        assert_eq!(row.imports.len(), 1);
        assert_eq!(row.imports[0].call_count, 1);
    }

    #[test]
    fn a_body_with_no_calls_produces_no_row() {
        let mut wasm = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        wasm.extend_from_slice(&[0x01, 0x04, 0x01, 0x60, 0x00, 0x00]);
        wasm.extend_from_slice(&[0x03, 0x02, 0x01, 0x00]);
        let mut export = vec![0x01, 0x02];
        export.extend_from_slice(b"id");
        export.extend_from_slice(&[0x00, 0x00]);
        wasm.push(0x07);
        wasm.push(export.len() as u8);
        wasm.extend_from_slice(&export);
        // Just nop, end.
        let body = vec![0x00, 0x01, 0x0b];
        let mut section = vec![0x01, body.len() as u8];
        section.extend_from_slice(&body);
        wasm.push(0x0a);
        wasm.push(section.len() as u8);
        wasm.extend_from_slice(&section);

        let map = build_host_import_heat_map(&wasm, "id_only", &ContractCostParams::network_default())
            .expect("parses");
        assert!(map.rows.is_empty(), "a function with no calls is not a heat-map row");
    }

    #[test]
    fn storage_reads_and_writes_are_priced_above_other_imports() {
        let params = ContractCostParams::network_default();
        let put = params.import_cost("env.put_contract_data");
        let get = params.import_cost("env.get_contract_data");
        let other = params.import_cost("env.some_arithmetic_or_math_helper");

        assert!(put > get, "a write must cost more than a read");
        assert!(get > other, "a ledger read must cost more than a plain import");
    }

    #[test]
    fn invalid_wasm_is_rejected_rather_than_reported_as_zero_calls() {
        assert!(build_host_import_heat_map(b"nope", "bad", &ContractCostParams::free()).is_err());
    }
}

/// Tests against the checked-in contract fixtures (issue #1008).
///
/// `storage_heavy` and `cpu_heavy` are separate crates built for `wasm32`, so
/// their artifacts only exist after a `cargo build --target wasm32-unknown-
/// unknown`. The repo already gates contract-artifact tests this way
/// (`runner/local.rs` skips when its hello-world WASM is absent), so these skip
/// identically rather than failing a fresh clone.
#[cfg(test)]
mod contract_fixture_tests {
    use super::*;

    fn fixture(path: &str) -> Option<Vec<u8>> {
        let full = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("contracts")
            .join(path);
        std::fs::read(full).ok().filter(|bytes| !bytes.is_empty())
    }

    /// `storage_heavy` is dominated by ledger writes, so its heaviest import
    /// must be a storage one and must outrank a plain arithmetic helper.
    #[test]
    fn storage_heavy_ranks_a_storage_import_first() {
        let Some(wasm) = fixture("storage_heavy/target/wasm32-unknown-unknown/release/storage-heavy.wasm") else {
            eprintln!("storage_heavy artifact not built; skipping");
            return;
        };

        let map =
            build_host_import_heat_map(&wasm, "storage_heavy", &ContractCostParams::network_default())
                .expect("storage_heavy parses");
        assert!(map.total_host_imports > 0, "a storage contract must import host functions");

        let heaviest = map
            .rows
            .iter()
            .flat_map(|r| r.top_imports.iter())
            .max_by_key(|i| (i.cost_class == CostClass::Storage, i.call_count, i.estimated_cost))
            .expect("at least one import somewhere in the module");

        assert_eq!(
            heaviest.cost_class,
            CostClass::Storage,
            "storage_heavy must rank a storage import above anything else, got {heaviest:?}"
        );
    }

    /// `cpu_heavy` is arithmetic, so it has comparatively few host imports. The
    /// assertion is about the shape of the report, not a magic number: a
    /// cpu-bound contract should not present a storage-dominated heat map.
    #[test]
    fn cpu_heavy_is_not_storage_dominated() {
        let Some(wasm) = fixture("cpu_heavy/target/wasm32-unknown-unknown/release/cpu_heavy.wasm") else {
            eprintln!("cpu_heavy artifact not built; skipping");
            return;
        };

        let map = build_host_import_heat_map(&wasm, "cpu_heavy", &ContractCostParams::network_default())
            .expect("cpu_heavy parses");

        let total_calls: u64 = map.rows.iter().map(|r| r.host_call_count).sum();
        let storage_calls: u64 = map
            .rows
            .iter()
            .flat_map(|r| r.imports.iter())
            .filter(|i| i.cost_class == CostClass::Storage)
            .map(|i| i.call_count)
            .sum();

        assert!(
            storage_calls < total_calls,
            "a cpu-bound contract should not be storage-dominated ({storage_calls} of {total_calls})"
        );
    }

    /// Both fixtures must produce a report whose estimates are labelled.
    #[test]
    fn both_fixtures_report_static_estimates() {
        for (name, path) in [
            ("storage_heavy", "storage_heavy/target/wasm32-unknown-unknown/release/storage-heavy.wasm"),
            ("cpu_heavy", "cpu_heavy/target/wasm32-unknown-unknown/release/cpu_heavy.wasm"),
        ] {
            let Some(wasm) = fixture(path) else {
                eprintln!("{name} artifact not built; skipping");
                continue;
            };
            let map = build_host_import_heat_map(&wasm, name, &ContractCostParams::network_default())
                .expect("fixture parses");
            assert_eq!(map.estimate_kind, EstimateKind::StaticEstimate);
            assert!(map.caveat.contains("cost_breakdown"));
        }
    }
}
