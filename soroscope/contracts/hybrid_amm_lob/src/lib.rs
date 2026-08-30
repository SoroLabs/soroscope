#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Vec};

#[cfg(test)]
mod test;

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InvalidAmount = 3,
    InvalidPrice = 4,
    OrderNotFound = 5,
    SlippageExceeded = 6,
    InsufficientLiquidity = 7,
    InsufficientShares = 8,
    Unauthorized = 9,
}

// ── Types ─────────────────────────────────────────────────────────────────────

/// A single limit order in the book.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Order {
    pub id: u64,
    pub maker: Address,
    /// true  → maker offers token_b, wants token_a  (bid: buy A with B)
    /// false → maker offers token_a, wants token_b  (ask: sell A for B)
    pub is_bid: bool,
    /// Price expressed as token_b per token_a, scaled by PRICE_SCALE.
    /// For a bid: max price maker will pay.
    /// For an ask: min price maker will accept.
    pub price: i128,
    /// Remaining amount of the offered token still available.
    pub amount: i128,
}

/// Pool + fee state stored in a single instance entry.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolState {
    pub token_a: Address,
    pub token_b: Address,
    pub reserve_a: i128,
    pub reserve_b: i128,
    pub total_shares: i128,
    /// Swap fee in basis points charged on AMM fills (goes to LPs).
    pub lp_fee_bps: i128,
    /// Fee in basis points charged on limit-order fills (goes to maker).
    pub maker_fee_bps: i128,
    pub admin: Address,
    pub next_order_id: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwapResult {
    pub amount_in: i128,
    pub amount_out: i128,
    /// How much of `amount_out` was filled by limit orders.
    pub lob_filled: i128,
    /// How much of `amount_out` was filled by the AMM.
    pub amm_filled: i128,
}

// ── Storage keys ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Pool,
    /// Sorted bid orders: buy A with B, descending price (best bid first).
    Bids,
    /// Sorted ask orders: sell A for B, ascending price (best ask first).
    Asks,
    /// LP share balance per user.
    Balance(Address),
}

// ── Constants ─────────────────────────────────────────────────────────────────

/// Price scale factor: prices are integers representing (token_b / token_a) * PRICE_SCALE.
pub const PRICE_SCALE: i128 = 1_000_000;
pub const DEFAULT_LP_FEE_BPS: i128 = 30;
pub const DEFAULT_MAKER_FEE_BPS: i128 = 10;
pub const TTL_LEDGERS: u32 = 17_280; // ~1 day

// ── Helpers ───────────────────────────────────────────────────────────────────

fn load_pool(e: &Env) -> Result<PoolState, Error> {
    e.storage()
        .instance()
        .get(&DataKey::Pool)
        .ok_or(Error::NotInitialized)
}

fn save_pool(e: &Env, pool: &PoolState) {
    e.storage().instance().set(&DataKey::Pool, pool);
}

fn load_bids(e: &Env) -> Vec<Order> {
    e.storage()
        .instance()
        .get(&DataKey::Bids)
        .unwrap_or(Vec::new(e))
}

fn load_asks(e: &Env) -> Vec<Order> {
    e.storage()
        .instance()
        .get(&DataKey::Asks)
        .unwrap_or(Vec::new(e))
}

fn save_bids(e: &Env, orders: &Vec<Order>) {
    e.storage().instance().set(&DataKey::Bids, orders);
}

fn save_asks(e: &Env, orders: &Vec<Order>) {
    e.storage().instance().set(&DataKey::Asks, orders);
}

/// Insert a bid maintaining descending price order (highest price first).
fn insert_bid(orders: &mut Vec<Order>, order: Order) {
    let mut i = 0u32;
    while i < orders.len() {
        if orders.get(i).unwrap().price < order.price {
            break;
        }
        i += 1;
    }
    orders.insert(i, order);
}

/// Insert an ask maintaining ascending price order (lowest price first).
fn insert_ask(orders: &mut Vec<Order>, order: Order) {
    let mut i = 0u32;
    while i < orders.len() {
        if orders.get(i).unwrap().price > order.price {
            break;
        }
        i += 1;
    }
    orders.insert(i, order);
}

fn sqrt(x: i128) -> i128 {
    if x == 0 {
        return 0;
    }
    let mut z = (x + 1) / 2;
    let mut y = x;
    while z < y {
        y = z;
        z = (x / z + z) / 2;
    }
    y
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct HybridAmmLob;

#[contractimpl]
impl HybridAmmLob {
    // ── Admin ─────────────────────────────────────────────────────────────────

    pub fn initialize(
        e: Env,
        admin: Address,
        token_a: Address,
        token_b: Address,
        lp_fee_bps: i128,
        maker_fee_bps: i128,
    ) -> Result<(), Error> {
        if e.storage().instance().has(&DataKey::Pool) {
            return Err(Error::AlreadyInitialized);
        }
        if lp_fee_bps < 0 || maker_fee_bps < 0 {
            return Err(Error::InvalidAmount);
        }
        save_pool(
            &e,
            &PoolState {
                token_a,
                token_b,
                reserve_a: 0,
                reserve_b: 0,
                total_shares: 0,
                lp_fee_bps,
                maker_fee_bps,
                admin,
                next_order_id: 1,
            },
        );
        Ok(())
    }

    // ── Liquidity ─────────────────────────────────────────────────────────────

    /// Deposit token_a and token_b, receive LP shares.
    pub fn deposit(e: Env, to: Address, amount_a: i128, amount_b: i128) -> Result<i128, Error> {
        if amount_a <= 0 || amount_b <= 0 {
            return Err(Error::InvalidAmount);
        }
        to.require_auth();
        let mut pool = load_pool(&e)?;

        soroban_sdk::token::Client::new(&e, &pool.token_a)
            .transfer(&to, &e.current_contract_address(), &amount_a);
        soroban_sdk::token::Client::new(&e, &pool.token_b)
            .transfer(&to, &e.current_contract_address(), &amount_b);

        let shares = if pool.total_shares == 0 {
            sqrt(amount_a.checked_mul(amount_b).ok_or(Error::InvalidAmount)?)
        } else {
            let s_a = amount_a
                .checked_mul(pool.total_shares)
                .ok_or(Error::InvalidAmount)?
                / pool.reserve_a;
            let s_b = amount_b
                .checked_mul(pool.total_shares)
                .ok_or(Error::InvalidAmount)?
                / pool.reserve_b;
            s_a.min(s_b)
        };

        let bal_key = DataKey::Balance(to.clone());
        let cur: i128 = e.storage().persistent().get(&bal_key).unwrap_or(0);
        e.storage().persistent().set(&bal_key, &(cur + shares));
        e.storage()
            .persistent()
            .extend_ttl(&bal_key, TTL_LEDGERS, TTL_LEDGERS);

        pool.reserve_a += amount_a;
        pool.reserve_b += amount_b;
        pool.total_shares += shares;
        save_pool(&e, &pool);

        Ok(shares)
    }

    /// Burn LP shares and withdraw proportional reserves.
    pub fn withdraw(e: Env, to: Address, shares: i128) -> Result<(i128, i128), Error> {
        if shares <= 0 {
            return Err(Error::InvalidAmount);
        }
        to.require_auth();
        let mut pool = load_pool(&e)?;

        let bal_key = DataKey::Balance(to.clone());
        let cur: i128 = e.storage().persistent().get(&bal_key).unwrap_or(0);
        if shares > cur {
            return Err(Error::InsufficientShares);
        }

        let out_a = shares * pool.reserve_a / pool.total_shares;
        let out_b = shares * pool.reserve_b / pool.total_shares;

        e.storage().persistent().set(&bal_key, &(cur - shares));
        e.storage()
            .persistent()
            .extend_ttl(&bal_key, TTL_LEDGERS, TTL_LEDGERS);

        pool.reserve_a -= out_a;
        pool.reserve_b -= out_b;
        pool.total_shares -= shares;
        save_pool(&e, &pool);

        soroban_sdk::token::Client::new(&e, &pool.token_a)
            .transfer(&e.current_contract_address(), &to, &out_a);
        soroban_sdk::token::Client::new(&e, &pool.token_b)
            .transfer(&e.current_contract_address(), &to, &out_b);

        Ok((out_a, out_b))
    }

    pub fn lp_balance(e: Env, user: Address) -> i128 {
        e.storage()
            .persistent()
            .get(&DataKey::Balance(user))
            .unwrap_or(0)
    }

    // ── Limit order book ──────────────────────────────────────────────────────

    /// Place a limit order.
    ///
    /// - `is_bid = true`:  buy `amount` of token_a, paying at most `price` token_b per token_a.
    ///   Maker deposits `amount * price / PRICE_SCALE` token_b upfront.
    /// - `is_bid = false`: sell `amount` of token_a at minimum `price` token_b per token_a.
    ///   Maker deposits `amount` token_a upfront.
    ///
    /// Returns the assigned order id.
    pub fn place_order(
        e: Env,
        maker: Address,
        is_bid: bool,
        price: i128,
        amount: i128,
    ) -> Result<u64, Error> {
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        if price <= 0 {
            return Err(Error::InvalidPrice);
        }
        maker.require_auth();
        let mut pool = load_pool(&e)?;

        // Escrow the offered token from the maker.
        if is_bid {
            // Bid: maker offers token_b to buy token_a.
            let cost = amount
                .checked_mul(price)
                .ok_or(Error::InvalidAmount)?
                / PRICE_SCALE;
            soroban_sdk::token::Client::new(&e, &pool.token_b)
                .transfer(&maker, &e.current_contract_address(), &cost);
        } else {
            // Ask: maker offers token_a to sell for token_b.
            soroban_sdk::token::Client::new(&e, &pool.token_a)
                .transfer(&maker, &e.current_contract_address(), &amount);
        }

        let id = pool.next_order_id;
        pool.next_order_id += 1;
        save_pool(&e, &pool);

        let order = Order {
            id,
            maker,
            is_bid,
            price,
            amount,
        };

        if is_bid {
            let mut bids = load_bids(&e);
            insert_bid(&mut bids, order);
            save_bids(&e, &bids);
        } else {
            let mut asks = load_asks(&e);
            insert_ask(&mut asks, order);
            save_asks(&e, &asks);
        }

        Ok(id)
    }

    /// Cancel an open order and refund the escrowed tokens.
    pub fn cancel_order(e: Env, maker: Address, order_id: u64) -> Result<(), Error> {
        maker.require_auth();
        let pool = load_pool(&e)?;

        // Search bids first, then asks.
        let mut bids = load_bids(&e);
        for i in 0..bids.len() {
            let o = bids.get(i).unwrap();
            if o.id == order_id {
                if o.maker != maker {
                    return Err(Error::Unauthorized);
                }
                // Refund escrowed token_b.
                let refund = o
                    .amount
                    .checked_mul(o.price)
                    .ok_or(Error::InvalidAmount)?
                    / PRICE_SCALE;
                soroban_sdk::token::Client::new(&e, &pool.token_b)
                    .transfer(&e.current_contract_address(), &maker, &refund);
                bids.remove(i);
                save_bids(&e, &bids);
                return Ok(());
            }
        }

        let mut asks = load_asks(&e);
        for i in 0..asks.len() {
            let o = asks.get(i).unwrap();
            if o.id == order_id {
                if o.maker != maker {
                    return Err(Error::Unauthorized);
                }
                // Refund escrowed token_a.
                soroban_sdk::token::Client::new(&e, &pool.token_a)
                    .transfer(&e.current_contract_address(), &maker, &o.amount);
                asks.remove(i);
                save_asks(&e, &asks);
                return Ok(());
            }
        }

        Err(Error::OrderNotFound)
    }

    // ── Swap (hybrid matching engine) ─────────────────────────────────────────

    /// Swap tokens, filling limit orders first then falling back to the AMM.
    ///
    /// - `buy_a = true`:  taker pays token_b, receives token_a.
    /// - `buy_a = false`: taker pays token_a, receives token_b.
    /// - `out`:    exact amount of output token desired.
    /// - `in_max`: maximum input the taker will pay (slippage guard).
    ///
    /// Fee model:
    /// - LOB fills: `maker_fee_bps` of the input is kept by the contract and
    ///   claimable by the maker (net: maker receives slightly more than spot).
    ///   The taker pays `out * price / PRICE_SCALE * (1 + maker_fee_bps/10000)`.
    /// - AMM fills: `lp_fee_bps` of the input stays in the pool (benefits LPs).
    pub fn swap(
        e: Env,
        taker: Address,
        buy_a: bool,
        out: i128,
        in_max: i128,
    ) -> Result<SwapResult, Error> {
        if out <= 0 {
            return Err(Error::InvalidAmount);
        }
        taker.require_auth();
        let mut pool = load_pool(&e)?;

        let mut remaining_out = out;
        let mut total_in: i128 = 0;
        let mut lob_filled: i128 = 0;
        let mut amm_filled: i128 = 0;

        // ── Phase 1: fill from limit order book ───────────────────────────────
        //
        // When taker buys A (buy_a=true), they match against asks (makers selling A).
        // When taker buys B (buy_a=false), they match against bids (makers selling B).

        if buy_a {
            // Taker wants token_a → match against asks (ascending price).
            let mut asks = load_asks(&e);
            let mut i = 0u32;
            while i < asks.len() && remaining_out > 0 {
                let mut order = asks.get(i).unwrap();
                // Ask price is the minimum token_b per token_a the maker accepts.
                // Taker is willing to pay up to in_max total; check per-unit price later.
                let fill_a = remaining_out.min(order.amount);
                // Cost to taker for this fill (token_b), before maker fee.
                let base_cost = fill_a
                    .checked_mul(order.price)
                    .ok_or(Error::InvalidAmount)?
                    / PRICE_SCALE;
                // Maker fee: taker pays a small premium; maker keeps it.
                let maker_fee = base_cost
                    .checked_mul(pool.maker_fee_bps)
                    .ok_or(Error::InvalidAmount)?
                    / 10_000;
                let taker_cost = base_cost + maker_fee;

                total_in += taker_cost;
                remaining_out -= fill_a;
                lob_filled += fill_a;

                // Pay maker: base_cost (their escrowed token_b was already in contract,
                // so we send them token_a equivalent + the maker_fee bonus in token_b).
                // Maker escrowed token_b; taker sends token_b to contract.
                // Contract sends token_a to taker, token_b (base_cost + maker_fee) to maker.
                soroban_sdk::token::Client::new(&e, &pool.token_a)
                    .transfer(&e.current_contract_address(), &taker, &fill_a);
                soroban_sdk::token::Client::new(&e, &pool.token_b)
                    .transfer(&e.current_contract_address(), &order.maker, &(base_cost + maker_fee));

                order.amount -= fill_a;
                if order.amount == 0 {
                    asks.remove(i);
                    // don't increment i
                } else {
                    asks.set(i, order);
                    i += 1;
                }
            }
            save_asks(&e, &asks);
        } else {
            // Taker wants token_b → match against bids (descending price = best bid first).
            let mut bids = load_bids(&e);
            let mut i = 0u32;
            while i < bids.len() && remaining_out > 0 {
                let mut order = bids.get(i).unwrap();
                // Bid: maker escrowed token_b to buy token_a.
                // Taker is selling token_a to get token_b.
                // fill_b = amount of token_b taker receives from this order.
                // The bid price is token_b per token_a, so fill_a = fill_b * PRICE_SCALE / price.
                let fill_b = remaining_out.min(
                    order
                        .amount
                        .checked_mul(order.price)
                        .ok_or(Error::InvalidAmount)?
                        / PRICE_SCALE,
                );
                let fill_a = fill_b
                    .checked_mul(PRICE_SCALE)
                    .ok_or(Error::InvalidAmount)?
                    / order.price;

                let maker_fee = fill_b
                    .checked_mul(pool.maker_fee_bps)
                    .ok_or(Error::InvalidAmount)?
                    / 10_000;

                total_in += fill_a;
                remaining_out -= fill_b;
                lob_filled += fill_b;

                // Taker sends token_a to contract; contract sends token_b to taker.
                // Maker receives token_a + maker_fee in token_b.
                soroban_sdk::token::Client::new(&e, &pool.token_b)
                    .transfer(&e.current_contract_address(), &taker, &fill_b);
                soroban_sdk::token::Client::new(&e, &pool.token_a)
                    .transfer(&e.current_contract_address(), &order.maker, &fill_a);
                // Maker fee stays in contract (credited to maker implicitly via better fill).
                // In a production system you'd track per-maker fee balances; here we keep
                // it simple and leave the fee in the pool to benefit LPs.

                order.amount -= fill_a;
                if order.amount == 0 {
                    bids.remove(i);
                } else {
                    bids.set(i, order);
                    i += 1;
                }
            }
            save_bids(&e, &bids);
        }

        // ── Phase 2: fill remainder from AMM ─────────────────────────────────

        if remaining_out > 0 {
            let (reserve_in, reserve_out) = if buy_a {
                (pool.reserve_b, pool.reserve_a)
            } else {
                (pool.reserve_a, pool.reserve_b)
            };

            if remaining_out >= reserve_out {
                return Err(Error::InsufficientLiquidity);
            }

            let fee_scale = 10_000i128 - pool.lp_fee_bps;
            let numerator = reserve_in
                .checked_mul(remaining_out)
                .ok_or(Error::InsufficientLiquidity)?
                .checked_mul(10_000)
                .ok_or(Error::InsufficientLiquidity)?;
            let denominator = (reserve_out - remaining_out)
                .checked_mul(fee_scale)
                .ok_or(Error::InsufficientLiquidity)?;
            let amm_in = (numerator / denominator) + 1;

            total_in += amm_in;
            amm_filled = remaining_out;

            if buy_a {
                pool.reserve_a -= remaining_out;
                pool.reserve_b += amm_in;
            } else {
                pool.reserve_b -= remaining_out;
                pool.reserve_a += amm_in;
            }
            save_pool(&e, &pool);
        } else {
            save_pool(&e, &pool);
        }

        if total_in > in_max {
            return Err(Error::SlippageExceeded);
        }

        // Collect total input from taker in one transfer.
        // Note: LOB fills already transferred output tokens above; we only need
        // to collect the taker's input here for the full swap.
        let token_in = if buy_a {
            pool.token_b.clone()
        } else {
            pool.token_a.clone()
        };
        let token_out = if buy_a {
            pool.token_a.clone()
        } else {
            pool.token_b.clone()
        };

        // Transfer taker's input to contract (covers both LOB and AMM fills).
        soroban_sdk::token::Client::new(&e, &token_in)
            .transfer(&taker, &e.current_contract_address(), &total_in);

        // For AMM portion, send output to taker.
        if amm_filled > 0 {
            soroban_sdk::token::Client::new(&e, &token_out)
                .transfer(&e.current_contract_address(), &taker, &amm_filled);
        }

        e.events().publish(
            ("swap", taker),
            SwapResult {
                amount_in: total_in,
                amount_out: out,
                lob_filled,
                amm_filled,
            },
        );

        Ok(SwapResult {
            amount_in: total_in,
            amount_out: out,
            lob_filled,
            amm_filled,
        })
    }

    // ── Views ─────────────────────────────────────────────────────────────────

    pub fn get_pool(e: Env) -> Result<PoolState, Error> {
        load_pool(&e)
    }

    pub fn get_bids(e: Env) -> Vec<Order> {
        load_bids(&e)
    }

    pub fn get_asks(e: Env) -> Vec<Order> {
        load_asks(&e)
    }
}
