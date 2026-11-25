#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, BytesN, Env, Map, String, Vec,
};

#[cfg(test)]
mod test;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    ProposalNotFound = 4,
    ProposalClosed = 5,
    InvalidVotingWindow = 6,
    InvalidCredits = 7,
    VoterNotRegistered = 8,
    InsufficientVotingUnits = 9,
    IdentityRequired = 10,
    IdentityAlreadyClaimed = 11,
    VoteSideMismatch = 12,
    NotSecurityCouncil = 13,
    CannotVeto = 14,
    SecurityCouncilNotSet = 15,
    InvalidExecutionThreshold = 16,
    DuplicateAdmin = 17,
    InsufficientApprovals = 18,
    DelegationLoop = 19,
    Overflow = 20,
    QuorumNotMet = 21,
    ProposalNotPassed = 22,
    InvalidQuorum = 23,
    QueueNotReady = 24,
    QueueExpired = 25,
    NotVetoGuardian = 26,
    InvalidProposalState = 27,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum VoteSupport {
    Against = 0,
    For = 1,
    Abstain = 2,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProposalState {
    Active,
    Queued,
    Executed,
    Expired,
    Vetoed,
    Cancelled,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GovernanceConfig {
    pub admin: Address,
    pub min_voting_units: i128,
    pub identity_required: bool,
    pub execution_threshold: u32,
    /// Minimum number of votes (for + against combined) required for a proposal
    /// to be considered valid. Prevents a proposal from passing with a single vote.
    pub min_quorum: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoterProfile {
    pub voting_units: i128,
    pub identity_commitment: BytesN<32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    pub id: u32,
    pub creator: Address,
    pub title: String,
    pub description: String,
    pub voting_ends_at: u32,
    pub for_votes: i128,
    pub against_votes: i128,
    pub open: bool,
    pub voter_receipts: Map<Address, VoteReceipt>,
    pub state: ProposalState,
    pub start_ledger: u32,
    pub abstain_votes: i128,
    pub eta: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoteReceipt {
    pub support: bool,
    pub credits_spent: i128,
    pub votes_cast: i128,
    pub voting_units_snapshot: i128,
    pub vote_support: VoteSupport,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    MinVotingUnits,
    IdentityRequired,
    VoterList,
    NextProposalId,
    Voter(Address),
    IdentityOwner(BytesN<32>),
    Proposal(u32),
    ProposalSnapshot(u32),
    Receipt(u32, Address),
    ExecutionAdmins,
    ExecutionThreshold,
    Delegation(Address),
    DelegationCheckpoints(Address),
    MinQuorum,
    GracePeriod,
    VetoGuardian,
}

const DEFAULT_GRACE_PERIOD: u32 = 10;

fn sqrt(x: i128) -> i128 {
    if x <= 0 {
        return 0;
    }

    let mut z = (x + 1) / 2;
    let mut y = x;
    while z < y {
        y = z;
        z = (x / z + z) / 2;
    }
    y
}

fn zero_identity(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[0; 32])
}

fn require_admin(env: &Env) -> Result<Address, Error> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(Error::NotInitialized)?;

    admin.require_auth();

    Ok(admin)
}

fn require_execution_approvals(env: &Env, approvers: &Vec<Address>) -> Result<(), Error> {
    let admins: Vec<Address> = env
        .storage()
        .instance()
        .get(&DataKey::ExecutionAdmins)
        .ok_or(Error::NotInitialized)?;
    let threshold: u32 = env
        .storage()
        .instance()
        .get(&DataKey::ExecutionThreshold)
        .ok_or(Error::NotInitialized)?;

    if approvers.len() < threshold {
        return Err(Error::InsufficientApprovals);
    }

    let mut approved = 0u32;
    let mut seen = Vec::new(env);
    for approver in approvers.iter() {
        if seen.iter().any(|address| address == approver) {
            continue;
        }
        seen.push_back(approver.clone());
        if !admins.iter().any(|admin| admin == approver) {
            return Err(Error::Unauthorized);
        }
        approver.require_auth();
        approved += 1;
    }

    if approved < threshold {
        return Err(Error::InsufficientApprovals);
    }
    Ok(())
}

fn get_config(env: &Env) -> Result<GovernanceConfig, Error> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(Error::NotInitialized)?;
    let min_voting_units = env
        .storage()
        .instance()
        .get(&DataKey::MinVotingUnits)
        .unwrap_or(1i128);
    let identity_required = env
        .storage()
        .instance()
        .get(&DataKey::IdentityRequired)
        .unwrap_or(false);
    let execution_threshold = env
        .storage()
        .instance()
        .get(&DataKey::ExecutionThreshold)
        .unwrap_or(1u32);
    let min_quorum = env
        .storage()
        .instance()
        .get(&DataKey::MinQuorum)
        .unwrap_or(1i128);

    Ok(GovernanceConfig {
        admin,
        min_voting_units,
        identity_required,
        execution_threshold,
        min_quorum,
    })
}

fn get_proposal(env: &Env, proposal_id: u32) -> Result<Proposal, Error> {
    env.storage()
        .persistent()
        .get(&DataKey::Proposal(proposal_id))
        .ok_or(Error::ProposalNotFound)
}

fn write_proposal(env: &Env, proposal: &Proposal) {
    env.storage()
        .persistent()
        .set(&DataKey::Proposal(proposal.id), proposal);
}

fn resolve_delegate_at(env: &Env, voter: Address, ledger: u32) -> Result<Address, Error> {
    let mut current = voter;
    let mut visited = Vec::new(env);

    loop {
        if visited.iter().any(|address| address == current) {
            return Err(Error::DelegationLoop);
        }
        visited.push_back(current.clone());

        let checkpoints: Vec<(u32, Address)> = env
            .storage()
            .persistent()
            .get(&DataKey::DelegationCheckpoints(current.clone()))
            .unwrap_or_else(|| Vec::new(env));
        let mut index = checkpoints.len();
        let mut next = None;
        while index > 0 {
            index -= 1;
            let (recorded_ledger, delegate) = checkpoints.get(index).unwrap();
            if recorded_ledger <= ledger {
                next = Some(delegate);
                break;
            }
        }

        match next {
            Some(delegate) => current = delegate,
            None => return Ok(current),
        }
    }
}

fn get_grace_period(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::GracePeriod)
        .unwrap_or(DEFAULT_GRACE_PERIOD)
}

fn get_veto_guardian(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .get(&DataKey::VetoGuardian)
        .ok_or(Error::NotVetoGuardian)
}

fn ensure_passing(proposal: &Proposal, min_quorum: i128) -> Result<(), Error> {
    let passing_votes = proposal
        .for_votes
        .checked_add(proposal.abstain_votes)
        .ok_or(Error::Overflow)?;
    if passing_votes < min_quorum {
        return Err(Error::QuorumNotMet);
    }
    if proposal.for_votes <= proposal.against_votes {
        return Err(Error::ProposalNotPassed);
    }
    Ok(())
}

fn ensure_no_delegation_loop(
    env: &Env,
    delegator: &Address,
    delegate: &Address,
) -> Result<(), Error> {
    let mut current = delegate.clone();
    let mut visited = Vec::new(env);

    loop {
        if current == *delegator || visited.iter().any(|address| address == current) {
            return Err(Error::DelegationLoop);
        }
        visited.push_back(current.clone());

        match env
            .storage()
            .persistent()
            .get::<_, Address>(&DataKey::Delegation(current.clone()))
        {
            Some(next) => current = next,
            None => return Ok(()),
        }
    }
}

#[contract]
pub struct GovernanceContract;

#[contractimpl]
impl GovernanceContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        min_voting_units: i128,
        identity_required: bool,
    ) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        if min_voting_units <= 0 {
            return Err(Error::InsufficientVotingUnits);
        }

        let mut admins = Vec::new(&env);
        admins.push_back(admin.clone());
        Self::initialize_with_admins(
            env,
            admin,
            admins,
            1,
            min_voting_units,
            identity_required,
            1,
        )
    }

    pub fn initialize_with_admins(
        env: Env,
        admin: Address,
        execution_admins: Vec<Address>,
        execution_threshold: u32,
        min_voting_units: i128,
        identity_required: bool,
        min_quorum: i128,
    ) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        if min_voting_units <= 0 {
            return Err(Error::InsufficientVotingUnits);
        }
        if execution_threshold == 0 || execution_threshold > execution_admins.len() {
            return Err(Error::InvalidExecutionThreshold);
        }
        if !execution_admins.iter().any(|candidate| candidate == admin) {
            return Err(Error::Unauthorized);
        }
        if min_quorum <= 0 {
            return Err(Error::InvalidQuorum);
        }
        let mut unique_admins = Vec::new(&env);
        for candidate in execution_admins.iter() {
            if unique_admins.iter().any(|existing| existing == candidate) {
                return Err(Error::DuplicateAdmin);
            }
            unique_admins.push_back(candidate);
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::MinVotingUnits, &min_voting_units);
        env.storage()
            .instance()
            .set(&DataKey::IdentityRequired, &identity_required);
        env.storage()
            .instance()
            .set(&DataKey::NextProposalId, &0u32);
        env.storage()
            .instance()
            .set(&DataKey::ExecutionAdmins, &unique_admins);
        env.storage()
            .instance()
            .set(&DataKey::ExecutionThreshold, &execution_threshold);
        env.storage()
            .instance()
            .set(&DataKey::MinQuorum, &min_quorum);

        Ok(())
    }

    pub fn get_execution_admins(env: Env) -> Result<Vec<Address>, Error> {
        env.storage()
            .instance()
            .get(&DataKey::ExecutionAdmins)
            .ok_or(Error::NotInitialized)
    }

    pub fn get_execution_threshold(env: Env) -> Result<u32, Error> {
        env.storage()
            .instance()
            .get(&DataKey::ExecutionThreshold)
            .ok_or(Error::NotInitialized)
    }

    pub fn get_min_quorum(env: Env) -> Result<i128, Error> {
        env.storage()
            .instance()
            .get(&DataKey::MinQuorum)
            .ok_or(Error::NotInitialized)
    }

    pub fn get_config(env: Env) -> Result<GovernanceConfig, Error> {
        get_config(&env)
    }

    pub fn register_voter(
        env: Env,
        voter: Address,
        voting_units: i128,
        identity_commitment: BytesN<32>,
    ) -> Result<VoterProfile, Error> {
        require_admin(&env)?;
        let config = get_config(&env)?;
        if voting_units < config.min_voting_units {
            return Err(Error::InsufficientVotingUnits);
        }

        let zero = zero_identity(&env);
        let has_identity = identity_commitment != zero;
        if config.identity_required && !has_identity {
            return Err(Error::IdentityRequired);
        }

        let voter_key = DataKey::Voter(voter.clone());
        let previous_profile: Option<VoterProfile> = env.storage().persistent().get(&voter_key);
        if let Some(existing) = previous_profile.clone() {
            if existing.identity_commitment != zero
                && existing.identity_commitment != identity_commitment
            {
                env.storage()
                    .persistent()
                    .remove(&DataKey::IdentityOwner(existing.identity_commitment));
            }
        }

        if has_identity {
            let identity_key = DataKey::IdentityOwner(identity_commitment.clone());
            if let Some(owner) = env.storage().persistent().get::<_, Address>(&identity_key) {
                if owner != voter {
                    return Err(Error::IdentityAlreadyClaimed);
                }
            }
            env.storage().persistent().set(&identity_key, &voter);
        }

        let profile = VoterProfile {
            voting_units,
            identity_commitment,
        };
        if previous_profile.is_none() {
            let mut voters: Vec<Address> = env
                .storage()
                .persistent()
                .get(&DataKey::VoterList)
                .unwrap_or_else(|| Vec::new(&env));
            voters.push_back(voter.clone());
            env.storage().persistent().set(&DataKey::VoterList, &voters);
        }
        env.storage().persistent().set(&voter_key, &profile);
        Ok(profile)
    }

    pub fn get_voter(env: Env, voter: Address) -> Result<VoterProfile, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Voter(voter))
            .ok_or(Error::VoterNotRegistered)
    }

    pub fn delegate(env: Env, delegator: Address, to: Address) -> Result<(), Error> {
        delegator.require_auth();

        if delegator == to {
            return Err(Error::DelegationLoop);
        }
        if !env
            .storage()
            .persistent()
            .has(&DataKey::Voter(delegator.clone()))
            || !env.storage().persistent().has(&DataKey::Voter(to.clone()))
        {
            return Err(Error::VoterNotRegistered);
        }

        ensure_no_delegation_loop(&env, &delegator, &to)?;

        let ledger = env.ledger().sequence();
        let mut checkpoints: Vec<(u32, Address)> = env
            .storage()
            .persistent()
            .get(&DataKey::DelegationCheckpoints(delegator.clone()))
            .unwrap_or_else(|| Vec::new(&env));
        checkpoints.push_back((ledger, to.clone()));
        env.storage().persistent().set(
            &DataKey::DelegationCheckpoints(delegator.clone()),
            &checkpoints,
        );
        env.storage()
            .persistent()
            .set(&DataKey::Delegation(delegator), &to);
        Ok(())
    }

    pub fn get_delegate(env: Env, delegator: Address) -> Option<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::Delegation(delegator))
    }

    pub fn set_grace_period(env: Env, grace_period: u32) -> Result<(), Error> {
        require_admin(&env)?;
        env.storage()
            .instance()
            .set(&DataKey::GracePeriod, &grace_period);
        Ok(())
    }

    pub fn get_grace_period(env: Env) -> u32 {
        get_grace_period(&env)
    }

    pub fn set_veto_guardian(env: Env, guardian: Address) -> Result<(), Error> {
        require_admin(&env)?;
        env.storage()
            .instance()
            .set(&DataKey::VetoGuardian, &guardian);
        Ok(())
    }

    pub fn get_veto_guardian(env: Env) -> Result<Address, Error> {
        get_veto_guardian(&env)
    }

    pub fn create_proposal(
        env: Env,
        title: String,
        description: String,
        voting_ends_at: u32,
    ) -> Result<Proposal, Error> {
        let admin = require_admin(&env)?;
        if voting_ends_at <= env.ledger().sequence() {
            return Err(Error::InvalidVotingWindow);
        }

        let proposal_id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextProposalId)
            .unwrap_or(0);
        let start_ledger = env.ledger().sequence();
        let mut voting_units_snapshot = Map::new(&env);
        let voters: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::VoterList)
            .unwrap_or_else(|| Vec::new(&env));
        for voter in voters.iter() {
            if let Some(profile) = env
                .storage()
                .persistent()
                .get::<_, VoterProfile>(&DataKey::Voter(voter.clone()))
            {
                let delegate = resolve_delegate_at(&env, voter, start_ledger)?;
                let delegated_units = voting_units_snapshot.get(delegate.clone()).unwrap_or(0i128);
                let total_units = delegated_units
                    .checked_add(profile.voting_units)
                    .ok_or(Error::InvalidCredits)?;
                voting_units_snapshot.set(delegate, total_units);
            }
        }
        env.storage().persistent().set(
            &DataKey::ProposalSnapshot(proposal_id),
            &voting_units_snapshot,
        );
        let proposal = Proposal {
            id: proposal_id,
            creator: admin,
            title,
            description,
            voting_ends_at,
            for_votes: 0,
            against_votes: 0,
            abstain_votes: 0,
            open: true,
            voter_receipts: Map::new(&env),
            state: ProposalState::Active,
            start_ledger,
            eta: 0,
        };

        write_proposal(&env, &proposal);
        env.storage()
            .instance()
            .set(&DataKey::NextProposalId, &proposal_id.saturating_add(1));

        Ok(proposal)
    }

    pub fn get_proposal(env: Env, proposal_id: u32) -> Result<Proposal, Error> {
        get_proposal(&env, proposal_id)
    }

    pub fn close_proposal(env: Env, proposal_id: u32) -> Result<Proposal, Error> {
        require_admin(&env)?;
        let mut proposal = get_proposal(&env, proposal_id)?;
        if proposal.state != ProposalState::Active {
            return Err(Error::InvalidProposalState);
        }
        proposal.open = false;
        proposal.state = ProposalState::Cancelled;
        write_proposal(&env, &proposal);
        Ok(proposal)
    }

    pub fn queue_proposal(env: Env, proposal_id: u32, eta: u32) -> Result<Proposal, Error> {
        require_admin(&env)?;
        let mut proposal = get_proposal(&env, proposal_id)?;
        let current_ledger = env.ledger().sequence();
        if proposal.state != ProposalState::Active
            || current_ledger <= proposal.voting_ends_at
            || eta < current_ledger
        {
            return Err(Error::InvalidProposalState);
        }

        let config = get_config(&env)?;
        ensure_passing(&proposal, config.min_quorum)?;

        proposal.open = false;
        proposal.state = ProposalState::Queued;
        proposal.eta = eta;
        write_proposal(&env, &proposal);
        Ok(proposal)
    }

    pub fn veto_proposal(env: Env, guardian: Address, proposal_id: u32) -> Result<Proposal, Error> {
        guardian.require_auth();
        if guardian != get_veto_guardian(&env)? {
            return Err(Error::NotVetoGuardian);
        }

        let mut proposal = get_proposal(&env, proposal_id)?;
        if proposal.state != ProposalState::Queued {
            return Err(Error::InvalidProposalState);
        }
        proposal.state = ProposalState::Vetoed;
        write_proposal(&env, &proposal);
        Ok(proposal)
    }

    pub fn expire_proposal(env: Env, proposal_id: u32) -> Result<Proposal, Error> {
        let mut proposal = get_proposal(&env, proposal_id)?;
        if proposal.state != ProposalState::Queued {
            return Err(Error::InvalidProposalState);
        }
        let deadline = proposal
            .eta
            .checked_add(get_grace_period(&env))
            .ok_or(Error::Overflow)?;
        if env.ledger().sequence() <= deadline {
            return Err(Error::QueueNotReady);
        }
        proposal.state = ProposalState::Expired;
        write_proposal(&env, &proposal);
        Ok(proposal)
    }

    /// Executes a proposal decision only after the configured M-of-N administrators approve it.
    /// Queued proposals are executable in `[eta, eta + grace_period]`; abstentions
    /// count toward quorum but never toward the `for` majority.
    pub fn execute_proposal(
        env: Env,
        proposal_id: u32,
        approvers: Vec<Address>,
    ) -> Result<Proposal, Error> {
        require_execution_approvals(&env, &approvers)?;
        let mut proposal = get_proposal(&env, proposal_id)?;
        let current_ledger = env.ledger().sequence();
        if proposal.state == ProposalState::Queued {
            if current_ledger < proposal.eta {
                return Err(Error::QueueNotReady);
            }
            let deadline = proposal
                .eta
                .checked_add(get_grace_period(&env))
                .ok_or(Error::Overflow)?;
            if current_ledger > deadline {
                return Err(Error::QueueExpired);
            }
        } else if proposal.state != ProposalState::Active
            || !proposal.open
            || current_ledger <= proposal.voting_ends_at
        {
            return Err(Error::ProposalClosed);
        }

        let config = get_config(&env)?;
        ensure_passing(&proposal, config.min_quorum)?;

        proposal.open = false;
        proposal.state = ProposalState::Executed;
        write_proposal(&env, &proposal);
        Ok(proposal)
    }

    pub fn quote_votes_for_credits(_env: Env, credits: i128) -> Result<i128, Error> {
        if credits < 0 {
            return Err(Error::InvalidCredits);
        }
        Ok(sqrt(credits))
    }

    pub fn quote_credits_for_votes(_env: Env, votes: i128) -> Result<i128, Error> {
        if votes < 0 {
            return Err(Error::InvalidCredits);
        }
        votes.checked_mul(votes).ok_or(Error::InvalidCredits)
    }

    /// Computes vote weight as balance * multiplier with checked math to protect against overflow.
    pub fn compute_vote_weight(_env: Env, balance: i128, multiplier: i128) -> Result<i128, Error> {
        let weight = balance.checked_mul(multiplier).ok_or(Error::Overflow)?;
        Ok(weight)
    }

    pub fn calculate_vote_weight(env: Env, balance: i128, multiplier: i128) -> Result<i128, Error> {
        Self::compute_vote_weight(env, balance, multiplier)
    }

    pub fn vote_weight(env: Env, balance: i128, multiplier: i128) -> Result<i128, Error> {
        Self::compute_vote_weight(env, balance, multiplier)
    }

    pub fn get_vote_receipt(env: Env, proposal_id: u32, voter: Address) -> Option<VoteReceipt> {
        let proposal = get_proposal(&env, proposal_id).ok()?;
        proposal.voter_receipts.get(voter)
    }

    pub fn cast_vote(
        env: Env,
        proposal_id: u32,
        voter: Address,
        support: bool,
        credits_to_spend: i128,
    ) -> Result<VoteReceipt, Error> {
        let vote_support = if support {
            VoteSupport::For
        } else {
            VoteSupport::Against
        };
        Self::cast_vote_with_support(env, proposal_id, voter, vote_support, credits_to_spend)
    }

    pub fn cast_vote_with_support(
        env: Env,
        proposal_id: u32,
        voter: Address,
        vote_support: VoteSupport,
        credits_to_spend: i128,
    ) -> Result<VoteReceipt, Error> {
        voter.require_auth();
        if credits_to_spend <= 0 {
            return Err(Error::InvalidCredits);
        }

        let mut proposal = get_proposal(&env, proposal_id)?;
        if !proposal.open || env.ledger().sequence() > proposal.voting_ends_at {
            return Err(Error::ProposalClosed);
        }

        let snapshot: Map<Address, i128> = env
            .storage()
            .persistent()
            .get(&DataKey::ProposalSnapshot(proposal_id))
            .ok_or(Error::VoterNotRegistered)?;
        let voting_units = snapshot
            .get(voter.clone())
            .ok_or(Error::VoterNotRegistered)?;

        let mut receipts = proposal.voter_receipts.clone();
        let mut receipt = receipts.get(voter.clone()).unwrap_or(VoteReceipt {
            support: vote_support == VoteSupport::For,
            credits_spent: 0,
            votes_cast: 0,
            voting_units_snapshot: voting_units,
            vote_support,
        });

        if receipt.credits_spent > 0 && receipt.vote_support != vote_support {
            return Err(Error::VoteSideMismatch);
        }

        let updated_credits = receipt
            .credits_spent
            .checked_add(credits_to_spend)
            .ok_or(Error::InvalidCredits)?;
        if updated_credits > receipt.voting_units_snapshot {
            return Err(Error::InsufficientVotingUnits);
        }

        let votes_before = sqrt(receipt.credits_spent);
        let votes_after = sqrt(updated_credits);
        if votes_after <= votes_before {
            return Err(Error::InvalidCredits);
        }

        let delta = votes_after - votes_before;
        match vote_support {
            VoteSupport::Against => {
                proposal.against_votes = proposal
                    .against_votes
                    .checked_add(delta)
                    .ok_or(Error::InvalidCredits)?;
            }
            VoteSupport::For => {
                proposal.for_votes = proposal
                    .for_votes
                    .checked_add(delta)
                    .ok_or(Error::InvalidCredits)?;
            }
            VoteSupport::Abstain => {
                proposal.abstain_votes = proposal
                    .abstain_votes
                    .checked_add(delta)
                    .ok_or(Error::InvalidCredits)?;
            }
        }

        receipt.support = vote_support == VoteSupport::For;
        receipt.vote_support = vote_support;
        receipt.credits_spent = updated_credits;
        receipt.votes_cast = votes_after;

        receipts.set(voter, receipt.clone());
        proposal.voter_receipts = receipts;

        write_proposal(&env, &proposal);

        Ok(receipt)
    }
}
