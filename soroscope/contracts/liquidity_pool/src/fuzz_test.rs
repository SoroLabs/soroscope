use crate::LiquidityPoolClient;
use proptest::prelude::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn test_swap_invariant(
        reserve_a in 1_000i128..1_000_000_000_000_000_000i128,
        reserve_b in 1_000i128..1_000_000_000_000_000_000i128,
        amount_out in 1i128..1_000_000_000_000_000_000i128,
        buy_a in any::<bool>(),
    ) {
        let e = Env::default();
        e.mock_all_auths();
        e.cost_estimate().budget().reset_unlimited();

        // Derive a strictly smaller amount_out using modulo so we don't reject generated examples
        let max_out = if buy_a { reserve_a } else { reserve_b };
        let valid_amount_out = (amount_out % (max_out - 1)) + 1;

        let admin = Address::generate(&e);
        let token_a = e.register_stellar_asset_contract_v2(admin.clone()).address();
        let token_b = e.register_stellar_asset_contract_v2(admin.clone()).address();

        let contract_id = e.register(crate::LiquidityPool, ());
        let client = LiquidityPoolClient::new(&e, &contract_id);

        client.initialize(&admin, &token_a, &token_b);

        let user = Address::generate(&e);
        let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
        let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

        // Give the user enough to deposit
        token_a_admin.mint(&user, &reserve_a);
        token_b_admin.mint(&user, &reserve_b);
        client.deposit(&user, &reserve_a, &reserve_b);

        let k_before = reserve_a * reserve_b; // Fits in i128 if inputs are < 10^18

        // We need another user for swapping
        let swapper = Address::generate(&e);
        // We need to give the swapper an effectively infinite input amount
        let in_max = 50_000_000_000_000_000_000i128; // Give swapper 50 tokens of 10^18 precision
        token_a_admin.mint(&swapper, &in_max);
        token_b_admin.mint(&swapper, &in_max);

        // Perform the swap
        // A swap can fail due to slippage exceeded if in_max wasn't enough, which is an expected error.
        // But with a huge in_max it shouldn't fail.
        let res = client.try_swap(&swapper, &buy_a, &valid_amount_out, &in_max);

        if let Ok(Ok(_)) = res {
            // Verify invariant: the reserves increased K
            let token_a_client = soroban_sdk::token::Client::new(&e, &token_a);
            let token_b_client = soroban_sdk::token::Client::new(&e, &token_b);

            let pool_balance_a = token_a_client.balance(&contract_id);
            let pool_balance_b = token_b_client.balance(&contract_id);

            let k_after = pool_balance_a * pool_balance_b;

            assert!(k_after >= k_before, "Invariant violated! K before: {}, K after: {}", k_before, k_after);
        }
    }
}
