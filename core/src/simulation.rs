use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use soroban_sdk::xdr::{
    Hash, HostFunction, InvokeContractArgs, InvokeHostFunctionOp, LedgerKey, Limits, Memo,
    MuxedAccount, Operation, OperationBody, Preconditions, ReadXdr, ScAddress, ScSymbol, ScVal,
    SequenceNumber, SorobanAuthorizationEntry, SorobanTransactionData, StringM, Transaction,
    TransactionExt, TransactionV1Envelope, Uint256, VecM, WriteXdr,
};
use std::path::Path;
use stellar_strkey::Strkey;
use thiserror::Error;
use tokio::fs;

/// Errors that can occur during simulation
#[derive(Error, Debug)]
pub enum SimulationError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("RPC request failed: {0}")]
    RpcRequestFailed(String),

    #[error("RPC node timeout")]
    NodeTimeout,

    #[error("Invalid contract: {0}")]
    InvalidContract(String),

    #[error("Invalid WASM file: {0}")]
    InvalidWasm(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("Base64 decode error: {0}")]
    Base64Error(#[from] base64::DecodeError),

    #[error("XDR decode error: {0}")]
    XdrError(String),
}

/// Soroban resource consumption data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SorobanResources {
    /// CPU instructions consumed
    pub cpu_instructions: u64,
    /// RAM bytes consumed
    pub ram_bytes: u64,
    /// Ledger read bytes
    pub ledger_read_bytes: u64,
    /// Ledger write bytes
    pub ledger_write_bytes: u64,
    /// Transaction size in bytes
    pub transaction_size_bytes: u64,
}

impl Default for SorobanResources {
    fn default() -> Self {
        Self {
            cpu_instructions: 0,
            ram_bytes: 0,
            ledger_read_bytes: 0,
            ledger_write_bytes: 0,
            transaction_size_bytes: 0,
        }
    }
}

/// Complete simulation result including resources and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    /// Resource consumption metrics
    pub resources: SorobanResources,
    /// Transaction hash (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_hash: Option<String>,
    /// Latest ledger at time of simulation
    pub latest_ledger: u64,
    /// Estimated cost in stroops
    pub cost_stroops: u64,
}

/// RPC request for simulating a transaction
#[derive(Debug, Serialize)]
struct SimulateTransactionRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    params: SimulateTransactionParams,
}

#[derive(Debug, Serialize)]
struct SimulateTransactionParams {
    transaction: String,
}

/// RPC response from simulateTransaction endpoint
#[derive(Debug, Deserialize)]
struct SimulateTransactionResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    id: u64,
    #[serde(flatten)]
    result: ResponseResult,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ResponseResult {
    Success { result: SimulationRpcResult },
    Error { error: RpcError },
}

#[derive(Debug, Deserialize)]
struct RpcError {
    code: i32,
    message: String,
    #[serde(default)]
    #[allow(dead_code)]
    data: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SimulationRpcResult {
    #[serde(default)]
    transaction_data: String,
    #[serde(default)]
    latest_ledger: u64,
    #[serde(default)]
    cost: Option<ResourceCost>,
    #[serde(default)]
    #[allow(dead_code)]
    results: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResourceCost {
    cpu_insns: String,
    mem_bytes: String,
}

/// Soroban RPC simulation engine
pub struct SimulationEngine {
    rpc_url: String,
    client: Client,
    request_timeout: std::time::Duration,
}

impl SimulationEngine {
    /// Create a new simulation engine
    ///
    /// # Arguments
    /// * `rpc_url` - The Soroban RPC endpoint URL (e.g., https://soroban-testnet.stellar.org)
    pub fn new(rpc_url: String) -> Self {
        Self {
            rpc_url,
            client: Client::new(),
            request_timeout: std::time::Duration::from_secs(30),
        }
    }

    /// Set custom request timeout
    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Simulate transaction from a WASM file
    ///
    /// # Arguments
    /// * `wasm_path` - Path to the .wasm contract file
    ///
    /// # Returns
    /// A `Result` containing `SimulationResult` on success, or `SimulationError` on failure
    pub async fn simulate_from_wasm<P: AsRef<Path>>(
        &self,
        wasm_path: P,
    ) -> Result<SimulationResult, SimulationError> {
        // Read WASM file
        let wasm_bytes = fs::read(wasm_path.as_ref()).await.map_err(|e| {
            SimulationError::InvalidWasm(format!("Failed to read WASM file: {}", e))
        })?;

        // Validate WASM
        self.validate_wasm(&wasm_bytes)?;

        // Encode WASM to base64 for transmission
        let wasm_base64 = BASE64.encode(&wasm_bytes);

        // Create transaction envelope (simplified for simulation)
        let transaction_xdr = self.create_upload_transaction(&wasm_base64)?;

        // Simulate via RPC
        self.simulate_transaction(&transaction_xdr).await
    }

    /// Simulate transaction from a deployed contract ID
    ///
    /// # Arguments
    /// * `contract_id` - The contract ID (e.g., C...)
    /// * `function_name` - Function to invoke
    /// * `args` - Function arguments (XDR encoded)
    ///
    /// # Returns
    /// A `Result` containing `SimulationResult` on success, or `SimulationError` on failure
    pub async fn simulate_from_contract_id(
        &self,
        contract_id: &str,
        function_name: &str,
        args: Vec<String>,
    ) -> Result<SimulationResult, SimulationError> {
        if contract_id.is_empty() {
            return Err(SimulationError::InvalidContract(
                "Contract ID cannot be empty".to_string(),
            ));
        }

        // Create invoke transaction
        let transaction_xdr = self.create_invoke_transaction(contract_id, function_name, args)?;

        // Simulate via RPC
        self.simulate_transaction(&transaction_xdr).await
    }

    /// Core simulation method that calls the RPC endpoint
    async fn simulate_transaction(
        &self,
        transaction_xdr: &str,
    ) -> Result<SimulationResult, SimulationError> {
        let request = SimulateTransactionRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "simulateTransaction".to_string(),
            params: SimulateTransactionParams {
                transaction: transaction_xdr.to_string(),
            },
        };

        tracing::debug!("Sending simulateTransaction request to {}", self.rpc_url);

        let response = tokio::time::timeout(
            self.request_timeout,
            self.client
                .post(&self.rpc_url)
                .json(&request)
                .send(),
        )
        .await
        .map_err(|_| SimulationError::NodeTimeout)?
        .map_err(|e| {
            if e.is_timeout() {
                SimulationError::NodeTimeout
            } else if e.is_connect() {
                SimulationError::NetworkError(e)
            } else {
                SimulationError::RpcRequestFailed(format!("Network error: {}", e))
            }
        })?;

        // Check HTTP status
        if !response.status().is_success() {
            return Err(SimulationError::RpcRequestFailed(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        let rpc_response: SimulateTransactionResponse = response
            .json()
            .await
            .map_err(|e| SimulationError::RpcRequestFailed(format!("Failed to parse response: {}", e)))?;

        // Handle RPC errors
        match rpc_response.result {
            ResponseResult::Error { error } => {
                tracing::error!("RPC error (code {}): {}", error.code, error.message);
                
                // Specific error handling
                match error.code {
                    -32600 => Err(SimulationError::InvalidContract(
                        "Invalid request format".to_string(),
                    )),
                    -32601 => Err(SimulationError::RpcRequestFailed(
                        "Method not found".to_string(),
                    )),
                    -32602 => Err(SimulationError::InvalidContract(
                        format!("Invalid parameters: {}", error.message),
                    )),
                    -32603 => Err(SimulationError::RpcRequestFailed(
                        format!("Internal error: {}", error.message),
                    )),
                    _ => Err(SimulationError::RpcRequestFailed(format!(
                        "RPC error {}: {}",
                        error.code, error.message
                    ))),
                }
            }
            ResponseResult::Success { result } => {
                tracing::info!("Simulation successful at ledger {}", result.latest_ledger);
                self.parse_simulation_result(result)
            }
        }
    }

    /// Parse RPC simulation result into our internal data model
    fn parse_simulation_result(
        &self,
        rpc_result: SimulationRpcResult,
    ) -> Result<SimulationResult, SimulationError> {
        let resources = if let Some(cost) = rpc_result.cost {
            // Parse CPU instructions
            let cpu_instructions = cost
                .cpu_insns
                .parse::<u64>()
                .unwrap_or_else(|_| {
                    tracing::warn!("Failed to parse cpu_insns, using 0");
                    0
                });

            // Parse memory bytes
            let ram_bytes = cost
                .mem_bytes
                .parse::<u64>()
                .unwrap_or_else(|_| {
                    tracing::warn!("Failed to parse mem_bytes, using 0");
                    0
                });

            // Extract footprint information from transaction_data
            let (ledger_read_bytes, ledger_write_bytes) = 
                self.extract_footprint_from_xdr(&rpc_result.transaction_data);

            SorobanResources {
                cpu_instructions,
                ram_bytes,
                ledger_read_bytes,
                ledger_write_bytes,
                transaction_size_bytes: rpc_result.transaction_data.len() as u64,
            }
        } else {
            tracing::warn!("No cost data in simulation result, using defaults");
            SorobanResources::default()
        };

        // Calculate estimated cost (simplified formula)
        let cost_stroops = self.calculate_cost(&resources);

        Ok(SimulationResult {
            resources,
            transaction_hash: None,
            latest_ledger: rpc_result.latest_ledger,
            cost_stroops,
        })
    }

    /// Extract ledger footprint from XDR transaction data
    ///
    /// Decodes the base64-encoded SorobanTransactionData XDR and extracts
    /// the read and write byte sizes from the footprint.
    fn extract_footprint_from_xdr(&self, transaction_data: &str) -> (u64, u64) {
        if transaction_data.is_empty() {
            tracing::debug!("Empty transaction data, returning zero footprint");
            return (0, 0);
        }

        // Decode base64 XDR string
        let xdr_bytes = match BASE64.decode(transaction_data) {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::warn!("Failed to decode base64 transaction data: {}", e);
                return (0, 0);
            }
        };

        // Parse the SorobanTransactionData XDR structure
        let soroban_data = match SorobanTransactionData::from_xdr(&xdr_bytes, Limits::none()) {
            Ok(data) => data,
            Err(e) => {
                tracing::warn!("Failed to parse SorobanTransactionData XDR: {}", e);
                return (0, 0);
            }
        };

        // Extract footprint from resources
        let footprint = &soroban_data.resources.footprint;

        // Calculate read bytes from read_only entries
        let read_bytes = self.calculate_ledger_keys_size(&footprint.read_only);

        // Calculate write bytes from read_write entries
        let write_bytes = self.calculate_ledger_keys_size(&footprint.read_write);

        tracing::debug!(
            "Extracted footprint: read_only={} keys ({} bytes), read_write={} keys ({} bytes)",
            footprint.read_only.len(),
            read_bytes,
            footprint.read_write.len(),
            write_bytes
        );

        (read_bytes, write_bytes)
    }

    /// Calculate the estimated size of ledger keys in bytes
    fn calculate_ledger_keys_size(
        &self,
        ledger_keys: &soroban_sdk::xdr::VecM<LedgerKey>,
    ) -> u64 {
        let mut total_bytes: u64 = 0;

        for ledger_key in ledger_keys.iter() {
            // Estimate size based on ledger key type
            let key_size = match ledger_key {
                LedgerKey::Account(_) => {
                    // Account keys are relatively small (account ID + sequence)
                    56 // Approximate size
                }
                LedgerKey::Trustline(_) => {
                    // Trustline keys include account + asset
                    72
                }
                LedgerKey::ContractData(contract_data) => {
                    // ContractData includes contract ID + key + durability
                    // Size varies based on the key complexity
                    let base_size = 32 + 4; // Contract ID + durability enum
                    let key_estimate = self.estimate_scval_size(&contract_data.key);
                    base_size + key_estimate
                }
                LedgerKey::ContractCode(_) => {
                    // ContractCode is just the hash
                    32
                }
                LedgerKey::Offer(_) => 48,
                LedgerKey::Data(_) => 64,
                LedgerKey::ClaimableBalance(_) => 36,
                LedgerKey::LiquidityPool(_) => 32,
                LedgerKey::ConfigSetting(_) => 8,
                LedgerKey::Ttl(_) => 32,
            };
            total_bytes += key_size as u64;
        }

        total_bytes
    }

    /// Estimate the size of an ScVal in bytes
    fn estimate_scval_size(&self, scval: &soroban_sdk::xdr::ScVal) -> u64 {
        use soroban_sdk::xdr::ScVal;

        match scval {
            ScVal::Bool(_) => 1,
            ScVal::Void => 0,
            ScVal::Error(_) => 8,
            ScVal::U32(_) | ScVal::I32(_) => 4,
            ScVal::U64(_) | ScVal::I64(_) => 8,
            ScVal::Timepoint(_) | ScVal::Duration(_) => 8,
            ScVal::U128(_) | ScVal::I128(_) => 16,
            ScVal::U256(_) | ScVal::I256(_) => 32,
            ScVal::Bytes(bytes) => bytes.len() as u64,
            ScVal::String(s) => s.len() as u64,
            ScVal::Symbol(sym) => sym.len() as u64,
            ScVal::Vec(Some(vec)) => {
                vec.iter().map(|v| self.estimate_scval_size(v)).sum::<u64>() + 4
            }
            ScVal::Vec(None) => 4,
            ScVal::Map(Some(map)) => {
                map.iter()
                    .map(|entry| {
                        self.estimate_scval_size(&entry.key)
                            + self.estimate_scval_size(&entry.val)
                    })
                    .sum::<u64>()
                    + 4
            }
            ScVal::Map(None) => 4,
            ScVal::Address(_) => 32,
            ScVal::LedgerKeyContractInstance => 32,
            ScVal::LedgerKeyNonce(_) => 32,
            ScVal::ContractInstance(_) => 64, // Estimate for contract instance
        }
    }

    /// Calculate estimated cost in stroops
    fn calculate_cost(&self, resources: &SorobanResources) -> u64 {
        // Simplified cost calculation
        // Real formula involves network fees, resource fees, etc.
        let cpu_cost = resources.cpu_instructions / 10000;
        let ram_cost = resources.ram_bytes / 1024;
        let ledger_cost = (resources.ledger_read_bytes + resources.ledger_write_bytes) / 1024;
        
        cpu_cost + ram_cost + ledger_cost
    }

    /// Validate WASM bytecode
    fn validate_wasm(&self, wasm: &[u8]) -> Result<(), SimulationError> {
        if wasm.is_empty() {
            return Err(SimulationError::InvalidWasm(
                "WASM bytecode is empty".to_string(),
            ));
        }

        // Check WASM magic number (0x00 0x61 0x73 0x6D)
        if wasm.len() < 4 || &wasm[0..4] != b"\0asm" {
            return Err(SimulationError::InvalidWasm(
                "Invalid WASM magic number".to_string(),
            ));
        }

        Ok(())
    }

    /// Create a simplified upload transaction for WASM simulation
    ///
    /// Creates a transaction with InvokeHostFunctionOp containing UploadWasm host function.
    /// Uses a placeholder source account since simulation doesn't require a real signature.
    fn create_upload_transaction(&self, wasm_base64: &str) -> Result<String, SimulationError> {
        // Decode the WASM from base64
        let wasm_bytes = BASE64
            .decode(wasm_base64)
            .map_err(|e| SimulationError::XdrError(format!("Failed to decode WASM base64: {}", e)))?;

        // Create the UploadWasm host function
        let host_function = HostFunction::UploadContractWasm(
            wasm_bytes
                .try_into()
                .map_err(|_| SimulationError::InvalidWasm("WASM too large".to_string()))?,
        );

        // Build the transaction with a placeholder source account
        self.build_invoke_host_function_transaction(host_function, vec![])
    }

    /// Create invoke transaction for contract call
    ///
    /// Creates a transaction with InvokeHostFunctionOp containing InvokeContract host function.
    fn create_invoke_transaction(
        &self,
        contract_id: &str,
        function_name: &str,
        args: Vec<String>,
    ) -> Result<String, SimulationError> {
        // Parse the contract ID (C... strkey format) to bytes
        let contract_hash = self.parse_contract_id(contract_id)?;

        // Create the contract address
        let contract_address = ScAddress::Contract(Hash(contract_hash));

        // Convert function name to ScSymbol
        let func_symbol: ScSymbol = function_name
            .try_into()
            .map_err(|_| SimulationError::InvalidContract("Invalid function name".to_string()))?;

        // Convert string arguments to ScVal (currently supporting basic types)
        let sc_args: VecM<ScVal> = args
            .iter()
            .map(|arg| self.parse_sc_val_arg(arg))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| SimulationError::InvalidContract("Too many arguments".to_string()))?;

        // Create the InvokeContract host function
        let host_function = HostFunction::InvokeContract(InvokeContractArgs {
            contract_address,
            function_name: func_symbol,
            args: sc_args,
        });

        // Build the transaction (auth will be populated after simulation)
        self.build_invoke_host_function_transaction(host_function, vec![])
    }

    /// Build a transaction envelope with an InvokeHostFunctionOp
    fn build_invoke_host_function_transaction(
        &self,
        host_function: HostFunction,
        auth: Vec<SorobanAuthorizationEntry>,
    ) -> Result<String, SimulationError> {
        // Create the InvokeHostFunctionOp
        let invoke_op = InvokeHostFunctionOp {
            host_function,
            auth: auth
                .try_into()
                .map_err(|_| SimulationError::XdrError("Too many auth entries".to_string()))?,
        };

        // Create operation with the invoke host function
        let operation = Operation {
            source_account: None, // Use transaction source
            body: OperationBody::InvokeHostFunction(invoke_op),
        };

        // Create a placeholder source account (32 zero bytes for simulation)
        // In a real scenario, this would be the actual account public key
        let source_account = MuxedAccount::Ed25519(Uint256([0u8; 32]));

        // Build the transaction
        let transaction = Transaction {
            source_account,
            fee: 100, // Base fee in stroops
            seq_num: SequenceNumber(0), // Placeholder sequence number
            cond: Preconditions::None,
            memo: Memo::None,
            operations: vec![operation]
                .try_into()
                .map_err(|_| SimulationError::XdrError("Failed to create operations".to_string()))?,
            ext: TransactionExt::V0,
        };

        // Wrap in a transaction envelope (unsigned for simulation)
        let envelope = TransactionV1Envelope {
            tx: transaction,
            signatures: VecM::default(), // No signatures needed for simulation
        };

        // Encode to XDR and then base64
        let xdr_bytes = envelope
            .to_xdr(Limits::none())
            .map_err(|e| SimulationError::XdrError(format!("Failed to encode XDR: {}", e)))?;

        Ok(BASE64.encode(&xdr_bytes))
    }

    /// Parse a contract ID from strkey format (C...) to raw bytes
    fn parse_contract_id(&self, contract_id: &str) -> Result<[u8; 32], SimulationError> {
        // Contract IDs start with 'C' in strkey format
        if !contract_id.starts_with('C') {
            return Err(SimulationError::InvalidContract(
                "Contract ID must start with 'C'".to_string(),
            ));
        }

        // Use stellar-strkey crate to decode
        let strkey = Strkey::from_string(contract_id).map_err(|e| {
            SimulationError::InvalidContract(format!("Invalid contract ID format: {}", e))
        })?;

        match strkey {
            Strkey::Contract(contract) => Ok(contract.0),
            _ => Err(SimulationError::InvalidContract(
                "Expected contract address".to_string(),
            )),
        }
    }

    /// Parse a string argument into an ScVal
    ///
    /// Supports common formats:
    /// - Integers: "123" -> ScVal::I128 or ScVal::U64
    /// - Booleans: "true"/"false" -> ScVal::Bool
    /// - Addresses: "G..." or "C..." -> ScVal::Address
    /// - Symbols: ":symbol_name" -> ScVal::Symbol
    /// - Strings: "\"text\"" -> ScVal::String
    /// - Hex bytes: "0x..." -> ScVal::Bytes
    fn parse_sc_val_arg(&self, arg: &str) -> Result<ScVal, SimulationError> {
        let arg = arg.trim();

        // Boolean
        if arg == "true" {
            return Ok(ScVal::Bool(true));
        }
        if arg == "false" {
            return Ok(ScVal::Bool(false));
        }

        // Void
        if arg == "void" || arg == "()" {
            return Ok(ScVal::Void);
        }

        // Symbol (prefixed with :)
        if let Some(sym) = arg.strip_prefix(':') {
            let symbol: ScSymbol = sym
                .try_into()
                .map_err(|_| SimulationError::InvalidContract("Invalid symbol".to_string()))?;
            return Ok(ScVal::Symbol(symbol));
        }

        // Hex bytes (prefixed with 0x)
        if let Some(hex_str) = arg.strip_prefix("0x") {
            let bytes = hex::decode(hex_str).map_err(|e| {
                SimulationError::InvalidContract(format!("Invalid hex bytes: {}", e))
            })?;
            return Ok(ScVal::Bytes(
                bytes
                    .try_into()
                    .map_err(|_| SimulationError::InvalidContract("Bytes too large".to_string()))?,
            ));
        }

        // Quoted string
        if arg.starts_with('"') && arg.ends_with('"') && arg.len() >= 2 {
            let s = &arg[1..arg.len() - 1];
            let string_m: StringM = s
                .as_bytes()
                .to_vec()
                .try_into()
                .map_err(|_| SimulationError::InvalidContract("String too large".to_string()))?;
            return Ok(ScVal::String(string_m.into()));
        }

        // Address (G... for account, C... for contract)
        if arg.starts_with('G') || arg.starts_with('C') {
            let address = self.parse_address(arg)?;
            return Ok(ScVal::Address(address));
        }

        // Try parsing as integer
        if let Ok(n) = arg.parse::<i64>() {
            return Ok(ScVal::I64(n));
        }
        if let Ok(n) = arg.parse::<u64>() {
            return Ok(ScVal::U64(n));
        }

        // Default: treat as symbol
        let symbol: ScSymbol = arg
            .try_into()
            .map_err(|_| SimulationError::InvalidContract("Cannot parse argument".to_string()))?;
        Ok(ScVal::Symbol(symbol))
    }

    /// Parse an address string to ScAddress
    fn parse_address(&self, address: &str) -> Result<ScAddress, SimulationError> {
        let strkey = Strkey::from_string(address).map_err(|e| {
            SimulationError::InvalidContract(format!("Invalid address format: {}", e))
        })?;

        match strkey {
            Strkey::Contract(contract) => Ok(ScAddress::Contract(Hash(contract.0))),
            Strkey::PublicKeyEd25519(pubkey) => Ok(ScAddress::Account(
                soroban_sdk::xdr::AccountId(soroban_sdk::xdr::PublicKey::PublicKeyTypeEd25519(
                    Uint256(pubkey.0),
                )),
            )),
            _ => Err(SimulationError::InvalidContract(
                "Address must be a contract (C...) or account (G...) address".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_soroban_resources_default() {
        let resources = SorobanResources::default();
        assert_eq!(resources.cpu_instructions, 0);
        assert_eq!(resources.ram_bytes, 0);
        assert_eq!(resources.ledger_read_bytes, 0);
        assert_eq!(resources.ledger_write_bytes, 0);
    }

    #[test]
    fn test_soroban_resources_serialization() {
        let resources = SorobanResources {
            cpu_instructions: 1000000,
            ram_bytes: 2048,
            ledger_read_bytes: 512,
            ledger_write_bytes: 256,
            transaction_size_bytes: 1024,
        };

        let json = serde_json::to_string(&resources).unwrap();
        assert!(json.contains("\"cpu_instructions\":1000000"));
        assert!(json.contains("\"ram_bytes\":2048"));
        assert!(json.contains("\"ledger_read_bytes\":512"));
        assert!(json.contains("\"ledger_write_bytes\":256"));

        let deserialized: SorobanResources = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, resources);
    }

    #[test]
    fn test_simulation_engine_creation() {
        let engine = SimulationEngine::new("https://soroban-testnet.stellar.org".to_string());
        assert_eq!(engine.rpc_url, "https://soroban-testnet.stellar.org");
    }

    #[test]
    fn test_simulation_engine_with_timeout() {
        let timeout = std::time::Duration::from_secs(60);
        let engine = SimulationEngine::new("https://soroban-testnet.stellar.org".to_string())
            .with_timeout(timeout);
        assert_eq!(engine.request_timeout, timeout);
    }

    #[test]
    fn test_validate_wasm_empty() {
        let engine = SimulationEngine::new("https://test.com".to_string());
        let result = engine.validate_wasm(&[]);
        assert!(matches!(result, Err(SimulationError::InvalidWasm(_))));
    }

    #[test]
    fn test_validate_wasm_invalid_magic() {
        let engine = SimulationEngine::new("https://test.com".to_string());
        let result = engine.validate_wasm(b"invalid");
        assert!(matches!(result, Err(SimulationError::InvalidWasm(_))));
    }

    #[test]
    fn test_validate_wasm_valid() {
        let engine = SimulationEngine::new("https://test.com".to_string());
        let result = engine.validate_wasm(b"\0asm\x01\0\0\0");
        assert!(result.is_ok());
    }

    #[test]
    fn test_calculate_cost() {
        let engine = SimulationEngine::new("https://test.com".to_string());
        let resources = SorobanResources {
            cpu_instructions: 1000000,
            ram_bytes: 2048,
            ledger_read_bytes: 512,
            ledger_write_bytes: 512,
            transaction_size_bytes: 1024,
        };
        let cost = engine.calculate_cost(&resources);
        assert!(cost > 0);
    }

    #[tokio::test]
    async fn test_simulate_from_contract_id_empty() {
        let engine = SimulationEngine::new("https://test.com".to_string());
        let result = engine
            .simulate_from_contract_id("", "test_function", vec![])
            .await;
        assert!(matches!(result, Err(SimulationError::InvalidContract(_))));
    }

    #[test]
    fn test_simulation_error_display() {
        let err = SimulationError::NodeTimeout;
        assert_eq!(err.to_string(), "RPC node timeout");

        let err = SimulationError::InvalidContract("test".to_string());
        assert_eq!(err.to_string(), "Invalid contract: test");

        let err = SimulationError::XdrError("invalid xdr".to_string());
        assert_eq!(err.to_string(), "XDR decode error: invalid xdr");
    }

    #[test]
    fn test_extract_footprint_empty_data() {
        let engine = SimulationEngine::new("https://test.com".to_string());
        let (read, write) = engine.extract_footprint_from_xdr("");
        assert_eq!(read, 0);
        assert_eq!(write, 0);
    }

    #[test]
    fn test_extract_footprint_invalid_base64() {
        let engine = SimulationEngine::new("https://test.com".to_string());
        let (read, write) = engine.extract_footprint_from_xdr("not-valid-base64!!!");
        assert_eq!(read, 0);
        assert_eq!(write, 0);
    }

    #[test]
    fn test_extract_footprint_invalid_xdr() {
        let engine = SimulationEngine::new("https://test.com".to_string());
        // Valid base64 but invalid XDR
        let (read, write) = engine.extract_footprint_from_xdr("SGVsbG8gV29ybGQ=");
        assert_eq!(read, 0);
        assert_eq!(write, 0);
    }

    #[test]
    fn test_estimate_scval_size_primitives() {
        use soroban_sdk::xdr::ScVal;

        let engine = SimulationEngine::new("https://test.com".to_string());

        assert_eq!(engine.estimate_scval_size(&ScVal::Bool(true)), 1);
        assert_eq!(engine.estimate_scval_size(&ScVal::Void), 0);
        assert_eq!(engine.estimate_scval_size(&ScVal::U32(42)), 4);
        assert_eq!(engine.estimate_scval_size(&ScVal::I32(-42)), 4);
        assert_eq!(engine.estimate_scval_size(&ScVal::U64(1000)), 8);
        assert_eq!(engine.estimate_scval_size(&ScVal::I64(-1000)), 8);
    }

    #[test]
    fn test_parse_sc_val_arg_bool() {
        let engine = SimulationEngine::new("https://test.com".to_string());

        let result = engine.parse_sc_val_arg("true").unwrap();
        assert!(matches!(result, ScVal::Bool(true)));

        let result = engine.parse_sc_val_arg("false").unwrap();
        assert!(matches!(result, ScVal::Bool(false)));
    }

    #[test]
    fn test_parse_sc_val_arg_void() {
        let engine = SimulationEngine::new("https://test.com".to_string());

        let result = engine.parse_sc_val_arg("void").unwrap();
        assert!(matches!(result, ScVal::Void));

        let result = engine.parse_sc_val_arg("()").unwrap();
        assert!(matches!(result, ScVal::Void));
    }

    #[test]
    fn test_parse_sc_val_arg_symbol() {
        let engine = SimulationEngine::new("https://test.com".to_string());

        let result = engine.parse_sc_val_arg(":my_symbol").unwrap();
        assert!(matches!(result, ScVal::Symbol(_)));
    }

    #[test]
    fn test_parse_sc_val_arg_integer() {
        let engine = SimulationEngine::new("https://test.com".to_string());

        let result = engine.parse_sc_val_arg("42").unwrap();
        assert!(matches!(result, ScVal::I64(42)));

        let result = engine.parse_sc_val_arg("-100").unwrap();
        assert!(matches!(result, ScVal::I64(-100)));
    }

    #[test]
    fn test_parse_sc_val_arg_hex_bytes() {
        let engine = SimulationEngine::new("https://test.com".to_string());

        let result = engine.parse_sc_val_arg("0xdeadbeef").unwrap();
        assert!(matches!(result, ScVal::Bytes(_)));
    }

    #[test]
    fn test_parse_contract_id_valid() {
        let engine = SimulationEngine::new("https://test.com".to_string());

        // Valid contract ID format
        let contract_id = "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC";
        let result = engine.parse_contract_id(contract_id);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 32);
    }

    #[test]
    fn test_parse_contract_id_invalid_prefix() {
        let engine = SimulationEngine::new("https://test.com".to_string());

        let result = engine.parse_contract_id("GDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC");
        assert!(matches!(result, Err(SimulationError::InvalidContract(_))));
    }

    #[test]
    fn test_parse_address_contract() {
        let engine = SimulationEngine::new("https://test.com".to_string());

        let contract_id = "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC";
        let result = engine.parse_address(contract_id);
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), ScAddress::Contract(_)));
    }

    #[test]
    fn test_parse_address_account() {
        let engine = SimulationEngine::new("https://test.com".to_string());

        let account_id = "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN7";
        let result = engine.parse_address(account_id);
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), ScAddress::Account(_)));
    }

    #[test]
    fn test_create_upload_transaction() {
        let engine = SimulationEngine::new("https://test.com".to_string());

        // Valid WASM bytes encoded in base64
        let wasm_base64 = BASE64.encode(b"\0asm\x01\0\0\0");
        let result = engine.create_upload_transaction(&wasm_base64);
        assert!(result.is_ok());

        // The result should be a valid base64 string
        let xdr_base64 = result.unwrap();
        assert!(!xdr_base64.is_empty());
        assert!(BASE64.decode(&xdr_base64).is_ok());
    }

    #[test]
    fn test_create_invoke_transaction() {
        let engine = SimulationEngine::new("https://test.com".to_string());

        let contract_id = "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC";
        let function_name = "hello";
        let args = vec!["true".to_string(), "42".to_string()];

        let result = engine.create_invoke_transaction(contract_id, function_name, args);
        assert!(result.is_ok());

        // The result should be a valid base64 string
        let xdr_base64 = result.unwrap();
        assert!(!xdr_base64.is_empty());
        assert!(BASE64.decode(&xdr_base64).is_ok());
    }
}
