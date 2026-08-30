// tests/engine_di_test.rs
use soroscope::engine::{
    SimulationEngine, MockProvider, NoOpCache,
    SimulationRpcResult, ProviderError,
};

#[cfg(test)]
mod tests {
    use super::*;
    
    struct MockParser;
    
    impl soroscope::engine::Parser for MockParser {
        fn parse_contract_id(&self, contract_id: &str) -> Result<[u8; 32], soroscope::engine::ParserError> {
            Ok([0u8; 32])
        }
        
        fn parse_sc_val_arg(&self, arg: &str) -> Result<soroban_sdk::xdr::ScVal, soroscope::engine::ParserError> {
            use soroban_sdk::xdr::ScVal;
            Ok(ScVal::Void)
        }
    }
    
    #[tokio::test]
    async fn test_simulation_with_mock_provider() {
        // Create mock provider with canned response
        let mock_result = SimulationRpcResult {
            transaction_data: "test_data".to_string(),
            latest_ledger: 12345,
            cpu_insns: Some(1000000),
            mem_bytes: Some(1024),
        };
        
        let mock_provider = MockProvider::new()
            .with_simulate_result(Ok(mock_result));
        
        let cache = NoOpCache::new();
        let parser = MockParser;
        
        let engine = SimulationEngine::new(mock_provider, cache, parser);
        
        let result = engine.simulate_transaction("test_xdr").await;
        
        assert!(result.is_ok());
        let simulation_result = result.unwrap();
        assert_eq!(simulation_result.latest_ledger, 12345);
        assert_eq!(simulation_result.resources.cpu_instructions, 1000000);
        assert_eq!(simulation_result.resources.ram_bytes, 1024);
    }
}
