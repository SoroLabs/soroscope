use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, String};
use soroscope_governance_contract::{Error, GovernanceContract, GovernanceContractClient};

#[test]
fn delegation_rejects_cycles_and_aggregates_voting_units() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(GovernanceContract, ());
    let admin = Address::generate(&env);
    let voter_a = Address::generate(&env);
    let voter_b = Address::generate(&env);
    let voter_c = Address::generate(&env);
    let voter_d = Address::generate(&env);
    let client = GovernanceContractClient::new(&env, &contract_id);
    let identity = BytesN::from_array(&env, &[0; 32]);

    client.initialize(&admin, &1, &false);
    client.register_voter(&voter_a, &25, &identity);
    client.register_voter(&voter_b, &10, &identity);
    client.register_voter(&voter_c, &5, &identity);
    client.register_voter(&voter_d, &1, &identity);

    client.delegate(&voter_a, &voter_b);
    assert_eq!(
        client.try_delegate(&voter_b, &voter_a),
        Err(Ok(Error::DelegationLoop))
    );
    assert_eq!(
        client.try_delegate(&voter_a, &voter_a),
        Err(Ok(Error::DelegationLoop))
    );

    client.delegate(&voter_b, &voter_c);
    assert_eq!(
        client.try_delegate(&voter_c, &voter_a),
        Err(Ok(Error::DelegationLoop))
    );
    client.delegate(&voter_d, &voter_a);
    assert_eq!(
        client.try_delegate(&voter_a, &voter_d),
        Err(Ok(Error::DelegationLoop))
    );

    let proposal = client.create_proposal(
        &String::from_str(&env, "Delegated voting"),
        &String::from_str(&env, "Delegated units aggregate at the final delegate"),
        &100,
    );

    assert_eq!(
        client.try_cast_vote(&proposal.id, &voter_a, &true, &1),
        Err(Ok(Error::VoterNotRegistered))
    );
    assert_eq!(
        client.try_cast_vote(&proposal.id, &voter_b, &true, &1),
        Err(Ok(Error::VoterNotRegistered))
    );
    assert_eq!(
        client
            .cast_vote(&proposal.id, &voter_c, &true, &41)
            .credits_spent,
        41
    );
}
