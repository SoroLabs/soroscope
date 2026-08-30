use crate::{MultiYieldVault, MultiYieldVaultClient};
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

// ── Mock AMM pool ─────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
enum MockKey {
    ReserveA,
    ReserveB,
    Fee,
    TotalLp,
    LpBalance(Address),
}

#[contract]
struct MockPool;

#[contractimpl]
impl MockPool {
    pub fn init(e: Env, reserve_a: i128, reserve_b: i128, fee_bps: i128) {
        e.storage().instance().set(&MockKey::ReserveA, &reserve_a);
        e.storage().instance().set(&MockKey::ReserveB, &reserve_b);
        e.storage().instance().set(&MockKey::Fee, &fee_bps);
        e.storage().instance().set(&MockKey::TotalLp, &0i128);
    }

    pub fn deposit(e: Env, to: Address, amount_a: i128, amount_b: i128) -> i128 {
        let ra: i128 = e.storage().instance().get(&MockKey::ReserveA).unwrap_or(0);
        let rb: i128 = e.storage().instance().get(&MockKey::ReserveB).unwrap_or(0);
        let total_lp: i128 = e.storage().instance().get(&MockKey::TotalLp).unwrap_or(0);

        let deposit_amount = amount_a.max(amount_b);
        let lp = if total_lp == 0 { deposit_amount } else { deposit_amount };

        e.storage().instance().set(&MockKey::ReserveA, &(ra + amount_a));
        e.storage().instance().set(&MockKey::ReserveB, &(rb + amount_b));
        e.storage().instance().set(&MockKey::TotalLp, &(total_lp + lp));

        let key = MockKey::LpBalance(to.clone());
        let cur: i128 = e.storage().persistent().get(&key).unwrap_or(0);
        e.storage().persistent().set(&key, &(cur + lp));
        lp
    }

    pub fn withdraw(e: Env, to: Address, share_amount: i128) -> (i128, i128) {
        let ra: i128 = e.storage().instance().get(&MockKey::ReserveA).unwrap_or(0);
        let rb: i128 = e.storage().instance().get(&MockKey::ReserveB).unwrap_or(0);
        let total_lp: i128 = e.storage().instance().get(&MockKey::TotalLp).unwrap_or(1);

        let out_a = share_amount * ra / total_lp;
        let out_b = share_amount * rb / total_lp;

        e.storage().instance().set(&MockKey::ReserveA, &(ra - out_a));
        e.storage().instance().set(&MockKey::ReserveB, &(rb - out_b));
        e.storage().instance().set(&MockKey::TotalLp, &(total_lp - share_amount));

        let key = MockKey::LpBalance(to.clone());
        let cur: i128 = e.storage().persistent().get(&key).unwrap_or(0);
        e.storage().persistent().set(&key, &(cur - share_amount));
        (out_a, out_b)
    }

    pub fn get_reserve_a(e: Env) -> i128 {
        e.storage().instance().get(&MockKey::ReserveA).unwrap_or(0)
    }

    pub fn get_reserve_b(e: Env) -> i128 {
        e.storage().instance().get(&MockKey::ReserveB).unwrap_or(0)
    }

    pub fn get_fee(e: Env) -> i128 {
        e.storage().instance().get(&MockKey::Fee).unwrap_or(30)
    }

    pub fn token_a(e: Env) -> Address {
        Address::generate(&e) // unused in vault logic
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn setup(e: &Env) -> (MultiYieldVaultClient, Address, Address) {
    let admin = Address::generate(e);
    let deposit_token = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let vault_id = e.register(MultiYieldVault, ());
    let client = MultiYieldVaultClient::new(e, &vault_id);
    client.initialize(&admin, &deposit_token, &100, &10_000);
    (client, admin, deposit_token)
}

fn register_mock_pool(e: &Env, admin: &Address, reserve: i128, fee_bps: i128) -> Address {
    let pool_id = e.register(MockPool, ());
    let pool_client = MockPoolClient::new(e, &pool_id);
    pool_client.init(&reserve, &reserve, &fee_bps);
    pool_id
}

fn mint(e: &Env, admin: &Address, token: &Address, to: &Address, amount: i128) {
    soroban_sdk::token::StellarAssetClient::new(e, token).mint(to, &amount);
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn test_initialize() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, _, _) = setup(&e);
    let vault = client.get_vault();
    assert_eq!(vault.total_shares, 0);
    assert_eq!(vault.slippage_bps, 100);
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn test_double_initialize() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, deposit_token) = setup(&e);
    client.initialize(&admin, &deposit_token, &100, &10_000);
}

#[test]
fn test_register_pool() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, _) = setup(&e);
    let pool = register_mock_pool(&e, &admin, 10_000, 30);
    client.register_pool(&pool, &true);
    assert_eq!(client.get_pools().len(), 1);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_register_duplicate_pool() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, _) = setup(&e);
    let pool = register_mock_pool(&e, &admin, 10_000, 30);
    client.register_pool(&pool, &true);
    client.register_pool(&pool, &true); // duplicate
}

#[test]
fn test_apr_estimation() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, _) = setup(&e);

    let pool_low = register_mock_pool(&e, &admin, 10_000, 10);  // 10 bps fee
    let pool_high = register_mock_pool(&e, &admin, 10_000, 50); // 50 bps fee

    client.register_pool(&pool_low, &true);
    client.register_pool(&pool_high, &true);

    let aprs = client.get_aprs();
    assert_eq!(aprs.len(), 2);
    // Higher fee → higher APR estimate.
    assert!(aprs.get(1).unwrap() > aprs.get(0).unwrap());
}

#[test]
fn test_deposit_routes_to_best_pool() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, deposit_token) = setup(&e);

    let pool_low = register_mock_pool(&e, &admin, 10_000, 10);
    let pool_high = register_mock_pool(&e, &admin, 10_000, 50);
    client.register_pool(&pool_low, &true);
    client.register_pool(&pool_high, &true);

    let user = Address::generate(&e);
    mint(&e, &admin, &deposit_token, &user, 1_000);

    let shares = client.deposit(&user, &1_000);
    assert_eq!(shares, 1_000);
    assert_eq!(client.vault_balance(&user), 1_000);

    // LP shares should be in pool_high (index 1), not pool_low (index 0).
    let pools = client.get_pools();
    assert_eq!(pools.get(0).unwrap().lp_shares, 0);
    assert!(pools.get(1).unwrap().lp_shares > 0);
}

#[test]
fn test_withdraw_returns_tokens() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, deposit_token) = setup(&e);

    let pool = register_mock_pool(&e, &admin, 10_000, 30);
    client.register_pool(&pool, &true);

    let user = Address::generate(&e);
    mint(&e, &admin, &deposit_token, &user, 1_000);
    client.deposit(&user, &1_000);

    let received = client.withdraw(&user, &500);
    assert!(received > 0);
    assert_eq!(client.vault_balance(&user), 500);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_withdraw_too_many_shares() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, deposit_token) = setup(&e);

    let pool = register_mock_pool(&e, &admin, 10_000, 30);
    client.register_pool(&pool, &true);

    let user = Address::generate(&e);
    mint(&e, &admin, &deposit_token, &user, 1_000);
    client.deposit(&user, &1_000);
    client.withdraw(&user, &2_000); // more than owned
}

#[test]
fn test_rebalance_moves_funds_to_best_pool() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, deposit_token) = setup(&e);

    // Register low-fee pool first so initial deposit goes there.
    // Then register high-fee pool and rebalance.
    let pool_low = register_mock_pool(&e, &admin, 10_000, 10);
    client.register_pool(&pool_low, &true);

    let user = Address::generate(&e);
    mint(&e, &admin, &deposit_token, &user, 1_000);
    client.deposit(&user, &1_000);

    // Confirm funds are in pool_low.
    assert!(client.get_pools().get(0).unwrap().lp_shares > 0);

    // Register a better pool.
    let pool_high = register_mock_pool(&e, &admin, 10_000, 50);
    client.register_pool(&pool_high, &true);

    client.rebalance();

    let pools = client.get_pools();
    // pool_low should be drained.
    assert_eq!(pools.get(0).unwrap().lp_shares, 0);
    // pool_high should have received the funds.
    assert!(pools.get(1).unwrap().lp_shares > 0);
}

#[test]
fn test_rebalance_noop_with_one_pool() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, admin, deposit_token) = setup(&e);

    let pool = register_mock_pool(&e, &admin, 10_000, 30);
    client.register_pool(&pool, &true);

    let user = Address::generate(&e);
    mint(&e, &admin, &deposit_token, &user, 500);
    client.deposit(&user, &500);

    let lp_before = client.get_pools().get(0).unwrap().lp_shares;
    client.rebalance(); // should be a no-op
    let lp_after = client.get_pools().get(0).unwrap().lp_shares;
    assert_eq!(lp_before, lp_after);
}

#[test]
fn test_set_slippage() {
    let e = Env::default();
    e.mock_all_auths();
    let (client, _, _) = setup(&e);
    client.set_slippage(&200);
    assert_eq!(client.get_vault().slippage_bps, 200);
}
