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
    /// The AMM leg would move the pool's spot price further than
    /// `max_price_deviation_bps`.
    PriceDeviationExceeded = 10,
    /// A guard parameter was outside its permitted range.
    InvalidConfig = 11,
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

/// Matching-engine safeguards (issue #641).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Guards {
    /// Maximum number of limit orders that may be consumed per ledger, summed
    /// across every swap in that ledger. Bounds worst-case execution cost and
    /// stops a single block from sweeping the book through thin ticks.
    pub max_match_depth: u32,
    /// Maximum tolerated spot-price movement, in basis points. Applied both to
    /// the pool price move caused by the AMM leg and to how far a limit order
    /// may be priced away from the pool before it stops being matchable.
    pub max_price_deviation_bps: i128,
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
    /// Matching-engine safeguards.
    pub guards: Guards,
    pub admin: Address,
    pub next_order_id: u64,
}

/// Per-ledger execution-depth accounting for the matching engine.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchDepth {
    /// Ledger sequence the `consumed` count applies to.
    pub ledger: u32,
    /// Limit orders already matched during that ledger.
    pub consumed: u32,
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
    /// Execution depth consumed during the current ledger.
    MatchDepth,
}

// ── Constants ─────────────────────────────────────────────────────────────────

/// Price scale factor: prices are integers representing (token_b / token_a) * PRICE_SCALE.
pub const PRICE_SCALE: i128 = 1_000_000;
pub const DEFAULT_LP_FEE_BPS: i128 = 30;
pub const DEFAULT_MAKER_FEE_BPS: i128 = 10;
pub const TTL_LEDGERS: u32 = 17_280; // ~1 day

/// Basis-point denominator.
pub const BPS_DENOMINATOR: i128 = 10_000;
/// Suggested execution-depth budget for a single ledger.
pub const DEFAULT_MAX_MATCH_DEPTH: u32 = 8;
/// Hard ceiling on `max_match_depth`, so no configuration can make a swap
/// iterate an unbounded number of orders.
pub const MAX_MATCH_DEPTH_LIMIT: u32 = 64;
/// Suggested price-deviation tolerance (5%).
pub const DEFAULT_MAX_PRICE_DEVIATION_BPS: i128 = 500;
/// How long the per-ledger depth counter is kept alive. It only has to outlive
/// the ledger it describes.
pub const MATCH_DEPTH_TTL_LEDGERS: u32 = 64;

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

/// Reject guard settings that would defeat their own purpose: a zero depth
/// budget makes the book unmatchable, an oversized one makes swap cost
/// unbounded, and a deviation tolerance of 0 or >100% is meaningless.
fn validate_guards(guards: &Guards) -> Result<(), Error> {
    if guards.max_match_depth == 0 || guards.max_match_depth > MAX_MATCH_DEPTH_LIMIT {
        return Err(Error::InvalidConfig);
    }
    if guards.max_price_deviation_bps <= 0 || guards.max_price_deviation_bps > BPS_DENOMINATOR {
        return Err(Error::InvalidConfig);
    }
    Ok(())
}

/// Pool spot price (token_b per token_a, scaled by `PRICE_SCALE`).
///
/// Returns `None` when the pool holds no liquidity, or when reserves are so
/// lopsided the scaled price truncates to zero. In both cases there is no
/// usable reference price, so deviation checks are skipped rather than
/// measured against a meaningless number.
fn spot_price(pool: &PoolState) -> Option<i128> {
    if pool.reserve_a <= 0 || pool.reserve_b <= 0 {
        return None;
    }
    let scaled = pool.reserve_b.checked_mul(PRICE_SCALE)?;
    let price = scaled / pool.reserve_a;
    if price <= 0 {
        None
    } else {
        Some(price)
    }
}

/// Absolute distance of `price` from `reference`, in basis points.
/// `reference` is always a positive value produced by [`spot_price`].
fn deviation_bps(price: i128, reference: i128) -> Result<i128, Error> {
    let diff = (price - reference).abs();
    Ok(diff
        .checked_mul(BPS_DENOMINATOR)
        .ok_or(Error::InvalidPrice)?
        / reference)
}

/// Depth already consumed during the current ledger. A counter left over from
/// an earlier ledger reads as zero.
fn load_depth_used(e: &Env) -> u32 {
    match e
        .storage()
        .temporary()
        .get::<DataKey, MatchDepth>(&DataKey::MatchDepth)
    {
        Some(d) if d.ledger == e.ledger().sequence() => d.consumed,
        _ => 0,
    }
}

fn save_depth_used(e: &Env, consumed: u32) {
    e.storage().temporary().set(
        &DataKey::MatchDepth,
        &MatchDepth {
            ledger: e.ledger().sequence(),
            consumed,
        },
    );
    e.storage().temporary().extend_ttl(
        &DataKey::MatchDepth,
        MATCH_DEPTH_TTL_LEDGERS,
        MATCH_DEPTH_TTL_LEDGERS,
    );
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
        guards: Guards,
    ) -> Result<(), Error> {
        if e.storage().instance().has(&DataKey::Pool) {
            return Err(Error::AlreadyInitialized);
        }
        if lp_fee_bps < 0 || maker_fee_bps < 0 {
            return Err(Error::InvalidAmount);
        }
        validate_guards(&guards)?;
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
                guards,
                admin,
                next_order_id: 1,
            },
        );
        Ok(())
    }

    /// Update the matching-engine safeguards. Admin only.
    pub fn set_guards(e: Env, guards: Guards) -> Result<(), Error> {
        let mut pool = load_pool(&e)?;
        pool.admin.require_auth();
        validate_guards(&guards)?;
        pool.guards = guards;
        save_pool(&e, &pool);
        Ok(())
    }

    pub fn get_guards(e: Env) -> Result<Guards, Error> {
        Ok(load_pool(&e)?.guards)
    }

    // ── Liquidity ─────────────────────────────────────────────────────────────

    /// Deposit token_a and token_b, receive LP shares.
    pub fn deposit(e: Env, to: Address, amount_a: i128, amount_b: i128) -> Result<i128, Error> {
        if amount_a <= 0 || amount_b <= 0 {
            return Err(Error::InvalidAmount);
        }
        to.require_auth();
        let mut pool = load_pool(&e)?;

        soroban_sdk::token::Client::new(&e, &pool.token_a).transfer(
            &to,
            &e.current_contract_address(),
            &amount_a,
        );
        soroban_sdk::token::Client::new(&e, &pool.token_b).transfer(
            &to,
            &e.current_contract_address(),
            &amount_b,
        );

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

        soroban_sdk::token::Client::new(&e, &pool.token_a).transfer(
            &e.current_contract_address(),
            &to,
            &out_a,
        );
        soroban_sdk::token::Client::new(&e, &pool.token_b).transfer(
            &e.current_contract_address(),
            &to,
            &out_b,
        );

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
            let cost = amount.checked_mul(price).ok_or(Error::InvalidAmount)? / PRICE_SCALE;
            soroban_sdk::token::Client::new(&e, &pool.token_b).transfer(
                &maker,
                &e.current_contract_address(),
                &cost,
            );
        } else {
            // Ask: maker offers token_a to sell for token_b.
            soroban_sdk::token::Client::new(&e, &pool.token_a).transfer(
                &maker,
                &e.current_contract_address(),
                &amount,
            );
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
                let refund =
                    o.amount.checked_mul(o.price).ok_or(Error::InvalidAmount)? / PRICE_SCALE;
                soroban_sdk::token::Client::new(&e, &pool.token_b).transfer(
                    &e.current_contract_address(),
                    &maker,
                    &refund,
                );
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
                soroban_sdk::token::Client::new(&e, &pool.token_a).transfer(
                    &e.current_contract_address(),
                    &maker,
                    &o.amount,
                );
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
    /// - LOB fills: the taker pays the maker a `maker_fee_bps` premium on top
    ///   of the order's price, settled directly between the two parties.
    /// - AMM fills: `lp_fee_bps` of the input stays in the pool (benefits LPs).
    ///
    /// Safeguards (see issue #641):
    /// - **Execution depth.** At most `max_match_depth` limit orders may be
    ///   consumed per ledger, counted across every swap in that ledger. Once the
    ///   budget is spent the book stops being consumed and the AMM prices the
    ///   remainder, so no single block can cascade through thin ticks. Matching
    ///   stops rather than reverting, so an exhausted budget never makes the
    ///   pool unswappable for the rest of the ledger.
    /// - **Price band.** Limit orders priced worse than the pool's spot price
    ///   by more than `max_price_deviation_bps` are not matched; the AMM prices
    ///   that remainder instead.
    /// - **Pool deviation.** If the AMM leg would move the pool's spot price by
    ///   more than `max_price_deviation_bps`, the swap reverts with
    ///   [`Error::PriceDeviationExceeded`].
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
        let mut amm_in: i128 = 0;

        // Pool spot price captured before anything moves. It anchors both the
        // limit-order price band and the post-swap pool deviation check.
        // `None` means the pool is empty, so there is nothing to deviate from
        // and the book is the only venue.
        let reference_price = spot_price(&pool);

        // Execution-depth budget left in this ledger.
        let depth_used = load_depth_used(&e);
        let depth_budget = pool.guards.max_match_depth.saturating_sub(depth_used);
        let mut matched: u32 = 0;

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

                // Price band: asks are sorted ascending, so the first order
                // priced above tolerance means every later one is worse. Stop
                // and let the AMM price the remainder.
                if let Some(reference) = reference_price {
                    if order.price > reference
                        && deviation_bps(order.price, reference)?
                            > pool.guards.max_price_deviation_bps
                    {
                        break;
                    }
                }

                // Execution depth: a matchable order is sitting right here but
                // this ledger's budget is spent. Stop consuming the book and let
                // the AMM price the remainder, where the deviation guard applies.
                if matched >= depth_budget {
                    break;
                }

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
                    / BPS_DENOMINATOR;
                let taker_cost = base_cost + maker_fee;

                total_in += taker_cost;
                remaining_out -= fill_a;
                lob_filled += fill_a;
                matched += 1;

                // Settle maker-to-taker directly: the taker's token_b never has
                // to sit in the contract first, and the contract only releases
                // the token_a this maker escrowed when placing the order.
                soroban_sdk::token::Client::new(&e, &pool.token_b).transfer(
                    &taker,
                    &order.maker,
                    &taker_cost,
                );
                soroban_sdk::token::Client::new(&e, &pool.token_a).transfer(
                    &e.current_contract_address(),
                    &taker,
                    &fill_a,
                );

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

                // Price band: bids are sorted descending, so the first order
                // priced below tolerance means every later one is worse.
                if let Some(reference) = reference_price {
                    if order.price < reference
                        && deviation_bps(order.price, reference)?
                            > pool.guards.max_price_deviation_bps
                    {
                        break;
                    }
                }

                // Execution depth: budget for this ledger is spent. Stop here;
                // the AMM prices the remainder under the deviation guard.
                if matched >= depth_budget {
                    break;
                }

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

                // Maker fee is a premium on the taker's input, mirroring the
                // ask side, so the taker still receives exactly `fill_b`.
                let maker_fee = fill_a
                    .checked_mul(pool.maker_fee_bps)
                    .ok_or(Error::InvalidAmount)?
                    / BPS_DENOMINATOR;
                let taker_cost = fill_a + maker_fee;

                total_in += taker_cost;
                remaining_out -= fill_b;
                lob_filled += fill_b;
                matched += 1;

                // Settle maker-to-taker directly; the contract only releases the
                // token_b this maker escrowed when placing the order.
                soroban_sdk::token::Client::new(&e, &pool.token_a).transfer(
                    &taker,
                    &order.maker,
                    &taker_cost,
                );
                soroban_sdk::token::Client::new(&e, &pool.token_b).transfer(
                    &e.current_contract_address(),
                    &taker,
                    &fill_b,
                );

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

        if matched > 0 {
            save_depth_used(&e, depth_used + matched);
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

            let fee_scale = BPS_DENOMINATOR - pool.lp_fee_bps;
            let numerator = reserve_in
                .checked_mul(remaining_out)
                .ok_or(Error::InsufficientLiquidity)?
                .checked_mul(BPS_DENOMINATOR)
                .ok_or(Error::InsufficientLiquidity)?;
            let denominator = (reserve_out - remaining_out)
                .checked_mul(fee_scale)
                .ok_or(Error::InsufficientLiquidity)?;
            amm_in = (numerator / denominator) + 1;

            total_in += amm_in;
            amm_filled = remaining_out;

            if buy_a {
                pool.reserve_a -= remaining_out;
                pool.reserve_b += amm_in;
            } else {
                pool.reserve_b -= remaining_out;
                pool.reserve_a += amm_in;
            }

            // Pool deviation guard: reject a swap that drags the pool's spot
            // price outside tolerance. This is the safeguard against draining a
            // thin pool to manufacture a price.
            if let Some(reference) = reference_price {
                let post_price = spot_price(&pool).ok_or(Error::InsufficientLiquidity)?;
                if deviation_bps(post_price, reference)? > pool.guards.max_price_deviation_bps {
                    return Err(Error::PriceDeviationExceeded);
                }
            }

            save_pool(&e, &pool);
        } else {
            save_pool(&e, &pool);
        }

        if total_in > in_max {
            return Err(Error::SlippageExceeded);
        }

        // Settle the AMM leg. LOB fills were already settled maker-to-taker
        // inside phase 1, so only the pool's share moves here.
        if amm_filled > 0 {
            let token_in = if buy_a { &pool.token_b } else { &pool.token_a };
            let token_out = if buy_a { &pool.token_a } else { &pool.token_b };

            soroban_sdk::token::Client::new(&e, token_in).transfer(
                &taker,
                &e.current_contract_address(),
                &amm_in,
            );
            soroban_sdk::token::Client::new(&e, token_out).transfer(
                &e.current_contract_address(),
                &taker,
                &amm_filled,
            );
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

    /// Limit orders already matched during the current ledger.
    pub fn get_match_depth_used(e: Env) -> u32 {
        load_depth_used(&e)
    }

    /// Execution depth still available in the current ledger.
    pub fn get_match_depth_remaining(e: Env) -> Result<u32, Error> {
        let pool = load_pool(&e)?;
        Ok(pool
            .guards
            .max_match_depth
            .saturating_sub(load_depth_used(&e)))
    }

    /// Pool spot price (token_b per token_a, scaled by `PRICE_SCALE`), or 0
    /// when the pool holds no liquidity.
    pub fn get_spot_price(e: Env) -> Result<i128, Error> {
        Ok(spot_price(&load_pool(&e)?).unwrap_or(0))
    }
}
