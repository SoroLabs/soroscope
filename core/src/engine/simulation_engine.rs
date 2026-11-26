// core/src/engine/simulation_engine.rs
use super::traits::{SimulationProvider, StateCache, Parser, SimulationRpcResult};
use crate::simulation::{
    classify_ledger_key, BytesByDurability, LedgerKeyKind, SimulationError, SimulationResult,
    SorobanResources,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use soroban_sdk::xdr::{Limits, SorobanTransactionData, ReadXdr};

#[allow(dead_code)]
pub struct SimulationEngine<P, C, R> {
    provider: P,
    cache: C,
    #[allow(dead_code)]
    parser: R,
}

impl<P, C, R> SimulationEngine<P, C, R>
where
    P: SimulationProvider,
    C: StateCache,
    R: Parser,
{
    pub fn new(provider: P, cache: C, parser: R) -> Self {
        Self {
            provider,
            cache,
            parser,
        }
    }

    pub async fn simulate_transaction(
        &self,
        transaction_xdr: &str,
    ) -> Result<SimulationResult, SimulationError> {
        // Check cache first
        let cache_key = format!("sim:{}", transaction_xdr);
        if let Some(cached) = self.cache.get(&cache_key).await {
            tracing::debug!("Cache hit for simulation");
            if let Ok(result) = serde_json::from_slice(&cached) {
                return Ok(result);
            }
        }

        // Call provider
        let rpc_result = self.provider
            .simulate_transaction(transaction_xdr)
            .await
            .map_err(|e| SimulationError::RpcRequestFailed(e.to_string()))?;

        // Parse result
        let result = self.parse_simulation_result(rpc_result)?;

        // Cache result
        if let Ok(cached) = serde_json::to_vec(&result) {
            let _ = self.cache.set(&cache_key, cached).await;
        }

        Ok(result)
    }

    fn parse_simulation_result(
        &self,
        rpc_result: SimulationRpcResult,
    ) -> Result<SimulationResult, SimulationError> {
        let (resources, bytes_by_durability) =
            if let (Some(cpu), Some(mem)) = (rpc_result.cpu_insns, rpc_result.mem_bytes) {
                let bytes_by_durability =
                    self.extract_footprint_bytes_by_durability(&rpc_result.transaction_data);
                let ledger_read_bytes = bytes_by_durability.read.total();
                let ledger_write_bytes = bytes_by_durability.write.total();

                (SorobanResources {
                cpu_instructions: cpu,
                ram_bytes: mem,
                ledger_read_bytes,
                ledger_write_bytes,
                transaction_size_bytes: rpc_result.transaction_data.len() as u64,
                                ..Default::default()
}, bytes_by_durability)
            } else {
                (SorobanResources::default(), BytesByDurability::default())
            };
        
        let cost_stroops = self.calculate_cost(&resources);

        Ok(SimulationResult {
            resources,
            bytes_by_durability,
            transaction_hash: None,
            latest_ledger: rpc_result.latest_ledger,
            cost_stroops,
            rent_bytes: None,
            state_dependency: None,
            ttl_analysis: None,
            transaction_data: rpc_result.transaction_data,
            call_graph: None,
            state_snapshot: None,
            protocol_version: 0,
            cost_breakdown: None,
            ..Default::default()
        })
    }

    fn extract_footprint_from_xdr(&self, transaction_data: &str) -> (u64, u64) {
        let bytes = self.extract_footprint_bytes_by_durability(transaction_data);
        (bytes.read.total(), bytes.write.total())
    }

    fn extract_footprint_bytes_by_durability(&self, transaction_data: &str) -> BytesByDurability {
        if transaction_data.is_empty() {
            return BytesByDurability::default();
        }

        let xdr_bytes = match BASE64.decode(transaction_data) {
            Ok(bytes) => bytes,
            Err(_) => return BytesByDurability::default(),
        };

        let soroban_data = match SorobanTransactionData::from_xdr(&xdr_bytes, Limits::none()) {
            Ok(data) => data,
            Err(_) => return BytesByDurability::default(),
        };

        let footprint = &soroban_data.resources.footprint;
        let mut bytes = BytesByDurability::default();
        for key in footprint.read_only.iter() {
            bytes.read.add(classify_ledger_key(key), 64);
        }
        for key in footprint.read_write.iter() {
            bytes.write.add(classify_ledger_key(key), 64);
        }
        bytes
    }

    fn calculate_cost(&self, resources: &SorobanResources) -> u64 {
        let cpu_cost = resources.cpu_instructions / 10000;
        let ram_cost = resources.ram_bytes / 1024;
        let ledger_cost = (resources.ledger_read_bytes + resources.ledger_write_bytes) / 1024;
        cpu_cost + ram_cost + ledger_cost
    }
}
