#![cfg(test)]

extern crate std;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, BytesN as _, Ledger},
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
        .initialize_with_admins(&admin_a, &admins, &2, &9, &false);
    (env, contract_id, admin_a, admin_b, outsider)
}

fn expired_proposal(client: &GovernanceContractClient<'_>, env: &Env) -> Proposal {
    env.ledger().with_mut(|ledger| ledger.sequence_number = 10);
    let proposal = client.create_proposal(
        &String::from_str(env, "Execute approved decision"),
        &String::from_str(env, "Requires M-of-N authorization"),
        &11,
    );
    env.ledger().with_mut(|ledger| ledger.sequence_number = 12);
    proposal
}

#[test]
fn execution_requires_distinct_threshold_admin_approvals() {
    let (env, contract_id, admin_a, admin_b, _) = setup_multisig();
    let client = GovernanceContractClient::new(&env, &contract_id);
    let proposal = expired_proposal(&client, &env);
    let mut one_approval = Vec::new(&env);
    one_approval.push_back(admin_a.clone());
    assert_eq!(client.try_execute_proposal(&proposal.id, &one_approval), Err(Ok(Error::InsufficientApprovals)));
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
    assert_eq!(client.try_execute_proposal(&proposal.id, &duplicate_approvals), Err(Ok(Error::InsufficientApprovals)));
    let mut invalid_approvals = Vec::new(&env);
    invalid_approvals.push_back(outsider.clone());
    invalid_approvals.push_back(outsider);
    assert_eq!(client.try_execute_proposal(&proposal.id, &invalid_approvals), Err(Ok(Error::Unauthorized)));
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
