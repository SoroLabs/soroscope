#![allow(
    dead_code,
    clippy::large_enum_variant,
    clippy::manual_inspect,
    clippy::needless_borrows_for_generic_args
)]

use crate::insights::InsightsEngine;
use crate::simulation::{SimulationEngine, SimulationResult, SorobanResources};
use crate::task_queue::{BoundedTaskDispatcher, TaskPriority};
use crate::ws::SimulationBus;
use crate::AppError;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use redis::{AsyncCommands, Client as RedisClient};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::any::AnyQueryResult;
use sqlx::{PgPool, SqlitePool};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use utoipa::ToSchema;
use uuid::Uuid;

/// Database pool type - supports both PostgreSQL and SQLite
#[derive(Clone)]
pub enum DbPool {
    Postgres(PgPool),
    Sqlite(SqlitePool),
}

impl DbPool {
    pub async fn execute(&self, query: &str) -> Result<AnyQueryResult, sqlx::Error> {
        match self {
            DbPool::Postgres(pool) => {
                let result = sqlx::query(query).execute(pool).await?;
                Ok(AnyQueryResult {
                    rows_affected: result.rows_affected(),
                    last_insert_id: None,
                })
            }
            DbPool::Sqlite(pool) => {
                let result = sqlx::query(query).execute(pool).await?;
                Ok(AnyQueryResult {
                    rows_affected: result.rows_affected(),
                    last_insert_id: Some(result.last_insert_rowid()),
                })
            }
        }
    }
}

/// Unique identifier for a job
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[sqlx(transparent)]
pub struct JobId(pub Uuid);

impl JobId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for JobId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

/// Status of a job in its lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[sqlx(rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JobStatus {
    Queued,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

/// Type of analysis job
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum JobType {
    Analyze,
    Compare,
    OptimizeLimits,
}

/// Payload for different job types
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case", tag = "type", content = "data")]
pub enum JobPayload {
    Analyze {
        contract_id: String,
        function_name: String,
        args: Option<Vec<String>>,
        ledger_overrides: Option<HashMap<String, String>>,
    },
    Compare {
        mode: String,
        current_wasm: Option<Vec<u8>>,
        base_wasm: Option<Vec<u8>>,
        contract_id: Option<String>,
        function_name: Option<String>,
        args: Vec<String>,
    },
    OptimizeLimits {
        contract_id: String,
        function_name: String,
        args: Vec<String>,
        safety_margin: f64,
    },
}

impl JobPayload {
    /// The contract this job targets, if any (`Compare` in `local_vs_local`
    /// mode has no contract).
    pub fn contract_id(&self) -> Option<&str> {
        match self {
            JobPayload::Analyze { contract_id, .. } => Some(contract_id),
            JobPayload::OptimizeLimits { contract_id, .. } => Some(contract_id),
            JobPayload::Compare { contract_id, .. } => contract_id.as_deref(),
        }
    }

    /// The contract function this job invokes, if any.
    pub fn function_name(&self) -> Option<&str> {
        match self {
            JobPayload::Analyze { function_name, .. } => Some(function_name),
            JobPayload::OptimizeLimits { function_name, .. } => Some(function_name),
            JobPayload::Compare { function_name, .. } => function_name.as_deref(),
        }
    }
}

/// Filters accepted by [`JobQueue::list`] when browsing contract execution
/// history.
#[derive(Debug, Clone, Default)]
pub struct JobListFilter {
    pub status: Option<JobStatus>,
    pub job_type: Option<JobType>,
    pub contract_id: Option<String>,
}

/// Progress information for a job
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct JobProgress {
    pub percent: i32,
    pub message: String,
    pub updated_at: DateTime<Utc>,
}

/// Result of a completed job
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case", tag = "status", content = "data")]
pub enum JobResult {
    Success {
        #[serde(skip_serializing_if = "Option::is_none")]
        resources: Option<SorobanResources>,
        #[serde(skip_serializing_if = "Option::is_none")]
        simulation_result: Option<SimulationResult>,
        #[serde(skip_serializing_if = "Option::is_none")]
        optimization: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        comparison: Option<Value>,
    },
    Failed {
        error: String,
        error_type: String,
    },
}

/// Webhook configuration for job notifications
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WebhookConfig {
    pub callback_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
}

/// A job in the queue
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct Job {
    pub id: JobId,
    pub job_type: JobType,
    pub status: JobStatus,
    pub payload: Value,
    pub result: Option<Value>,
    pub progress_percent: i32,
    pub progress_message: String,
    pub webhook_url: Option<String>,
    pub webhook_headers: Option<Value>,
    pub webhook_secret: Option<String>,
    pub error_message: Option<String>,
    pub error_type: Option<String>,
    pub timeout_secs: i32,
    pub retry_count: i32,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

impl Job {
    pub fn get_progress(&self) -> JobProgress {
        JobProgress {
            percent: self.progress_percent,
            message: self.progress_message.clone(),
            updated_at: self.updated_at,
        }
    }

    pub fn get_result(&self) -> Option<JobResult> {
        self.result
            .as_ref()
            .and_then(|r| serde_json::from_value(r.clone()).ok())
    }

    pub fn get_payload(&self) -> Option<JobPayload> {
        serde_json::from_value(self.payload.clone()).ok()
    }

    pub fn get_webhook_config(&self) -> Option<WebhookConfig> {
        self.webhook_url.as_ref().map(|url| WebhookConfig {
            callback_url: url.clone(),
            headers: self
                .webhook_headers
                .as_ref()
                .and_then(|h| serde_json::from_value(h.clone()).ok()),
            secret: self.webhook_secret.clone(),
        })
    }
}

/// Errors that can occur in job operations
#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error("Job not found: {0}")]
    NotFound(JobId),
    #[error("Job cannot be cancelled in status: {0:?}")]
    CannotCancel(JobStatus),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Job processing failed: {0}")]
    ProcessingFailed(String),
    #[error("Webhook delivery failed: {0}")]
    WebhookFailed(String),
}

/// Configuration for the job queue
#[derive(Debug, Clone)]
pub struct JobQueueConfig {
    pub job_timeout_secs: u64,
    pub cleanup_interval_secs: u64,
    pub retention_secs: u64,
    pub webhook_timeout_secs: u64,
    pub webhook_max_retries: u32,
    pub max_concurrent_jobs: usize,
    pub max_job_retries: i32,
    /// Maximum number of retry-scheduling tasks that may be in flight at
    /// once. Retry scheduling is best-effort background work: once this
    /// cap is hit, further retries are dropped (and the job stays queued
    /// for the next cleanup/requeue pass) rather than piling up spawned
    /// tasks without bound under a sustained failure storm.
    pub retry_queue_capacity: usize,
}

impl Default for JobQueueConfig {
    fn default() -> Self {
        Self {
            job_timeout_secs: 300,
            cleanup_interval_secs: 3600,
            retention_secs: 3600,
            webhook_timeout_secs: 10,
            webhook_max_retries: 3,
            max_concurrent_jobs: 10,
            max_job_retries: 3,
            retry_queue_capacity: 256,
        }
    }
}

/// SQL-based job queue
pub struct JobQueue {
    pool: DbPool,
    redis: RedisClient,
    config: JobQueueConfig,
    retry_dispatcher: BoundedTaskDispatcher,
}

impl JobQueue {
    pub async fn new(
        database_url: &str,
        redis_url: &str,
        config: JobQueueConfig,
    ) -> Result<Self, JobError> {
        let pool = if database_url.starts_with("postgres://") {
            let pool = PgPool::connect(database_url).await?;
            DbPool::Postgres(pool)
        } else {
            let pool = SqlitePool::connect(database_url).await?;
            DbPool::Sqlite(pool)
        };

        let redis = RedisClient::open(redis_url).map_err(|e| {
            JobError::ProcessingFailed(format!("Failed to connect to Redis: {}", e))
        })?;

        // Run migrations
        Self::run_migrations(&pool).await?;

        let retry_dispatcher = BoundedTaskDispatcher::new(config.retry_queue_capacity);

        Ok(Self {
            pool,
            redis,
            config,
            retry_dispatcher,
        })
    }

    async fn run_migrations(pool: &DbPool) -> Result<(), JobError> {
        let migration_sql = include_str!("../migrations/001_create_jobs_table.sql");

        // Split and execute each statement
        for statement in migration_sql.split(";") {
            let stmt = statement.trim();
            if !stmt.is_empty() {
                pool.execute(stmt).await?;
            }
        }

        Ok(())
    }

    /// Submit a new job to the queue
    pub async fn submit(
        &self,
        job_type: JobType,
        payload: JobPayload,
        webhook: Option<WebhookConfig>,
    ) -> Result<JobId, JobError> {
        let id = JobId::new();
        let payload_json = serde_json::to_value(&payload).map_err(|e| {
            JobError::ProcessingFailed(format!("Failed to serialize payload: {}", e))
        })?;

        let (webhook_url, webhook_headers, webhook_secret) = match webhook {
            Some(w) => (
                Some(w.callback_url),
                w.headers
                    .map(|h| serde_json::to_value(h).unwrap_or_default()),
                w.secret,
            ),
            None => (None, None, None),
        };

        match &self.pool {
            DbPool::Postgres(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO jobs (id, job_type, status, payload, webhook_url, webhook_headers, webhook_secret, timeout_secs)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                    "#
                )
                .bind(&id)
                .bind(&job_type)
                .bind(&JobStatus::Queued)
                .bind(&payload_json)
                .bind(&webhook_url)
                .bind(&webhook_headers)
                .bind(&webhook_secret)
                .bind(self.config.job_timeout_secs as i32)
                .execute(pool)
                .await?;
            }
            DbPool::Sqlite(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO jobs (id, job_type, status, payload, webhook_url, webhook_headers, webhook_secret, timeout_secs)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                    "#
                )
                .bind(&id.0.to_string())
                .bind(&job_type)
                .bind("QUEUED")
                .bind(&payload_json)
                .bind(&webhook_url)
                .bind(&webhook_headers)
                .bind(&webhook_secret)
                .bind(self.config.job_timeout_secs as i32)
                .execute(pool)
                .await?;
            }
        }

        // Push JobId to Redis queue
        let mut conn = self
            .redis
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| {
                JobError::ProcessingFailed(format!("Failed to get Redis connection: {}", e))
            })?;

        conn.lpush::<_, _, ()>("soroscope:jobs:queue", id.0.to_string())
            .await
            .map_err(|e| JobError::ProcessingFailed(format!("Redis LPUSH failed: {}", e)))?;

        tracing::info!(job_id = %id, "Job submitted to Redis queue");
        Ok(id)
    }

    /// Number of jobs currently waiting in the Redis queue. Used for the
    /// `job_queue_depth` metric.
    pub async fn queue_depth(&self) -> Result<i64, JobError> {
        let mut conn = self
            .redis
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| {
                JobError::ProcessingFailed(format!("Failed to get Redis connection: {}", e))
            })?;

        let depth: i64 = conn
            .llen("soroscope:jobs:queue")
            .await
            .map_err(|e| JobError::ProcessingFailed(format!("Redis LLEN failed: {}", e)))?;

        Ok(depth)
    }

    /// Get a job by ID
    pub async fn get(&self, id: &JobId) -> Result<Option<Job>, JobError> {
        let job = match &self.pool {
            DbPool::Postgres(pool) => {
                sqlx::query_as::<_, Job>("SELECT * FROM jobs WHERE id = $1")
                    .bind(id)
                    .fetch_optional(pool)
                    .await?
            }
            DbPool::Sqlite(pool) => {
                // For SQLite, we need to manually map since sqlx::Type might not work perfectly
                let row = sqlx::query("SELECT * FROM jobs WHERE id = ?1")
                    .bind(id.0.to_string())
                    .fetch_optional(pool)
                    .await?;

                row.map(|r| self.row_to_job(&r)).transpose()?
            }
        };

        Ok(job)
    }

    /// Get the next queued job for processing
    pub async fn get_next_queued(&self) -> Result<Option<Job>, JobError> {
        let job =
            match &self.pool {
                DbPool::Postgres(pool) => sqlx::query_as::<_, Job>(
                    "SELECT * FROM jobs WHERE status = 'QUEUED' ORDER BY created_at ASC LIMIT 1",
                )
                .fetch_optional(pool)
                .await?,
                DbPool::Sqlite(pool) => {
                    let row = sqlx::query(
                    "SELECT * FROM jobs WHERE status = 'QUEUED' ORDER BY created_at ASC LIMIT 1"
                )
                .fetch_optional(pool)
                .await?;

                    row.map(|r| self.row_to_job(&r)).transpose()?
                }
            };

        Ok(job)
    }

    /// List jobs (contract execution history), most-recently-created first,
    /// with optional filtering and offset/limit pagination. Used by the
    /// GraphQL `contractExecutions` query.
    ///
    /// `status` and `job_type` are applied as indexed SQL predicates.
    /// `contract_id` lives inside the JSON `payload` column rather than an
    /// indexed one, so it is applied in-memory after the page is fetched —
    /// a page may return fewer than `limit` rows when combined with a
    /// `contract_id` filter.
    pub async fn list(
        &self,
        filter: &JobListFilter,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Job>, JobError> {
        let limit = limit.clamp(1, 200);
        let offset = offset.max(0);

        let mut jobs = match &self.pool {
            DbPool::Postgres(pool) => match (&filter.status, &filter.job_type) {
                (Some(status), Some(job_type)) => sqlx::query_as::<_, Job>(
                    "SELECT * FROM jobs WHERE status = $1 AND job_type = $2 ORDER BY created_at DESC LIMIT $3 OFFSET $4",
                )
                .bind(status)
                .bind(job_type)
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?,
                (Some(status), None) => sqlx::query_as::<_, Job>(
                    "SELECT * FROM jobs WHERE status = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
                )
                .bind(status)
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?,
                (None, Some(job_type)) => sqlx::query_as::<_, Job>(
                    "SELECT * FROM jobs WHERE job_type = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
                )
                .bind(job_type)
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?,
                (None, None) => sqlx::query_as::<_, Job>(
                    "SELECT * FROM jobs ORDER BY created_at DESC LIMIT $1 OFFSET $2",
                )
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await?,
            },
            DbPool::Sqlite(pool) => {
                let rows = match (&filter.status, &filter.job_type) {
                    (Some(status), Some(job_type)) => sqlx::query(
                        "SELECT * FROM jobs WHERE status = ?1 AND job_type = ?2 ORDER BY created_at DESC LIMIT ?3 OFFSET ?4",
                    )
                    .bind(status)
                    .bind(job_type)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(pool)
                    .await?,
                    (Some(status), None) => sqlx::query(
                        "SELECT * FROM jobs WHERE status = ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3",
                    )
                    .bind(status)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(pool)
                    .await?,
                    (None, Some(job_type)) => sqlx::query(
                        "SELECT * FROM jobs WHERE job_type = ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3",
                    )
                    .bind(job_type)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(pool)
                    .await?,
                    (None, None) => sqlx::query(
                        "SELECT * FROM jobs ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
                    )
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(pool)
                    .await?,
                };

                rows.iter()
                    .map(|row| self.row_to_job(row))
                    .collect::<Result<Vec<_>, _>>()?
            }
        };

        if let Some(contract_id) = &filter.contract_id {
            jobs.retain(|job| {
                job.get_payload()
                    .and_then(|payload| payload.contract_id().map(str::to_string))
                    .as_deref()
                    == Some(contract_id.as_str())
            });
        }

        Ok(jobs)
    }

    /// Mark a job as processing
    pub async fn mark_processing(&self, id: &JobId) -> Result<(), JobError> {
        match &self.pool {
            DbPool::Postgres(pool) => {
                sqlx::query(
                    "UPDATE jobs SET status = 'PROCESSING', started_at = NOW(), progress_percent = 10, progress_message = 'Processing started' WHERE id = $1"
                )
                .bind(id)
                .execute(pool)
                .await?;
            }
            DbPool::Sqlite(pool) => {
                sqlx::query(
                    "UPDATE jobs SET status = 'PROCESSING', started_at = datetime('now'), progress_percent = 10, progress_message = 'Processing started' WHERE id = ?1"
                )
                .bind(id.0.to_string())
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }

    /// Update job progress
    pub async fn update_progress(
        &self,
        id: &JobId,
        percent: i32,
        message: &str,
    ) -> Result<(), JobError> {
        match &self.pool {
            DbPool::Postgres(pool) => {
                sqlx::query(
                    "UPDATE jobs SET progress_percent = $1, progress_message = $2 WHERE id = $3",
                )
                .bind(percent)
                .bind(message)
                .bind(id)
                .execute(pool)
                .await?;
            }
            DbPool::Sqlite(pool) => {
                sqlx::query(
                    "UPDATE jobs SET progress_percent = ?1, progress_message = ?2 WHERE id = ?3",
                )
                .bind(percent)
                .bind(message)
                .bind(id.0.to_string())
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }

    /// Complete a job with a result
    pub async fn complete(&self, id: &JobId, result: &JobResult) -> Result<(), JobError> {
        let result_json = serde_json::to_value(result).map_err(|e| {
            JobError::ProcessingFailed(format!("Failed to serialize result: {}", e))
        })?;

        match &self.pool {
            DbPool::Postgres(pool) => {
                sqlx::query(
                    "UPDATE jobs SET status = 'COMPLETED', result = $1, completed_at = NOW(), progress_percent = 100, progress_message = 'Completed' WHERE id = $2"
                )
                .bind(&result_json)
                .bind(id)
                .execute(pool)
                .await?;
            }
            DbPool::Sqlite(pool) => {
                sqlx::query(
                    "UPDATE jobs SET status = 'COMPLETED', result = ?1, completed_at = datetime('now'), progress_percent = 100, progress_message = 'Completed' WHERE id = ?2"
                )
                .bind(&result_json)
                .bind(id.0.to_string())
                .execute(pool)
                .await?;
            }
        }

        tracing::info!(job_id = %id, "Job completed");
        Ok(())
    }

    /// Mark a job as failed
    pub async fn fail(&self, id: &JobId, error: &str, error_type: &str) -> Result<(), JobError> {
        let result = JobResult::Failed {
            error: error.to_string(),
            error_type: error_type.to_string(),
        };
        let result_json = serde_json::to_value(&result).unwrap_or_default();

        match &self.pool {
            DbPool::Postgres(pool) => {
                sqlx::query(
                    "UPDATE jobs SET status = 'FAILED', result = $1, error_message = $2, error_type = $3, completed_at = NOW(), progress_message = 'Failed' WHERE id = $4"
                )
                .bind(&result_json)
                .bind(error)
                .bind(error_type)
                .bind(id)
                .execute(pool)
                .await?;
            }
            DbPool::Sqlite(pool) => {
                sqlx::query(
                    "UPDATE jobs SET status = 'FAILED', result = ?1, error_message = ?2, error_type = ?3, completed_at = datetime('now'), progress_message = 'Failed' WHERE id = ?4"
                )
                .bind(&result_json)
                .bind(error)
                .bind(error_type)
                .bind(id.0.to_string())
                .execute(pool)
                .await?;
            }
        }

        tracing::error!(job_id = %id, error = %error, "Job failed");
        Ok(())
    }

    /// Cancel a job
    pub async fn cancel(&self, id: &JobId) -> Result<Job, JobError> {
        let job = self.get(id).await?.ok_or(JobError::NotFound(*id))?;

        match job.status {
            JobStatus::Queued | JobStatus::Processing => {
                match &self.pool {
                    DbPool::Postgres(pool) => {
                        sqlx::query(
                            "UPDATE jobs SET status = 'CANCELLED', completed_at = NOW(), progress_message = 'Cancelled' WHERE id = $1"
                        )
                        .bind(id)
                        .execute(pool)
                        .await?;
                    }
                    DbPool::Sqlite(pool) => {
                        sqlx::query(
                            "UPDATE jobs SET status = 'CANCELLED', completed_at = datetime('now'), progress_message = 'Cancelled' WHERE id = ?1"
                        )
                        .bind(id.0.to_string())
                        .execute(pool)
                        .await?;
                    }
                }

                tracing::info!(job_id = %id, "Job cancelled");
                self.get(id).await?.ok_or(JobError::NotFound(*id))
            }
            status => Err(JobError::CannotCancel(status)),
        }
    }

    /// Cleanup old completed jobs
    pub async fn cleanup(&self) -> Result<u64, JobError> {
        let deleted = match &self.pool {
            DbPool::Postgres(pool) => {
                let result = sqlx::query(
                    "DELETE FROM jobs WHERE status IN ('COMPLETED', 'FAILED', 'CANCELLED') AND completed_at < NOW() - INTERVAL '1 hour' * $1"
                )
                .bind(self.config.retention_secs as f64 / 3600.0)
                .execute(pool)
                .await?;
                result.rows_affected()
            }
            DbPool::Sqlite(pool) => {
                let result = sqlx::query(
                    "DELETE FROM jobs WHERE status IN ('COMPLETED', 'FAILED', 'CANCELLED') AND completed_at < datetime('now', '-' || ?1 || ' seconds')"
                )
                .bind(self.config.retention_secs as i64)
                .execute(pool)
                .await?;
                result.rows_affected()
            }
        };

        if deleted > 0 {
            tracing::info!(count = deleted, "Cleaned up old jobs");
        }
        Ok(deleted)
    }

    /// Retry a failed job with exponential backoff
    pub async fn retry_job(&self, job: &Job) -> Result<(), JobError> {
        if job.retry_count >= self.config.max_job_retries {
            tracing::warn!(job_id = %job.id, "Max retries reached, marking as failed");
            return Ok(());
        }

        let new_retry_count = job.retry_count + 1;
        let delay_secs = 2_u64.pow(new_retry_count as u32 - 1) * 30; // 30s, 60s, 120s...

        // Update retry count in DB
        match &self.pool {
            DbPool::Postgres(pool) => {
                sqlx::query("UPDATE jobs SET retry_count = $1, status = 'QUEUED' WHERE id = $2")
                    .bind(new_retry_count)
                    .bind(&job.id)
                    .execute(pool)
                    .await?;
            }
            DbPool::Sqlite(pool) => {
                sqlx::query("UPDATE jobs SET retry_count = ?1, status = 'QUEUED' WHERE id = ?2")
                    .bind(new_retry_count)
                    .bind(job.id.0.to_string())
                    .execute(pool)
                    .await?;
            }
        }

        // Push back to Redis queue after delay (using a simple sleep for now or a delayed set)
        // For a robust implementation, we'd use a sorted set for delayed jobs.
        // Retry scheduling is best-effort background work: dispatch it through
        // the bounded task queue (as `Low` priority) so a sustained failure
        // storm can't spawn an unbounded number of pending sleep-then-requeue
        // tasks. If the dispatcher is saturated, the retry is dropped — the
        // job stays QUEUED in the database and will still be picked up by a
        // subsequent cleanup/requeue pass.
        let queue = self.clone();
        let id_str = job.id.0.to_string();
        let outcome = self
            .retry_dispatcher
            .dispatch(TaskPriority::Low, async move {
                tokio::time::sleep(Duration::from_secs(delay_secs)).await;
                let mut conn = match queue.redis.get_multiplexed_async_connection().await {
                    Ok(c) => c,
                    Err(_) => return,
                };
                let _: Result<(), _> = conn.lpush("soroscope:jobs:queue", id_str).await;
            })
            .await;

        if outcome == crate::task_queue::DispatchOutcome::Dropped {
            tracing::warn!(job_id = %job.id, "Retry dispatcher saturated; retry scheduling dropped for this attempt");
        }

        tracing::info!(job_id = %job.id, retry_count = new_retry_count, delay_secs, "Job scheduled for retry");
        Ok(())
    }

    /// Spawn a background cleanup task that exits when `shutdown` fires.
    pub fn spawn_cleanup_task(
        &self,
        mut shutdown: tokio::sync::broadcast::Receiver<()>,
    ) -> tokio::task::JoinHandle<()> {
        let queue = self.clone();
        let interval_secs = self.config.cleanup_interval_secs;

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(interval_secs));

            loop {
                tokio::select! {
                    biased;
                    _ = shutdown.recv() => {
                        tracing::info!("Job queue cleanup task shutting down");
                        break;
                    }
                    _ = interval.tick() => {
                        tokio::select! {
                            biased;
                            _ = shutdown.recv() => {
                                tracing::info!("Job queue cleanup task shutting down");
                                break;
                            }
                            result = queue.cleanup() => {
                                if let Err(e) = result {
                                    tracing::error!("Cleanup task error: {}", e);
                                }
                            }
                        }
                    }
                }
            }
        })
    }

    fn row_to_job(&self, row: &sqlx::sqlite::SqliteRow) -> Result<Job, JobError> {
        // Manual mapping for SQLite since FromRow might have issues
        use sqlx::Row;

        let id_str: String = row.try_get("id")?;
        let id = JobId(
            Uuid::parse_str(&id_str)
                .map_err(|_| JobError::ProcessingFailed("Invalid UUID".to_string()))?,
        );

        let job_type_str: String = row.try_get("job_type")?;
        let job_type = match job_type_str.as_str() {
            "analyze" => JobType::Analyze,
            "compare" => JobType::Compare,
            "optimize_limits" => JobType::OptimizeLimits,
            other => {
                return Err(JobError::ProcessingFailed(format!(
                    "Unknown job_type '{}'",
                    other
                )))
            }
        };

        let status_str: String = row.try_get("status")?;
        let status = match status_str.as_str() {
            "QUEUED" => JobStatus::Queued,
            "PROCESSING" => JobStatus::Processing,
            "COMPLETED" => JobStatus::Completed,
            "FAILED" => JobStatus::Failed,
            "CANCELLED" => JobStatus::Cancelled,
            other => {
                return Err(JobError::ProcessingFailed(format!(
                    "Unknown status '{}'",
                    other
                )))
            }
        };

        Ok(Job {
            id,
            job_type,
            status,
            payload: row.try_get("payload").unwrap_or_default(),
            result: row.try_get("result")?,
            progress_percent: row.try_get("progress_percent")?,
            progress_message: row.try_get("progress_message")?,
            webhook_url: row.try_get("webhook_url")?,
            webhook_headers: row.try_get("webhook_headers")?,
            webhook_secret: row.try_get("webhook_secret")?,
            error_message: row.try_get("error_message")?,
            error_type: row.try_get("error_type")?,
            timeout_secs: row.try_get("timeout_secs")?,
            retry_count: row.try_get("retry_count")?,
            created_at: row.try_get("created_at")?,
            started_at: row.try_get("started_at")?,
            completed_at: row.try_get("completed_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl Clone for JobQueue {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            redis: self.redis.clone(),
            config: self.config.clone(),
            retry_dispatcher: self.retry_dispatcher.clone(),
        }
    }
}

/// Request to submit a new job
#[derive(Debug, Deserialize, ToSchema)]
pub struct SubmitJobRequest {
    pub job_type: JobType,
    pub payload: JobPayload,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook: Option<WebhookConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
}

/// Response from submitting a job
#[derive(Debug, Serialize, ToSchema)]
pub struct SubmitJobResponse {
    pub job_id: String,
    pub status: JobStatus,
    pub message: String,
}

#[utoipa::path(
    post,
    path = "/jobs/submit",
    request_body = SubmitJobRequest,
    responses(
        (status = 202, description = "Job accepted", body = SubmitJobResponse),
        (status = 500, description = "Internal server error")
    ),
    tag = "Jobs"
)]
pub async fn submit_job_handler(
    State(state): State<Arc<crate::AppState>>,
    Json(payload): Json<SubmitJobRequest>,
) -> Result<(StatusCode, Json<SubmitJobResponse>), AppError> {
    let job_id = state
        .job_queue
        .submit(payload.job_type, payload.payload, payload.webhook)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok((
        StatusCode::ACCEPTED,
        Json(SubmitJobResponse {
            job_id: job_id.to_string(),
            status: JobStatus::Queued,
            message: "Job submitted successfully".to_string(),
        }),
    ))
}

#[utoipa::path(
    get,
    path = "/jobs/{id}",
    responses(
        (status = 200, description = "Job details", body = Job),
        (status = 404, description = "Job not found")
    ),
    params(
        ("id" = String, Path, description = "Job ID")
    ),
    tag = "Jobs"
)]
pub async fn get_job_handler(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Job>, AppError> {
    let job_id = JobId::from_str(&id).map_err(|_| AppError::BadRequest("Invalid job ID".into()))?;
    let job = state
        .job_queue
        .get(&job_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Job {} not found", id)))?;

    Ok(Json(job))
}

#[utoipa::path(
    post,
    path = "/jobs/{id}/cancel",
    responses(
        (status = 200, description = "Job cancelled", body = Job),
        (status = 400, description = "Job cannot be cancelled"),
        (status = 404, description = "Job not found")
    ),
    params(
        ("id" = String, Path, description = "Job ID")
    ),
    tag = "Jobs"
)]
pub async fn cancel_job_handler(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Job>, AppError> {
    let job_id = JobId::from_str(&id).map_err(|_| AppError::BadRequest("Invalid job ID".into()))?;
    let job = state.job_queue.cancel(&job_id).await.map_err(|e| match e {
        JobError::NotFound(_) => AppError::NotFound(format!("Job {} not found", id)),
        JobError::CannotCancel(_) => AppError::BadRequest(e.to_string()),
        _ => AppError::Internal(e.to_string()),
    })?;

    Ok(Json(job))
}

/// Job worker that processes jobs from the database queue
pub struct JobWorker {
    queue: JobQueue,
    engine: SimulationEngine,
    insights_engine: InsightsEngine,
    config: JobQueueConfig,
    http_client: Client,
    /// Optional pub/sub bus for real-time WebSocket streaming.
    /// When `None` the worker runs in polling-only mode (backwards-compatible).
    bus: Option<Arc<SimulationBus>>,
}

impl JobWorker {
    pub fn new(
        queue: JobQueue,
        engine: SimulationEngine,
        insights_engine: InsightsEngine,
        config: JobQueueConfig,
    ) -> Self {
        Self {
            queue,
            engine,
            insights_engine,
            config,
            http_client: Client::new(),
            bus: None,
        }
    }

    /// Attach a [`SimulationBus`] so the worker publishes real-time events.
    pub fn with_bus(mut self, bus: Arc<SimulationBus>) -> Self {
        self.bus = Some(bus);
        self
    }

    /// Start the worker loop until a shutdown signal is received.
    ///
    /// Redis blocking pops use a short timeout so the loop can observe
    /// shutdown promptly instead of hanging forever on `brpoplpush`.
    pub async fn run(self, mut shutdown: tokio::sync::broadcast::Receiver<()>) {
        let worker_id = Uuid::new_v4().to_string();
        tracing::info!(worker_id = %worker_id, "Job worker started");

        // Spawn heartbeat task (also listens for shutdown)
        let redis_clone = self.queue.redis.clone();
        let worker_id_clone = worker_id.clone();
        let mut heartbeat_shutdown = shutdown.resubscribe();
        let heartbeat_handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(10));
            let mut conn = match redis_clone.get_multiplexed_async_connection().await {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("Heartbeat task failed to get Redis connection: {}", e);
                    return;
                }
            };

            loop {
                tokio::select! {
                    _ = heartbeat_shutdown.recv() => {
                        tracing::info!(worker_id = %worker_id_clone, "Heartbeat task shutting down");
                        break;
                    }
                    _ = interval.tick() => {
                        let key = format!("soroscope:workers:{}:heartbeat", worker_id_clone);
                        let _: Result<(), _> = conn.set_ex(key, "alive", 30).await;
                    }
                }
            }
        });

        let semaphore = Arc::new(tokio::sync::Semaphore::new(self.config.max_concurrent_jobs));

        loop {
            let mut conn = match self.queue.redis.get_multiplexed_async_connection().await {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("Worker failed to get Redis connection: {}", e);
                    tokio::select! {
                        _ = shutdown.recv() => break,
                        _ = tokio::time::sleep(Duration::from_secs(5)) => continue,
                    }
                }
            };

            // Reliability pattern: RPOPLPUSH (or BLMOVE)
            // Pop from main queue and push to processing list.
            // Timeout of 1s lets the worker wake and check shutdown.
            let job_id_res: Result<Option<String>, _> = tokio::select! {
                _ = shutdown.recv() => {
                    tracing::info!(worker_id = %worker_id, "Job worker shutting down");
                    break;
                }
                result = conn.brpoplpush(
                    "soroscope:jobs:queue",
                    "soroscope:jobs:processing",
                    1.0,
                ) => result,
            };

            match job_id_res {
                Ok(Some(id_str)) => {
                    let job_id = match JobId::from_str(&id_str) {
                        Ok(id) => id,
                        Err(_) => continue,
                    };

                    let permit = match semaphore.clone().acquire_owned().await {
                        Ok(p) => p,
                        Err(e) => {
                            tracing::error!("Failed to acquire semaphore: {}", e);
                            continue;
                        }
                    };

                    let queue = self.queue.clone();
                    let engine = self.engine.clone();
                    let insights = self.insights_engine.clone();
                    let config = self.config.clone();
                    let http_client = self.http_client.clone();
                    let bus = self.bus.clone();
                    let id_str_clone = id_str.clone();

                    tokio::spawn(async move {
                        let _permit = permit;

                        if let Err(e) = Self::process_job(
                            &queue,
                            job_id,
                            engine,
                            insights,
                            config,
                            http_client,
                            bus,
                        )
                        .await
                        {
                            tracing::error!("Job processing error: {}", e);
                        }

                        // Clean up processing list after completion
                        let mut conn = match queue.redis.get_multiplexed_async_connection().await {
                            Ok(c) => c,
                            Err(_) => return,
                        };
                        let _: Result<(), _> = conn
                            .lrem("soroscope:jobs:processing", 1, id_str_clone)
                            .await;
                    });
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::error!("Error fetching next job from Redis: {}", e);
                    tokio::select! {
                        _ = shutdown.recv() => break,
                        _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                    }
                }
            }
        }

        // Ensure the heartbeat task exits with the worker.
        let _ = heartbeat_handle.await;
        tracing::info!(worker_id = %worker_id, "Job worker stopped");
    }

    async fn process_job(
        queue: &JobQueue,
        job_id: JobId,
        engine: SimulationEngine,
        insights_engine: InsightsEngine,
        config: JobQueueConfig,
        http_client: Client,
        bus: Option<Arc<SimulationBus>>,
    ) -> Result<(), JobError> {
        let job = queue
            .get(&job_id)
            .await?
            .ok_or(JobError::NotFound(job_id))?;
        tracing::info!(job_id = %job.id, "Processing job");

        // Mark as processing and emit first progress event
        queue.mark_processing(&job.id).await?;
        if let Some(b) = &bus {
            b.publish(SimulationBus::progress(&job.id, 10, "Processing started"));
        }

        // Process with timeout
        let timeout = Duration::from_secs(job.timeout_secs as u64);
        let result = tokio::time::timeout(
            timeout,
            Self::execute_job(&job, &engine, &insights_engine, queue, bus.clone()),
        )
        .await;

        // Handle result, emit terminal event, and optionally send webhook
        match result {
            Ok(Ok(job_result)) => {
                queue.complete(&job.id, &job_result).await?;

                // Emit completed event with resource summary
                if let Some(b) = &bus {
                    if let JobResult::Success {
                        simulation_result: Some(ref sim),
                        ..
                    } = job_result
                    {
                        b.publish(SimulationBus::completed(
                            &job.id,
                            &sim.resources,
                            sim.cost_stroops,
                        ));
                    } else {
                        // OptimizeLimits / Compare jobs: emit a generic completion
                        b.publish(SimulationBus::progress(&job.id, 100, "Completed"));
                    }
                }

                if let Some(webhook_config) = job.get_webhook_config() {
                    Self::send_webhook(
                        &http_client,
                        &webhook_config,
                        &job.id,
                        JobStatus::Completed,
                        Some(&job_result),
                        config.webhook_timeout_secs,
                        config.webhook_max_retries,
                    )
                    .await;
                }
            }
            Ok(Err(e)) => {
                let error_msg = e.to_string();
                queue.fail(&job.id, &error_msg, "ProcessingError").await?;

                // Attempt retry
                let _ = queue.retry_job(&job).await;

                if let Some(b) = &bus {
                    b.publish(SimulationBus::failed(
                        &job.id,
                        &error_msg,
                        "ProcessingError",
                    ));
                }

                if let Some(webhook_config) = job.get_webhook_config() {
                    Self::send_webhook(
                        &http_client,
                        &webhook_config,
                        &job.id,
                        JobStatus::Failed,
                        None,
                        config.webhook_timeout_secs,
                        config.webhook_max_retries,
                    )
                    .await;
                }
            }
            Err(_) => {
                let error_msg = format!("Job timed out after {} seconds", job.timeout_secs);
                queue.fail(&job.id, &error_msg, "Timeout").await?;

                // Attempt retry
                let _ = queue.retry_job(&job).await;

                if let Some(b) = &bus {
                    b.publish(SimulationBus::failed(&job.id, &error_msg, "Timeout"));
                }

                if let Some(webhook_config) = job.get_webhook_config() {
                    Self::send_webhook(
                        &http_client,
                        &webhook_config,
                        &job.id,
                        JobStatus::Failed,
                        None,
                        config.webhook_timeout_secs,
                        config.webhook_max_retries,
                    )
                    .await;
                }
            }
        }

        Ok(())
    }

    async fn execute_job(
        job: &Job,
        engine: &SimulationEngine,
        insights_engine: &InsightsEngine,
        queue: &JobQueue,
        bus: Option<Arc<SimulationBus>>,
    ) -> Result<JobResult, Box<dyn std::error::Error + Send + Sync>> {
        let payload = job.get_payload().ok_or("Invalid payload")?;

        /// Helper: update DB progress and publish WebSocket event simultaneously.
        macro_rules! progress {
            ($percent:expr, $msg:expr) => {{
                let _ = queue.update_progress(&job.id, $percent, $msg).await;
                if let Some(ref b) = bus {
                    b.publish(SimulationBus::progress(&job.id, $percent, $msg));
                }
            }};
        }

        match payload {
            JobPayload::Analyze {
                contract_id,
                function_name,
                args,
                ledger_overrides,
            } => {
                progress!(30, "Running simulation");

                let sim_result = engine
                    .simulate_from_contract_id(
                        &contract_id,
                        &function_name,
                        args.unwrap_or_default(),
                        ledger_overrides,
                        None,
                        None,
                    )
                    .await
                    .map_err(|e| {
                        // If this was a provider failover during simulation, emit the
                        // event so WebSocket clients see the provider switch in real time.
                        // (The SimulationEngine surfaces failovers via its error chain;
                        // here we inspect the message for a best-effort broadcast.)
                        let msg = e.to_string();
                        if let Some(ref b) = bus {
                            if msg.contains("failover") || msg.contains("provider") {
                                b.publish(SimulationBus::provider_failover(
                                    &job.id,
                                    "unknown",
                                    "next-available",
                                    &msg,
                                ));
                            }
                        }
                        e
                    })?;

                // If the engine ran in consensus mode, surface the quorum result.
                // (SimulationEngine sets `sim_result.provider_name` when consensus
                //  succeeded; we broadcast an agreement event here.)
                if let Some(ref b) = bus {
                    b.publish(SimulationBus::consensus_check(
                        &job.id,
                        true,   // reached this point → consensus passed (or failover mode)
                        vec![], // provider names are opaque at this layer
                        None,
                    ));
                }

                progress!(70, "Generating insights");
                let _insights = insights_engine.analyze(&sim_result.resources);

                progress!(90, "Finalizing results");

                Ok(JobResult::Success {
                    resources: Some(sim_result.resources.clone()),
                    simulation_result: Some(sim_result),
                    optimization: None,
                    comparison: None,
                })
            }
            JobPayload::OptimizeLimits {
                contract_id,
                function_name,
                args,
                safety_margin,
            } => {
                progress!(30, "Running optimization");

                let report = engine
                    .optimize_limits(&contract_id, &function_name, args, safety_margin)
                    .await?;

                progress!(90, "Finalizing results");

                Ok(JobResult::Success {
                    resources: None,
                    simulation_result: None,
                    optimization: Some(serde_json::to_value(report)?),
                    comparison: None,
                })
            }
            _ => Ok(JobResult::Success {
                resources: None,
                simulation_result: None,
                optimization: None,
                comparison: Some(serde_json::json!({"status": "Not fully implemented"})),
            }),
        }
    }

    async fn send_webhook(
        client: &Client,
        config: &WebhookConfig,
        job_id: &JobId,
        status: JobStatus,
        result: Option<&JobResult>,
        timeout_secs: u64,
        max_retries: u32,
    ) {
        let payload = serde_json::json!({
            "job_id": job_id.to_string(),
            "status": status,
            "result": result,
            "timestamp": Utc::now().to_rfc3339(),
        });

        let timeout = Duration::from_secs(timeout_secs);
        let mut last_error = None;

        for attempt in 1..=max_retries {
            let mut request = client
                .post(&config.callback_url)
                .json(&payload)
                .timeout(timeout);

            // Add custom headers if provided
            if let Some(headers) = &config.headers {
                for (key, value) in headers {
                    request = request.header(key, value);
                }
            }

            match request.send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        tracing::info!(job_id = %job_id, attempt, "Webhook delivered");
                        return;
                    } else {
                        last_error = Some(format!("HTTP {}", response.status()));
                    }
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                }
            }

            if attempt < max_retries {
                tokio::time::sleep(Duration::from_millis(1000 * 2_u64.pow(attempt - 1))).await;
            }
        }

        tracing::error!(job_id = %job_id, error = ?last_error, "Webhook failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // `JobQueue::new` runs the checked-in Postgres-dialect migration SQL
    // (`CREATE OR REPLACE FUNCTION ... plpgsql`), which SQLite cannot parse.
    // These tests exercise `list`/`row_to_job` directly against a hand-rolled
    // SQLite-compatible schema instead, so they cover the actual query and
    // row-mapping logic without depending on that unrelated, pre-existing
    // migration gap.
    async fn sqlite_pool_with_jobs_table() -> sqlx::SqlitePool {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite pool");
        sqlx::query(
            r#"
            CREATE TABLE jobs (
                id TEXT PRIMARY KEY,
                job_type TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'QUEUED',
                payload TEXT NOT NULL,
                result TEXT,
                progress_percent INTEGER NOT NULL DEFAULT 0,
                progress_message TEXT NOT NULL DEFAULT 'Queued',
                webhook_url TEXT,
                webhook_headers TEXT,
                webhook_secret TEXT,
                error_message TEXT,
                error_type TEXT,
                timeout_secs INTEGER NOT NULL DEFAULT 300,
                retry_count INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                started_at TEXT,
                completed_at TEXT,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .expect("create jobs table");
        pool
    }

    fn test_queue(pool: sqlx::SqlitePool) -> JobQueue {
        JobQueue {
            pool: DbPool::Sqlite(pool),
            redis: RedisClient::open("redis://127.0.0.1:6379").expect("parse redis url"),
            config: JobQueueConfig::default(),
            retry_dispatcher: crate::task_queue::BoundedTaskDispatcher::new(8),
        }
    }

    async fn insert_job(
        pool: &sqlx::SqlitePool,
        job_type: &JobType,
        status: &str,
        payload: &JobPayload,
    ) -> JobId {
        let id = JobId::new();
        let now = Utc::now().to_rfc3339();
        let payload_json = serde_json::to_value(payload).unwrap();
        sqlx::query(
            "INSERT INTO jobs (id, job_type, status, payload, progress_message, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, 'Queued', ?5, ?5)",
        )
        .bind(id.0.to_string())
        .bind(job_type)
        .bind(status)
        .bind(payload_json)
        .bind(now)
        .execute(pool)
        .await
        .expect("insert job");
        id
    }

    fn analyze_payload(contract_id: &str) -> JobPayload {
        JobPayload::Analyze {
            contract_id: contract_id.to_string(),
            function_name: "hello".to_string(),
            args: None,
            ledger_overrides: None,
        }
    }

    #[tokio::test]
    async fn list_round_trips_job_type_and_status_from_sqlite() {
        let pool = sqlite_pool_with_jobs_table().await;
        insert_job(
            &pool,
            &JobType::OptimizeLimits,
            "COMPLETED",
            &JobPayload::OptimizeLimits {
                contract_id: "CABC".into(),
                function_name: "swap".into(),
                args: vec![],
                safety_margin: 0.05,
            },
        )
        .await;
        let queue = test_queue(pool);

        let jobs = queue
            .list(&JobListFilter::default(), 10, 0)
            .await
            .expect("list should succeed");

        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].job_type, JobType::OptimizeLimits);
        assert_eq!(jobs[0].status, JobStatus::Completed);
    }

    #[tokio::test]
    async fn list_filters_by_status_and_job_type() {
        let pool = sqlite_pool_with_jobs_table().await;
        insert_job(
            &pool,
            &JobType::Analyze,
            "COMPLETED",
            &analyze_payload("CABC"),
        )
        .await;
        insert_job(&pool, &JobType::Analyze, "FAILED", &analyze_payload("CXYZ")).await;
        let queue = test_queue(pool);

        let completed = queue
            .list(
                &JobListFilter {
                    status: Some(JobStatus::Completed),
                    ..Default::default()
                },
                10,
                0,
            )
            .await
            .unwrap();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].status, JobStatus::Completed);
    }

    #[tokio::test]
    async fn list_filters_by_contract_id() {
        let pool = sqlite_pool_with_jobs_table().await;
        insert_job(
            &pool,
            &JobType::Analyze,
            "COMPLETED",
            &analyze_payload("CABC"),
        )
        .await;
        insert_job(
            &pool,
            &JobType::Analyze,
            "COMPLETED",
            &analyze_payload("CXYZ"),
        )
        .await;
        let queue = test_queue(pool);

        let filtered = queue
            .list(
                &JobListFilter {
                    contract_id: Some("CXYZ".to_string()),
                    ..Default::default()
                },
                10,
                0,
            )
            .await
            .unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(
            filtered[0].get_payload().unwrap().contract_id(),
            Some("CXYZ".to_string()).as_deref()
        );
    }

    #[tokio::test]
    async fn list_respects_limit_and_offset() {
        let pool = sqlite_pool_with_jobs_table().await;
        for i in 0..5 {
            insert_job(
                &pool,
                &JobType::Analyze,
                "COMPLETED",
                &analyze_payload(&format!("C{i}")),
            )
            .await;
        }
        let queue = test_queue(pool);

        let page = queue.list(&JobListFilter::default(), 2, 1).await.unwrap();
        assert_eq!(page.len(), 2);
    }

    #[test]
    fn job_payload_contract_id_and_function_name_accessors() {
        let analyze = analyze_payload("CABC");
        assert_eq!(analyze.contract_id(), Some("CABC"));
        assert_eq!(analyze.function_name(), Some("hello"));

        let compare_local = JobPayload::Compare {
            mode: "local_vs_local".into(),
            current_wasm: None,
            base_wasm: None,
            contract_id: None,
            function_name: None,
            args: vec![],
        };
        assert_eq!(compare_local.contract_id(), None);
    }
}
