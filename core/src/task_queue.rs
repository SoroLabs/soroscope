//! Bounded background task dispatcher.
//!
//! `tokio::spawn` has no built-in limit: a producer that keeps scheduling
//! background work (retry scheduling, telemetry fan-out, notifications) can
//! pile up an unbounded number of pending tasks and exhaust memory before
//! any of them get to run. [`BoundedTaskDispatcher`] caps how many spawned
//! tasks may be in flight at once via a semaphore and, for low-priority
//! work, drops (evicts) new tasks instead of queuing them indefinitely once
//! that cap is reached.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Relative importance of a dispatched task, used to decide what happens
/// when the dispatcher is saturated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskPriority {
    /// Must eventually run: dispatch blocks (bounded backpressure) until a
    /// slot frees up rather than dropping the work.
    Normal,
    /// Best-effort: dropped immediately if the dispatcher is saturated
    /// (e.g. retry scheduling, telemetry) rather than piling up in memory.
    Low,
}

/// Outcome of a dispatch attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchOutcome {
    /// The task was spawned onto the background runtime.
    Spawned,
    /// The dispatcher was saturated and the low-priority task was dropped.
    Dropped,
}

/// Semaphore-bounded background task dispatcher with eviction for
/// low-priority work.
#[derive(Clone)]
pub struct BoundedTaskDispatcher {
    semaphore: Arc<Semaphore>,
    capacity: usize,
    dropped: Arc<AtomicU64>,
}

impl BoundedTaskDispatcher {
    /// Create a dispatcher that allows at most `capacity` tasks in flight.
    pub fn new(capacity: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(capacity)),
            capacity,
            dropped: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Maximum number of tasks this dispatcher allows in flight at once.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of low-priority tasks dropped since creation because the
    /// dispatcher was saturated.
    pub fn dropped_count(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Dispatch `task` onto a background Tokio task, honouring `priority`.
    ///
    /// `Normal` tasks always run: dispatch waits for a free slot, providing
    /// backpressure instead of unbounded growth. `Low` tasks are dropped
    /// immediately if no slot is free, so best-effort background work can
    /// never accumulate without bound.
    pub async fn dispatch<F>(&self, priority: TaskPriority, task: F) -> DispatchOutcome
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let permit = match priority {
            TaskPriority::Normal => match self.semaphore.clone().acquire_owned().await {
                Ok(permit) => permit,
                // Semaphore closed: dispatcher is shutting down.
                Err(_) => return DispatchOutcome::Dropped,
            },
            TaskPriority::Low => match self.semaphore.clone().try_acquire_owned() {
                Ok(permit) => permit,
                Err(_) => {
                    let dropped_total = self.dropped.fetch_add(1, Ordering::Relaxed) + 1;
                    tracing::warn!(
                        capacity = self.capacity,
                        dropped_total,
                        "bounded task dispatcher saturated — dropping low-priority task"
                    );
                    return DispatchOutcome::Dropped;
                }
            },
        };

        tokio::spawn(async move {
            task.await;
            drop(permit);
        });

        DispatchOutcome::Spawned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::time::Duration;

    #[tokio::test]
    async fn low_priority_tasks_are_dropped_once_saturated() {
        let dispatcher = BoundedTaskDispatcher::new(1);
        let started = Arc::new(AtomicUsize::new(0));

        // Occupy the single slot with a long-running task.
        let held = started.clone();
        let outcome = dispatcher
            .dispatch(TaskPriority::Low, async move {
                held.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(200)).await;
            })
            .await;
        assert_eq!(outcome, DispatchOutcome::Spawned);

        // Give the spawned task a chance to acquire the permit.
        tokio::time::sleep(Duration::from_millis(20)).await;

        // The dispatcher is now saturated; a second low-priority task must
        // be dropped rather than queued.
        let evicted = dispatcher
            .dispatch(TaskPriority::Low, async {
                unreachable!("dropped tasks must never run");
            })
            .await;
        assert_eq!(evicted, DispatchOutcome::Dropped);
        assert_eq!(dispatcher.dropped_count(), 1);
    }

    #[tokio::test]
    async fn normal_priority_tasks_wait_for_a_free_slot_instead_of_dropping() {
        let dispatcher = BoundedTaskDispatcher::new(1);

        dispatcher
            .dispatch(TaskPriority::Low, async {
                tokio::time::sleep(Duration::from_millis(50)).await;
            })
            .await;
        tokio::time::sleep(Duration::from_millis(10)).await;

        let ran = Arc::new(AtomicUsize::new(0));
        let ran_clone = ran.clone();
        let outcome = tokio::time::timeout(
            Duration::from_millis(500),
            dispatcher.dispatch(TaskPriority::Normal, async move {
                ran_clone.fetch_add(1, Ordering::SeqCst);
            }),
        )
        .await
        .expect("normal-priority dispatch should not hang indefinitely");

        assert_eq!(outcome, DispatchOutcome::Spawned);
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(ran.load(Ordering::SeqCst), 1);
        assert_eq!(dispatcher.dropped_count(), 0);
    }

    #[tokio::test]
    async fn capacity_and_dropped_count_are_reported() {
        let dispatcher = BoundedTaskDispatcher::new(4);
        assert_eq!(dispatcher.capacity(), 4);
        assert_eq!(dispatcher.dropped_count(), 0);
    }
}
