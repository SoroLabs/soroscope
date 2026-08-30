use crate::fee_store::{FeeStore, LedgerFeeSample};
use crate::rpc_provider::ProviderRegistry;
use chrono::Utc;
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tracing;

/// Errors that can occur during fee collection
#[derive(Error, Debug)]
pub enum FeeCollectorError {
    #[error("RPC request failed: {0}")]
    RpcRequestFailed(String),

    #[error("RPC node timeout")]
    NodeTimeout,

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Store error: {0}")]
    StoreError(String),

    #[error("No healthy providers available")]
    NoHealthyProviders,
}

/// Configuration for the fee collector
#[derive(Debug, Clone)]
pub struct FeeCollectorConfig {
    /// How often to collect fee data (in seconds)
    pub collection_interval_secs: u64,
    /// How many ledgers to fetch in one batch
    pub batch_size: u64,
    /// Request timeout
    pub request_timeout: Duration,
}

impl Default for FeeCollectorConfig {
    fn default() -> Self {
        Self {
            collection_interval_secs: 5, // Stellar ledgers close every ~5 seconds
            batch_size: 10,
            request_timeout: Duration::from_secs(10),
        }
    }
}

/// Fee collector that polls RPC nodes for ledger fee data
pub struct FeeCollector {
    registry: Arc<ProviderRegistry>,
    store: Arc<FeeStore>,
    client: Client,
    config: FeeCollectorConfig,
    last_collected_sequence: std::sync::atomic::AtomicU64,
}

impl FeeCollector {
    /// Create a new fee collector
    pub fn new(
        registry: Arc<ProviderRegistry>,
        store: Arc<FeeStore>,
        config: FeeCollectorConfig,
    ) -> Self {
        Self {
            registry,
            store,
            client: Client::builder()
                .timeout(config.request_timeout)
                .build()
                .expect("Failed to create HTTP client"),
            config,
            last_collected_sequence: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Run the background collection loop
    pub async fn run_collection_loop(self: Arc<Self>) {
        let mut interval =
            tokio::time::interval(Duration::from_secs(self.config.collection_interval_secs));

        tracing::info!(
            interval_secs = self.config.collection_interval_secs,
            "Fee collector started"
        );

        loop {
            interval.tick().await;

            if let Err(e) = self.collect_latest_fees().await {
                tracing::error!(error = %e, "Failed to collect fee data");
            }
        }
    }

    /// Collect fee data from the latest ledger
    async fn collect_latest_fees(&self) -> Result<(), FeeCollectorError> {
        // Get latest ledger sequence
        let latest_sequence = self.get_latest_ledger_sequence().await?;

        let last_collected = self
            .last_collected_sequence
            .load(std::sync::atomic::Ordering::Relaxed);

        // Skip if we've already collected this ledger
        if latest_sequence <= last_collected {
            tracing::debug!(
                latest = latest_sequence,
                last_collected = last_collected,
                "No new ledgers to collect"
            );
            return Ok(());
        }

        // Fetch ledger details
        let sample = self.fetch_ledger_fee_data(latest_sequence).await?;

        // Store in database
        self.store
            .upsert_ledger_sample(&sample)
            .await
            .map_err(|e| FeeCollectorError::StoreError(e.to_string()))?;

        // Update last collected sequence
        self.last_collected_sequence
            .store(latest_sequence, std::sync::atomic::Ordering::Relaxed);

        tracing::info!(
            ledger = latest_sequence,
            base_fee = sample.base_fee,
            transaction_count = sample.transaction_count,
            "Collected fee data"
        );

        Ok(())
    }

    /// Get the latest ledger sequence from RPC
    async fn get_latest_ledger_sequence(&self) -> Result<u64, FeeCollectorError> {
        let providers = self.registry.healthy_providers().await;
        if providers.is_empty() {
            return Err(FeeCollectorError::NoHealthyProviders);
        }

        // Try the first healthy provider
        let provider = &providers[0];

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getLatestLedger",
            "params": null
        });

        let mut req = self.client.post(&provider.url).json(&body);

        // Attach auth headers if present
        if let (Some(header), Some(value)) = (&provider.auth_header, &provider.auth_value) {
            req = req.header(header.as_str(), value.as_str());
        }

        let response = req
            .send()
            .await
            .map_err(|e| FeeCollectorError::RpcRequestFailed(e.to_string()))?;

        if !response.status().is_success() {
            return Err(FeeCollectorError::RpcRequestFailed(format!(
                "HTTP {}",
                response.status()
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| FeeCollectorError::ParseError(e.to_string()))?;

        let sequence = json["result"]["sequence"].as_u64().ok_or_else(|| {
            FeeCollectorError::ParseError("Missing sequence in response".to_string())
        })?;

        Ok(sequence)
    }

    /// Fetch detailed fee data for a specific ledger
    async fn fetch_ledger_fee_data(
        &self,
        sequence: u64,
    ) -> Result<LedgerFeeSample, FeeCollectorError> {
        let providers = self.registry.healthy_providers().await;
        if providers.is_empty() {
            return Err(FeeCollectorError::NoHealthyProviders);
        }

        let provider = &providers[0];

        // Try getLedgers endpoint first (if available)
        // Fallback to parsing from getTransactions if needed
        match self.fetch_from_get_ledgers(provider, sequence).await {
            Ok(sample) => Ok(sample),
            Err(e) => {
                tracing::warn!(error = %e, "getLedgers not available, using fallback");
                self.fetch_from_get_transactions(provider, sequence).await
            }
        }
    }

    /// Fetch fee data using getLedgers RPC method
    async fn fetch_from_get_ledgers(
        &self,
        provider: &crate::rpc_provider::RpcProvider,
        sequence: u64,
    ) -> Result<LedgerFeeSample, FeeCollectorError> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getLedgers",
            "params": {
                "startLedger": sequence,
                "limit": 1
            }
        });

        let mut req = self.client.post(&provider.url).json(&body);

        if let (Some(header), Some(value)) = (&provider.auth_header, &provider.auth_value) {
            req = req.header(header.as_str(), value.as_str());
        }

        let response = req
            .send()
            .await
            .map_err(|e| FeeCollectorError::RpcRequestFailed(e.to_string()))?;

        if !response.status().is_success() {
            return Err(FeeCollectorError::RpcRequestFailed(format!(
                "HTTP {}",
                response.status()
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| FeeCollectorError::ParseError(e.to_string()))?;

        // Parse ledger data from response
        let ledgers = json["result"]["ledgers"]
            .as_array()
            .ok_or_else(|| FeeCollectorError::ParseError("Missing ledgers array".to_string()))?;

        if ledgers.is_empty() {
            return Err(FeeCollectorError::ParseError(
                "No ledger data returned".to_string(),
            ));
        }

        let ledger = &ledgers[0];
        self.parse_ledger_sample(sequence, ledger)
    }

    /// Fallback: fetch fee data using getTransactions
    async fn fetch_from_get_transactions(
        &self,
        provider: &crate::rpc_provider::RpcProvider,
        sequence: u64,
    ) -> Result<LedgerFeeSample, FeeCollectorError> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getTransactions",
            "params": {
                "startLedger": sequence,
                "limit": 100
            }
        });

        let mut req = self.client.post(&provider.url).json(&body);

        if let (Some(header), Some(value)) = (&provider.auth_header, &provider.auth_value) {
            req = req.header(header.as_str(), value.as_str());
        }

        let response = req
            .send()
            .await
            .map_err(|e| FeeCollectorError::RpcRequestFailed(e.to_string()))?;

        if !response.status().is_success() {
            return Err(FeeCollectorError::RpcRequestFailed(format!(
                "HTTP {}",
                response.status()
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| FeeCollectorError::ParseError(e.to_string()))?;

        let transactions = json["result"]["transactions"].as_array().ok_or_else(|| {
            FeeCollectorError::ParseError("Missing transactions array".to_string())
        })?;

        // Calculate fee statistics from transactions
        let mut total_fee_charged: i64 = 0;
        let mut max_fee: i64 = 0;
        let mut tx_count: i32 = 0;

        for tx in transactions {
            if let Some(fee_charged) = tx["feeCharged"].as_str() {
                if let Ok(fee) = fee_charged.parse::<i64>() {
                    total_fee_charged += fee;
                    max_fee = max_fee.max(fee);
                    tx_count += 1;
                }
            }
        }

        let avg_fee = if tx_count > 0 {
            total_fee_charged / tx_count as i64
        } else {
            100 // Default base fee
        };

        Ok(LedgerFeeSample {
            ledger_sequence: sequence as i64,
            collected_at: Utc::now(),
            base_reserve: 0, // Not available from this endpoint
            base_fee: avg_fee,
            max_fee: if max_fee > 0 { max_fee } else { avg_fee },
            fee_charged: total_fee_charged,
            transaction_count: tx_count,
            ledger_close_time: Utc::now(),
        })
    }

    /// Parse ledger sample from getLedgers response
    fn parse_ledger_sample(
        &self,
        sequence: u64,
        ledger: &serde_json::Value,
    ) -> Result<LedgerFeeSample, FeeCollectorError> {
        let base_fee = ledger["header"]["baseFee"]
            .as_str()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(100); // Default base fee

        let base_reserve = ledger["header"]["baseReserve"]
            .as_str()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);

        let tx_count = ledger["header"]["txSetSize"].as_u64().unwrap_or(0) as i32;

        // Parse close time
        let close_time_str = ledger["header"]["closeTime"].as_str().unwrap_or("0");

        let close_timestamp = close_time_str.parse::<i64>().unwrap_or(0);

        let ledger_close_time = if close_timestamp > 0 {
            chrono::DateTime::from_timestamp(close_timestamp, 0).unwrap_or_else(Utc::now)
        } else {
            Utc::now()
        };

        Ok(LedgerFeeSample {
            ledger_sequence: sequence as i64,
            collected_at: Utc::now(),
            base_reserve,
            base_fee,
            max_fee: base_fee, // Will be updated from transaction data
            fee_charged: 0,    // Will be calculated from transactions
            transaction_count: tx_count,
            ledger_close_time,
        })
    }

    /// Get the last collected sequence
    pub fn get_last_collected(&self) -> u64 {
        self.last_collected_sequence
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = FeeCollectorConfig::default();
        assert_eq!(config.collection_interval_secs, 5);
        assert_eq!(config.batch_size, 10);
        assert_eq!(config.request_timeout, Duration::from_secs(10));
    }
}
