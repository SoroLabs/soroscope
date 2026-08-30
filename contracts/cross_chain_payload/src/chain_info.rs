use soroban_sdk::{contracttype, BytesN, String};

/// Identifies a blockchain network
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChainInfo {
    /// Unique chain identifier (e.g., chain ID as defined by the network)
    pub chain_id: u64,
    /// Human-readable name of the chain (e.g., "stellar", "ethereum")
    pub chain_name: String,
    /// Network version or fork identifier
    pub network_version: u32,
    /// Contract registry or bridge contract identifier on this chain
    pub bridge_contract: BytesN<32>,
    /// Consensus round or epoch number
    pub consensus_round: u64,
    /// Whether this chain is active in the cross-chain network
    pub is_active: bool,
}

/// Represents a bridge endpoint between two chains
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeEndpoint {
    /// Source chain information
    pub source_chain: ChainInfo,
    /// Destination chain information
    pub destination_chain: ChainInfo,
    /// Bridge fee percentage (e.g., 100 = 1%)
    pub fee_percentage: u32,
    /// Minimum liquidity required for bridge
    pub min_liquidity: i128,
    /// Whether the bridge is enabled
    pub is_enabled: bool,
}

/// Represents a validator set for a specific chain
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatorSet {
    /// Chain this validator set is for
    pub chain_info: ChainInfo,
    /// Number of validators required to reach consensus
    pub quorum_threshold: u32,
    /// Total number of validators in the set
    pub total_validators: u32,
    /// Hash of the current validator list
    pub validator_list_hash: BytesN<32>,
    /// Version of this validator set
    pub version: u32,
}
