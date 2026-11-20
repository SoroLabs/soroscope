use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    FromRow, SqlitePool,
};
use std::str::FromStr;
use thiserror::Error;
use utoipa::ToSchema;
use uuid::Uuid;

/// Errors that can occur during fee store operations
#[derive(Error, Debug)]
pub enum FeeStoreError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Record not found: {0}")]
    NotFound(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),
}

/// Historical fee sample from a ledger header
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct LedgerFeeSample {
    pub ledger_sequence: i64,
    pub collected_at: DateTime<Utc>,
    pub base_reserve: i64,
    pub base_fee: i64,
    pub max_fee: i64,
    pub fee_charged: i64,
    pub transaction_count: i32,
    pub ledger_close_time: DateTime<Utc>,
}

/// Individual transaction fee record
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct TransactionFeeRecord {
    pub id: String,
    pub ledger_sequence: i64,
    pub tx_hash: String,
    pub fee_bid: i64,
    pub fee_charged: i64,
    pub resource_fee: i64,
    pub inclusion_success: bool,
    pub recorded_at: DateTime<Utc>,
}

/// Rows deleted per statement by [`FeeStore::run_maintenance`] (Issue #1407).
pub const MAINTENANCE_BATCH_SIZE: i64 = 1000;

/// Outcome of one [`FeeStore::run_maintenance`] pass.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MaintenanceReport {
    /// Ledger fee samples deleted.
    pub samples_deleted: u64,
    /// Transaction fee records deleted.
    pub records_deleted: u64,
    /// `true` when the WAL checkpoint could not complete because readers or
    /// writers were active (it is retried on the next pass).
    pub checkpoint_busy: bool,
}

/// Thread-safe fee data store backed by SQLite
pub struct FeeStore {
    pool: SqlitePool,
}

impl FeeStore {
    /// Create a new fee store, building a connection pool configured for concurrent writes.
    ///
    /// - WAL journal mode: allows concurrent readers alongside a writer, eliminating
    ///   the `SQLITE_BUSY` / "database is locked" errors that occurred in the default
    ///   DELETE mode when multiple async tasks wrote simultaneously.
    /// - `busy_timeout(5000 ms)`: instead of returning `SQLITE_BUSY` immediately,
    ///   SQLite will retry for up to 5 s before giving up, covering brief contention.
    /// - `max_connections(5)`: caps the pool so writers queue rather than race.
    pub async fn connect(database_url: &str) -> Result<Self, FeeStoreError> {
        let options = SqliteConnectOptions::from_str(database_url)?
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(std::time::Duration::from_millis(5000))
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        Ok(Self { pool })
    }

    /// Create a fee store from an already-configured pool (used in tests).
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Returns a reference to the inner connection pool, e.g. for running migrations.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Insert or update a ledger fee sample (upsert)
    pub async fn upsert_ledger_sample(
        &self,
        sample: &LedgerFeeSample,
    ) -> Result<(), FeeStoreError> {
        sqlx::query(
            r#"
            INSERT INTO ledger_fee_samples (
                ledger_sequence, collected_at, base_reserve, base_fee, 
                max_fee, fee_charged, transaction_count, ledger_close_time
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(ledger_sequence) DO UPDATE SET
                collected_at = excluded.collected_at,
                base_reserve = excluded.base_reserve,
                base_fee = excluded.base_fee,
                max_fee = excluded.max_fee,
                fee_charged = excluded.fee_charged,
                transaction_count = excluded.transaction_count,
                ledger_close_time = excluded.ledger_close_time
            "#,
        )
        .bind(sample.ledger_sequence)
        .bind(sample.collected_at)
        .bind(sample.base_reserve)
        .bind(sample.base_fee)
        .bind(sample.max_fee)
        .bind(sample.fee_charged)
        .bind(sample.transaction_count)
        .bind(sample.ledger_close_time)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Insert a transaction fee record
    pub async fn insert_tx_record(
        &self,
        record: &TransactionFeeRecord,
    ) -> Result<(), FeeStoreError> {
        sqlx::query(
            r#"
            INSERT INTO transaction_fee_records (
                id, ledger_sequence, tx_hash, fee_bid, fee_charged,
                resource_fee, inclusion_success, recorded_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )
        .bind(&record.id)
        .bind(record.ledger_sequence)
        .bind(&record.tx_hash)
        .bind(record.fee_bid)
        .bind(record.fee_charged)
        .bind(record.resource_fee)
        .bind(record.inclusion_success)
        .bind(record.recorded_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get recent ledger fee samples (most recent first)
    pub async fn get_recent_samples(
        &self,
        limit: i64,
    ) -> Result<Vec<LedgerFeeSample>, FeeStoreError> {
        let samples = sqlx::query_as::<_, LedgerFeeSample>(
            r#"
            SELECT 
                ledger_sequence, collected_at, base_reserve, base_fee,
                max_fee, fee_charged, transaction_count, ledger_close_time
            FROM ledger_fee_samples
            ORDER BY ledger_sequence DESC
            LIMIT ?1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(samples)
    }

    /// Get ledger samples within a sequence range
    pub async fn get_samples_in_range(
        &self,
        from_sequence: i64,
        to_sequence: i64,
    ) -> Result<Vec<LedgerFeeSample>, FeeStoreError> {
        let samples = sqlx::query_as::<_, LedgerFeeSample>(
            r#"
            SELECT 
                ledger_sequence, collected_at, base_reserve, base_fee,
                max_fee, fee_charged, transaction_count, ledger_close_time
            FROM ledger_fee_samples
            WHERE ledger_sequence BETWEEN ?1 AND ?2
            ORDER BY ledger_sequence ASC
            "#,
        )
        .bind(from_sequence)
        .bind(to_sequence)
        .fetch_all(&self.pool)
        .await?;

        Ok(samples)
    }

    /// Get the latest ledger sequence in the database
    pub async fn get_latest_sequence(&self) -> Result<Option<i64>, FeeStoreError> {
        let latest = sqlx::query_scalar::<_, Option<i64>>(
            r#"
            SELECT MAX(ledger_sequence) as latest FROM ledger_fee_samples
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(latest)
    }

    /// Get transaction records for a specific ledger
    pub async fn get_tx_records_for_ledger(
        &self,
        ledger_sequence: i64,
    ) -> Result<Vec<TransactionFeeRecord>, FeeStoreError> {
        let records = sqlx::query_as::<_, TransactionFeeRecord>(
            r#"
            SELECT 
                id, ledger_sequence, tx_hash, fee_bid, fee_charged,
                resource_fee, inclusion_success, recorded_at
            FROM transaction_fee_records
            WHERE ledger_sequence = ?1
            ORDER BY recorded_at ASC
            "#,
        )
        .bind(ledger_sequence)
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }

    /// Delete old samples beyond retention period
    pub async fn cleanup_old_samples(&self, retention_days: i32) -> Result<u64, FeeStoreError> {
        let result = sqlx::query(
            r#"
            DELETE FROM ledger_fee_samples
            WHERE ledger_close_time < datetime('now', ? || ' days')
            "#,
        )
        .bind(format!("-{}", retention_days))
        .execute(&self.pool)
        .await?;

        let deleted = result.rows_affected();

        if deleted > 0 {
            tracing::info!(deleted, "Cleaned up old fee samples");
        }

        Ok(deleted)
    }

    /// Retention maintenance (Issue #1407).
    ///
    /// Deletes ledger samples and transaction records older than
    /// `retention_days` in bounded batches of `batch_size` rows (one short
    /// transaction per batch, yielding between batches so concurrent readers
    /// and writers are never starved), then truncates the WAL with
    /// `PRAGMA wal_checkpoint(TRUNCATE)` and refreshes query planner stats with
    /// `PRAGMA optimize`.
    ///
    /// Transaction records are pruned first, including any that still point at
    /// an expiring sample, so the foreign key never blocks the sample delete.
    pub async fn run_maintenance(
        &self,
        retention_days: u32,
        batch_size: i64,
    ) -> Result<MaintenanceReport, FeeStoreError> {
        let cutoff = Utc::now() - chrono::Duration::days(i64::from(retention_days));
        let batch_size = batch_size.max(1);
        let mut report = MaintenanceReport::default();

        report.records_deleted = self
            .delete_in_batches(
                r#"
                DELETE FROM transaction_fee_records
                WHERE rowid IN (
                    SELECT rowid FROM transaction_fee_records
                    WHERE recorded_at < ?1
                       OR ledger_sequence IN (
                           SELECT ledger_sequence FROM ledger_fee_samples
                           WHERE ledger_close_time < ?1
                       )
                    LIMIT ?2
                )
                "#,
                cutoff,
                batch_size,
            )
            .await?;

        report.samples_deleted = self
            .delete_in_batches(
                r#"
                DELETE FROM ledger_fee_samples
                WHERE rowid IN (
                    SELECT rowid FROM ledger_fee_samples
                    WHERE ledger_close_time < ?1
                    LIMIT ?2
                )
                "#,
                cutoff,
                batch_size,
            )
            .await?;

        // TRUNCATE resets the WAL file to zero bytes. Readers keep reading
        // throughout; if one is still using the WAL the checkpoint reports
        // busy instead of blocking, and the next pass retries.
        let (busy, _log_frames, _checkpointed) =
            sqlx::query_as::<_, (i64, i64, i64)>("PRAGMA wal_checkpoint(TRUNCATE)")
                .fetch_one(&self.pool)
                .await?;
        report.checkpoint_busy = busy != 0;

        sqlx::query("PRAGMA optimize").execute(&self.pool).await?;

        tracing::info!(
            samples_deleted = report.samples_deleted,
            records_deleted = report.records_deleted,
            checkpoint_busy = report.checkpoint_busy,
            retention_days,
            "Fee store maintenance completed"
        );

        Ok(report)
    }

    /// Run a `DELETE ... LIMIT ?2` statement repeatedly until a batch deletes
    /// fewer than `batch_size` rows. Returns the total rows deleted.
    async fn delete_in_batches(
        &self,
        sql: &str,
        cutoff: DateTime<Utc>,
        batch_size: i64,
    ) -> Result<u64, FeeStoreError> {
        let mut total = 0u64;
        loop {
            let deleted = sqlx::query(sql)
                .bind(cutoff)
                .bind(batch_size)
                .execute(&self.pool)
                .await?
                .rows_affected();
            total += deleted;
            if deleted < batch_size as u64 {
                return Ok(total);
            }
            tokio::task::yield_now().await;
        }
    }

    /// Get count of stored samples
    pub async fn get_sample_count(&self) -> Result<i64, FeeStoreError> {
        let count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) as count FROM ledger_fee_samples
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(count)
    }

    /// Close the database connection pool
    pub async fn close(&self) {
        tracing::info!("Closing FeeStore database connections");
        self.pool.close().await;
        tracing::info!("FeeStore database connections closed");
    }

    /// Batch insert multiple ledger samples
    pub async fn batch_insert_samples(
        &self,
        samples: &[LedgerFeeSample],
    ) -> Result<(), FeeStoreError> {
        if samples.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;

        for sample in samples {
            sqlx::query(
                r#"
                INSERT INTO ledger_fee_samples (
                    ledger_sequence, collected_at, base_reserve, base_fee, 
                    max_fee, fee_charged, transaction_count, ledger_close_time
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ON CONFLICT(ledger_sequence) DO NOTHING
                "#,
            )
            .bind(sample.ledger_sequence)
            .bind(sample.collected_at)
            .bind(sample.base_reserve)
            .bind(sample.base_fee)
            .bind(sample.max_fee)
            .bind(sample.fee_charged)
            .bind(sample.transaction_count)
            .bind(sample.ledger_close_time)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}

/// Helper to create a new transaction fee record with auto-generated ID
impl TransactionFeeRecord {
    pub fn new(
        ledger_sequence: i64,
        tx_hash: String,
        fee_bid: i64,
        fee_charged: i64,
        resource_fee: i64,
        inclusion_success: bool,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            ledger_sequence,
            tx_hash,
            fee_bid,
            fee_charged,
            resource_fee,
            inclusion_success,
            recorded_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
    use std::str::FromStr;
    use std::sync::Arc;

    #[test]
    fn test_transaction_fee_record_creation() {
        let record = TransactionFeeRecord::new(12345, "abc123".to_string(), 100, 100, 50, true);

        assert_eq!(record.ledger_sequence, 12345);
        assert_eq!(record.tx_hash, "abc123");
        assert_eq!(record.fee_bid, 100);
        assert!(!record.id.is_empty());
    }

    /// Verify that concurrent writes to an in-memory SQLite database configured with
    /// WAL mode and a busy timeout produce zero `DatabaseLocked` errors.
    #[tokio::test]
    async fn test_concurrent_writes_no_database_locked() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .unwrap()
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(std::time::Duration::from_millis(5000));

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .expect("in-memory sqlite pool");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS ledger_fee_samples (
                ledger_sequence BIGINT PRIMARY KEY,
                collected_at TEXT NOT NULL,
                base_reserve BIGINT NOT NULL,
                base_fee BIGINT NOT NULL,
                max_fee BIGINT NOT NULL,
                fee_charged BIGINT NOT NULL,
                transaction_count INTEGER NOT NULL,
                ledger_close_time TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create table");

        let store = Arc::new(FeeStore::new(pool));
        let mut handles = Vec::new();

        for i in 0..20_u32 {
            let store = Arc::clone(&store);
            let sample = LedgerFeeSample {
                ledger_sequence: i as i64,
                collected_at: Utc::now(),
                base_reserve: 100,
                base_fee: 100,
                max_fee: 1000,
                fee_charged: 100,
                transaction_count: 1,
                ledger_close_time: Utc::now(),
            };
            handles.push(tokio::spawn(
                async move { store.upsert_ledger_sample(&sample).await },
            ));
        }

        let mut errors = 0u32;
        for handle in handles {
            if let Ok(Err(e)) = handle.await {
                eprintln!("Write error: {e}");
                errors += 1;
            }
        }

        assert_eq!(errors, 0, "expected zero DatabaseLocked errors under concurrent writes");
    }

    // ── Retention maintenance (Issue #1407) ───────────────────────────────

    async fn maintenance_store() -> FeeStore {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .unwrap()
            .journal_mode(SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("in-memory sqlite pool");

        sqlx::query(
            "CREATE TABLE ledger_fee_samples (
                ledger_sequence BIGINT PRIMARY KEY,
                collected_at TEXT NOT NULL,
                base_reserve BIGINT NOT NULL,
                base_fee BIGINT NOT NULL,
                max_fee BIGINT NOT NULL,
                fee_charged BIGINT NOT NULL,
                transaction_count INTEGER NOT NULL,
                ledger_close_time TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .expect("create samples table");
        sqlx::query(
            "CREATE TABLE transaction_fee_records (
                id TEXT PRIMARY KEY,
                ledger_sequence BIGINT NOT NULL,
                tx_hash VARCHAR(64) NOT NULL,
                fee_bid BIGINT NOT NULL,
                fee_charged BIGINT NOT NULL,
                resource_fee BIGINT NOT NULL,
                inclusion_success BOOLEAN NOT NULL,
                recorded_at TIMESTAMP NOT NULL,
                FOREIGN KEY (ledger_sequence) REFERENCES ledger_fee_samples(ledger_sequence)
            )",
        )
        .execute(&pool)
        .await
        .expect("create records table");

        FeeStore::new(pool)
    }

    fn sample_at(ledger_sequence: i64, closed_at: DateTime<Utc>) -> LedgerFeeSample {
        LedgerFeeSample {
            ledger_sequence,
            collected_at: closed_at,
            base_reserve: 100,
            base_fee: 100,
            max_fee: 1000,
            fee_charged: 100,
            transaction_count: 1,
            ledger_close_time: closed_at,
        }
    }

    fn record_at(ledger_sequence: i64, recorded_at: DateTime<Utc>) -> TransactionFeeRecord {
        TransactionFeeRecord {
            recorded_at,
            ..TransactionFeeRecord::new(
                ledger_sequence,
                format!("tx{ledger_sequence}"),
                100,
                100,
                50,
                true,
            )
        }
    }

    #[tokio::test]
    async fn maintenance_purges_old_rows_and_keeps_recent_ones() {
        let store = maintenance_store().await;
        let old = Utc::now() - chrono::Duration::days(40);
        let recent = Utc::now() - chrono::Duration::days(1);

        // Five expired ledgers (with records) and two recent ones.
        for seq in 0..5 {
            store
                .upsert_ledger_sample(&sample_at(seq, old))
                .await
                .unwrap();
            store.insert_tx_record(&record_at(seq, old)).await.unwrap();
        }
        for seq in 100..102 {
            store
                .upsert_ledger_sample(&sample_at(seq, recent))
                .await
                .unwrap();
            store
                .insert_tx_record(&record_at(seq, recent))
                .await
                .unwrap();
        }

        // A batch size of 2 forces several bounded batches per table.
        let report = store.run_maintenance(30, 2).await.expect("maintenance");

        assert_eq!(report.samples_deleted, 5);
        assert_eq!(report.records_deleted, 5);
        assert_eq!(store.get_sample_count().await.unwrap(), 2);
        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transaction_fee_records")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(remaining, 2);
        assert_eq!(store.get_tx_records_for_ledger(100).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn maintenance_removes_recent_records_that_point_at_expired_samples() {
        let store = maintenance_store().await;
        let old = Utc::now() - chrono::Duration::days(40);

        store
            .upsert_ledger_sample(&sample_at(1, old))
            .await
            .unwrap();
        // Recorded recently, but its ledger is past retention.
        store
            .insert_tx_record(&record_at(1, Utc::now()))
            .await
            .unwrap();

        let report = store
            .run_maintenance(30, MAINTENANCE_BATCH_SIZE)
            .await
            .unwrap();

        assert_eq!(report.samples_deleted, 1);
        assert_eq!(report.records_deleted, 1);
        assert_eq!(store.get_sample_count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn maintenance_on_an_empty_store_is_a_no_op() {
        let store = maintenance_store().await;

        let report = store
            .run_maintenance(30, MAINTENANCE_BATCH_SIZE)
            .await
            .unwrap();

        assert_eq!(report.samples_deleted, 0);
        assert_eq!(report.records_deleted, 0);
        assert!(!report.checkpoint_busy);
    }
}
