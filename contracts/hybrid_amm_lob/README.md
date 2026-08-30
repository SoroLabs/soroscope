# Hybrid AMM + Limit Order Book

A Soroban contract that routes a swap through a limit order book first and falls
back to a constant-product AMM for whatever the book cannot fill.

`swap` is **exact-output**: the caller names the output amount they want and the
maximum input they will pay (`in_max`).

## Matching safeguards

Matching the book against pool reserves is the interesting risk here. A large
taker order can cascade through thinly-priced ticks and drag the pool's spot
price with it, which is exactly the state a manipulator wants to manufacture.
Three guards bound that, configured through the `Guards` struct.

### 1. Execution depth limit

`Guards.max_match_depth` caps how many limit orders may be consumed **per
ledger**, summed across every swap in that ledger — not per call, so splitting
one sweep into several transactions in the same block buys no extra depth. The
counter lives in temporary storage keyed to the ledger sequence and resets on
its own when the sequence moves on.

When the budget runs out, matching **stops** and the AMM prices the remainder
under the deviation guard below. It deliberately does not revert: the budget is
a single global counter, so aborting instead would let one cheap actor exhaust
it early in a block and make the pool unswappable for everyone else for the rest
of that ledger.

`MAX_MATCH_DEPTH_LIMIT` (64) is a hard ceiling on the setting itself, so no
configuration can make a single swap iterate an unbounded number of orders.

### 2. Limit-order price band

An order priced worse than the pool's spot price by more than
`Guards.max_price_deviation_bps` is not matched. Because the book is kept sorted
by price, the first out-of-band order means every order behind it is worse, so
matching stops there and the AMM prices the rest.

The band is one-sided — it only blocks prices *unfavourable* to the taker. An
order priced better than spot is the maker's business, not a hazard.

If the pool holds no liquidity there is no reference price to measure against,
so the band is inactive and the book is the only venue.

### 3. Pool price deviation

After the AMM leg is applied, the pool's new spot price is compared against the
price captured before the swap. If it moved more than
`Guards.max_price_deviation_bps`, the swap reverts with
`PriceDeviationExceeded` and the whole transaction unwinds.

This is the backstop that makes guard 1 safe to degrade rather than revert: the
size pushed onto the AMM still cannot move the pool past tolerance.

## Configuration

```rust
Guards {
    max_match_depth: 8,           // DEFAULT_MAX_MATCH_DEPTH
    max_price_deviation_bps: 500, // DEFAULT_MAX_PRICE_DEVIATION_BPS (5%)
}
```

Set at `initialize` and changeable afterwards by the admin via `set_guards`.
Both values are validated on the way in: depth must be within
`1..=MAX_MATCH_DEPTH_LIMIT` and the tolerance within `1..=10_000` bps.
Anything else is rejected with `InvalidConfig`.

## Interface

| Function | Purpose |
| --- | --- |
| `initialize(admin, token_a, token_b, lp_fee_bps, maker_fee_bps, guards)` | One-time setup |
| `set_guards(guards)` | Admin-only safeguard update |
| `deposit(to, amount_a, amount_b)` / `withdraw(to, shares)` | AMM liquidity |
| `place_order(maker, is_bid, price, amount)` / `cancel_order(maker, order_id)` | Book management |
| `swap(taker, buy_a, out, in_max)` | Hybrid exact-output swap |
| `get_pool()` / `get_guards()` / `get_bids()` / `get_asks()` | Views |
| `get_spot_price()` | Pool spot price, or 0 when the pool is empty |
| `get_match_depth_used()` / `get_match_depth_remaining()` | Current ledger's depth budget |

Prices are `token_b` per `token_a`, scaled by `PRICE_SCALE` (1e6).

## Settlement

Limit-order fills settle directly between taker and maker: the taker pays the
maker, and the contract releases only the tokens that maker escrowed when
placing the order. The contract never has to front a payout from its own
balance. The AMM leg settles separately against the pool reserves.

## Errors

| Code | Error |
| --- | --- |
| 6 | `SlippageExceeded` — total input exceeded `in_max` |
| 7 | `InsufficientLiquidity` — AMM cannot cover the remainder |
| 10 | `PriceDeviationExceeded` — AMM leg moved spot past tolerance |
| 11 | `InvalidConfig` — guard parameter out of range |

## Build & test

This contract is its own Cargo workspace, because the repo-root `Cargo.lock` is
currently unparseable (a bad merge in `d58ebad` left duplicate keys in it) and
every root-level cargo command fails as a result. Run from this directory:

```bash
cd contracts/hybrid_amm_lob
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo build --target wasm32-unknown-unknown --release
```
