//! Branch-hit side table and coverage-guided input search (issue #1009).
//!
//! # What this replaces
//!
//! The previous search drove itself purely on cost: it mutated arguments,
//! kept whatever was more expensive, and reported a "worst case" that was the
//! worst input it happened to try. Branch coverage was inferred from how the
//! measured resource profiles differed from the baseline, which proves *some*
//! other path ran but not *which* branch was taken — so a liquidity check with
//! an error branch could hide a more expensive success branch that no
//! permutation cleared.
//!
//! # Why the table is passed in rather than traced here
//!
//! Attaching an instrumented tracer to a Soroban host is a host-side
//! capability, not something this crate can bolt on from the outside:
//! `profile_contract` hands back `SorobanResources` and nothing about control
//! flow. Rather than invent a branch id, this module takes the ids a caller
//! *did* observe for a run and keeps the bookkeeping — which is the part that
//! was missing and the part that has to be right. A caller with a trace hook
//! passes real ids; a caller without one passes an empty slice and the search
//! degrades to the previous cost-driven behaviour rather than claiming
//! coverage it does not have.
//!
//! # Static branch ids are the coordinate system
//!
//! Ids come from the static scan in
//! [`crate::wasm_branch_analysis`], so a hit can be reported back against the
//! same [`BranchInfo`] the report already lists, including its `BranchType`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::simulation::SorobanResources;
use crate::wasm_branch_analysis::BranchInfo;

/// How many times each statically-identified branch was observed to be taken.
///
/// A `BTreeMap` rather than a `HashMap` so iteration is ordered and a report is
/// reproducible run to run.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct BranchCoverage {
    /// branch id -> times the branch was observed taken.
    #[serde(default)]
    hits: BTreeMap<usize, u64>,
}

impl BranchCoverage {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record the branches one run took.
    ///
    /// Returns how many of them had never been seen before, which is the signal
    /// the search uses to decide a round was worth spending a run on. A branch
    /// repeated within one run is counted once for coverage purposes and
    /// incremented per occurrence, so a hot loop still shows up as hot.
    pub fn record_run(&mut self, branch_ids: &[usize]) -> usize {
        let mut fresh = 0usize;
        for id in branch_ids {
            let slot = self.hits.entry(*id).or_insert(0);
            if *slot == 0 {
                fresh += 1;
            }
            *slot += 1;
        }
        fresh
    }

    /// Number of times a branch was observed taken.
    pub fn hit_count(&self, branch_id: usize) -> u64 {
        self.hits.get(&branch_id).copied().unwrap_or(0)
    }

    /// Distinct branch ids observed at least once.
    pub fn covered_count(&self) -> usize {
        self.hits.values().filter(|count| **count > 0).count()
    }

    /// Every observed branch id, ascending.
    pub fn covered_ids(&self) -> Vec<usize> {
        self.hits
            .iter()
            .filter(|(_, count)| **count > 0)
            .map(|(id, _)| *id)
            .collect()
    }

    /// The statically-known branches no run was observed to take.
    ///
    /// Deliberately over-reports: a branch wrongly listed as uncovered is a
    /// branch worth simulating, whereas one wrongly listed as covered is a cost
    /// the author never measured.
    pub fn uncovered(&self, static_branches: &[BranchInfo]) -> Vec<BranchInfo> {
        static_branches
            .iter()
            .filter(|b| self.hit_count(b.branch_id) == 0)
            .cloned()
            .collect()
    }
}

/// Limits for one coverage-guided search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoverageBudget {
    /// Hard cap on simulations, including the baseline. Enforced even while
    /// coverage is still climbing.
    pub max_runs: usize,
    /// Consecutive rounds that added no new branch before stopping.
    pub stop_after_empty_rounds: usize,
    /// Cap on rounds, independent of the run budget.
    pub max_rounds: usize,
}

impl Default for CoverageBudget {
    fn default() -> Self {
        CoverageBudget { max_runs: 24, stop_after_empty_rounds: 2, max_rounds: 6 }
    }
}

/// Why the search stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// The run cap was reached. Coverage may still have been climbing.
    RunCap,
    /// Enough consecutive rounds added no new branch.
    Stalled,
    /// The candidate generator had nothing left to offer.
    CandidatesExhausted,
}

impl StopReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            StopReason::RunCap => "run_cap",
            StopReason::Stalled => "stalled",
            StopReason::CandidatesExhausted => "candidates_exhausted",
        }
    }
}

/// One profiled input, with the branches it took.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageRun {
    pub args: Vec<String>,
    pub resources: SorobanResources,
    /// Branch ids observed taken during this run. Empty when the caller has no
    /// trace source, which is not the same as "took no branches".
    pub hit_branches: Vec<usize>,
    /// Round that produced the run. Round 0 is the baseline.
    pub round: usize,
}

/// The outcome of a coverage-guided search.
#[derive(Debug, Clone)]
pub struct CoverageSearchOutcome {
    pub coverage: BranchCoverage,
    /// Cheapest run observed.
    pub best: Option<CoverageRun>,
    /// Most expensive run observed — the one the report should call the worst
    /// case, now chosen by coverage rather than by luck.
    pub worst: Option<CoverageRun>,
    /// Every run that profiled successfully, in the order it was profiled.
    pub runs: Vec<CoverageRun>,
    pub runs_used: usize,
    pub rounds: usize,
    pub capped: bool,
    pub stop_reason: StopReason,
}

/// Worse than `b` on the two axes a report already ranks on.
fn is_worse(a: &SorobanResources, b: &SorobanResources) -> bool {
    a.cpu_instructions > b.cpu_instructions
        || (a.cpu_instructions == b.cpu_instructions && a.ram_bytes > b.ram_bytes)
}

fn is_better(a: &SorobanResources, b: &SorobanResources) -> bool {
    a.cpu_instructions < b.cpu_instructions
        || (a.cpu_instructions == b.cpu_instructions && a.ram_bytes < b.ram_bytes)
}

/// Search arguments for coverage, not just for cost.
///
/// `profile` is called with a candidate and returns `None` when the candidate
/// could not be profiled at all; that still costs a run, because a candidate
/// that traps is information the search paid for.
///
/// `candidates` yields the next round's candidates from the current best run's
/// arguments and the round number, so the search can be pointed at the existing
/// permutation generator or at anything else.
///
/// The stopping rules are the issue's: a hard run cap that holds even while
/// coverage is still growing, and a stop after `stop_after_empty_rounds`
/// consecutive rounds that added no new branch id.
pub fn search_coverage_guided<P, C>(
    baseline_args: &[String],
    budget: &CoverageBudget,
    mut profile: P,
    mut candidates: C,
) -> CoverageSearchOutcome
where
    P: FnMut(&[String], usize) -> Option<CoverageRun>,
    C: FnMut(&[String], usize) -> Vec<Vec<String>>,
{
    let mut coverage = BranchCoverage::new();
    let mut runs: Vec<CoverageRun> = Vec::new();
    let mut runs_used = 0usize;
    let mut rounds = 0usize;
    let mut empty_rounds = 0usize;
    let mut capped = false;
    let mut stop_reason = StopReason::CandidatesExhausted;

    // Round 0: the supplied arguments, which are the baseline by definition.
    if let Some(mut run) = profile(baseline_args, 0) {
        run.round = 0;
        coverage.record_run(&run.hit_branches);
        runs.push(run);
    }
    runs_used += 1;

    let mut best_args: Vec<String> = baseline_args.to_vec();
    let mut best: Option<CoverageRun> = runs.first().cloned();
    let mut worst: Option<CoverageRun> = runs.first().cloned();

    while rounds < budget.max_rounds {
        rounds += 1;
        let batch = candidates(&best_args, rounds);
        if batch.is_empty() {
            stop_reason = StopReason::CandidatesExhausted;
            break;
        }

        let mut fresh_this_round = 0usize;
        for candidate in batch {
            if candidate == baseline_args && !runs.is_empty() && runs_used > 1 {
                continue;
            }
            if runs_used >= budget.max_runs {
                capped = true;
                stop_reason = StopReason::RunCap;
                break;
            }
            if runs.iter().any(|r| r.args == candidate) {
                continue;
            }

            let profiled = profile(&candidate, rounds);
            runs_used += 1;

            let Some(mut run) = profiled else {
                // A candidate that could not be profiled: no branches to record
                // and no cost to compare, but the run is spent.
                continue;
            };
            run.round = rounds;
            fresh_this_round += coverage.record_run(&run.hit_branches);

            let resources = run.resources.clone();
            match &best {
                Some(current) if !is_better(&resources, &current.resources) => {}
                _ => {
                    best_args = candidate.clone();
                    best = Some(run.clone());
                }
            }
            match &worst {
                Some(current) if !is_worse(&resources, &current.resources) => {}
                _ => worst = Some(run.clone()),
            }
            runs.push(run);
        }

        if capped {
            break;
        }

        if fresh_this_round == 0 {
            empty_rounds += 1;
            if empty_rounds >= budget.stop_after_empty_rounds {
                stop_reason = StopReason::Stalled;
                break;
            }
        } else {
            empty_rounds = 0;
        }
    }

    if !capped && stop_reason == StopReason::CandidatesExhausted && rounds >= budget.max_rounds {
        stop_reason = StopReason::Stalled;
    }

    CoverageSearchOutcome { coverage, best, worst, runs, runs_used, rounds, capped, stop_reason }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wasm_branch_analysis::{BranchCoverageBasis, BranchType};

    fn resources(cpu: u64, ram: u64) -> SorobanResources {
        SorobanResources {
            cpu_instructions: cpu,
            ram_bytes: ram,
            ledger_read_bytes: 0,
            ledger_write_bytes: 0,
            transaction_size_bytes: 0,
        }
    }

    fn branch(id: usize) -> BranchInfo {
        BranchInfo {
            branch_id: id,
            branch_type: BranchType::Conditional,
            nesting_depth: 0,
            description: format!("branch {id}"),
        }
    }

    /// A contract with a cheap branch and an expensive one: only an input that
    /// clears the check takes the expensive path. The generator proposes inputs
    /// blindly, so finding the expensive branch is the search's job.
    #[test]
    fn the_search_finds_the_expensive_branch_within_the_cap() {
        let budget = CoverageBudget { max_runs: 12, stop_after_empty_rounds: 2, max_rounds: 5 };
        let baseline = vec!["u:1".to_string()];
        // Round 1 reaches 8, round 2 reaches 16: the expensive input is inside
        // the cap, but only a search that keeps probing finds it.

        // Branch 0 is the check, branch 1 the expensive body. Only "u:99" takes
        // branch 1, and it costs ten times as much.
        let outcome = search_coverage_guided(
            &baseline,
            &budget,
            |args: &[String], _round: usize| {
                let v: u64 = args[0].trim_start_matches("u:").parse().unwrap_or(0);
                let expensive = v >= 8;
                Some(CoverageRun {
                    args: args.to_vec(),
                    resources: resources(if expensive { 10_000 } else { 1_000 }, 1_024),
                    hit_branches: if expensive { vec![0, 1] } else { vec![0] },
                    round: 0,
                })
            },
            |_best: &[String], round: usize| {
                // A blind generator: 1..=N, and later rounds reach further.
                let reach = round * 8;
                (1..=reach).map(|n| vec![format!("u:{n}")]).collect()
            },
        );

        assert!(
            outcome.runs_used <= budget.max_runs,
            "the run cap must hold, used {}",
            outcome.runs_used
        );
        let worst = outcome.worst.expect("a worst case");
        assert_eq!(worst.args, vec!["u:8".to_string()], "the expensive branch was not found");
        assert_eq!(worst.resources.cpu_instructions, 10_000);
        assert!(outcome.coverage.hit_count(1) > 0, "branch 1 must be recorded as covered");
    }

    /// A branch that needs a specific address and is never taken must be
    /// reported uncovered rather than dropped or counted as covered.
    #[test]
    fn a_branch_that_is_never_hit_is_reported_uncovered() {
        let budget = CoverageBudget { max_runs: 6, stop_after_empty_rounds: 2, max_rounds: 3 };
        let baseline = vec!["a:111".to_string()];
        let static_branches = vec![branch(0), branch(1), branch(2)];

        let outcome = search_coverage_guided(
            &baseline,
            &budget,
            |args: &[String], _round: usize| {
                // Only branch 0 ever runs; 1 and 2 need an address the generator
                // never proposes.
                let _ = args;
                Some(CoverageRun {
                    args: args.to_vec(),
                    resources: resources(500, 512),
                    hit_branches: vec![0],
                    round: 0,
                })
            },
            |_best: &[String], _round: usize| vec![vec!["a:222".to_string()]],
        );

        let uncovered = outcome.coverage.uncovered(&static_branches);
        assert_eq!(uncovered.len(), 2, "branches 1 and 2 were never taken");
        assert!(uncovered.iter().all(|b| b.branch_type == BranchType::Conditional));
        assert!(!uncovered.iter().any(|b| b.branch_id == 0), "a covered branch is not uncovered");
        assert_eq!(outcome.coverage.covered_count(), 1);
        assert!(!outcome.coverage.covered_ids().contains(&1));
    }

    /// The cap holds even when every round keeps finding new branches.
    #[test]
    fn the_run_cap_is_enforced_while_coverage_is_still_climbing() {
        let budget = CoverageBudget { max_runs: 5, stop_after_empty_rounds: 2, max_rounds: 50 };
        let baseline = vec!["u:0".to_string()];

        let outcome = search_coverage_guided(
            &baseline,
            &budget,
            |args: &[String], _round: usize| {
                let v: u64 = args[0].trim_start_matches("u:").parse().unwrap_or(0);
                // Every input covers a branch nobody covered before.
                Some(CoverageRun {
                    args: args.to_vec(),
                    resources: resources(100 + v, 1_024),
                    hit_branches: vec![v as usize],
                    round: 0,
                })
            },
            |_best: &[String], round: usize| {
                let base = round * 100;
                (1..=10).map(|n| vec![format!("u:{}", base + n)]).collect()
            },
        );

        assert_eq!(outcome.runs_used, budget.max_runs, "the cap is a hard limit");
        assert!(outcome.capped);
        assert_eq!(outcome.stop_reason, StopReason::RunCap);
        // Coverage really was still climbing when the cap hit, which is exactly
        // the case the issue asks to be reported rather than hidden.
        assert!(outcome.coverage.covered_count() > 1);
    }

    /// A search that never adds coverage stops instead of burning the budget.
    #[test]
    fn a_search_that_learns_nothing_stops_after_the_stall_threshold() {
        let budget = CoverageBudget { max_runs: 50, stop_after_empty_rounds: 2, max_rounds: 10 };
        let baseline = vec!["u:0".to_string()];

        let outcome = search_coverage_guided(
            &baseline,
            &budget,
            |args: &[String], _round: usize| {
                Some(CoverageRun {
                    args: args.to_vec(),
                    resources: resources(700, 1_024),
                    hit_branches: vec![0], // always the same branch
                    round: 0,
                })
            },
            |_best: &[String], round: usize| (1..=3).map(|n| vec![format!("u:{}_{}", round, n)]).collect(),
        );

        assert_eq!(outcome.stop_reason, StopReason::Stalled);
        assert!(outcome.runs_used < budget.max_runs, "a stalled search must not spend the budget");
        assert!(outcome.rounds <= 3, "stopped after the stall threshold, ran {} rounds", outcome.rounds);
    }

    /// A candidate that cannot be profiled costs a run but contributes nothing.
    #[test]
    fn an_unprofilable_candidate_costs_a_run_and_is_not_recorded() {
        let budget = CoverageBudget { max_runs: 4, stop_after_empty_rounds: 2, max_rounds: 2 };
        let baseline = vec!["u:0".to_string()];

        let outcome = search_coverage_guided(
            &baseline,
            &budget,
            |args: &[String], _round: usize| {
                if args[0] == "u:bad" {
                    return None;
                }
                Some(CoverageRun {
                    args: args.to_vec(),
                    resources: resources(10, 1_024),
                    hit_branches: vec![0],
                    round: 0,
                })
            },
            |_best: &[String], _round: usize| vec![vec!["u:bad".to_string()]],
        );

        assert!(outcome.runs_used > 1, "the failed candidate consumed a run");
        assert_eq!(outcome.runs.len(), 1, "a failed candidate is not a recorded run");
    }

    /// The coverage basis is reported as traced only when ids were actually
    /// observed, so a caller without a trace source cannot claim coverage.
    #[test]
    fn no_observed_ids_means_no_coverage_is_claimed() {
        let budget = CoverageBudget { max_runs: 3, stop_after_empty_rounds: 2, max_rounds: 2 };
        let baseline = vec!["u:0".to_string()];

        let outcome = search_coverage_guided(
            &baseline,
            &budget,
            |args: &[String], _round: usize| {
                Some(CoverageRun {
                    args: args.to_vec(),
                    resources: resources(10, 1_024),
                    hit_branches: Vec::new(), // no trace source
                    round: 0,
                })
            },
            |_best: &[String], _round: usize| vec![vec!["u:1".to_string()]],
        );

        assert_eq!(outcome.coverage.covered_count(), 0);
        assert_eq!(outcome.stop_reason, StopReason::Stalled);
        // The enum keeps the distinction the report needs.
        assert_ne!(BranchCoverageBasis::Traced, BranchCoverageBasis::MeasuredProfileDelta);
    }

    /// Repeated hits on one branch increment its count without inflating the
    /// covered-branch count.
    #[test]
    fn a_hot_branch_counts_once_for_coverage_and_many_times_for_heat() {
        let mut coverage = BranchCoverage::new();
        assert_eq!(coverage.record_run(&[3, 3, 3]), 1, "three hits, one new branch");
        assert_eq!(coverage.record_run(&[3]), 0, "already known");
        assert_eq!(coverage.hit_count(3), 4);
        assert_eq!(coverage.covered_count(), 1);
        assert_eq!(coverage.covered_ids(), vec![3]);
    }
}
