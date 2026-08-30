#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol, Vec};

#[contracttype]
pub enum DataKey {
    Seller,
    NftContract,
    TokenId,
    PaymentToken,
    StartPrice,
    EndPrice,
    StartLedger,
    EndLedger,
    Sold,
}

#[contract]
pub struct DutchAuction;

#[contractimpl]
impl DutchAuction {
    pub fn initialize(
        env: Env,
        seller: Address,
        nft_contract: Address,
        token_id: i128,
        payment_token: Address,
        start_price: i128,
        end_price: i128,
        duration_ledgers: u32,
    ) {
        if env.storage().instance().has(&DataKey::Seller) {
            panic!("Already initialized");
        }

        if start_price <= 0 || end_price < 0 {
            panic!("prices must be non-negative and start_price must be > 0");
        }
        if end_price > start_price {
            panic!("end_price (floor) cannot exceed start_price");
        }
        if duration_ledgers == 0 {
            panic!("duration_ledgers must be greater than zero");
        }
        
        let start_ledger = env.ledger().sequence();
        let end_ledger = start_ledger + duration_ledgers;

        env.storage().instance().set(&DataKey::Seller, &seller);
        env.storage().instance().set(&DataKey::NftContract, &nft_contract);
        env.storage().instance().set(&DataKey::TokenId, &token_id);
        env.storage().instance().set(&DataKey::PaymentToken, &payment_token);
        env.storage().instance().set(&DataKey::StartPrice, &start_price);
        env.storage().instance().set(&DataKey::EndPrice, &end_price);
        env.storage().instance().set(&DataKey::StartLedger, &start_ledger);
        env.storage().instance().set(&DataKey::EndLedger, &end_ledger);
        env.storage().instance().set(&DataKey::Sold, &false);
    }

   /// Computes the current Dutch auction price via linear decay between
    /// `start_price` and `end_price` (the floor), bounded so the price
    /// never falls below `end_price` and never overflows, regardless of
    /// how far past `end_ledger` the current ledger sequence is.
    pub fn get_current_price(env: Env) -> i128 {
        let start_price: i128 = env.storage().instance().get(&DataKey::StartPrice).unwrap();
        let end_price: i128 = env.storage().instance().get(&DataKey::EndPrice).unwrap();
        let start_ledger: u32 = env.storage().instance().get(&DataKey::StartLedger).unwrap();
        let end_ledger: u32 = env.storage().instance().get(&DataKey::EndLedger).unwrap();
        let current_ledger = env.ledger().sequence();

        if current_ledger >= end_ledger {
            return end_price;
        }
        if current_ledger <= start_ledger {
            return start_price;
        }

        let total_duration: i128 = (end_ledger - start_ledger) as i128;
        let elapsed: i128 = (current_ledger - start_ledger) as i128;

        // price_drop can be negative only if end_price > start_price,
        // which `initialize` now rejects — kept as checked math regardless.
        let price_drop: i128 = start_price
            .checked_sub(end_price)
            .unwrap_or(0)
            .max(0);

        let current_drop: i128 = price_drop
            .checked_mul(elapsed)
            .and_then(|v| v.checked_div(total_duration))
            .unwrap_or(price_drop); // overflow fallback: treat as fully decayed

        let price = start_price
            .checked_sub(current_drop)
            .unwrap_or(end_price);

        // Explicit floor safeguard — price can never drop below end_price.
        if price < end_price {
            end_price
        } else {
            price
        }
    }

    pub fn buy(env: Env, buyer: Address) {
        buyer.require_auth();

        let sold: bool = env.storage().instance().get(&DataKey::Sold).unwrap();
        if sold {
            panic!("Already sold");
        }

        let current_price = Self::get_current_price(env.clone());

        let payment_token: Address = env.storage().instance().get(&DataKey::PaymentToken).unwrap();

        // Transfer payment from buyer to contract
        env.invoke_contract(
            &payment_token,
            &Symbol::new(&env, "transfer"),
            Vec::from_array(&env, [buyer.to_val(), env.current_contract_address().to_val(), current_price.into_val(&env)]),
        );

        // Transfer NFT to buyer
        let nft_contract: Address = env.storage().instance().get(&DataKey::NftContract).unwrap();
        let token_id: i128 = env.storage().instance().get(&DataKey::TokenId).unwrap();
        env.invoke_contract(
            &nft_contract,
            &Symbol::new(&env, "transfer"),
            Vec::from_array(&env, [env.current_contract_address().to_val(), buyer.to_val(), token_id.into_val(&env)]),
        );

        // Transfer payment to seller
        let seller: Address = env.storage().instance().get(&DataKey::Seller).unwrap();
        env.invoke_contract(
            &payment_token,
            &Symbol::new(&env, "transfer"),
            Vec::from_array(&env, [env.current_contract_address().to_val(), seller.to_val(), current_price.into_val(&env)]),
        );

        env.storage().instance().set(&DataKey::Sold, &true);
    }

    // If not sold by end, seller can reclaim NFT
    pub fn reclaim(env: Env) {
        let end_ledger: u32 = env.storage().instance().get(&DataKey::EndLedger).unwrap();
        if env.ledger().sequence() < end_ledger {
            panic!("Auction not ended");
        }

        let sold: bool = env.storage().instance().get(&DataKey::Sold).unwrap();
        if sold {
            panic!("Already sold");
        }

        let seller: Address = env.storage().instance().get(&DataKey::Seller).unwrap();
        seller.require_auth();

        let nft_contract: Address = env.storage().instance().get(&DataKey::NftContract).unwrap();
        let token_id: i128 = env.storage().instance().get(&DataKey::TokenId).unwrap();
        env.invoke_contract(
            &nft_contract,
            &Symbol::new(&env, "transfer"),
            Vec::from_array(&env, [env.current_contract_address().to_val(), seller.to_val(), token_id.into_val(&env)]),
        );

        env.storage().instance().set(&DataKey::Sold, &true);
    }

    pub fn is_sold(env: Env) -> bool {
        env.storage().instance().get(&DataKey::Sold).unwrap()
    }
}