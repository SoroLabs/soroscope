use soroban_sdk::xdr::ScVal;
// core/src/engine/traits.rs
use async_trait::async_trait;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProviderError {
    #[error("RPC request failed: {0}")]
    RpcRequestFailed(String),
    #[error("Node timeout")]
    NodeTimeout,
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Node error: {0}")]
    NodeError(String),
}

#[derive(Error, Debug)]
pub enum CacheError {
    #[error("Cache read failed: {0}")]
    ReadError(String),
    #[error("Cache write failed: {0}")]
    WriteError(String),
}

#[derive(Error, Debug)]
pub enum ParserError {
    #[error("Parse error: {0}")]
    ParseError(String),
}

#[async_trait]
pub trait SimulationProvider: Send + Sync {
    async fn simulate_transaction(
        &self,
        transaction_xdr: &str,
    ) -> Result<SimulationRpcResult, ProviderError>;
    
    async fn get_ledger_entries(
        &self,
        keys: &[String],
    ) -> Result<Vec<LedgerEntryInfo>, ProviderError>;
}

#[async_trait]
pub trait StateCache: Send + Sync {
    async fn get(&self, key: &str) -> Option<Vec<u8>>;
    async fn set(&self, key: &str, value: Vec<u8>) -> Result<(), CacheError>;
    async fn invalidate(&self, key: &str) -> Result<(), CacheError>;
}

pub trait Parser: Send + Sync {
    fn parse_contract_id(&self, contract_id: &str) -> Result<[u8; 32], ParserError>;
    fn parse_sc_val_arg(&self, arg: &str) -> Result<ScVal, ParserError>;
}

// Result types for RPC responses
#[derive(Debug, Clone)]
pub struct SimulationRpcResult {
    pub transaction_data: String,
    pub latest_ledger: u64,
    pub cpu_insns: Option<u64>,
    pub mem_bytes: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct LedgerEntryInfo {
    pub key: String,
    pub live_until_ledger_seq: Option<u32>,
}