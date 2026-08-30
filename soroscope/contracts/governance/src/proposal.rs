use crate::storage_types::{DataKey, Proposal, ProposalAction, ProposalState};
use soroban_sdk::{Address, Env, String, Vec};

pub fn read_proposal(e: &Env, id: u32) -> Proposal {
    let key = DataKey::Proposal(id);
    e.storage().persistent().get(&key).unwrap()
}

pub fn write_proposal(e: &Env, proposal: &Proposal) {
    let key = DataKey::Proposal(proposal.id);
    e.storage().persistent().set(&key, proposal);
}

pub fn create_proposal(
    e: &Env,
    proposer: Address,
    title: String,
    description: String,
    actions: Vec<ProposalAction>,
) -> u32 {
    let mut config = crate::admin::read_config(e);
    config.proposal_count += 1;
    let proposal_id = config.proposal_count;
    crate::admin::write_config(e, &config);

    let proposal = Proposal {
        id: proposal_id,
        proposer,
        title,
        description,
        actions,
        state: ProposalState::Draft,
        start_time: 0,
        end_time: 0,
        queued_time: 0,
        executed_time: 0,
        for_votes: 0,
        against_votes: 0,
        quorum_required: 0,
    };

    write_proposal(e, &proposal);
    proposal_id
}

pub fn start_voting(e: &Env, proposal_id: u32) {
    let mut proposal = read_proposal(e, proposal_id);
    assert!(matches!(proposal.state, ProposalState::Draft));

    let config = crate::admin::read_config(e);
    let current_time = e.ledger().timestamp();

    proposal.state = ProposalState::Voting;
    proposal.start_time = current_time;
    proposal.end_time = current_time + config.voting_period;
    proposal.quorum_required = crate::voting::calculate_quorum(e, &config);

    write_proposal(e, &proposal);
}

pub fn vote(e: &Env, proposal_id: u32, voter: Address, support: bool) {
    let mut proposal = read_proposal(e, proposal_id);
    assert!(matches!(proposal.state, ProposalState::Voting));
    assert!(!crate::voting::has_voted(e, proposal_id, voter.clone()));

    let current_time = e.ledger().timestamp();
    assert!(current_time >= proposal.start_time && current_time <= proposal.end_time);

    let voting_power = crate::voting::get_effective_voting_power(e, voter.clone());

    if support {
        proposal.for_votes += voting_power;
    } else {
        proposal.against_votes += voting_power;
    }

    crate::voting::set_voted(e, proposal_id, voter);
    write_proposal(e, &proposal);
}

pub fn queue_proposal(e: &Env, proposal_id: u32) {
    let mut proposal = read_proposal(e, proposal_id);
    assert!(matches!(proposal.state, ProposalState::Voting));

    let current_time = e.ledger().timestamp();
    assert!(current_time > proposal.end_time);

    // Check if proposal passed
    assert!(proposal.for_votes > proposal.against_votes);
    assert!(proposal.for_votes >= proposal.quorum_required);

    proposal.state = ProposalState::Queued;
    proposal.queued_time = current_time;

    write_proposal(e, &proposal);
}

pub fn execute_proposal(e: &Env, proposal_id: u32) {
    let mut proposal = read_proposal(e, proposal_id);
    assert!(matches!(proposal.state, ProposalState::Queued));

    let config = crate::admin::read_config(e);
    let current_time = e.ledger().timestamp();
    assert!(current_time >= proposal.queued_time + config.timelock_delay);

    // Execute actions (simplified - in real implementation, would call contracts)
    for action in proposal.actions.iter() {
        // Here we would execute the contract call
        // For now, just log it
        e.events().publish(("proposal_executed", proposal_id), action.clone());
    }

    proposal.state = ProposalState::Executed;
    proposal.executed_time = current_time;

    write_proposal(e, &proposal);
}

pub fn cancel_proposal(e: &Env, proposal_id: u32) {
    let mut proposal = read_proposal(e, proposal_id);
    assert!(!matches!(proposal.state, ProposalState::Executed));

    proposal.state = ProposalState::Cancelled;
    write_proposal(e, &proposal);
}