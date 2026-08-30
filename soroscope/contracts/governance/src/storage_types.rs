use soroban_sdk::{contracttype, Address, String, Vec};

#[derive(Clone)]
#[contracttype]
pub enum ProposalState {
    Draft,
    Voting,
    Queued,
    Executed,
    Cancelled,
}

#[derive(Clone)]
#[contracttype]
pub struct ProposalAction {
    pub contract_id: Address,
    pub function_name: String,
    pub args: Vec<String>, // Simplified, in real implementation might need proper types
}

#[derive(Clone)]
#[contracttype]
pub struct Proposal {
    pub id: u32,
    pub proposer: Address,
    pub title: String,
    pub description: String,
    pub actions: Vec<ProposalAction>,
    pub state: ProposalState,
    pub start_time: u64,
    pub end_time: u64,
    pub queued_time: u64,
    pub executed_time: u64,
    pub for_votes: i128,
    pub against_votes: i128,
    pub quorum_required: i128,
}

#[derive(Clone)]
#[contracttype]
pub struct GovernanceConfig {
    pub admin: Address,
    pub voting_period: u64, // in seconds
    pub timelock_delay: u64, // in seconds
    pub quorum_percentage: u32, // percentage of total voting power needed
    pub proposal_count: u32,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Proposal(u32),
    VotingPower(Address),
    DelegatedPower(Address), // delegator -> delegate
    Delegate(Address), // delegate -> total delegated power
    Config,
    HasVoted(u32, Address), // proposal_id, voter -> has_voted
}