use std::sync::Arc;
use tokio::runtime::{Builder, Runtime};

/// A dedicated thread pool configuration for heavy contract event processing.
/// This prevents CPU-intensive parsing from blocking the main HTTP async runtime.
#[derive(Clone)]
pub struct EventWorkerPool {
    runtime: Arc<Runtime>,
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
        })
    }

    /// Spawns an async task on the dedicated event worker pool.
    pub fn spawn<F>(&self, future: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: std::future::Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.runtime.spawn(future)
    }

    /// Spawns a blocking (CPU-heavy) task on the dedicated event worker pool.
    /// Use this for strict, heavy synchronous parsing logic.
    pub fn spawn_blocking<F, R>(&self, func: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        self.runtime.spawn_blocking(func)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_worker_pool_initialization() {
        let pool_result = EventWorkerPool::new(2);
        assert!(pool_result.is_ok(), "Worker pool should initialize successfully");
    }

    #[test]
    fn test_pool_spawns_async_task() {
        let pool = EventWorkerPool::new(2).expect("Failed to create worker pool");

        let result = pool.runtime.block_on(async {
            let handle = pool.spawn(async {
                100 + 42
            });
            handle.await.unwrap()
        });

        assert_eq!(result, 142, "Async task should execute and return correctly");
    }

    #[test]
    fn test_pool_spawns_blocking_task() {
        let pool = EventWorkerPool::new(2).expect("Failed to create worker pool");

        let result = pool.runtime.block_on(async {
            let handle = pool.spawn_blocking(|| {
                // Simulate a heavy CPU-bound parsing task
                let mut sum = 0;
                for i in 1..=1000 {
                    sum += i;
                }
                sum
            });
            handle.await.unwrap()
        });

        assert_eq!(result, 500500, "Blocking CPU task should compute correctly off-thread");
    }
}