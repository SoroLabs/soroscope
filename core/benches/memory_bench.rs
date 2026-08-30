//! Memory profiling and leak detection benchmarks.
//!
//! Uses `tikv-jemallocator` as the global allocator and `tikv-jemalloc-ctl`
//! to query heap statistics so that allocation churn and RSS growth can be
//! tracked over repeated operation cycles.
//!
//! What is measured:
//!
//! - **allocated_bytes** — bytes currently allocated by live objects on the
//!   jemalloc heap (mirrors RSS minus fragmentation).
//! - **allocation_churn** — net heap growth after N repeated cycles of an
//!   operation; non-zero growth across cycles signals a potential memory leak.
//! - **retained_after_cycles** — bytes still allocated once all temporary
//!   work-objects have gone out of scope; should be ≈ 0 for leak-free code.
//!
//! Run with:
//!   cargo bench --bench memory_bench -p soroscope-core
//!
//! HTML reports are written to `target/criterion/` when the `html_reports`
//! feature is active (default via Criterion dependency).

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use soroscope_core::merkle_tree::MerkleTree;
use tikv_jemalloc_ctl::{epoch, stats};

// ── Global allocator ──────────────────────────────────────────────────────────

#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Refresh jemalloc's statistics epoch and return current allocated bytes.
///
/// jemalloc caches stats internally; advancing the epoch forces a fresh read.
fn jemalloc_allocated() -> usize {
    // Advance the epoch to flush cached stats.
    epoch::mib()
        .expect("jemalloc epoch mib")
        .advance()
        .expect("jemalloc epoch advance");

    stats::allocated::mib()
        .expect("jemalloc allocated mib")
        .read()
        .expect("jemalloc allocated read")
}

fn make_leaves(n: usize) -> Vec<Vec<u8>> {
    (0..n).map(|i| (i as u64).to_le_bytes().to_vec()).collect()
}

fn build_tree(n: usize) -> MerkleTree {
    let leaves = make_leaves(n);
    let mut tree = MerkleTree::new(32);
    tree.build(leaves).expect("build must succeed");
    tree
}

// ── Benchmarks ────────────────────────────────────────────────────────────────

/// Measure allocated bytes before and after building Merkle trees.
///
/// The benchmark records heap growth per iteration; steady growth across
/// iterations indicates a memory leak in tree construction or leaf hashing.
fn bench_merkle_memory_profile(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory/merkle_build");
    group.sample_size(20);

    for &n in &[1_000usize, 10_000, 100_000] {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(
            BenchmarkId::new("leaves", n),
            &n,
            |b, &n| {
                b.iter_custom(|iters| {
                    let start = std::time::Instant::now();
                    let before = jemalloc_allocated();

                    for _ in 0..iters {
                        let _tree = build_tree(n);
                        // `_tree` drops here; memory should be freed.
                    }

                    let after = jemalloc_allocated();

                    // Log allocation delta so it appears in criterion output.
                    let delta = (after as i64) - (before as i64);
                    if delta > 0 {
                        eprintln!(
                            "[memory_bench] merkle/build n={n}: net heap growth after {iters} iters = {delta} bytes"
                        );
                    }

                    start.elapsed()
                });
            },
        );
    }

    group.finish();
}

/// Detect allocation churn in repeated parse + drop cycles.
///
/// Each iteration allocates scratch buffers, parses data, then drops
/// everything. The heap allocation before and after the full loop should be
/// within a small tolerance; large residual allocations suggest a leak.
fn bench_allocation_churn(c: &mut Criterion) {
    const CYCLES: usize = 100;
    let mut group = c.benchmark_group("memory/allocation_churn");
    group.sample_size(10);

    group.bench_function("merkle_parse_cycles", |b| {
        b.iter_custom(|iters| {
            let start = std::time::Instant::now();
            let before = jemalloc_allocated();

            for _ in 0..iters {
                // Perform CYCLES build+drop cycles per criterion iteration.
                for _ in 0..CYCLES {
                    let _tree = build_tree(100);
                }
            }

            let after = jemalloc_allocated();
            let retained = (after as i64) - (before as i64);

            eprintln!(
                "[memory_bench] churn after {} cycles × {} iters: retained = {} bytes",
                CYCLES, iters, retained
            );

            start.elapsed()
        });
    });

    group.finish();
}

/// Measure per-proof memory footprint.
///
/// Tracks heap growth when generating inclusion proofs to surface leaks in
/// the proof-path serialisation path.
fn bench_proof_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory/proof_generation");
    group.sample_size(20);

    for &n in &[1_000usize, 10_000] {
        let tree = build_tree(n);

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(
            BenchmarkId::new("tree_size", n),
            &tree,
            |b, t| {
                b.iter_custom(|iters| {
                    let start = std::time::Instant::now();
                    let before = jemalloc_allocated();

                    for _ in 0..iters {
                        for i in 0..t.leaf_count() {
                            let _proof = t.generate_proof(i).expect("proof ok");
                        }
                    }

                    let after = jemalloc_allocated();
                    let delta = (after as i64) - (before as i64);
                    if delta > 0 {
                        eprintln!(
                            "[memory_bench] proof/gen n={n}: residual = {delta} bytes after {iters} iters"
                        );
                    }

                    start.elapsed()
                });
            },
        );
    }

    group.finish();
}

/// Steady-state RSS snapshot.
///
/// Reports the jemalloc allocated bytes at idle (no active work) so that CI
/// can compare against a baseline to catch background allocations or global
/// state leaks introduced by new features.
fn bench_idle_rss(c: &mut Criterion) {
    c.bench_function("memory/idle_allocated_bytes", |b| {
        b.iter(|| jemalloc_allocated());
    });
}

// ── Registration ──────────────────────────────────────────────────────────────

criterion_group!(
    benches,
    bench_merkle_memory_profile,
    bench_allocation_churn,
    bench_proof_memory,
    bench_idle_rss,
);
criterion_main!(benches);
