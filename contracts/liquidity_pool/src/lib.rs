#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[cfg(test)]
mod test;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    TokenA,
    TokenB,
    ReserveA,
    ReserveB,
    ShareToken,
    TotalShares,
    Balance(Address),
}

#[contract]
pub struct LiquidityPool;

#[contractimpl]
impl LiquidityPool {
    pub fn initialize(e: Env, token_a: Address, token_b: Address) {
        if e.storage().instance().has(&DataKey::TokenA) {
            panic!("Already initialized");
        }
        e.storage().instance().set(&DataKey::TokenA, &token_a);
        e.storage().instance().set(&DataKey::TokenB, &token_b);
        e.storage().instance().set(&DataKey::ReserveA, &0i128);
        e.storage().instance().set(&DataKey::ReserveB, &0i128);
        e.storage().instance().set(&DataKey::TotalShares, &0i128);
    }

    pub fn deposit(e: Env, to: Address, amount_a: i128, amount_b: i128) -> i128 {
        to.require_auth();
        
        // Transfer tokens to the contract
        let token_a_addr: Address = e.storage().instance().get(&DataKey::TokenA).unwrap();
        let token_b_addr: Address = e.storage().instance().get(&DataKey::TokenB).unwrap();

        // Note: In a real production AMM, we'd check allowances or use transfer_from if supported,
        // but for this simplified version, we assume the user has sent tokens or we use the standard token client.
        // Soroban token interface standard: transfer(from, to, amount)
        let client_a = soroban_sdk::token::Client::new(&e, &token_a_addr);
        let client_b = soroban_sdk::token::Client::new(&e, &token_b_addr);

        client_a.transfer(&to, &e.current_contract_address(), &amount_a);
        client_b.transfer(&to, &e.current_contract_address(), &amount_b);

        let reserve_a: i128 = e.storage().instance().get(&DataKey::ReserveA).unwrap();
        let reserve_b: i128 = e.storage().instance().get(&DataKey::ReserveB).unwrap();
        let total_shares: i128 = e.storage().instance().get(&DataKey::TotalShares).unwrap();

        let shares: i128;
        if total_shares == 0 {
            // Initial liquidity = sqrt(amount_a * amount_b) - MINIMUM_LIQUIDITY (simplified to just sqrt here)
            // For integer arithmetic simplification, we'll just use geometric mean proxy or direct product if small enough, 
            // but standard is sqrt(a*b). Soroban doesn't have i128 sqrt easily accessible without imports or algos.
            // We'll use a simplified model: shares = amount_a + amount_b for this MVP unless we implement sqrt.
            // Let's implement a basic sqrt for i128 or just use linear sum for the "simplified" aspect if Acceptable.
            // Acceptance Criteria says "simplified". Linear sum is risky for arbitrage but functional for profiling storage.
            // actually typical CPMM requires product constraint. 
            // Let's use `shares = amount_a` assuming 1:1 initial ratio provided, or just `amount_a` as the initial share metric.
            shares = amount_a; 
        } else {
            // shares = min(amount_a * total / reserve_a, amount_b * total / reserve_b)
            let share_a = amount_a * total_shares / reserve_a;
            let share_b = amount_b * total_shares / reserve_b;
            if share_a < share_b {
                shares = share_a;
            } else {
                shares = share_b;
            }
        }

        // Mint shares (store balance)
        let user_share_key = DataKey::Balance(to.clone());
        let current_user_share: i128 = e.storage().instance().get(&user_share_key).unwrap_or(0);
        e.storage().instance().set(&user_share_key, &(current_user_share + shares));
        e.storage().instance().set(&DataKey::TotalShares, &(total_shares + shares));

        // Update reserves
        e.storage().instance().set(&DataKey::ReserveA, &(reserve_a + amount_a));
        e.storage().instance().set(&DataKey::ReserveB, &(reserve_b + amount_b));

        shares
    }

    pub fn swap(e: Env, to: Address, buy_a: bool, out: i128, in_max: i128) -> i128 {
        to.require_auth();
        
        let token_a: Address = e.storage().instance().get(&DataKey::TokenA).unwrap();
        let token_b: Address = e.storage().instance().get(&DataKey::TokenB).unwrap();
        let reserve_a: i128 = e.storage().instance().get(&DataKey::ReserveA).unwrap();
        let reserve_b: i128 = e.storage().instance().get(&DataKey::ReserveB).unwrap();

        let (reserve_in, reserve_out, token_in, token_out) = if buy_a {
            (reserve_b, reserve_a, token_b, token_a) // Buying A means paying with B
        } else {
            (reserve_a, reserve_b, token_a, token_b) // Buying B means paying with A
        };

        // K = Rin * Rout
        // (Rin + AmountIn) * (Rout - AmountOut) = K
        // Rin * Rout + AmountIn * Rout - Rin * AmountOut - AmountIn * AmountOut = Rin * Rout
        // AmountIn * (Rout - AmountOut) = Rin * AmountOut
        // AmountIn = (Rin * AmountOut) / (Rout - AmountOut)
        // Add 0.3% fee: AmountInWithFee = AmountIn * 1000 / 997
        
        if out >= reserve_out {
            panic!("Insufficient liquidity");
        }

        let numerator = reserve_in * out * 1000;
        let denominator = (reserve_out - out) * 997;
        let amount_in = (numerator / denominator) + 1;

        if amount_in > in_max {
            panic!("Slippage exceeded");
        }

        // Transfer In
        let client_in = soroban_sdk::token::Client::new(&e, &token_in);
        client_in.transfer(&to, &e.current_contract_address(), &amount_in);

        // Transfer Out
        let client_out = soroban_sdk::token::Client::new(&e, &token_out);
        client_out.transfer(&e.current_contract_address(), &to, &out);

        // Update Reserves
        if buy_a {
            e.storage().instance().set(&DataKey::ReserveA, &(reserve_a - out));
            e.storage().instance().set(&DataKey::ReserveB, &(reserve_b + amount_in));
        } else {
            e.storage().instance().set(&DataKey::ReserveA, &(reserve_a + amount_in));
            e.storage().instance().set(&DataKey::ReserveB, &(reserve_b - out));
        }

        amount_in
    }

    pub fn withdraw(e: Env, to: Address, share_amount: i128) -> (i128, i128) {
        to.require_auth();

        let user_share_key = DataKey::Balance(to.clone());
        let current_user_share: i128 = e.storage().instance().get(&user_share_key).unwrap_or(0);
        if share_amount > current_user_share {
            panic!("Insufficient shares");
        }

        let total_shares: i128 = e.storage().instance().get(&DataKey::TotalShares).unwrap();
        let reserve_a: i128 = e.storage().instance().get(&DataKey::ReserveA).unwrap();
        let reserve_b: i128 = e.storage().instance().get(&DataKey::ReserveB).unwrap();

        let amount_a = share_amount * reserve_a / total_shares;
        let amount_b = share_amount * reserve_b / total_shares;

        // Burn shares
        e.storage().instance().set(&user_share_key, &(current_user_share - share_amount));
        e.storage().instance().set(&DataKey::TotalShares, &(total_shares - share_amount));

        // Update reserves
        e.storage().instance().set(&DataKey::ReserveA, &(reserve_a - amount_a));
        e.storage().instance().set(&DataKey::ReserveB, &(reserve_b - amount_b));

        // Transfer tokens back
        let token_a: Address = e.storage().instance().get(&DataKey::TokenA).unwrap();
        let token_b: Address = e.storage().instance().get(&DataKey::TokenB).unwrap();
        
        let client_a = soroban_sdk::token::Client::new(&e, &token_a);
        let client_b = soroban_sdk::token::Client::new(&e, &token_b);

        client_a.transfer(&e.current_contract_address(), &to, &amount_a);
        client_b.transfer(&e.current_contract_address(), &to, &amount_b);

        (amount_a, amount_b)
    }
}
