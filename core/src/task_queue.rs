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
    use std::time::Duration;

    #[tokio::test]
    async fn low_priority_tasks_are_dropped_once_saturated() {
        let dispatcher = BoundedTaskDispatcher::new(1);
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();

        // Occupy the single slot with a task that holds its permit until released.
        let outcome = dispatcher
            .dispatch(TaskPriority::Low, async move {
                let _ = started_tx.send(());
                let _ = release_rx.await;
            })
            .await;
        assert_eq!(outcome, DispatchOutcome::Spawned);

        // Await the explicit notification that the task has started and acquired the permit.
        started_rx
            .await
            .expect("first task must start and acquire permit");

        // The dispatcher is now saturated; a second low-priority task must
        // be dropped rather than queued.
        let evicted = dispatcher
            .dispatch(TaskPriority::Low, async {
                unreachable!("dropped tasks must never run");
            })
            .await;
        assert_eq!(evicted, DispatchOutcome::Dropped);
        assert_eq!(dispatcher.dropped_count(), 1);

        // Cleanly release the first task.
        let _ = release_tx.send(());
    }

    #[tokio::test]
    async fn normal_priority_tasks_wait_for_a_free_slot_instead_of_dropping() {
        let dispatcher = BoundedTaskDispatcher::new(1);
        let (first_started_tx, first_started_rx) = tokio::sync::oneshot::channel();
        let (first_release_tx, first_release_rx) = tokio::sync::oneshot::channel();
        let (second_completed_tx, second_completed_rx) = tokio::sync::oneshot::channel();

        // First task occupies the only slot
        let outcome_1 = dispatcher
            .dispatch(TaskPriority::Low, async move {
                let _ = first_started_tx.send(());
                let _ = first_release_rx.await;
            })
            .await;
        assert_eq!(outcome_1, DispatchOutcome::Spawned);

        first_started_rx
            .await
            .expect("first task must start and acquire permit");

        // Normal priority task is dispatched; it should wait for a free slot
        let second_task = dispatcher.dispatch(TaskPriority::Normal, async move {
            let _ = second_completed_tx.send(());
        });

        // Release the first task so the normal-priority task can proceed
        let _ = first_release_tx.send(());

        let outcome_2 = tokio::time::timeout(Duration::from_secs(5), second_task)
            .await
            .expect("normal-priority dispatch should not hang indefinitely");
        assert_eq!(outcome_2, DispatchOutcome::Spawned);

        second_completed_rx
            .await
            .expect("second task must complete after permit is released");
        assert_eq!(dispatcher.dropped_count(), 0);
    }

    #[tokio::test]
    async fn capacity_and_dropped_count_are_reported() {
        let dispatcher = BoundedTaskDispatcher::new(4);
        assert_eq!(dispatcher.capacity(), 4);
        assert_eq!(dispatcher.dropped_count(), 0);
    }
}
