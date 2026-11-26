#![cfg(test)]

extern crate std;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, BytesN, Env, String, Vec,
};

fn setup(identity_required: bool) -> (Env, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(GovernanceContract, ());
    let admin = Address::generate(&env);
    let voter_a = Address::generate(&env);
    let voter_b = Address::generate(&env);

    let client = GovernanceContractClient::new(&env, &contract_id);
    client.initialize(&admin, &9, &identity_required);
    (env, contract_id, admin, voter_a, voter_b)
}

fn make_identity(env: &Env, seed: u8) -> BytesN<32> {
    BytesN::from_array(env, &[seed; 32])
}

fn setup_multisig() -> (Env, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(GovernanceContract, ());
    let admin_a = Address::generate(&env);
    let admin_b = Address::generate(&env);
    let outsider = Address::generate(&env);
    let mut admins = Vec::new(&env);
    admins.push_back(admin_a.clone());
    admins.push_back(admin_b.clone());
    GovernanceContractClient::new(&env, &contract_id)
        .initialize_with_admins(&admin_a, &admins, &2, &9, &false, &2);
    (env, contract_id, admin_a, admin_b, outsider)
}

fn expired_proposal(client: &GovernanceContractClient<'_>, env: &Env) -> Proposal {
    // Register the voter before the proposal so they are in its voting snapshot.
    let voter = Address::generate(env);
    client.register_voter(&voter, &9, &BytesN::from_array(env, &[42u8; 32]));
    env.ledger().with_mut(|ledger| ledger.sequence_number = 10);
    let proposal = client.create_proposal(
        &String::from_str(env, "Execute approved decision"),
        &String::from_str(env, "Requires M-of-N authorization"),
        &11,
    );
    // Cast 2 votes (4 credits) to meet the quorum of 2 configured in setup_multisig.
    client.cast_vote(&proposal.id, &voter, &true, &4);
    env.ledger().with_mut(|ledger| ledger.sequence_number = 12);
    // Re-fetch so the returned proposal carries the updated vote counts.
    client.get_proposal(&proposal.id)
}

#[test]
fn execution_requires_distinct_threshold_admin_approvals() {
    let (env, contract_id, admin_a, admin_b, _) = setup_multisig();
    let client = GovernanceContractClient::new(&env, &contract_id);
    let proposal = expired_proposal(&client, &env);
    let mut one_approval = Vec::new(&env);
    one_approval.push_back(admin_a.clone());
    assert_eq!(
        client.try_execute_proposal(&proposal.id, &one_approval),
        Err(Ok(Error::InsufficientApprovals))
    );

    let mut approvals = Vec::new(&env);
    approvals.push_back(admin_a);
    approvals.push_back(admin_b);
    assert!(!client.execute_proposal(&proposal.id, &approvals).open);
}

#[test]
fn duplicate_or_non_admin_execution_approvals_are_rejected() {
    let (env, contract_id, admin_a, _, outsider) = setup_multisig();
    let client = GovernanceContractClient::new(&env, &contract_id);
    let proposal = expired_proposal(&client, &env);
    let mut duplicate_approvals = Vec::new(&env);
    duplicate_approvals.push_back(admin_a.clone());
    duplicate_approvals.push_back(admin_a);
    assert_eq!(
        client.try_execute_proposal(&proposal.id, &duplicate_approvals),
        Err(Ok(Error::InsufficientApprovals))
    );

    let mut invalid_approvals = Vec::new(&env);
    invalid_approvals.push_back(outsider.clone());
    invalid_approvals.push_back(outsider);
    assert_eq!(
        client.try_execute_proposal(&proposal.id, &invalid_approvals),
        Err(Ok(Error::Unauthorized))
    );
}

#[test]
fn quadratic_votes_follow_square_root_curve() {
    let (env, contract_id, admin, voter, _) = setup(true);
    let client = GovernanceContractClient::new(&env, &contract_id);
    client.register_voter(&voter, &25, &make_identity(&env, 7));

    env.ledger().with_mut(|ledger| {
        ledger.sequence_number = 25;
    });

    let title = String::from_str(&env, "Ship quadratic voting");
    let description = String::from_str(&env, "Reduce whale influence in governance");
    let proposal = client.create_proposal(&title, &description, &60);

    let first = client.cast_vote(&proposal.id, &voter, &true, &9);
    assert_eq!(first.credits_spent, 9);
    assert_eq!(first.votes_cast, 3);

    let second = client.cast_vote(&proposal.id, &voter, &true, &7);
    assert_eq!(second.credits_spent, 16);
    assert_eq!(second.votes_cast, 4);

    let stored = client.get_proposal(&proposal.id);
    assert_eq!(stored.for_votes, 4);
    assert_eq!(stored.against_votes, 0);

    let quote = client.quote_votes_for_credits(&25);
    assert_eq!(quote, 5);

    let cost = client.quote_credits_for_votes(&5);
    assert_eq!(cost, 25);

    let _ = admin;
}

#[test]
fn split_accounts_below_threshold_are_rejected() {
    let (env, contract_id, _admin, voter, _) = setup(false);
    let client = GovernanceContractClient::new(&env, &contract_id);

    let err = client.try_register_voter(&voter, &4, &make_identity(&env, 1));
    assert_eq!(err, Err(Ok(Error::InsufficientVotingUnits)));
}

#[test]
fn duplicate_identity_commitments_are_blocked() {
    let (env, contract_id, _admin, voter_a, voter_b) = setup(true);
    let client = GovernanceContractClient::new(&env, &contract_id);
    let identity = make_identity(&env, 9);

    client.register_voter(&voter_a, &16, &identity);
    let err = client.try_register_voter(&voter_b, &16, &identity);

    assert_eq!(err, Err(Ok(Error::IdentityAlreadyClaimed)));
}

#[test]
fn voter_cannot_flip_sides_on_same_proposal() {
    let (env, contract_id, _admin, voter, _) = setup(true);
    let client = GovernanceContractClient::new(&env, &contract_id);
    client.register_voter(&voter, &16, &make_identity(&env, 2));

    env.ledger().with_mut(|ledger| {
        ledger.sequence_number = 50;
    });

    let proposal = client.create_proposal(
        &String::from_str(&env, "Treasury reallocation"),
        &String::from_str(&env, "Test proposal"),
        &80,
    );
    client.cast_vote(&proposal.id, &voter, &true, &4);

    let err = client.try_cast_vote(&proposal.id, &voter, &false, &5);
    assert_eq!(err, Err(Ok(Error::VoteSideMismatch)));
}

#[test]
fn votes_cannot_exceed_registered_units() {
    let (env, contract_id, _admin, voter, _) = setup(true);
    let client = GovernanceContractClient::new(&env, &contract_id);
    client.register_voter(&voter, &10, &make_identity(&env, 5));

    env.ledger().with_mut(|ledger| {
        ledger.sequence_number = 100;
    });

    let proposal = client.create_proposal(
        &String::from_str(&env, "Cap credits"),
        &String::from_str(&env, "Voting units snapshot should cap spend"),
        &150,
    );

    let err = client.try_cast_vote(&proposal.id, &voter, &true, &11);
    assert_eq!(err, Err(Ok(Error::InsufficientVotingUnits)));
}

#[test]
fn vote_weight_computation_succeeds_for_normal_values() {
    let env = Env::default();
    let contract_id = env.register(GovernanceContract, ());
    let client = GovernanceContractClient::new(&env, &contract_id);

    assert_eq!(client.compute_vote_weight(&100, &2), 200);
    assert_eq!(client.calculate_vote_weight(&1_000_000, &1), 1_000_000);
    assert_eq!(client.vote_weight(&500, &0), 0);
    assert_eq!(client.compute_vote_weight(&i128::MAX, &1), i128::MAX);
}

#[test]
fn vote_weight_overflows_on_large_balance_and_multiplier() {
    let env = Env::default();
    let contract_id = env.register(GovernanceContract, ());
    let client = GovernanceContractClient::new(&env, &contract_id);

    // Whale wallet with near-i128::MAX balance and multiplier > 1
    assert_eq!(
        client.try_compute_vote_weight(&i128::MAX, &2),
        Err(Ok(Error::Overflow))
    );
    assert_eq!(
        client.try_calculate_vote_weight(&(i128::MAX / 2 + 1), &2),
        Err(Ok(Error::Overflow))
    );
    assert_eq!(
        client.try_vote_weight(&(i128::MAX / 10 + 1), &10),
        Err(Ok(Error::Overflow))
    );
    assert_eq!(
        client.try_compute_vote_weight(&i128::MAX, &i128::MAX),
        Err(Ok(Error::Overflow))
    );
}

#[test]
fn execute_proposal_fails_without_quorum() {
    // setup_multisig initialises with min_quorum = 2, so a proposal with
    // only 1 total vote (sqrt(1) = 1) must be rejected by execute_proposal.
    let (env, contract_id, admin_a, admin_b, _) = setup_multisig();
    let client = GovernanceContractClient::new(&env, &contract_id);

    // Register a voter with 9 units and cast just 1 credit (= 1 vote).
    let voter = Address::generate(&env);
    client.register_voter(&voter, &9, &BytesN::from_array(&env, &[99u8; 32]));

    env.ledger().with_mut(|ledger| ledger.sequence_number = 10);
    let proposal = client.create_proposal(
        &String::from_str(&env, "Low turnout proposal"),
        &String::from_str(&env, "Should fail quorum"),
        &11,
    );
    // Cast only 1 vote — below the quorum of 2.
    client.cast_vote(&proposal.id, &voter, &true, &1);

    env.ledger().with_mut(|ledger| ledger.sequence_number = 12);

    let mut approvals = Vec::new(&env);
    approvals.push_back(admin_a);
    approvals.push_back(admin_b);

    // Must fail because total_votes (1) < min_quorum (2).
    assert_eq!(
        client.try_execute_proposal(&proposal.id, &approvals),
        Err(Ok(Error::QuorumNotMet))
    );
}

#[test]
fn transferred_voting_units_snapshot_prevents_double_vote() {
    let (env, contract_id, _admin, voter_a, voter_b) = setup(true);
    let client = GovernanceContractClient::new(&env, &contract_id);
    client.register_voter(&voter_a, &25, &make_identity(&env, 10));
    client.register_voter(&voter_b, &10, &make_identity(&env, 11));

    env.ledger().with_mut(|ledger| {
        ledger.sequence_number = 30;
    });

    let proposal = client.create_proposal(
        &String::from_str(&env, "Prevent double voting"),
        &String::from_str(&env, "Voting units are snapshotted at proposal creation"),
        &90,
    );

    // voter_b's snapshot is 10 units; they cannot spend 11 credits.
    let err = client.try_cast_vote(&proposal.id, &voter_b, &true, &11);
    assert_eq!(err, Err(Ok(Error::InsufficientVotingUnits)));
}

fn approvals(env: &Env, admin_a: &Address, admin_b: &Address) -> Vec<Address> {
    Vec::from_array(env, [admin_a.clone(), admin_b.clone()])
}

#[test]
fn delegation_checkpoint_is_fixed_at_proposal_start_ledger() {
    let (env, contract_id, _admin, voter_a, voter_b) = setup(false);
    let client = GovernanceContractClient::new(&env, &contract_id);
    let voter_c = Address::generate(&env);
    client.register_voter(&voter_a, &25, &make_identity(&env, 31));
    client.register_voter(&voter_b, &16, &make_identity(&env, 32));
    client.register_voter(&voter_c, &16, &make_identity(&env, 33));

    env.ledger().with_mut(|ledger| ledger.sequence_number = 5);
    client.delegate(&voter_b, &voter_a);

    env.ledger().with_mut(|ledger| ledger.sequence_number = 10);
    let first = client.create_proposal(
        &String::from_str(&env, "Checkpointed delegation"),
        &String::from_str(&env, "Snapshot taken at start ledger"),
        &20,
    );
    env.ledger().with_mut(|ledger| ledger.sequence_number = 11);
    client.delegate(&voter_b, &voter_c);

    assert_eq!(
        client.cast_vote(&first.id, &voter_a, &true, &36).votes_cast,
        6
    );

    env.ledger().with_mut(|ledger| ledger.sequence_number = 12);
    let second = client.create_proposal(
        &String::from_str(&env, "Updated delegation"),
        &String::from_str(&env, "New snapshot uses the new delegate"),
        &30,
    );
    assert_eq!(
        client.try_cast_vote(&second.id, &voter_a, &true, &36),
        Err(Ok(Error::InsufficientVotingUnits))
    );
}

#[test]
fn abstain_counts_toward_quorum_without_counting_as_for() {
    let (env, contract_id, admin_a, admin_b, _) = setup_multisig();
    let client = GovernanceContractClient::new(&env, &contract_id);
    let voter_for = Address::generate(&env);
    let voter_abstain = Address::generate(&env);
    client.register_voter(&voter_for, &9, &make_identity(&env, 41));
    client.register_voter(&voter_abstain, &9, &make_identity(&env, 42));

    env.ledger().with_mut(|ledger| ledger.sequence_number = 10);
    let proposal = client.create_proposal(
        &String::from_str(&env, "Abstain-assisted quorum"),
        &String::from_str(&env, "For plus abstain meets quorum"),
        &11,
    );
    client.cast_vote_with_support(&proposal.id, &voter_for, &VoteSupport::For, &1);
    client.cast_vote_with_support(&proposal.id, &voter_abstain, &VoteSupport::Abstain, &1);

    let tallies = client.get_proposal(&proposal.id);
    assert_eq!(tallies.for_votes, 1);
    assert_eq!(tallies.abstain_votes, 1);
    assert_eq!(tallies.against_votes, 0);

    env.ledger().with_mut(|ledger| ledger.sequence_number = 12);
    let executed = client.execute_proposal(&proposal.id, &approvals(&env, &admin_a, &admin_b));
    assert_eq!(executed.state, ProposalState::Executed);

    env.ledger().with_mut(|ledger| ledger.sequence_number = 20);
    let abstain_only = client.create_proposal(
        &String::from_str(&env, "Abstain-only proposal"),
        &String::from_str(&env, "Quorum without a for majority"),
        &21,
    );
    client.cast_vote_with_support(&abstain_only.id, &voter_for, &VoteSupport::Abstain, &1);
    client.cast_vote_with_support(&abstain_only.id, &voter_abstain, &VoteSupport::Abstain, &1);
    env.ledger().with_mut(|ledger| ledger.sequence_number = 22);
    assert_eq!(
        client.try_execute_proposal(&abstain_only.id, &approvals(&env, &admin_a, &admin_b)),
        Err(Ok(Error::ProposalNotPassed))
    );
}

#[test]
fn queued_proposal_executes_only_inside_grace_window() {
    let (env, contract_id, admin_a, admin_b, _) = setup_multisig();
    let client = GovernanceContractClient::new(&env, &contract_id);
    client.set_grace_period(&5);
    assert_eq!(client.get_grace_period(), 5);

    let voter = Address::generate(&env);
    client.register_voter(&voter, &9, &make_identity(&env, 51));
    env.ledger().with_mut(|ledger| ledger.sequence_number = 10);
    let proposal = client.create_proposal(
        &String::from_str(&env, "Queued proposal"),
        &String::from_str(&env, "Timelocked execution"),
        &11,
    );
    client.cast_vote(&proposal.id, &voter, &true, &4);

    env.ledger().with_mut(|ledger| ledger.sequence_number = 12);
    let queued = client.queue_proposal(&proposal.id, &15);
    assert_eq!(queued.state, ProposalState::Queued);

    env.ledger().with_mut(|ledger| ledger.sequence_number = 14);
    assert_eq!(
        client.try_execute_proposal(&proposal.id, &approvals(&env, &admin_a, &admin_b)),
        Err(Ok(Error::QueueNotReady))
    );

    env.ledger().with_mut(|ledger| ledger.sequence_number = 15);
    let executed = client.execute_proposal(&proposal.id, &approvals(&env, &admin_a, &admin_b));
    assert_eq!(executed.state, ProposalState::Executed);
}

#[test]
fn expired_queued_proposal_is_rejected() {
    let (env, contract_id, admin_a, admin_b, _) = setup_multisig();
    let client = GovernanceContractClient::new(&env, &contract_id);
    client.set_grace_period(&5);

    let voter = Address::generate(&env);
    client.register_voter(&voter, &9, &make_identity(&env, 61));
    env.ledger().with_mut(|ledger| ledger.sequence_number = 10);
    let proposal = client.create_proposal(
        &String::from_str(&env, "Expiring proposal"),
        &String::from_str(&env, "Grace period elapsed"),
        &11,
    );
    client.cast_vote(&proposal.id, &voter, &true, &4);
    env.ledger().with_mut(|ledger| ledger.sequence_number = 12);
    client.queue_proposal(&proposal.id, &15);

    env.ledger().with_mut(|ledger| ledger.sequence_number = 21);
    assert_eq!(
        client.try_execute_proposal(&proposal.id, &approvals(&env, &admin_a, &admin_b)),
        Err(Ok(Error::QueueExpired))
    );
    assert_eq!(
        client.expire_proposal(&proposal.id).state,
        ProposalState::Expired
    );
    assert_eq!(
        client.try_execute_proposal(&proposal.id, &approvals(&env, &admin_a, &admin_b)),
        Err(Ok(Error::ProposalClosed))
    );
}

#[test]
fn veto_guardian_can_veto_a_queued_proposal() {
    let (env, contract_id, admin_a, admin_b, _) = setup_multisig();
    let client = GovernanceContractClient::new(&env, &contract_id);
    let guardian = Address::generate(&env);
    client.set_veto_guardian(&guardian);
    assert_eq!(client.get_veto_guardian(), guardian);

    let voter = Address::generate(&env);
    client.register_voter(&voter, &9, &make_identity(&env, 71));
    env.ledger().with_mut(|ledger| ledger.sequence_number = 10);
    let proposal = client.create_proposal(
        &String::from_str(&env, "Vetoed proposal"),
        &String::from_str(&env, "Guardian intervention"),
        &11,
    );
    client.cast_vote(&proposal.id, &voter, &true, &4);
    env.ledger().with_mut(|ledger| ledger.sequence_number = 12);
    client.queue_proposal(&proposal.id, &15);

    assert_eq!(
        client.try_veto_proposal(&admin_a, &proposal.id),
        Err(Ok(Error::NotVetoGuardian))
    );
    assert_eq!(
        client.veto_proposal(&guardian, &proposal.id).state,
        ProposalState::Vetoed
    );
    assert_eq!(
        client.try_execute_proposal(&proposal.id, &approvals(&env, &admin_a, &admin_b)),
        Err(Ok(Error::ProposalClosed))
    );
}

#[test]
fn execute_proposal_fails_when_quorum_met_but_majority_against() {
    let (env, contract_id, admin_a, admin_b, _) = setup_multisig();
    let client = GovernanceContractClient::new(&env, &contract_id);

    let voter_for = Address::generate(&env);
    let voter_against = Address::generate(&env);
    client.register_voter(&voter_for, &9, &BytesN::from_array(&env, &[1u8; 32]));
    client.register_voter(&voter_against, &9, &BytesN::from_array(&env, &[2u8; 32]));

    env.ledger().with_mut(|ledger| ledger.sequence_number = 10);
    let proposal = client.create_proposal(
        &String::from_str(&env, "Contested proposal"),
        &String::from_str(&env, "Quorum reached but rejected"),
        &11,
    );
    client.cast_vote(&proposal.id, &voter_for, &true, &4);
    client.cast_vote(&proposal.id, &voter_against, &false, &9);

    env.ledger().with_mut(|ledger| ledger.sequence_number = 12);

    let mut approvals = Vec::new(&env);
    approvals.push_back(admin_a);
    approvals.push_back(admin_b);
    assert_eq!(
        client.try_execute_proposal(&proposal.id, &approvals),
        Err(Ok(Error::ProposalNotPassed))
    );
}
