use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::{Builder, Runtime};
use tokio::sync::{Notify, Semaphore};

use crate::errors::AppError;

/// Default number of tasks admitted (queued + running) before new
/// submissions are shed (issue #1409).
pub const DEFAULT_QUEUE_CAPACITY: usize = 1024;
/// Default longest time a task may wait for a worker before it is skipped.
pub const DEFAULT_MAX_QUEUE_WAIT_MS: u64 = 5_000;

/// Why a bounded submission did not run.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PoolError {
    /// The admission queue was full; the task was rejected immediately.
    #[error("worker pool queue is full")]
    ServiceOverloaded,
    /// The task waited longer than `max_queue_wait_ms` and was skipped
    /// before it consumed any CPU.
    #[error("task waited {waited_ms}ms for a worker (limit {limit_ms}ms)")]
    QueueWaitExceeded { waited_ms: u64, limit_ms: u64 },
    /// The task itself panicked or was cancelled.
    #[error("worker task failed: {0}")]
    TaskFailed(String),
}

impl From<PoolError> for AppError {
    fn from(err: PoolError) -> Self {
        match err {
            PoolError::ServiceOverloaded | PoolError::QueueWaitExceeded { .. } => {
                AppError::ServiceOverloaded(err.to_string())
            }
            PoolError::TaskFailed(msg) => AppError::Internal(msg),
        }
    }
}

/// Prometheus handles updated by the bounded worker pool.
#[derive(Clone)]
pub struct WorkerPoolMetrics {
    pub active_tasks: prometheus::IntGauge,
    pub queued_tasks: prometheus::IntGauge,
    pub shed_total: prometheus::IntCounter,
}

/// Admission control shared by every clone of the pool.
struct Backpressure {
    /// One permit per admitted task (queued or running).
    queue_slots: Arc<Semaphore>,
    /// One permit per concurrently running task.
    workers: Arc<Semaphore>,
    max_queue_wait: Duration,
    active: AtomicUsize,
    queued: AtomicUsize,
    shed: AtomicU64,
    metrics: Option<WorkerPoolMetrics>,
}

impl Backpressure {
    fn publish(&self) {
        if let Some(m) = &self.metrics {
            m.active_tasks.set(self.active.load(Ordering::Relaxed) as i64);
            m.queued_tasks.set(self.queued.load(Ordering::Relaxed) as i64);
        }
    }

    fn record_shed(&self) {
        self.shed.fetch_add(1, Ordering::Relaxed);
        if let Some(m) = &self.metrics {
            m.shed_total.inc();
        }
    }
}

/// A dedicated thread pool configuration for heavy contract event processing.
/// This prevents CPU-intensive parsing from blocking the main HTTP async runtime.
#[derive(Clone)]
pub struct EventWorkerPool {
    runtime: Arc<Runtime>,
    backpressure: Arc<Backpressure>,
    /// Set once draining begins; new task submissions are rejected.
    shutting_down: Arc<AtomicBool>,
    /// Notified whenever an in-flight task completes so the drain loop can re-check.
    idle_notify: Arc<Notify>,
    /// Number of tasks currently spawned and not yet finished.
    in_flight: Arc<std::sync::atomic::AtomicUsize>,
}

impl EventWorkerPool {
    /// Initializes a new dedicated Tokio runtime for event processing.
    ///
    /// # Arguments
    /// * `worker_threads` - The number of OS threads to allocate to this pool.
    pub fn new(worker_threads: usize) -> std::io::Result<Self> {
        Self::with_backpressure(
            worker_threads,
            worker_threads,
            DEFAULT_QUEUE_CAPACITY,
            DEFAULT_MAX_QUEUE_WAIT_MS,
        )
    }

    /// Initializes the pool with explicit backpressure limits for
    /// [`Self::try_submit_blocking`].
    ///
    /// # Arguments
    /// * `worker_threads` - The number of OS threads to allocate to this pool.
    /// * `max_concurrent` - Bounded tasks allowed to run at once.
    /// * `queue_capacity` - Bounded tasks admitted (queued + running) before
    ///   new submissions fail fast with [`PoolError::ServiceOverloaded`].
    /// * `max_queue_wait_ms` - A task still waiting for a worker after this
    ///   long is skipped with [`PoolError::QueueWaitExceeded`].
    pub fn with_backpressure(
        worker_threads: usize,
        max_concurrent: usize,
        queue_capacity: usize,
        max_queue_wait_ms: u64,
    ) -> std::io::Result<Self> {
        let runtime = Builder::new_multi_thread()
            .worker_threads(worker_threads)
            .thread_name("event-parser-worker")
            .enable_all()
            .build()?;

        Ok(Self {
            runtime: Arc::new(runtime),
            backpressure: Arc::new(Backpressure {
                queue_slots: Arc::new(Semaphore::new(queue_capacity.max(1))),
                workers: Arc::new(Semaphore::new(max_concurrent.max(1))),
                max_queue_wait: Duration::from_millis(max_queue_wait_ms),
                active: AtomicUsize::new(0),
                queued: AtomicUsize::new(0),
                shed: AtomicU64::new(0),
                metrics: None,
            }),
            shutting_down: Arc::new(AtomicBool::new(false)),
            idle_notify: Arc::new(Notify::new()),
            in_flight: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        })
    }

    /// Report `worker_pool_*` metrics through these handles.
    ///
    /// Call before the pool is cloned or shared; clones made earlier keep
    /// reporting without metrics.
    pub fn with_metrics(mut self, metrics: WorkerPoolMetrics) -> Self {
        if let Some(bp) = Arc::get_mut(&mut self.backpressure) {
            bp.metrics = Some(metrics);
            bp.publish();
        }
        self
    }

    /// Bounded tasks currently running.
    pub fn active_tasks(&self) -> usize {
        self.backpressure.active.load(Ordering::Relaxed)
    }

    /// Bounded tasks admitted and waiting for a worker.
    pub fn queued_tasks(&self) -> usize {
        self.backpressure.queued.load(Ordering::Relaxed)
    }

    /// Bounded tasks shed since creation (rejected or skipped).
    pub fn shed_total(&self) -> u64 {
        self.backpressure.shed.load(Ordering::Relaxed)
    }

    /// Submit a blocking (CPU-heavy) task with backpressure.
    ///
    /// Returns `Err(PoolError::ServiceOverloaded)` immediately when the
    /// admission queue is full, so callers can answer HTTP 503 instead of
    /// queueing unbounded work. An admitted task that then waits longer than
    /// `max_queue_wait_ms` for a worker resolves to
    /// `Err(PoolError::QueueWaitExceeded)` without running `func`.
    pub fn try_submit_blocking<F, R>(
        &self,
        func: F,
    ) -> Result<tokio::task::JoinHandle<Result<R, PoolError>>, PoolError>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        let bp = Arc::clone(&self.backpressure);
        let slot = match Arc::clone(&bp.queue_slots).try_acquire_owned() {
            Ok(slot) => slot,
            Err(_) => {
                bp.record_shed();
                return Err(PoolError::ServiceOverloaded);
            }
        };

        let enqueued_at = Instant::now();
        bp.queued.fetch_add(1, Ordering::Relaxed);
        bp.publish();

        let runtime = Arc::clone(&self.runtime);
        Ok(self.runtime.spawn(async move {
            let _slot = slot;
            let worker = Arc::clone(&bp.workers).acquire_owned().await;
            bp.queued.fetch_sub(1, Ordering::Relaxed);

            let waited = enqueued_at.elapsed();
            if waited > bp.max_queue_wait {
                bp.record_shed();
                bp.publish();
                return Err(PoolError::QueueWaitExceeded {
                    waited_ms: waited.as_millis() as u64,
                    limit_ms: bp.max_queue_wait.as_millis() as u64,
                });
            }
            let _worker = worker.map_err(|e| PoolError::TaskFailed(e.to_string()))?;

            bp.active.fetch_add(1, Ordering::Relaxed);
            bp.publish();
            let result = runtime
                .spawn_blocking(func)
                .await
                .map_err(|e| PoolError::TaskFailed(e.to_string()));
            bp.active.fetch_sub(1, Ordering::Relaxed);
            bp.publish();
            result
        }))
    }

    /// Returns `true` once the pool has begun draining and rejects new work.
    pub fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::SeqCst)
    }

    /// Returns the number of tasks currently spawned and not yet finished.
    pub fn in_flight(&self) -> usize {
        self.in_flight.load(Ordering::SeqCst)
    }

    /// Wraps a task future so the in-flight counter is decremented and waiters
    /// are notified when the task finishes (including on panic/cancellation).
    fn track<F>(&self, future: F) -> impl std::future::Future<Output = F::Output>
    where
        F: std::future::Future,
    {
        let in_flight = Arc::clone(&self.in_flight);
        let idle_notify = Arc::clone(&self.idle_notify);
        in_flight.fetch_add(1, Ordering::SeqCst);
        async move {
            let _guard = InFlightGuard {
                in_flight,
                idle_notify,
            };
            future.await
        }
    }

    /// Spawns an async task on the dedicated event worker pool.
    ///
    /// Returns `None` if the pool is already draining and no longer accepts work.
    pub fn spawn<F>(&self, future: F) -> Option<tokio::task::JoinHandle<F::Output>>
    where
        F: std::future::Future + Send + 'static,
        F::Output: Send + 'static,
    {
        if self.is_shutting_down() {
            return None;
        }
        let tracked = self.track(future);
        Some(self.runtime.spawn(tracked))
    }

    /// Spawns a blocking (CPU-heavy) task on the dedicated event worker pool.
    /// Use this for strict, heavy synchronous parsing logic.
    ///
    /// Returns `None` if the pool is already draining and no longer accepts work.
    pub fn spawn_blocking<F, R>(&self, func: F) -> Option<tokio::task::JoinHandle<R>>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        if self.is_shutting_down() {
            return None;
        }
        let tracked = self.track(async move { func() });
        Some(self.runtime.spawn(tracked))
    }

    /// Begins draining the pool: rejects new tasks and waits up to `timeout`
    /// for all in-flight tasks to complete.
    ///
    /// Returns `true` if the pool became fully idle within the timeout, or
    /// `false` if tasks were still running when the deadline elapsed.
    pub async fn drain(&self, timeout: Duration) -> bool {
        self.shutting_down.store(true, Ordering::SeqCst);

        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if self.in_flight() == 0 {
                return true;
            }
            let notified = self.idle_notify.notified();
            // Re-check after registering interest to avoid a lost wakeup.
            if self.in_flight() == 0 {
                return true;
            }
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return self.in_flight() == 0;
            }
            if tokio::time::timeout(remaining, notified).await.is_err() {
                return self.in_flight() == 0;
            }
        }
    }
}

/// Decrements the in-flight counter and wakes drain waiters when dropped.
struct InFlightGuard {
    in_flight: Arc<std::sync::atomic::AtomicUsize>,
    idle_notify: Arc<Notify>,
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
        self.idle_notify.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_pool_initialization() {
        let pool_result = EventWorkerPool::new(2);
        assert!(
            pool_result.is_ok(),
            "Worker pool should initialize successfully"
        );
    }

    #[test]
    fn test_pool_spawns_async_task() {
        let pool = EventWorkerPool::new(2).expect("Failed to create worker pool");

        let result = pool.runtime.block_on(async {
            let handle = pool.spawn(async { 100 + 42 }).expect("pool should accept work");
            handle.await.unwrap()
        });

        assert_eq!(
            result, 142,
            "Async task should execute and return correctly"
        );
    }

    #[test]
    fn test_pool_spawns_blocking_task() {
        let pool = EventWorkerPool::new(2).expect("Failed to create worker pool");

        let result = pool.runtime.block_on(async {
            let handle = pool
                .spawn_blocking(|| {
                    // Simulate a heavy CPU-bound parsing task
                    let mut sum = 0;
                    for i in 1..=1000 {
                        sum += i;
                    }
                    sum
                })
                .expect("pool should accept work");
            handle.await.unwrap()
        });

        assert_eq!(
            result, 500500,
            "Blocking CPU task should compute correctly off-thread"
        );
    }

    #[test]
    fn worker_pool_runs_a_bounded_task() {
        let pool = EventWorkerPool::with_backpressure(2, 2, 4, 1_000).expect("pool");

        let result = pool.runtime.block_on(async {
            pool.try_submit_blocking(|| 7 * 6)
                .expect("admitted")
                .await
                .expect("joined")
        });

        assert_eq!(result, Ok(42));
        assert_eq!(pool.shed_total(), 0);
        assert_eq!(pool.active_tasks(), 0);
        assert_eq!(pool.queued_tasks(), 0);
    }

    #[test]
    fn worker_pool_sheds_new_tasks_when_the_queue_is_full() {
        // One worker, two admission slots: one running, one queued.
        let pool = EventWorkerPool::with_backpressure(2, 1, 2, 60_000).expect("pool");
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();

        let first = pool
            .try_submit_blocking(move || {
                let _ = release_rx.recv();
            })
            .expect("first admitted");
        let second = pool.try_submit_blocking(|| ()).expect("second admitted");

        let third = pool.try_submit_blocking(|| ());
        assert_eq!(third.err(), Some(PoolError::ServiceOverloaded));
        assert_eq!(pool.shed_total(), 1);

        let overloaded: AppError = PoolError::ServiceOverloaded.into();
        assert!(matches!(overloaded, AppError::ServiceOverloaded(_)));

        release_tx.send(()).expect("release first task");
        pool.runtime.block_on(async {
            assert_eq!(first.await.expect("joined"), Ok(()));
            assert_eq!(second.await.expect("joined"), Ok(()));
        });
    }

    #[test]
    fn worker_pool_skips_tasks_that_waited_too_long() {
        // One worker held busy longer than the 20ms queue-wait limit.
        let pool = EventWorkerPool::with_backpressure(2, 1, 8, 20).expect("pool");
        let ran = Arc::new(AtomicUsize::new(0));

        let blocker = pool
            .try_submit_blocking(|| std::thread::sleep(Duration::from_millis(200)))
            .expect("blocker admitted");
        // Give the blocker time to take the only worker.
        std::thread::sleep(Duration::from_millis(50));

        let ran_in_task = Arc::clone(&ran);
        let stale = pool
            .try_submit_blocking(move || {
                ran_in_task.fetch_add(1, Ordering::SeqCst);
            })
            .expect("stale admitted");

        pool.runtime.block_on(async {
            assert_eq!(blocker.await.expect("joined"), Ok(()));
            let outcome = stale.await.expect("joined");
            assert!(matches!(outcome, Err(PoolError::QueueWaitExceeded { limit_ms: 20, .. })));
        });

        assert_eq!(ran.load(Ordering::SeqCst), 0, "a stale task must not run");
        assert_eq!(pool.shed_total(), 1);
    }

    #[test]
    fn worker_pool_publishes_metrics() {
        let metrics = crate::metrics::AppMetrics::new().expect("metrics");
        let pool = EventWorkerPool::with_backpressure(2, 1, 1, 60_000)
            .expect("pool")
            .with_metrics(metrics.worker_pool_metrics());

        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let held = pool
            .try_submit_blocking(move || {
                let _ = release_rx.recv();
            })
            .expect("admitted");
        assert!(pool.try_submit_blocking(|| ()).is_err());
        assert_eq!(metrics.worker_pool_shed_total.get(), 1);

        release_tx.send(()).expect("release");
        pool.runtime.block_on(async {
            held.await.expect("joined").expect("ran");
        });
        assert_eq!(metrics.worker_pool_active_tasks.get(), 0);
        assert_eq!(metrics.worker_pool_queued_tasks.get(), 0);
    }

    #[test]
    fn test_drain_waits_for_in_flight_tasks() {
        let pool = EventWorkerPool::new(2).expect("Failed to create worker pool");

        pool.runtime.block_on(async {
            let handle = pool
                .spawn(async {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    7
                })
                .expect("pool should accept work");

            let drained = pool.drain(Duration::from_secs(5)).await;
            assert!(drained, "drain should complete once tasks finish");
            assert_eq!(handle.await.unwrap(), 7);
            assert_eq!(pool.in_flight(), 0);
        });
    }

    #[test]
    fn test_drain_rejects_new_tasks() {
        let pool = EventWorkerPool::new(2).expect("Failed to create worker pool");

        pool.runtime.block_on(async {
            assert!(pool.drain(Duration::from_millis(10)).await);
            assert!(pool.is_shutting_down());
            assert!(pool.spawn(async { 1 }).is_none());
            assert!(pool.spawn_blocking(|| 1).is_none());
        });
    }

    #[test]
    fn test_drain_times_out_with_pending_tasks() {
        let pool = EventWorkerPool::new(2).expect("Failed to create worker pool");

        pool.runtime.block_on(async {
            let _handle = pool
                .spawn(async {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                })
                .expect("pool should accept work");

            let drained = pool.drain(Duration::from_millis(10)).await;
            assert!(!drained, "drain should report timeout while tasks run");
        });
    }
    }
}
