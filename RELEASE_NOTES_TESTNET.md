# Testnet Release — Final Release Notes

Release: Testnet release of SoroLabs/soroscope — curated set of new contracts, security tooling, and integration tests for the Soroban ecosystem.

## Summary
- **Scope:** New AMM and auction primitives, governance & emergency controls, token/transfer utilities, oracle tooling, and developer test harnesses.
- **Target:** Public testnet verification and community testing ahead of mainnet readiness.

## Highlights
- **AMMs & Trading:** `concentrated_amm`, `liquidity_pool`, `hybrid_amm_lob`, `flash_loan_vault`, `multi_yield_vault`
- **Auctions:** `english_auction`, `dutch_auction`, `auction_factory`
- **Governance & Security:** `governance`, `emergency_guard`, `timelocked_escrow`, `staking_rewards`
- **Token & Transfer Utilities:** `token`, `private_transfer`, `batch_transfer`, `proxy`, `soulbound_token`
- **Oracles & Cross-Chain:** `twap_oracle`, `oracle_aggregator`, `cross_chain_verifier`
- **Identity & Auth:** `did_registry`, `typed_data_auth`
- **Developer Utilities:** `factory`, `error_codes`, `core` helpers, and deterministic snapshot tests under `test_snapshots/`

## What's New (Short)
- Modular AMM suite: concentrated liquidity plus hybrid orderbook support.
- Auction factory: parameterized auction templates for quick launches.
- Emergency guard: on-chain emergency control patterns and integration guidance.
- Oracles: TWAP and aggregated feeds for price discovery.
- Deterministic snapshot tests recorded in `contracts/*/test_snapshots` for CI parity.

## Testing & Local Validation (Community Guide)

Prerequisites
- Install Rust and Cargo.
- Add the WASM target:

```bash
rustup target add wasm32-unknown-unknown
```

- (Optional) Install Soroban CLI or local testnet tooling if you plan to deploy locally.

Run tests
- Run the full workspace test suite:

```bash
cargo test --workspace
```

- Run tests for a single contract (example):

```bash
cargo test --manifest-path contracts/<contract>/Cargo.toml
```

Build contract WASM

```bash
cargo build --manifest-path contracts/<contract>/Cargo.toml --release --target wasm32-unknown-unknown
# wasm artifact: contracts/<contract>/target/wasm32-unknown-unknown/release/<package>.wasm
```

Snapshot tests
- Many crates include `test_snapshots/`. Run `cargo test` inside those crate folders to validate snapshots.

Deploy & interact (illustrative)
- Use your Soroban-compatible CLI to deploy and invoke built WASM artifacts. Example commands (replace placeholders with your tooling and auth):

```bash
# illustrative — adapt to your Soroban CLI
soroban contract deploy --wasm target/wasm32-unknown-unknown/release/<package>.wasm
soroban contract invoke --id <contract-id> --fn <method> --arg ...
```

Web UI (optional)

```bash
cd web
npm install
npm run dev
```

## Upgrade & Compatibility Notes
- Ensure CI and local dev images include `wasm32-unknown-unknown`.
- Review `core/src/auth.rs` and `error_codes` for changed enums and error shapes before integrating clients.
- Contracts assume updated gas/lifecycle expectations; test representative flows when deploying to public testnet.

## How to Verify Before Opening PRs
- Run `cargo test --workspace` and fix regressions.
- Build all contract WASM artifacts and spot-check a deploy+invoke on a local or public testnet.
- Add/update unit tests or snapshots for any contract logic changes.

## Reporting Issues & Contribution
- Open issues with the `contract:` or `test:` label and include: failing command, crate path, Rust version, and minimal failing output.
- Follow `CONTRIBUTING.md` for PR guidance; include tests or snapshots for logic changes.

## Acknowledgements
- Thanks to all contributors and auditors for tests, snapshots, and reviews.

---

If you want changes (format, additional deploy examples, or to place this in another path), tell me where to put it or which sections to expand.
