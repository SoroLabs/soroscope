use prometheus::{HistogramVec, IntCounterVec, Opts, Registry};

#[derive(Clone)]
pub struct AppMetrics {
    pub registry: Registry,
    pub simulation_latency_seconds: HistogramVec,
    pub rpc_error_count_total: IntCounterVec,
    pub simulation_requests_total: IntCounterVec,
    pub resource_utilization_percent: prometheus::GaugeVec,
    /// Host-wide CPU usage percentage (0–100) sampled by the system
    /// alarm monitor (issue #592). Label keys are static so scrapers
    /// see a single `local` series.
    pub host_cpu_usage_percent: prometheus::GaugeVec,
    /// Host-wide memory usage percentage (0–100) sampled by the
    /// system alarm monitor (issue #592).
    pub host_memory_usage_percent: prometheus::GaugeVec,
    /// Resident memory size of the SoroScope process itself, in bytes.
    pub process_memory_bytes: prometheus::GaugeVec,
    /// Wall-clock time spent per indexing/collection cycle, by stage.
    pub indexing_latency_seconds: HistogramVec,
    /// Ledger events successfully processed, by stage.
    pub events_processed_total: IntCounterVec,
    /// Indexing cycle failures, by stage.
    pub indexing_errors_total: IntCounterVec,
    /// Depth of background job queues, by queue name.
    pub job_queue_depth: prometheus::GaugeVec,
    /// Fee quote cache lookups served from memory (issue #1259).
    pub fee_quote_cache_hits_total: prometheus::IntCounter,
    /// Fee quote cache lookups that had to fetch a fresh quote (issue #1259).
    pub fee_quote_cache_misses_total: prometheus::IntCounter,
    /// Bounded worker pool tasks currently executing (issue #1409).
    pub worker_pool_active_tasks: prometheus::IntGauge,
    /// Bounded worker pool tasks admitted and waiting for a worker (issue #1409).
    pub worker_pool_queued_tasks: prometheus::IntGauge,
    /// Tasks shed by the bounded worker pool: rejected when full or skipped
    /// after waiting longer than `max_queue_wait_ms` (issue #1409).
    pub worker_pool_shed_total: prometheus::IntCounter,
    /// Resource exhaustion alarm severity per resource: 0 normal, 1 warning,
    /// 2 critical (issue #1411).
    pub resource_alarm_severity: prometheus::IntGaugeVec,
}

/// The `resource_alarm_severity{resource}` gauge published by
/// `sys_alarms::ResourceAlarmMonitor` (issue #1411).
pub fn resource_alarm_severity_gauge() -> Result<prometheus::IntGaugeVec, prometheus::Error> {
    prometheus::IntGaugeVec::new(
        Opts::new(
            "resource_alarm_severity",
            "Resource exhaustion alarm severity (0 normal, 1 warning, 2 critical)",
        ),
        &["resource"],
    )
}

impl AppMetrics {
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();

        let simulation_latency_seconds = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "simulation_latency_seconds",
                "Latency of simulation requests in seconds",
            ),
            &["endpoint"],
        )?;
        let rpc_error_count_total = IntCounterVec::new(
            Opts::new(
                "rpc_error_count_total",
                "Total number of RPC and simulation errors",
            ),
            &["endpoint", "error_type"],
        )?;
        let simulation_requests_total = IntCounterVec::new(
            Opts::new(
                "simulation_requests_total",
                "Total number of simulation requests by endpoint and cache status",
            ),
            &["endpoint", "cache_status"],
        )?;
        let resource_utilization_percent = prometheus::GaugeVec::new(
            Opts::new(
                "resource_utilization_percent",
                "Resource utilization percentage from latest simulation sample",
            ),
            &["resource"],
        )?;
        let host_cpu_usage_percent = prometheus::GaugeVec::new(
            Opts::new(
                "host_cpu_usage_percent",
                "Host-wide CPU usage percentage (0-100) sampled by the system alarm monitor",
            ),
            &["host"],
        )?;
        let host_memory_usage_percent = prometheus::GaugeVec::new(
            Opts::new(
                "host_memory_usage_percent",
                "Host-wide memory usage percentage (0-100) sampled by the system alarm monitor",
            ),
            &["host"],
        )?;
        let process_memory_bytes = prometheus::GaugeVec::new(
            Opts::new(
                "process_memory_bytes",
                "Resident memory size of the SoroScope process in bytes",
            ),
            &["process"],
        )?;
        let indexing_latency_seconds = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "indexing_latency_seconds",
                "Latency of ledger indexing/collection cycles in seconds",
            ),
            &["stage"],
        )?;
        let events_processed_total = IntCounterVec::new(
            Opts::new(
                "events_processed_total",
                "Total number of ledger events successfully processed",
            ),
            &["stage"],
        )?;
        let indexing_errors_total = IntCounterVec::new(
            Opts::new(
                "indexing_errors_total",
                "Total number of indexing cycle failures",
            ),
            &["stage"],
        )?;
        let job_queue_depth = prometheus::GaugeVec::new(
            Opts::new("job_queue_depth", "Current depth of background job queues"),
            &["queue"],
        )?;
        let fee_quote_cache_hits_total = prometheus::IntCounter::new(
            "fee_quote_cache_hits_total",
            "Fee quote lookups served from the in-memory cache",
        )?;
        let fee_quote_cache_misses_total = prometheus::IntCounter::new(
            "fee_quote_cache_misses_total",
            "Fee quote lookups that required a fresh fetch",
        )?;
        let worker_pool_active_tasks = prometheus::IntGauge::new(
            "worker_pool_active_tasks",
            "Bounded worker pool tasks currently executing",
        )?;
        let worker_pool_queued_tasks = prometheus::IntGauge::new(
            "worker_pool_queued_tasks",
            "Bounded worker pool tasks waiting for a worker",
        )?;
        let worker_pool_shed_total = prometheus::IntCounter::new(
            "worker_pool_shed_total",
            "Tasks shed by the bounded worker pool (queue full or queue wait exceeded)",
        )?;

        registry.register(Box::new(simulation_latency_seconds.clone()))?;
        registry.register(Box::new(rpc_error_count_total.clone()))?;
        registry.register(Box::new(simulation_requests_total.clone()))?;
        registry.register(Box::new(resource_utilization_percent.clone()))?;
        registry.register(Box::new(host_cpu_usage_percent.clone()))?;
        registry.register(Box::new(host_memory_usage_percent.clone()))?;
        registry.register(Box::new(process_memory_bytes.clone()))?;
        registry.register(Box::new(indexing_latency_seconds.clone()))?;
        registry.register(Box::new(events_processed_total.clone()))?;
        registry.register(Box::new(indexing_errors_total.clone()))?;
        registry.register(Box::new(job_queue_depth.clone()))?;
        registry.register(Box::new(fee_quote_cache_hits_total.clone()))?;
        registry.register(Box::new(fee_quote_cache_misses_total.clone()))?;
        registry.register(Box::new(worker_pool_active_tasks.clone()))?;
        registry.register(Box::new(worker_pool_queued_tasks.clone()))?;
        registry.register(Box::new(worker_pool_shed_total.clone()))?;
        let resource_alarm_severity = resource_alarm_severity_gauge()?;
        registry.register(Box::new(resource_alarm_severity.clone()))?;

        Ok(Self {
            registry,
            simulation_latency_seconds,
            rpc_error_count_total,
            simulation_requests_total,
            resource_utilization_percent,
            host_cpu_usage_percent,
            host_memory_usage_percent,
            process_memory_bytes,
            indexing_latency_seconds,
            events_processed_total,
            indexing_errors_total,
            job_queue_depth,
            fee_quote_cache_hits_total,
            fee_quote_cache_misses_total,
            worker_pool_active_tasks,
            worker_pool_queued_tasks,
            worker_pool_shed_total,
            resource_alarm_severity,
        })
    }

    /// Handles for [`crate::worker_pool::EventWorkerPool::with_metrics`].
    pub fn worker_pool_metrics(&self) -> crate::worker_pool::WorkerPoolMetrics {
        crate::worker_pool::WorkerPoolMetrics {
            active_tasks: self.worker_pool_active_tasks.clone(),
            queued_tasks: self.worker_pool_queued_tasks.clone(),
            shed_total: self.worker_pool_shed_total.clone(),
        }
    }
}
