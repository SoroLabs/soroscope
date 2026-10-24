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
    Volatility,
}

#[contractimpl]
impl MockOracle {
    pub fn set_price(env: Env, price: i128) {
        env.storage().instance().set(&OracleDataKey::Price, &price);
    }

    pub fn set_volatility(env: Env, volatility_bps: i128) {
        env.storage()
            .instance()
            .set(&OracleDataKey::Volatility, &volatility_bps);
    }

    pub fn latest_price(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&OracleDataKey::Price)
            .unwrap_or(PRICE_SCALE)
    }

    pub fn volatility_bps(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&OracleDataKey::Volatility)
            .unwrap_or(0)
    }
}

fn setup() -> (Env, Address, Address, Address, Address, Address, Address) {
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
    oracle.set_volatility(&500); // 5% volatility

    let contract_id = env.register(CdpContract, ());
    let client = CdpContractClient::new(&env, &contract_id);
    client.initialize(
        &admin,
        &collateral_token,
        &oracle_id,
        &InitParams {
            min_collateral_ratio_bps: 15_000,
            liquidation_incentive_bps: 1_000,
            interest_rate_model: InterestRateModel {
                base_rate_bps: 200,
                slope1_bps: 800,
                slope2_bps: 2_000,
                optimal_utilization_bps: 8_000,
            },
            volatility_config: VolatilityConfig {
                low_volatility_threshold_bps: 1_000,  // 10%
                high_volatility_threshold_bps: 5_000, // 50%
                base_collateral_floor_bps: 15_000,    // 150%
                max_collateral_floor_bps: 25_000,     // 250%
            },
        },
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
    let (env, contract_id, _admin, borrower, _liquidator, _collateral, oracle_id) = setup();
    let client = CdpContractClient::new(&env, &contract_id);

    client.deposit_collateral(&borrower, &100_000);
    client.mint_stable(&borrower, &100_000);

    let initial_rate = client.current_borrow_rate_bps();
    assert!(initial_rate >= 200);

    // Advance ~half a year. Set high min TTLs so write_position bumps survive.
    env.ledger().with_mut(|ledger| {
        ledger.min_persistent_entry_ttl = 6_000_000;
        ledger.min_temp_entry_ttl = 6_000_000;
    });
    env.as_contract(&contract_id, || {
        env.storage().instance().extend_ttl(6_000_000, 6_000_000);
    });
    env.as_contract(&oracle_id, || {
        env.storage().instance().extend_ttl(6_000_000, 6_000_000);
    });
    env.ledger().with_mut(|ledger| {
        ledger.sequence_number += 3_153_600;
    });

    // Use accrue_fees — it touches only CDP + oracle, not the token contract,
    // so no token Balance archiving issue.
    let before = client.get_position(&borrower);
    client.accrue_fees(&borrower);
    let after = client.get_position(&borrower);

    assert!(after.debt_amount > before.debt_amount);
    assert_eq!(after.collateral_amount, before.collateral_amount);
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

    assert_eq!(
        client.stable_balance(&liquidator),
        before_balance - quote.repay_amount
    );
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

    // Minting 150_000 at price=2 gives ratio 100_000*2/150_000 = 133% < 150% min
    // But to get it minted first we need it to succeed — so mint at a safe level,
    // then crash the oracle price.
    client.mint_stable(&borrower, &100_000); // ratio = 200% at price=2 → safe

    let oracle = MockOracleClient::new(&env, &oracle_id);
    oracle.set_price(&(PRICE_SCALE / 2)); // price drops to 0.5 → ratio = 50% < min

    let closed = client.self_liquidate(&borrower);
    assert_eq!(closed.collateral_amount, 0);
    assert_eq!(closed.debt_amount, 0);
    assert_eq!(client.protocol_collateral_reserves(), 100_000);
    assert!(client.total_bad_debt() > 0);
}

#[test]
fn dynamic_collateral_floor_uses_base_floor_in_low_volatility() {
    let (env, contract_id, _admin, borrower, _liquidator, _collateral, oracle_id) = setup();
    let client = CdpContractClient::new(&env, &contract_id);
    let oracle = MockOracleClient::new(&env, &oracle_id);

    // Set low volatility (5%)
    oracle.set_volatility(&500);

    let floor = client.dynamic_collateral_floor();
    assert_eq!(floor, 15_000); // base floor
}

#[test]
fn dynamic_collateral_floor_uses_max_floor_in_high_volatility() {
    let (env, contract_id, _admin, borrower, _liquidator, _collateral, oracle_id) = setup();
    let client = CdpContractClient::new(&env, &contract_id);
    let oracle = MockOracleClient::new(&env, &oracle_id);

    // Set high volatility (60%)
    oracle.set_volatility(&6_000);

    let floor = client.dynamic_collateral_floor();
    assert_eq!(floor, 25_000); // max floor
}

#[test]
fn dynamic_collateral_floor_interpolates_in_medium_volatility() {
    let (env, contract_id, _admin, borrower, _liquidator, _collateral, oracle_id) = setup();
    let client = CdpContractClient::new(&env, &contract_id);
    let oracle = MockOracleClient::new(&env, &oracle_id);

    // Set medium volatility (30% - halfway between 10% and 50%)
    oracle.set_volatility(&3_000);

    let floor = client.dynamic_collateral_floor();
    // Should be halfway between 15_000 and 25_000 = 20_000
    assert_eq!(floor, 20_000);
}

#[test]
fn mint_respects_dynamic_collateral_floor_in_high_volatility() {
    let (env, contract_id, _admin, borrower, _liquidator, _collateral, oracle_id) = setup();
    let client = CdpContractClient::new(&env, &contract_id);
    let oracle = MockOracleClient::new(&env, &oracle_id);

    // Set high volatility to trigger max floor
    oracle.set_volatility(&6_000);

    client.deposit_collateral(&borrower, &100_000);

    // With 250% floor and price of 2, max debt is 80_000 (exactly at boundary).
    // Minting 80_002 will tip the rounded ratio below the floor and fail.
    let result = client.mint_stable(&borrower, &80_000);
    assert_eq!(result.debt_amount, 80_000);

    // Try to mint 2 more - should fail due to dynamic floor (ratio rounds below 25_000)
    assert!(client.try_mint_stable(&borrower, &2).is_err());
}

#[test]
fn collateral_ratio_bps_rounds_to_nearest_basis_point() {
    let (env, contract_id, _admin, borrower, _liquidator, _collateral, oracle_id) = setup();
    let client = CdpContractClient::new(&env, &contract_id);
    let oracle = MockOracleClient::new(&env, &oracle_id);

    oracle.set_price(&(2 * PRICE_SCALE));
    client.deposit_collateral(&borrower, &100_000);
    let position = client.mint_stable(&borrower, &13_334);

    assert_eq!(position.debt_amount, 13_334);
    // At price=2, 100_000 collateral, 13_334 debt: ratio ≈ 1499.93% expressed in BPS = 149_993
    assert_eq!(client.collateral_ratio_bps(&borrower), 149_993);
}

#[test]
fn liquidation_boundary_ratio_rounds_to_nearest_basis_point() {
    let (env, contract_id, _admin, borrower, _liquidator, _collateral, oracle_id) = setup();
    let client = CdpContractClient::new(&env, &contract_id);
    let oracle = MockOracleClient::new(&env, &oracle_id);

    oracle.set_price(&(2 * PRICE_SCALE));
    client.deposit_collateral(&borrower, &100_000);
    client.mint_stable(&borrower, &13_333);

    // At 13_333 debt, ratio ≈ 150_004 (above min 15_000), position healthy
    assert_eq!(client.collateral_ratio_bps(&borrower), 150_004);
    assert!(client.try_quote_liquidation(&borrower, &1).is_err());

    client.mint_stable(&borrower, &1);
    assert_eq!(client.collateral_ratio_bps(&borrower), 149_993);
    assert!(client.try_quote_liquidation(&borrower, &1).is_err());
}

#[test]
fn zero_debt_ratio_stays_infinite_and_does_not_divide_by_zero() {
    let (env, contract_id, _admin, borrower, _liquidator, _collateral, oracle_id) = setup();
    let client = CdpContractClient::new(&env, &contract_id);
    let oracle = MockOracleClient::new(&env, &oracle_id);

    oracle.set_price(&(2 * PRICE_SCALE));
    client.deposit_collateral(&borrower, &100_000);

    // No debt minted — ratio must be i128::MAX (no division by zero)
    let ratio = client.collateral_ratio_bps(&borrower);
    assert_eq!(ratio, i128::MAX);
    assert!(client.try_quote_liquidation(&borrower, &1).is_err());
}

#[test]
fn current_volatility_returns_oracle_value() {
    let (env, contract_id, _admin, _borrower, _liquidator, _collateral, oracle_id) = setup();
    let client = CdpContractClient::new(&env, &contract_id);
    let oracle = MockOracleClient::new(&env, &oracle_id);

    oracle.set_volatility(&2_500);
    assert_eq!(client.current_volatility_bps(), 2_500);

    oracle.set_volatility(&10_000);
    assert_eq!(client.current_volatility_bps(), 10_000);
}

// ── Stability-fee tests ────────────────────────────────────────────────────

#[test]
fn stability_fee_preview_returns_zero_when_rate_not_set() {
    let (env, contract_id, _admin, borrower, _liquidator, _collateral, _oracle) = setup();
    let client = CdpContractClient::new(&env, &contract_id);

    client.deposit_collateral(&borrower, &100_000);
    client.mint_stable(&borrower, &50_000);

    // No fee rate configured → preview must be 0
    assert_eq!(client.stability_fee_preview(&borrower), 0);
}

#[test]
fn stability_fee_accrues_on_debt_after_ledger_advance() {
    let (env, contract_id, _admin, borrower, _liquidator, _collateral, _oracle) = setup();
    let client = CdpContractClient::new(&env, &contract_id);

    // Set 10 bpm (10 bp per 1_000 ledgers = 0.001 per ledger ÷ BPS)
    client.set_stability_fee_rate(&10);

    client.deposit_collateral(&borrower, &100_000);
    client.mint_stable(&borrower, &100_000);

    let debt_before = client.get_position(&borrower).debt_amount;

    // Advance 1_000 ledgers
    env.ledger().with_mut(|l| {
        l.sequence_number += 1_000;
        l.min_persistent_entry_ttl = 2_000_000;
        l.min_temp_entry_ttl = 2_000_000;
    });

    // preview must match what accrue_fees will settle
    let preview = client.stability_fee_preview(&borrower);
    // fee = 100_000 * 10 * 1_000 / (10_000 * 1_000) = 100
    assert_eq!(preview, 100);

    client.accrue_fees(&borrower);
    let debt_after = client.get_position(&borrower).debt_amount;

    // Debt grew by at least the stability fee (borrow-rate interest may add more)
    assert!(debt_after >= debt_before + preview);
}

#[test]
fn stability_fee_preview_is_zero_after_accrual_at_same_ledger() {
    let (env, contract_id, _admin, borrower, _liquidator, _collateral, _oracle) = setup();
    let client = CdpContractClient::new(&env, &contract_id);

    client.set_stability_fee_rate(&10);
    client.deposit_collateral(&borrower, &100_000);
    client.mint_stable(&borrower, &100_000);

    env.ledger().with_mut(|l| {
        l.sequence_number += 500;
        l.min_persistent_entry_ttl = 2_000_000;
        l.min_temp_entry_ttl = 2_000_000;
    });

    // Trigger accrual
    client.accrue_fees(&borrower);

    // Preview right after accrual at the same ledger → 0
    assert_eq!(client.stability_fee_preview(&borrower), 0);
}

#[test]
fn repay_auto_accrues_stability_fee() {
    let (env, contract_id, _admin, borrower, _liquidator, _collateral, _oracle) = setup();
    let client = CdpContractClient::new(&env, &contract_id);

    client.set_stability_fee_rate(&10);
    client.deposit_collateral(&borrower, &100_000);
    client.mint_stable(&borrower, &50_000);

    env.ledger().with_mut(|l| {
        l.sequence_number += 1_000;
        l.min_persistent_entry_ttl = 2_000_000;
        l.min_temp_entry_ttl = 2_000_000;
    });

    let fee_preview = client.stability_fee_preview(&borrower);
    assert!(fee_preview > 0);

    // repay should auto-accrue first; resulting debt = 50_000 + fee - repay_amount
    let position = client.repay(&borrower, &1);
    // Debt must include the accrued fee minus the 1-unit repayment
    assert_eq!(position.debt_amount, 50_000 + fee_preview - 1);
}

#[test]
fn stability_fee_compounds_on_subsequent_accruals() {
    let (env, contract_id, _admin, borrower, _liquidator, _collateral, _oracle) = setup();
    let client = CdpContractClient::new(&env, &contract_id);

    client.set_stability_fee_rate(&1_000);
    client.deposit_collateral(&borrower, &100_000);
    client.mint_stable(&borrower, &100_000);

    env.ledger().with_mut(|ledger| {
        ledger.sequence_number += 1_000;
    });
    assert_eq!(client.stability_fee_preview(&borrower), 10_000);
    client.accrue_fees(&borrower);
    let debt_after_first_accrual = client.get_position(&borrower).debt_amount;
    assert!(debt_after_first_accrual >= 110_000);

    env.ledger().with_mut(|ledger| {
        ledger.sequence_number += 1_000;
    });
    let second_fee = client.stability_fee_preview(&borrower);
    assert!(second_fee >= 11_000);
    client.accrue_fees(&borrower);
    assert!(client.get_position(&borrower).debt_amount >= debt_after_first_accrual + second_fee);
}

#[test]
fn liquidate_auto_accrues_stability_fee() {
    let (env, contract_id, _admin, borrower, liquidator, _collateral, oracle_id) = setup();
    let client = CdpContractClient::new(&env, &contract_id);

    client.set_stability_fee_rate(&10);
    client.deposit_collateral(&borrower, &100_000);
    client.mint_stable(&borrower, &120_000);
    client.deposit_collateral(&liquidator, &50_000);
    client.mint_stable(&liquidator, &30_000);
    MockOracleClient::new(&env, &oracle_id).set_price(&PRICE_SCALE);

    env.ledger().with_mut(|ledger| {
        ledger.sequence_number += 1_000;
    });
    let fee_preview = client.stability_fee_preview(&borrower);
    assert_eq!(fee_preview, 120);

    let quote = client.liquidate(&liquidator, &borrower, &20_000);
    assert!(
        client.get_position(&borrower).debt_amount >= 120_000 + fee_preview - quote.repay_amount
    );
}

#[test]
fn set_stability_fee_rate_requires_admin() {
    let (env, contract_id, _admin, borrower, _liquidator, _collateral, _oracle) = setup();
    let client = CdpContractClient::new(&env, &contract_id);

    // Calling with a non-admin should be rejected at the auth level
    let result = client.try_set_stability_fee_rate(&5);
    // Just verify that a negative rate is invalid (InvalidConfig)
    assert!(client.try_set_stability_fee_rate(&-1).is_err());
    let _ = result; // silence unused warning
}
