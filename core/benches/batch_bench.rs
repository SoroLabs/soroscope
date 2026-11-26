//! Sequential vs parallel batch simulation throughput (issue #1131).
//!
//! Measures wall-clock time for running N independent simulations
//! sequentially versus through
//! [`soroscope_core::simulation_service::simulate_batch_concurrent`],
//! which bounds concurrency with a semaphore (`SIM_PARALLELISM`, default 4).
//!
//! Run with:
//!   cargo bench --bench batch_bench -p soroscope-core
//!
//! HTML reports are written to `target/criterion/` when the `html_reports`
//! feature is active (default via Criterion dependency).

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use soroscope_core::simulation_service::simulate_batch_concurrent;
use std::time::Duration;

fn fake_simulation_latency() -> Duration {
    Duration::from_millis(5)
}

fn bench_batch(c: &mut Criterion) {
    let runtime =
        tokio::runtime::Runtime::new().expect("bench tokio runtime should build");
    let mut group = c.benchmark_group("batch_simulation");
    for size in [4u64, 16u64] {
        group.bench_with_input(
            BenchmarkId::new("sequential", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    runtime.block_on(async {
                        for i in 0..size {
                            tokio::time::sleep(fake_simulation_latency()).await;
                            std::hint::black_box(i);
                        }
                    });
                });
            },
        );
        group.bench_with_input(BenchmarkId::new("parallel", size), &size, |b, &size| {
            b.iter(|| {
                runtime.block_on(async {
                    let functions: Vec<String> =
                        (0..size).map(|i| format!("fn{i}")).collect();
                    let results = simulate_batch_concurrent(functions, |_| async {
                        tokio::time::sleep(fake_simulation_latency()).await;
                        1u64
                    })
                    .await;
                    std::hint::black_box(results);
                });
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_batch);
criterion_main!(benches);
