// core/src/engine/real_provider.rs
use super::traits::{SimulationProvider, ProviderError, SimulationRpcResult};
use async_trait::async_trait;
use reqwest::Client;
use std::time::Duration;

pub struct RealRpcProvider {
    client: Client,
    rpc_url: String,
    timeout: Duration,
}

impl RealRpcProvider {
    pub fn new(rpc_url: String) -> Self {
        Self {
            client: Client::new(),
            rpc_url,
            timeout: Duration::from_secs(30),
        }
    }
    
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

#[async_trait]
impl SimulationProvider for RealRpcProvider {
    async fn simulate_transaction(
        &self,
        transaction_xdr: &str,
    ) -> Result<SimulationRpcResult, ProviderError> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "simulateTransaction",
            "params": {
                "transaction": transaction_xdr
            }
        });
        
        let response = tokio::time::timeout(
            self.timeout,
            self.client.post(&self.rpc_url).json(&request).send()
        )
        .await
        .map_err(|_| ProviderError::NodeTimeout)?
        .map_err(|e| ProviderError::NetworkError(e.to_string()))?;
        
        if !response.status().is_success() {
            return Err(ProviderError::RpcRequestFailed(format!(
                "HTTP error: {}",
                response.status()
            )));
        }
        
        #[derive(serde::Deserialize)]
        struct RpcResponse {
            result: SimulateResult,
        }
        
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct SimulateResult {
            transaction_data: String,
            latest_ledger: u64,
            cost: Option<Cost>,
        }
        
        #[derive(serde::Deserialize)]
        struct Cost {
            cpu_insns: String,
            mem_bytes: String,
        }
        
        let rpc_response: RpcResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::RpcRequestFailed(format!("Parse error: {}", e)))?;
        
        Ok(SimulationRpcResult {
            transaction_data: rpc_response.result.transaction_data,
            latest_ledger: rpc_response.result.latest_ledger,
            cpu_insns: rpc_response.result.cost.as_ref().and_then(|c| c.cpu_insns.parse().ok()),
            mem_bytes: rpc_response.result.cost.as_ref().and_then(|c| c.mem_bytes.parse().ok()),
        })
    }
    
    async fn get_ledger_entries(
        &self,
        keys: &[String],
    ) -> Result<Vec<super::traits::LedgerEntryInfo>, ProviderError> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getLedgerEntries",
            "params": {
                "keys": keys
            }
        });
        
        let response = tokio::time::timeout(
            self.timeout,
            self.client.post(&self.rpc_url).json(&request).send()
        )
        .await
        .map_err(|_| ProviderError::NodeTimeout)?
        .map_err(|e| ProviderError::NetworkError(e.to_string()))?;
        
        #[derive(serde::Deserialize)]
        struct RpcResponse {
            result: GetLedgerEntriesResult,
        }
        
        #[derive(serde::Deserialize)]
        struct GetLedgerEntriesResult {
            entries: Vec<LedgerEntry>,
        }
        
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct LedgerEntry {
            key: String,
            live_until_ledger_seq: Option<u32>,
        }
        
        let rpc_response: RpcResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::RpcRequestFailed(format!("Parse error: {}", e)))?;
        
        Ok(rpc_response
            .result
            .entries
            .into_iter()
            .map(|entry| super::traits::LedgerEntryInfo {
                key: entry.key,
                live_until_ledger_seq: entry.live_until_ledger_seq,
            })
            .collect())
    }
}
