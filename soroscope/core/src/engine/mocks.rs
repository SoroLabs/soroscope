// core/src/engine/mocks.rs
use super::traits::*;
use async_trait::async_trait;

#[cfg(test)]
pub struct MockProvider {
    pub simulate_result: Option<Result<SimulationRpcResult, ProviderError>>,
    pub ledger_entries_result: Option<Result<Vec<LedgerEntryInfo>, ProviderError>>,
}

#[cfg(test)]
impl MockProvider {
    pub fn new() -> Self {
        Self {
            simulate_result: None,
            ledger_entries_result: None,
        }
    }
    
    pub fn with_simulate_result(mut self, result: Result<SimulationRpcResult, ProviderError>) -> Self {
        self.simulate_result = Some(result);
        self
    }
}

#[cfg(test)]
#[async_trait]
impl SimulationProvider for MockProvider {
    async fn simulate_transaction(
        &self,
        _transaction_xdr: &str,
    ) -> Result<SimulationRpcResult, ProviderError> {
        self.simulate_result.clone().unwrap_or_else(|| {
            Err(ProviderError::RpcRequestFailed("No mock result set".to_string()))
        })
    }
    
    async fn get_ledger_entries(
        &self,
        _keys: &[String],
    ) -> Result<Vec<LedgerEntryInfo>, ProviderError> {
        self.ledger_entries_result.clone().unwrap_or_else(|| {
            Ok(vec![])
        })
    }
}

#[cfg(test)]
pub struct MockCache {
    pub stored: std::collections::HashMap<String, Vec<u8>>,
}

#[cfg(test)]
impl MockCache {
    pub fn new() -> Self {
        Self {
            stored: std::collections::HashMap::new(),
        }
    }
}

#[cfg(test)]
#[async_trait]
impl StateCache for MockCache {
    async fn get(&self, key: &str) -> Option<Vec<u8>> {
        self.stored.get(key).cloned()
    }
    
    async fn set(&self, key: &str, value: Vec<u8>) -> Result<(), CacheError> {
        // In a real mock, we'd need mutability, but for tests we can use Arc<Mutex>
        // This is a simplified version
        Ok(())
    }
    
    async fn invalidate(&self, key: &str) -> Result<(), CacheError> {
        Ok(())
    }
}
