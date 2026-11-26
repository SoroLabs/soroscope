//! Bounded background task dispatcher and live telemetry event broadcasting queue (Issue #17).
//!
//! # Features
//! - [`BoundedTaskDispatcher`]: Caps how many spawned tasks may be in flight at once
//!   via a semaphore and evicts low-priority work when saturated.
//! - [`TelemetryEventQueue`]: High-throughput Multi-Producer Multi-Consumer (MPMC) broadcast
//!   channel for live telemetry events, with non-blocking dispatch, slow-subscriber lag handling,
//!   and dropped event metrics counters.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Semaphore};

// ── Bounded Task Dispatcher ─────────────────────────────────────────────────

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

// ── Live Telemetry Event Broadcasting Queue (Issue #17) ─────────────────────

/// Default capacity for the telemetry event broadcast buffer.
pub const DEFAULT_TELEMETRY_CAPACITY: usize = 1024;

/// A structured telemetry event emitted across the live broadcast queue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelemetryEvent {
    /// Unique event identifier.
    pub id: String,
    /// Name or topic of the telemetry event (e.g. "simulation.step", "rpc.latency", "system.memory").
    pub name: String,
    /// Event data payload.
    pub payload: serde_json::Value,
    /// UTC timestamp of when the event occurred.
    pub timestamp: DateTime<Utc>,
    /// Associated key-value metadata tags.
    pub tags: Vec<(String, String)>,
}

impl TelemetryEvent {
    /// Construct a new `TelemetryEvent` with a freshly generated UUID and current UTC timestamp.
    pub fn new(name: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            payload,
            timestamp: Utc::now(),
            tags: Vec::new(),
        }
    }

    /// Attach a key-value tag to this telemetry event.
    pub fn with_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.push((key.into(), value.into()));
        self
    }
}

/// Error encountered during telemetry event receiving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryRecvError {
    /// The broadcast channel was closed.
    Closed,
    /// The subscriber lagged behind and skipped `u64` events.
    Lagged(u64),
}

impl fmt::Display for TelemetryRecvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TelemetryRecvError::Closed => write!(f, "telemetry broadcast channel closed"),
            TelemetryRecvError::Lagged(n) => write!(f, "subscriber lagged behind and dropped {} events", n),
        }
    }
}

impl std::error::Error for TelemetryRecvError {}

/// Error encountered during non-blocking try_receive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryTryRecvError {
    /// No event is currently available.
    Empty,
    /// The broadcast channel was closed.
    Closed,
    /// The subscriber lagged behind and skipped `u64` events.
    Lagged(u64),
}

impl fmt::Display for TelemetryTryRecvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TelemetryTryRecvError::Empty => write!(f, "no telemetry event available"),
            TelemetryTryRecvError::Closed => write!(f, "telemetry broadcast channel closed"),
            TelemetryTryRecvError::Lagged(n) => write!(f, "subscriber lagged behind and dropped {} events", n),
        }
    }
}

impl std::error::Error for TelemetryTryRecvError {}

/// Multi-Producer Multi-Consumer (MPMC) live telemetry event broadcasting queue.
///
/// Backed by a Tokio broadcast channel:
/// - Multiple producers can concurrently broadcast events without blocking or deadlocking.
/// - Multiple subscribers receive an independent clone of the event stream.
/// - Slow subscribers that fall behind buffer capacity drop oldest unread messages without
///   blocking publishers or other subscribers.
/// - Dropped event metrics are tracked globally and per-subscriber.
#[derive(Clone)]
pub struct TelemetryEventQueue {
    sender: broadcast::Sender<TelemetryEvent>,
    capacity: usize,
    dropped_events: Arc<AtomicU64>,
    total_published: Arc<AtomicU64>,
}

impl TelemetryEventQueue {
    /// Create a new telemetry event broadcast queue with the specified buffer capacity.
    pub fn new(capacity: usize) -> Self {
        let capacity = if capacity == 0 { DEFAULT_TELEMETRY_CAPACITY } else { capacity };
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            capacity,
            dropped_events: Arc::new(AtomicU64::new(0)),
            total_published: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Buffer capacity per subscriber before old events are dropped.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of active subscribers currently listening.
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Total number of events dropped across all subscribers due to buffer saturation.
    pub fn dropped_events_count(&self) -> u64 {
        self.dropped_events.load(Ordering::Relaxed)
    }

    /// Total number of events successfully published to the queue.
    pub fn total_published(&self) -> u64 {
        self.total_published.load(Ordering::Relaxed)
    }

    /// Broadcast a telemetry event to all active subscribers.
    ///
    /// Never blocks: if there are no subscribers, the event is safely discarded and
    /// `Ok(0)` is returned. If subscribers are slow, they will drop older events when
    /// reading without stalling the publisher.
    pub fn publish(&self, event: TelemetryEvent) -> usize {
        self.total_published.fetch_add(1, Ordering::Relaxed);
        match self.sender.send(event) {
            Ok(receivers) => receivers,
            Err(_) => 0, // No active receivers
        }
    }

    /// Subscribe to the telemetry event broadcast stream.
    pub fn subscribe(&self) -> TelemetrySubscriber {
        TelemetrySubscriber {
            receiver: self.sender.subscribe(),
            dropped_events: self.dropped_events.clone(),
            local_dropped: 0,
        }
    }

    /// Create a dedicated producer handle for multi-producer broadcasting.
    pub fn producer(&self) -> TelemetryEventProducer {
        TelemetryEventProducer {
            queue: self.clone(),
        }
    }
}

impl Default for TelemetryEventQueue {
    fn default() -> Self {
        Self::new(DEFAULT_TELEMETRY_CAPACITY)
    }
}

/// Producer handle for publishing telemetry events concurrently.
#[derive(Clone)]
pub struct TelemetryEventProducer {
    queue: TelemetryEventQueue,
}

impl TelemetryEventProducer {
    /// Publish a telemetry event.
    pub fn publish(&self, event: TelemetryEvent) -> usize {
        self.queue.publish(event)
    }

    /// Total published counter.
    pub fn total_published(&self) -> u64 {
        self.queue.total_published()
    }
}

/// Consumer subscriber handle for receiving live telemetry events.
pub struct TelemetrySubscriber {
    receiver: broadcast::Receiver<TelemetryEvent>,
    dropped_events: Arc<AtomicU64>,
    local_dropped: u64,
}

impl TelemetrySubscriber {
    /// Receive the next telemetry event.
    ///
    /// If the subscriber lagged behind, this method logs the dropped counter, updates
    /// the metrics, and returns `Err(TelemetryRecvError::Lagged(count))`. Calling `recv()`
    /// again will resume receiving current events.
    pub async fn recv(&mut self) -> Result<TelemetryEvent, TelemetryRecvError> {
        match self.receiver.recv().await {
            Ok(event) => Ok(event),
            Err(broadcast::error::RecvError::Lagged(dropped)) => {
                self.local_dropped += dropped;
                let total = self.dropped_events.fetch_add(dropped, Ordering::Relaxed) + dropped;
                tracing::warn!(
                    dropped_this_lag = dropped,
                    local_dropped = self.local_dropped,
                    global_dropped_total = total,
                    "telemetry subscriber lagged behind — dropped unconsumed events"
                );
                Err(TelemetryRecvError::Lagged(dropped))
            }
            Err(broadcast::error::RecvError::Closed) => Err(TelemetryRecvError::Closed),
        }
    }

    /// Lossy receive: automatically handles and skips lag notices, returning the next
    /// available event directly while logging and recording dropped event counters.
    pub async fn recv_lossy(&mut self) -> Result<TelemetryEvent, TelemetryRecvError> {
        loop {
            match self.recv().await {
                Ok(event) => return Ok(event),
                Err(TelemetryRecvError::Lagged(_)) => continue,
                Err(TelemetryRecvError::Closed) => return Err(TelemetryRecvError::Closed),
            }
        }
    }

    /// Try to receive the next telemetry event synchronously without blocking.
    pub fn try_recv(&mut self) -> Result<TelemetryEvent, TelemetryTryRecvError> {
        match self.receiver.try_recv() {
            Ok(event) => Ok(event),
            Err(broadcast::error::TryRecvError::Lagged(dropped)) => {
                self.local_dropped += dropped;
                let total = self.dropped_events.fetch_add(dropped, Ordering::Relaxed) + dropped;
                tracing::warn!(
                    dropped_this_lag = dropped,
                    local_dropped = self.local_dropped,
                    global_dropped_total = total,
                    "telemetry subscriber lagged behind — dropped unconsumed events"
                );
                Err(TelemetryTryRecvError::Lagged(dropped))
            }
            Err(broadcast::error::TryRecvError::Empty) => Err(TelemetryTryRecvError::Empty),
            Err(broadcast::error::TryRecvError::Closed) => Err(TelemetryTryRecvError::Closed),
        }
    }

    /// Number of dropped events observed by this subscriber instance.
    pub fn local_dropped_count(&self) -> u64 {
        self.local_dropped
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

        let outcome = dispatcher
            .dispatch(TaskPriority::Low, async move {
                let _ = started_tx.send(());
                let _ = release_rx.await;
            })
            .await;
        assert_eq!(outcome, DispatchOutcome::Spawned);

        started_rx
            .await
            .expect("first task must start and acquire permit");

        let evicted = dispatcher
            .dispatch(TaskPriority::Low, async {
                unreachable!("dropped tasks must never run");
            })
            .await;
        assert_eq!(evicted, DispatchOutcome::Dropped);
        assert_eq!(dispatcher.dropped_count(), 1);

        let _ = release_tx.send(());
    }

    #[tokio::test]
    async fn normal_priority_tasks_wait_for_a_free_slot_instead_of_dropping() {
        let dispatcher = BoundedTaskDispatcher::new(1);
        let (first_started_tx, first_started_rx) = tokio::sync::oneshot::channel();
        let (first_release_tx, first_release_rx) = tokio::sync::oneshot::channel();
        let (second_completed_tx, second_completed_rx) = tokio::sync::oneshot::channel();

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

        let second_task = dispatcher.dispatch(TaskPriority::Normal, async move {
            let _ = second_completed_tx.send(());
        });

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

    #[tokio::test]
    async fn telemetry_event_creation_and_tags() {
        let event = TelemetryEvent::new("job.progress", serde_json::json!({ "percent": 50 }))
            .with_tag("job_id", "123")
            .with_tag("mode", "simulate");

        assert_eq!(event.name, "job.progress");
        assert_eq!(event.payload["percent"], 50);
        assert_eq!(event.tags.len(), 2);
        assert_eq!(event.tags[0], ("job_id".to_string(), "123".to_string()));
        assert_eq!(event.tags[1], ("mode".to_string(), "simulate".to_string()));
        assert!(!event.id.is_empty());
    }

    #[tokio::test]
    async fn telemetry_broadcast_multi_consumer() {
        let queue = TelemetryEventQueue::new(32);
        assert_eq!(queue.subscriber_count(), 0);

        let mut sub1 = queue.subscribe();
        let mut sub2 = queue.subscribe();
        assert_eq!(queue.subscriber_count(), 2);

        let event = TelemetryEvent::new("metric.cpu", serde_json::json!({ "usage": 42.5 }));
        let receivers = queue.publish(event.clone());
        assert_eq!(receivers, 2);
        assert_eq!(queue.total_published(), 1);

        let received1 = sub1.recv().await.expect("sub1 must receive event");
        let received2 = sub2.recv().await.expect("sub2 must receive event");

        assert_eq!(received1.name, "metric.cpu");
        assert_eq!(received2.name, "metric.cpu");
        assert_eq!(received1.payload, received2.payload);
    }

    #[tokio::test]
    async fn telemetry_multi_producer_broadcast() {
        let queue = TelemetryEventQueue::new(64);
        let mut subscriber = queue.subscribe();

        let prod1 = queue.producer();
        let prod2 = queue.producer();

        prod1.publish(TelemetryEvent::new("event.from_p1", serde_json::json!({ "sender": 1 })));
        prod2.publish(TelemetryEvent::new("event.from_p2", serde_json::json!({ "sender": 2 })));

        let e1 = subscriber.recv().await.expect("must receive event 1");
        let e2 = subscriber.recv().await.expect("must receive event 2");

        assert_eq!(e1.name, "event.from_p1");
        assert_eq!(e2.name, "event.from_p2");
        assert_eq!(queue.total_published(), 2);
    }

    #[tokio::test]
    async fn telemetry_zero_subscribers_does_not_fail() {
        let queue = TelemetryEventQueue::new(16);
        let receivers = queue.publish(TelemetryEvent::new("lonely.event", serde_json::json!({})));
        assert_eq!(receivers, 0);
        assert_eq!(queue.total_published(), 1);
        assert_eq!(queue.dropped_events_count(), 0);
    }

    #[tokio::test]
    async fn slow_subscribers_drop_events_without_blocking_queue() {
        let capacity = 4;
        let queue = TelemetryEventQueue::new(capacity);

        let mut slow_sub = queue.subscribe();
        let mut fast_sub = queue.subscribe();

        // Publish more events than the buffer capacity
        for i in 0..12 {
            queue.publish(TelemetryEvent::new("stream", serde_json::json!({ "seq": i })));
            // Fast subscriber consumes immediately
            let fast_ev = fast_sub.recv().await.expect("fast sub must receive immediately");
            assert_eq!(fast_ev.payload["seq"], i);
        }

        // Fast subscriber had 0 dropped events
        assert_eq!(fast_sub.local_dropped_count(), 0);

        // Slow subscriber now attempts to read: it should detect lag and dropped messages
        let slow_res = slow_sub.recv().await;
        assert!(matches!(slow_res, Err(TelemetryRecvError::Lagged(_))));
        assert!(slow_sub.local_dropped_count() > 0);
        assert!(queue.dropped_events_count() > 0);

        // After acknowledging lag, slow subscriber receives current in-buffer events
        let next_ev = slow_sub.recv().await.expect("must receive latest event");
        assert!(next_ev.payload["seq"].as_i64().unwrap() >= 8);
    }

    #[tokio::test]
    async fn high_throughput_zero_deadlock() {
        let queue = TelemetryEventQueue::new(128);
        let num_producers = 8;
        let events_per_producer = 500;
        let num_subscribers = 4;

        let mut subscriber_tasks = Vec::new();
        for _ in 0..num_subscribers {
            let mut sub = queue.subscribe();
            let handle = tokio::spawn(async move {
                let mut count = 0;
                while count < events_per_producer {
                    match sub.recv_lossy().await {
                        Ok(_) => count += 1,
                        Err(TelemetryRecvError::Closed) => break,
                        _ => {}
                    }
                }
                count
            });
            subscriber_tasks.push(handle);
        }

        let mut producer_tasks = Vec::new();
        for p in 0..num_producers {
            let prod = queue.producer();
            let handle = tokio::spawn(async move {
                for i in 0..events_per_producer {
                    prod.publish(TelemetryEvent::new(
                        "load.test",
                        serde_json::json!({ "producer": p, "i": i }),
                    ));
                    if i % 50 == 0 {
                        tokio::task::yield_now().await;
                    }
                }
            });
            producer_tasks.push(handle);
        }

        // Wait for all producers to finish
        for p_handle in producer_tasks {
            tokio::time::timeout(Duration::from_secs(10), p_handle)
                .await
                .expect("producer timeout: deadlock detected")
                .expect("producer panic");
        }

        assert_eq!(queue.total_published(), (num_producers * events_per_producer) as u64);

        // Ensure subscribers finish or read without deadlocking
        for s_handle in subscriber_tasks {
            let _ = tokio::time::timeout(Duration::from_secs(5), s_handle).await;
        }
    }
}
