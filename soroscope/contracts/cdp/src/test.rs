#![cfg(test)]

use super::*;
use soroban_sdk::{
    contract, contractimpl, contracttype,
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    Address, Env,
};

#[contract]
struct MockOracle;

#[contracttype]
#[derive(Clone)]
enum OracleDataKey {
    Price,
}

#[contractimpl]
impl MockOracle {
    pub fn set_price(env: Env, price: i128) {
        env.storage().instance().set(&OracleDataKey::Price, &price);
    }

    pub fn latest_price(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&OracleDataKey::Price)
            .unwrap_or(PRICE_SCALE)
    }
}

fn setup() -> (
    Env,
    Address,
    Address,
    Address,
    Address,
    Address,
    Address,
) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let borrower = Address::generate(&env);
    let liquidator = Address::generate(&env);

    let collateral_token = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let collateral_admin = StellarAssetClient::new(&env, &collateral_token);
    collateral_admin.mint(&borrower, &1_000_000);
    collateral_admin.mint(&liquidator, &1_000_000);

    let oracle_id = env.register(MockOracle, ());
    let oracle = MockOracleClient::new(&env, &oracle_id);
    oracle.set_price(&(2 * PRICE_SCALE));

    let contract_id = env.register(CdpContract, ());
    let client = CdpContractClient::new(&env, &contract_id);
    client.initialize(
        &admin,
        &collateral_token,
        &oracle_id,
        &15_000,
        &1_000,
        &200,
        &800,
        &2_000,
        &8_000,
    );

    (
        env,
        contract_id,
        admin,
        borrower,
        liquidator,
        collateral_token,
        oracle_id,
    )
}

#[test]
fn mint_and_accrue_interest_from_utilization() {
    let (env, contract_id, _admin, borrower, _liquidator, _collateral, _oracle) = setup();
    let client = CdpContractClient::new(&env, &contract_id);

    client.deposit_collateral(&borrower, &100_000);
    client.mint_stable(&borrower, &100_000);

    let initial_rate = client.current_borrow_rate_bps();
    assert!(initial_rate >= 200);

    env.ledger().with_mut(|ledger| {
        ledger.sequence_number += 3_153_600;
    });

    let before = client.get_position(&borrower);
    client.deposit_collateral(&borrower, &1);
    let after = client.get_position(&borrower);

    assert!(after.debt_amount > before.debt_amount);
    assert_eq!(after.collateral_amount, before.collateral_amount + 1);
}

#[test]
fn liquidation_quote_applies_incentive_bonus() {
    let (env, contract_id, _admin, borrower, liquidator, _collateral, oracle_id) = setup();
    let client = CdpContractClient::new(&env, &contract_id);

    client.deposit_collateral(&borrower, &100_000);
    client.mint_stable(&borrower, &120_000);
    client.deposit_collateral(&liquidator, &50_000);
    client.mint_stable(&liquidator, &30_000);

    let oracle = MockOracleClient::new(&env, &oracle_id);
    oracle.set_price(&PRICE_SCALE);

    let quote = client.quote_liquidation(&borrower, &20_000);
    assert_eq!(quote.repay_amount, 20_000);
    assert_eq!(quote.incentive_amount, 2_000);
    assert_eq!(quote.collateral_to_seize, 22_000);
}

#[test]
fn third_party_liquidation_burns_stable_and_seizes_collateral() {
    let (env, contract_id, _admin, borrower, liquidator, collateral_token, oracle_id) = setup();
    let client = CdpContractClient::new(&env, &contract_id);

    client.deposit_collateral(&borrower, &100_000);
    client.mint_stable(&borrower, &120_000);
    client.deposit_collateral(&liquidator, &60_000);
    client.mint_stable(&liquidator, &40_000);

    let oracle = MockOracleClient::new(&env, &oracle_id);
    oracle.set_price(&PRICE_SCALE);

    let before_balance = client.stable_balance(&liquidator);
    let collateral_client = soroban_sdk::token::Client::new(&env, &collateral_token);
    let before_collateral = collateral_client.balance(&liquidator);

    let quote = client.liquidate(&liquidator, &borrower, &20_000);

    assert_eq!(client.stable_balance(&liquidator), before_balance - quote.repay_amount);
    assert_eq!(
        collateral_client.balance(&liquidator),
        before_collateral + quote.collateral_to_seize
    );
    assert_eq!(client.get_position(&borrower).debt_amount, 100_000);
}

#[test]
fn self_liquidation_moves_collateral_to_protocol_and_tracks_bad_debt() {
    let (env, contract_id, _admin, borrower, _liquidator, _collateral, oracle_id) = setup();
    let client = CdpContractClient::new(&env, &contract_id);

    client.deposit_collateral(&borrower, &100_000);
    client.mint_stable(&borrower, &150_000);

    let oracle = MockOracleClient::new(&env, &oracle_id);
    oracle.set_price(&(PRICE_SCALE / 2));

    let closed = client.self_liquidate(&borrower);
    assert_eq!(closed.collateral_amount, 0);
    assert_eq!(closed.debt_amount, 0);
    assert_eq!(client.protocol_collateral_reserves(), 100_000);
    assert!(client.total_bad_debt() > 0);
}
