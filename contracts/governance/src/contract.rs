use crate::admin::{get_security_council, read_administrator, read_config, set_security_council, write_administrator, write_config};
use crate::proposal::{cancel_proposal, create_proposal, execute_proposal, queue_proposal, read_proposal, start_voting, veto_proposal};
use crate::storage_types::{GovernanceConfig, Proposal, ProposalAction, ProposalState};
use crate::voting::{delegate_voting_power, get_effective_voting_power, read_voting_power, vote, write_voting_power};
use soroban_sdk::{contract, contractimpl, Address, Env, String, Vec};

pub trait GovernanceTrait {
    // Initialization
    fn initialize(e: Env, admin: Address, voting_period: u64, timelock_delay: u64, quorum_percentage: u32);

    // Admin functions
    fn set_admin(e: Env, new_admin: Address);
    fn set_voting_power(e: Env, user: Address, power: i128);
    fn cancel_proposal_admin(e: Env, proposal_id: u32);
    fn set_security_council(e: Env, security_council: Address);

    // Proposal lifecycle
    fn create_proposal(e: Env, title: String, description: String, actions: Vec<ProposalAction>) -> u32;
    fn start_voting(e: Env, proposal_id: u32);
    fn cast_vote(e: Env, proposal_id: u32, support: bool);
    fn queue_proposal(e: Env, proposal_id: u32);
    fn execute_proposal(e: Env, proposal_id: u32);
    fn veto_proposal(e: Env, proposal_id: u32);

    // Delegation
    fn delegate(e: Env, delegate: Address);

    // Queries
    fn get_proposal(e: Env, proposal_id: u32) -> Proposal;
    fn get_voting_power(e: Env, user: Address) -> i128;
    fn get_effective_voting_power(e: Env, user: Address) -> i128;
    fn get_config(e: Env) -> GovernanceConfig;
    fn get_security_council(e: Env) -> Option<Address>;
}

#[contract]
pub struct Governance;

#[contractimpl]
impl GovernanceTrait for Governance {
    fn initialize(e: Env, admin: Address, voting_period: u64, timelock_delay: u64, quorum_percentage: u32) {
        let config = GovernanceConfig {
            admin,
            voting_period,
            timelock_delay,
            quorum_percentage,
            proposal_count: 0,
        };
        write_config(&e, &config);
    }

    fn set_admin(e: Env, new_admin: Address) {
        let admin = read_administrator(&e);
        admin.require_auth();
        write_administrator(&e, &new_admin);
    }

    fn set_voting_power(e: Env, user: Address, power: i128) {
        let admin = read_administrator(&e);
        admin.require_auth();
        write_voting_power(&e, user, power);
    }

    fn cancel_proposal_admin(e: Env, proposal_id: u32) {
        let admin = read_administrator(&e);
        admin.require_auth();
        cancel_proposal(&e, proposal_id);
    }

    fn set_security_council(e: Env, security_council: Address) {
        let admin = read_administrator(&e);
        admin.require_auth();
        set_security_council(&e, &security_council);
    }

    fn create_proposal(e: Env, title: String, description: String, actions: Vec<ProposalAction>) -> u32 {
        let proposer = e.invoker();
        create_proposal(&e, proposer, title, description, actions)
    }

    fn start_voting(e: Env, proposal_id: u32) {
        let admin = read_administrator(&e);
        admin.require_auth();
        start_voting(&e, proposal_id);
    }

    fn cast_vote(e: Env, proposal_id: u32, support: bool) {
        let voter = e.invoker();
        vote(&e, proposal_id, voter, support);
    }

    fn queue_proposal(e: Env, proposal_id: u32) {
        queue_proposal(&e, proposal_id);
    }

    fn execute_proposal(e: Env, proposal_id: u32) {
        execute_proposal(&e, proposal_id);
    }

    fn veto_proposal(e: Env, proposal_id: u32) {
        let security_council = e.invoker();
        security_council.require_auth();
        veto_proposal(&e, security_council, proposal_id);
    }

    fn delegate(e: Env, delegate: Address) {
        let delegator = e.invoker();
        delegate_voting_power(&e, delegator, delegate);
    }

    fn get_proposal(e: Env, proposal_id: u32) -> Proposal {
        read_proposal(&e, proposal_id)
    }

    fn get_voting_power(e: Env, user: Address) -> i128 {
        read_voting_power(&e, user)
    }

    fn get_effective_voting_power(e: Env, user: Address) -> i128 {
        get_effective_voting_power(&e, user)
    }

    fn get_config(e: Env) -> GovernanceConfig {
        read_config(&e)
    }

    fn get_security_council(e: Env) -> Option<Address> {
        get_security_council(&e)
    }
}