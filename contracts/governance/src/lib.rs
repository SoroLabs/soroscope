#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, BytesN, Env, Map, String,
    contract, contracterror, contractimpl, contracttype, Address, BytesN, Env, String, Vec,
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
    InvalidExecutionThreshold = 13,
    DuplicateAdmin = 14,
    InsufficientApprovals = 15,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GovernanceConfig {
    pub admin: Address,
    pub min_voting_units: i128,
    pub identity_required: bool,
    pub execution_threshold: u32,
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
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoteReceipt {
    pub support: bool,
    pub credits_spent: i128,
    pub votes_cast: i128,
    pub voting_units_snapshot: i128,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    MinVotingUnits,
    IdentityRequired,
    NextProposalId,
    Voter(Address),
    IdentityOwner(BytesN<32>),
    Proposal(u32),
    Receipt(u32, Address),
    ExecutionAdmins,
    ExecutionThreshold,
}

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

    Ok(GovernanceConfig {
        admin,
        min_voting_units,
        identity_required,
        execution_threshold,
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
        Self::initialize_with_admins(env, admin, admins, 1, min_voting_units, identity_required)
    }

    pub fn initialize_with_admins(
        env: Env,
        admin: Address,
        execution_admins: Vec<Address>,
        execution_threshold: u32,
        min_voting_units: i128,
        identity_required: bool,
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
        env.storage().persistent().set(&voter_key, &profile);
        Ok(profile)
    }

    pub fn get_voter(env: Env, voter: Address) -> Result<VoterProfile, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Voter(voter))
            .ok_or(Error::VoterNotRegistered)
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
        let proposal = Proposal {
            id: proposal_id,
            creator: admin,
            title,
            description,
            voting_ends_at,
            for_votes: 0,
            against_votes: 0,
            open: true,
            voter_receipts: Map::new(&env),
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
        proposal.open = false;
        write_proposal(&env, &proposal);
        Ok(proposal)
    }

    /// Executes a proposal decision only after the configured M-of-N administrators approve it.
    pub fn execute_proposal(
        env: Env,
        proposal_id: u32,
        approvers: Vec<Address>,
    ) -> Result<Proposal, Error> {
        require_execution_approvals(&env, &approvers)?;
        let mut proposal = get_proposal(&env, proposal_id)?;
        if !proposal.open || env.ledger().sequence() <= proposal.voting_ends_at {
            return Err(Error::ProposalClosed);
        }
        proposal.open = false;
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
        Ok(votes.checked_mul(votes).ok_or(Error::InvalidCredits)?)
    }

    pub fn get_vote_receipt(
        env: Env,
        proposal_id: u32,
        voter: Address,
    ) -> Option<VoteReceipt> {
        let proposal = get_proposal(&env, proposal_id).ok()?;
        proposal.voter_receipts.get(voter)
    pub fn get_vote_receipt(env: Env, proposal_id: u32, voter: Address) -> Option<VoteReceipt> {
        env.storage()
            .persistent()
            .get(&DataKey::Receipt(proposal_id, voter))
    }

    pub fn cast_vote(
        env: Env,
        proposal_id: u32,
        voter: Address,
        support: bool,
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

        let profile: VoterProfile = env
            .storage()
            .persistent()
            .get(&DataKey::Voter(voter.clone()))
            .ok_or(Error::VoterNotRegistered)?;

        let mut receipts = proposal.voter_receipts.clone();
        let mut receipt = receipts.get(voter.clone()).unwrap_or(VoteReceipt {
            support,
            credits_spent: 0,
            votes_cast: 0,
            voting_units_snapshot: profile.voting_units,
        });

        if receipt.credits_spent > 0 && receipt.support != support {
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
        if support {
            proposal.for_votes = proposal
                .for_votes
                .checked_add(delta)
                .ok_or(Error::InvalidCredits)?;
        } else {
            proposal.against_votes = proposal
                .against_votes
                .checked_add(delta)
                .ok_or(Error::InvalidCredits)?;
        }

        receipt.support = support;
        receipt.credits_spent = updated_credits;
        receipt.votes_cast = votes_after;

        receipts.set(voter, receipt.clone());
        proposal.voter_receipts = receipts;

        write_proposal(&env, &proposal);

        Ok(receipt)
    }
}
