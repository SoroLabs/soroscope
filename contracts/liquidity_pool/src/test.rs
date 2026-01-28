#![cfg(test)]
use super::*;
use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env};

#[test]
fn test() {
    let e = Env::default();
    e.mock_all_auths();

    let contract_id = e.register_contract(None, LiquidityPool);
    let client = LiquidityPoolClient::new(&e, &contract_id);

    // Setup tokens
    let admin = Address::generate(&e);
    let token_a = e.register_stellar_asset_contract(admin.clone());
    let token_b = e.register_stellar_asset_contract(admin.clone());
    
    let token_a_client = soroban_sdk::token::Client::new(&e, &token_a);
    let token_b_client = soroban_sdk::token::Client::new(&e, &token_b);

    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);

    e.budget().reset_unlimited();

    // Check initialize
    client.initialize(&token_a, &token_b);

    let token_a_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_a);
    let token_b_admin = soroban_sdk::token::StellarAssetClient::new(&e, &token_b);

    // Mint tokens to users
    token_a_admin.mint(&user1, &10000);
    token_b_admin.mint(&user1, &10000);
    token_a_admin.mint(&user2, &10000);
    token_b_admin.mint(&user2, &10000);

    // User 1 Deposits 1000 of each
    let shares = client.deposit(&user1, &1000, &1000);
    assert_eq!(shares, 1000);
    
    // Check reserves
    // Note: We can't check private storage directly easily without client exposing getters or events, 
    // but for now we trust the swap logic to prove reserves are there.

    // User 2 Swaps 100 A for B
    // Reserve A = 1000, Reserve B = 1000
    // AmountIn = 100 (A)
    // Buy B (buy_a = false)
    // Expected Out:
    // k = 1000 * 1000 = 1,000,000
    // (1000 + 100) * (1000 - out) = 1,000,000
    // 1100 * (1000 - out) = 1,000,000
    // 1,100,000 - 1100*out = 1,000,000
    // 1100*out = 100,000
    // out = 90.90.. -> 90
    // With fee:
    // AmountInWithFee = 100 * 1000/997 ~= 100 (negligible diff for small nums but let's see formula)
    // numerator = 1000 * 100 * 1000 = 100,000,000
    // denominator = (1000 + 100) * 997 = 1100 * 997 = 1,096,700 ?? No wait formula is for AmountIn
    // Let's rely on the function: swap(buy_a=false, out=90)
    // We specify OUT.
    
    let out_amount = 90;
    let in_max = 110;
    
    // Swap 90 B out, pay with A
    let paid = client.swap(&user2, &false, &out_amount, &in_max);
    
    // Check balances
    assert_eq!(token_b_client.balance(&user2), 10000 + 90);
    assert_eq!(token_a_client.balance(&user2), 10000 - paid);

    // User 1 Withdraws
    let (withdrawn_a, withdrawn_b) = client.withdraw(&user1, &1000);
    // Should get roughly remaining reserves
    assert!(withdrawn_a > 1000); // Gained fees (paid by user2)
    assert!(withdrawn_b < 1000); // Lost due to User 2 taking B
}
