use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use thiserror::Error;
use tracing;
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

/// Thread-safe fee data store backed by SQLite/PostgreSQL
pub struct FeeStore {
    pool: SqlitePool,
}

impl FeeStore {
    /// Create a new fee store with the given database pool
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
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

    #[test]
    fn test_transaction_fee_record_creation() {
        let record = TransactionFeeRecord::new(12345, "abc123".to_string(), 100, 100, 50, true);

        assert_eq!(record.ledger_sequence, 12345);
        assert_eq!(record.tx_hash, "abc123");
        assert_eq!(record.fee_bid, 100);
        assert!(!record.id.is_empty());
    }
}
