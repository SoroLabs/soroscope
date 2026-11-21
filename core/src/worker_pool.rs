use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::{Builder, Runtime};
use tokio::sync::Notify;

/// A dedicated thread pool configuration for heavy contract event processing.
/// This prevents CPU-intensive parsing from blocking the main HTTP async runtime.
#[derive(Clone)]
pub struct EventWorkerPool {
    runtime: Arc<Runtime>,
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
        let runtime = Builder::new_multi_thread()
            .worker_threads(worker_threads)
            .thread_name("event-parser-worker")
            .enable_all()
            .build()?;

        Ok(Self {
            runtime: Arc::new(runtime),
            shutting_down: Arc::new(AtomicBool::new(false)),
            idle_notify: Arc::new(Notify::new()),
            in_flight: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        })
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
