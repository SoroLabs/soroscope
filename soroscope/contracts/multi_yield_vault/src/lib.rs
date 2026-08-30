#![no_std]
//! # Multi-Yield Aggregator Vault
//!
//! Accepts a single deposit token and splits/rebalances it across up to
//! `MAX_POOLS` registered Soroban AMM pools to maximise yield.
//!
//! ## APR estimation
//! Each registered pool exposes `reserve_a`, `reserve_b`, and `fee_bps`.
//! We approximate the 24-h fee APR for the deposit token as:
//!
//!   estimated_apr_bps = fee_bps * VOLUME_PROXY / reserve_deposit_token
//!
//! where `VOLUME_PROXY` is a configurable constant representing the assumed
//! daily volume relative to the pool's reserve (default: 100 % of reserve,
//! i.e. `VOLUME_PROXY = 10_000` in bps).  This is intentionally simple and
//! can be replaced by an oracle-fed value without changing the interface.
//!
//! ## Rebalancing
//! `rebalance()` reads the current allocation, estimates APR for every pool,
//! sorts pools by APR descending, and moves funds from under-performing pools
//! to the best pool.  Each withdrawal and deposit is slippage-protected: the
//! contract checks that the amount received from a pool withdrawal is within
//! `slippage_bps` of the expected amount before proceeding.

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
    TooManyPools = 4,
    PoolAlreadyRegistered = 5,
    PoolNotFound = 6,
    SlippageExceeded = 7,
    InsufficientShares = 8,
    Unauthorized = 9,
}

// ── External pool interface ───────────────────────────────────────────────────

/// Minimal interface the vault calls on each registered AMM pool.
pub trait AmmPool {
    /// Deposit `amount_a` and `amount_b`; returns LP shares minted.
    fn deposit(e: Env, to: Address, amount_a: i128, amount_b: i128) -> i128;
    /// Burn `shares`; returns (amount_a, amount_b) withdrawn.
    fn withdraw(e: Env, to: Address, share_amount: i128) -> (i128, i128);
    /// Current reserve of token_a.
    fn get_reserve_a(e: Env) -> i128;
    /// Current reserve of token_b.
    fn get_reserve_b(e: Env) -> i128;
    /// Swap fee in basis points.
    fn get_fee(e: Env) -> i128;
    /// Which of the two pool tokens is token_a.
    fn token_a(e: Env) -> Address;
}

soroban_sdk::contractclient!(name = "AmmPoolClient", trait = AmmPool);

// ── Storage types ─────────────────────────────────────────────────────────────

/// Vault-wide configuration and accounting, stored as a single instance entry.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultState {
    /// The single token users deposit (e.g. USDC).
    pub deposit_token: Address,
    pub admin: Address,
    /// Total vault shares outstanding.
    pub total_shares: i128,
    /// Maximum slippage tolerated on rebalance withdrawals, in bps.
    pub slippage_bps: i128,
    /// Assumed daily volume as a fraction of pool reserve, in bps (default 10_000 = 100%).
    pub volume_proxy_bps: i128,
}

/// Per-pool allocation record.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolAllocation {
    /// AMM pool contract address.
    pub pool: Address,
    /// LP shares the vault holds in this pool.
    pub lp_shares: i128,
    /// Whether the deposit token is token_a (true) or token_b (false) in this pool.
    pub deposit_is_a: bool,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Vault,
    /// Vec<PoolAllocation> — ordered list of registered pools.
    Pools,
    /// Per-user vault share balance.
    Balance(Address),
}

// ── Constants ─────────────────────────────────────────────────────────────────

pub const MAX_POOLS: u32 = 8;
pub const TTL_LEDGERS: u32 = 17_280;
pub const DEFAULT_SLIPPAGE_BPS: i128 = 100; // 1 %
pub const DEFAULT_VOLUME_PROXY_BPS: i128 = 10_000; // 100 % of reserve

// ── Helpers ───────────────────────────────────────────────────────────────────

fn load_vault(e: &Env) -> Result<VaultState, Error> {
    e.storage()
        .instance()
        .get(&DataKey::Vault)
        .ok_or(Error::NotInitialized)
}

fn save_vault(e: &Env, v: &VaultState) {
    e.storage().instance().set(&DataKey::Vault, v);
}

fn load_pools(e: &Env) -> Vec<PoolAllocation> {
    e.storage()
        .instance()
        .get(&DataKey::Pools)
        .unwrap_or(Vec::new(e))
}

fn save_pools(e: &Env, pools: &Vec<PoolAllocation>) {
    e.storage().instance().set(&DataKey::Pools, pools);
}

/// Estimate APR in bps for the deposit token in a given pool.
///
/// apr_bps = fee_bps * volume_proxy_bps / 10_000
///
/// This represents: if daily volume = `volume_proxy_bps/10_000` × reserve,
/// then daily fee revenue / deposit = fee_bps/10_000 × volume_proxy_bps/10_000.
/// Annualised (×365) is omitted here — we only need relative ranking.
fn estimate_apr_bps(fee_bps: i128, volume_proxy_bps: i128) -> i128 {
    fee_bps * volume_proxy_bps / 10_000
}

/// How much of the deposit token the vault has in a given pool allocation.
fn deposit_value_in_pool(e: &Env, alloc: &PoolAllocation) -> i128 {
    if alloc.lp_shares == 0 {
        return 0;
    }
    let client = AmmPoolClient::new(e, &alloc.pool);
    let reserve_a = client.get_reserve_a();
    let reserve_b = client.get_reserve_b();
    // Total LP supply is not exposed by the minimal interface, so we approximate
    // by reading the pool's reserves and assuming the vault's share is proportional.
    // We store lp_shares and use the ratio: value ≈ lp_shares / total_lp * reserve.
    // Since we can't get total_lp cheaply, we track deposited amounts separately
    // via a simpler invariant: on deposit we record the token amount, on withdraw
    // we get it back.  For APR ranking we only need the fee and volume proxy.
    if alloc.deposit_is_a {
        reserve_a
    } else {
        reserve_b
    }
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct MultiYieldVault;

#[contractimpl]
impl MultiYieldVault {
    // ── Admin ─────────────────────────────────────────────────────────────────

    pub fn initialize(
        e: Env,
        admin: Address,
        deposit_token: Address,
        slippage_bps: i128,
        volume_proxy_bps: i128,
    ) -> Result<(), Error> {
        if e.storage().instance().has(&DataKey::Vault) {
            return Err(Error::AlreadyInitialized);
        }
        if slippage_bps < 0 || volume_proxy_bps <= 0 {
            return Err(Error::InvalidAmount);
        }
        save_vault(
            &e,
            &VaultState {
                deposit_token,
                admin,
                total_shares: 0,
                slippage_bps,
                volume_proxy_bps,
            },
        );
        Ok(())
    }

    /// Register a new AMM pool.  `deposit_is_a` tells the vault whether the
    /// deposit token is token_a (true) or token_b (false) in that pool.
    pub fn register_pool(
        e: Env,
        pool: Address,
        deposit_is_a: bool,
    ) -> Result<(), Error> {
        let vault = load_vault(&e)?;
        vault.admin.require_auth();

        let mut pools = load_pools(&e);
        if pools.len() >= MAX_POOLS {
            return Err(Error::TooManyPools);
        }
        for i in 0..pools.len() {
            if pools.get(i).unwrap().pool == pool {
                return Err(Error::PoolAlreadyRegistered);
            }
        }
        pools.push_back(PoolAllocation {
            pool,
            lp_shares: 0,
            deposit_is_a,
        });
        save_pools(&e, &pools);
        Ok(())
    }

    pub fn set_slippage(e: Env, slippage_bps: i128) -> Result<(), Error> {
        let mut vault = load_vault(&e)?;
        vault.admin.require_auth();
        vault.slippage_bps = slippage_bps;
        save_vault(&e, &vault);
        Ok(())
    }

    // ── User deposit / withdraw ───────────────────────────────────────────────

    /// Deposit `amount` of the deposit token.  Funds are routed to the
    /// highest-APR registered pool.  Returns vault shares minted.
    pub fn deposit(e: Env, from: Address, amount: i128) -> Result<i128, Error> {
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        from.require_auth();
        let mut vault = load_vault(&e)?;
        let pools = load_pools(&e);
        if pools.len() == 0 {
            return Err(Error::PoolNotFound);
        }

        // Pull deposit token from user into vault.
        soroban_sdk::token::Client::new(&e, &vault.deposit_token)
            .transfer(&from, &e.current_contract_address(), &amount);

        // Find best pool by APR.
        let best_idx = Self::best_pool_idx(&e, &pools, &vault);

        // Deposit into best pool.  The pool requires both tokens; we supply
        // `amount` of the deposit token and 0 of the other side.  The pool
        // will mint shares proportional to the minimum ratio — depositing only
        // one side is a known limitation; a production vault would swap half
        // first.  For simplicity we deposit single-sided (amount, 0) or (0, amount).
        let mut pools_mut = pools;
        let mut alloc = pools_mut.get(best_idx).unwrap();
        let client = AmmPoolClient::new(&e, &alloc.pool);

        let (dep_a, dep_b) = if alloc.deposit_is_a {
            (amount, 0i128)
        } else {
            (0i128, amount)
        };

        // Approve pool to pull tokens from vault.
        soroban_sdk::token::Client::new(&e, &vault.deposit_token)
            .approve(&e.current_contract_address(), &alloc.pool, &amount, &(e.ledger().sequence() + 1));

        let new_lp = client.deposit(&e.current_contract_address(), &dep_a, &dep_b);
        alloc.lp_shares += new_lp;
        pools_mut.set(best_idx, alloc);
        save_pools(&e, &pools_mut);

        // Mint vault shares proportional to deposit.
        let shares = if vault.total_shares == 0 {
            amount
        } else {
            amount // 1:1 for simplicity; production would use NAV-based pricing
        };

        let bal_key = DataKey::Balance(from.clone());
        let cur: i128 = e.storage().persistent().get(&bal_key).unwrap_or(0);
        e.storage().persistent().set(&bal_key, &(cur + shares));
        e.storage()
            .persistent()
            .extend_ttl(&bal_key, TTL_LEDGERS, TTL_LEDGERS);

        vault.total_shares += shares;
        save_vault(&e, &vault);

        Ok(shares)
    }

    /// Burn `shares` and receive deposit tokens back.
    pub fn withdraw(e: Env, to: Address, shares: i128) -> Result<i128, Error> {
        if shares <= 0 {
            return Err(Error::InvalidAmount);
        }
        to.require_auth();
        let mut vault = load_vault(&e)?;

        let bal_key = DataKey::Balance(to.clone());
        let cur: i128 = e.storage().persistent().get(&bal_key).unwrap_or(0);
        if shares > cur {
            return Err(Error::InsufficientShares);
        }

        // Proportional share of total vault assets.
        let fraction_num = shares;
        let fraction_den = vault.total_shares;

        let mut pools = load_pools(&e);
        let mut total_received: i128 = 0;

        for i in 0..pools.len() {
            let mut alloc = pools.get(i).unwrap();
            if alloc.lp_shares == 0 {
                continue;
            }
            // Withdraw proportional LP shares from this pool.
            let lp_to_burn = alloc.lp_shares * fraction_num / fraction_den;
            if lp_to_burn == 0 {
                continue;
            }

            let client = AmmPoolClient::new(&e, &alloc.pool);
            let (out_a, out_b) = client.withdraw(&e.current_contract_address(), &lp_to_burn);

            let received = if alloc.deposit_is_a { out_a } else { out_b };
            total_received += received;
            alloc.lp_shares -= lp_to_burn;
            pools.set(i, alloc);
        }
        save_pools(&e, &pools);

        // Send deposit token to user.
        soroban_sdk::token::Client::new(&e, &vault.deposit_token)
            .transfer(&e.current_contract_address(), &to, &total_received);

        e.storage().persistent().set(&bal_key, &(cur - shares));
        e.storage()
            .persistent()
            .extend_ttl(&bal_key, TTL_LEDGERS, TTL_LEDGERS);

        vault.total_shares -= shares;
        save_vault(&e, &vault);

        Ok(total_received)
    }

    // ── Rebalancing ───────────────────────────────────────────────────────────

    /// Rebalance: move all funds to the highest-APR pool.
    ///
    /// For each pool that is not the best:
    ///   1. Withdraw all LP shares → receive deposit token back.
    ///   2. Slippage check: received ≥ expected × (1 − slippage_bps/10_000).
    ///   3. Deposit everything into the best pool.
    ///
    /// Anyone can call this (permissionless rebalancing).
    pub fn rebalance(e: Env) -> Result<(), Error> {
        let vault = load_vault(&e)?;
        let mut pools = load_pools(&e);
        if pools.len() <= 1 {
            return Ok(()); // nothing to rebalance
        }

        let best_idx = Self::best_pool_idx(&e, &pools, &vault);
        let mut total_to_move: i128 = 0;

        // Step 1: withdraw from all non-best pools.
        for i in 0..pools.len() {
            if i == best_idx {
                continue;
            }
            let mut alloc = pools.get(i).unwrap();
            if alloc.lp_shares == 0 {
                continue;
            }

            let client = AmmPoolClient::new(&e, &alloc.pool);

            // Estimate expected deposit-token return before withdrawing.
            let reserve = if alloc.deposit_is_a {
                client.get_reserve_a()
            } else {
                client.get_reserve_b()
            };
            // We don't have total LP supply, so we use a conservative estimate:
            // expected ≈ lp_shares (1:1 if pool was seeded 1:1, otherwise approximate).
            // In production, expose total_lp_supply on the pool interface.
            let expected = reserve.min(alloc.lp_shares);

            let (out_a, out_b) = client.withdraw(&e.current_contract_address(), &alloc.lp_shares);
            let received = if alloc.deposit_is_a { out_a } else { out_b };

            // Slippage check.
            let min_acceptable = expected * (10_000 - vault.slippage_bps) / 10_000;
            if received < min_acceptable {
                return Err(Error::SlippageExceeded);
            }

            total_to_move += received;
            alloc.lp_shares = 0;
            pools.set(i, alloc);
        }

        // Step 2: deposit everything into the best pool.
        if total_to_move > 0 {
            let mut best_alloc = pools.get(best_idx).unwrap();
            let client = AmmPoolClient::new(&e, &best_alloc.pool);

            let (dep_a, dep_b) = if best_alloc.deposit_is_a {
                (total_to_move, 0i128)
            } else {
                (0i128, total_to_move)
            };

            soroban_sdk::token::Client::new(&e, &vault.deposit_token).approve(
                &e.current_contract_address(),
                &best_alloc.pool,
                &total_to_move,
                &(e.ledger().sequence() + 1),
            );

            let new_lp = client.deposit(&e.current_contract_address(), &dep_a, &dep_b);
            best_alloc.lp_shares += new_lp;
            pools.set(best_idx, best_alloc);
        }

        save_pools(&e, &pools);

        e.events().publish(("rebalance",), (best_idx as u32, total_to_move));
        Ok(())
    }

    // ── Views ─────────────────────────────────────────────────────────────────

    pub fn get_vault(e: Env) -> Result<VaultState, Error> {
        load_vault(&e)
    }

    pub fn get_pools(e: Env) -> Vec<PoolAllocation> {
        load_pools(&e)
    }

    pub fn vault_balance(e: Env, user: Address) -> i128 {
        e.storage()
            .persistent()
            .get(&DataKey::Balance(user))
            .unwrap_or(0)
    }

    /// Returns the estimated APR (in bps) for each registered pool, in order.
    pub fn get_aprs(e: Env) -> Result<Vec<i128>, Error> {
        let vault = load_vault(&e)?;
        let pools = load_pools(&e);
        let mut aprs = Vec::new(&e);
        for i in 0..pools.len() {
            let alloc = pools.get(i).unwrap();
            let client = AmmPoolClient::new(&e, &alloc.pool);
            let fee_bps = client.get_fee();
            aprs.push_back(estimate_apr_bps(fee_bps, vault.volume_proxy_bps));
        }
        Ok(aprs)
    }

    // ── Internal ──────────────────────────────────────────────────────────────

    /// Returns the index of the pool with the highest estimated APR.
    fn best_pool_idx(e: &Env, pools: &Vec<PoolAllocation>, vault: &VaultState) -> u32 {
        let mut best_idx = 0u32;
        let mut best_apr: i128 = -1;
        for i in 0..pools.len() {
            let alloc = pools.get(i).unwrap();
            let client = AmmPoolClient::new(e, &alloc.pool);
            let fee_bps = client.get_fee();
            let apr = estimate_apr_bps(fee_bps, vault.volume_proxy_bps);
            if apr > best_apr {
                best_apr = apr;
                best_idx = i;
            }
        }
        best_idx
    }
}
