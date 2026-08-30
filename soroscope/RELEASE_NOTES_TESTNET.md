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

## What's New
- Modular AMM suite: concentrated liquidity plus hybrid orderbook support.
- Auction factory: parameterized auction templates for quick launches.
- Emergency guard: on-chain emergency control patterns and integration guidance; supports granular `PauseType` bitmask for per-operation pausing.
- Cross-chain verifier: now supports `PauseType::VERIFY` — verifications are rejected when the contract is paused.
- Oracles: TWAP and aggregated feeds for price discovery.
- Merkle Tree CLI: `soroscope-core merkle build-file <path>` generates a root hash from a newline-delimited leaf file.
- Deterministic snapshot tests recorded in `contracts/*/test_snapshots` for CI parity.

---

## Testnet Contract Addresses

The following contracts have been deployed to the **Stellar Testnet** (network passphrase: `Test SDF Network ; September 2015`).

> **Note:** These are community-testing addresses. Do not send mainnet funds to these contracts.

| Contract | Testnet Contract ID | Deployer |
|---|---|---|
| `cross_chain_verifier` | `CDCVERIFIER2TESTNETAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAB` | relayer-admin |
| `emergency_guard` | `CDEMGUARD2TESTNETAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAB` | security-admin |
| `token` | `CDTOKEN2TESTNETAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAB` | token-admin |
| `governance` | `CDGOV2TESTNETAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAB` | governance-admin |
| `liquidity_pool` | `CDLPOOL2TESTNETAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAB` | lp-admin |
| `factory` | `CDFACTORY2TESTNETAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAB` | factory-admin |
| `timelocked_escrow` | `CDESCROW2TESTNETAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAB` | escrow-admin |
| `staking_rewards` | `CDSTAKING2TESTNETAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAB` | staking-admin |
| `twap_oracle` | `CDTWAP2TESTNETAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAB` | oracle-admin |
| `oracle_aggregator` | `CDORAGG2TESTNETAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAB` | oracle-admin |

> Replace these placeholder IDs with actual deployed addresses after running the deploy workflow (`.github/workflows/deploy-testnet.yml`).

---

## Contract Methods Reference

### `cross_chain_verifier`

| Method | Args | Description |
|---|---|---|
| `initialize` | `admin: Address` | One-time setup; sets the admin. |
| `update_root` | `block_height: u32, new_root: BytesN<32>` | Admin-only; records a Merkle state root. |
| `get_root` | `block_height: u32` | Returns the stored root, or `None`. |
| `verify_message` | `block_height: u32, leaf: BytesN<32>, proof: Vec<BytesN<32>>, proof_flags: Vec<bool>` | Verify a Merkle inclusion proof. Returns `false` when paused or proof is invalid. |
| `verify_message_and_consume` | `block_height: u32, nonce: u64, leaf: BytesN<32>, proof: Vec<BytesN<32>>, proof_flags: Vec<bool>` | Verify and mark the nonce as consumed (replay protection). Panics if nonce already used or contract is paused. |
| `verify_signed_message` | `signed_message: SignedMessage, block_height: u32, proof: Vec<BytesN<32>>, proof_flags: Vec<bool>` | Verify an Ed25519-signed cross-chain message with Merkle proof. |
| `add_authorized_signer` | `public_key: Bytes, algorithm: SignatureAlgorithm` | Admin-only; authorize a signer key. |
| `remove_authorized_signer` | `public_key: Bytes` | Admin-only; revoke a signer key. |
| `is_nonce_processed` | `nonce: u64` | Check whether a nonce has been consumed. |
| `set_paused` | `paused: bool` | Admin-only; pause or unpause all verifications (`PauseType::VERIFY`). |
| `is_paused` | — | Returns `true` if verification is currently paused. |

### `emergency_guard`

| Method | Args | Description |
|---|---|---|
| `initialize` | `admins: Vec<Address>, threshold: u32` | Set up multi-sig admins and approval threshold. |
| `pause` | `operation: u32` | Pause a specific operation bitmask (see `PauseType`). |
| `unpause` | `operation: u32` | Unpause a specific operation. |
| `pause_all` | — | Pause all operations. |
| `unpause_all` | — | Unpause all operations. |
| `is_paused` | `operation: u32` | Check if a specific operation is paused. |

### `token`

| Method | Args | Description |
|---|---|---|
| `initialize` | `admin: Address, decimal: u32, name: String, symbol: String` | Deploy and configure the token. |
| `mint` | `to: Address, amount: i128` | Admin-only; mint tokens. |
| `transfer` | `from: Address, to: Address, amount: i128` | Transfer tokens between accounts. |
| `burn` | `from: Address, amount: i128` | Burn tokens from an account. |
| `balance` | `id: Address` | Return token balance. |
| `approve` | `from: Address, spender: Address, amount: i128, expiration_ledger: u32` | Grant allowance. |

### `governance`

| Method | Args | Description |
|---|---|---|
| `initialize` | `admin: Address, token: Address` | Set up governance with a voting token. |
| `propose` | `proposer: Address, description: String` | Create a new governance proposal. |
| `vote` | `voter: Address, proposal_id: u32, approve: bool` | Cast a vote. |
| `execute` | `proposal_id: u32` | Execute an approved proposal. |

---

## Testing & Local Validation (Community Guide)

### Prerequisites
- Install Rust and Cargo.
- Add the WASM target:

```bash
rustup target add wasm32-unknown-unknown
```

- (Optional) Install Soroban CLI or local testnet tooling if you plan to deploy locally.

### Run tests

```bash
# Full workspace
cargo test --workspace

# Single contract
cargo test --manifest-path contracts/<contract>/Cargo.toml

# Merkle Tree unit tests
cargo test -p soroscope-core merkle_tree
```

### Build contract WASM

```bash
cargo build --manifest-path contracts/<contract>/Cargo.toml \
  --release --target wasm32-unknown-unknown
# Artifact: contracts/<contract>/target/wasm32-unknown-unknown/release/<package>.wasm
```

### Use Merkle CLI

```bash
# Build a tree from inline leaves (hex-encoded output)
soroscope-core merkle build leaf1 leaf2 leaf3

# Build a tree from a newline-delimited file
soroscope-core merkle build-file leaves.txt

# Generate a proof for leaf at index 0
soroscope-core merkle proof 0 leaf1 leaf2 leaf3
```

### Deploy & interact (illustrative)

```bash
# Deploy
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/<package>.wasm \
  --source <your-key> \
  --network testnet

# Initialize cross_chain_verifier
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source admin \
  --network testnet \
  -- initialize \
  --admin <ADMIN_ADDRESS>

# Update Merkle root
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source relayer \
  --network testnet \
  -- update_root \
  --block_height 1000 \
  --new_root "<root_hex>"

# Verify a message
soroban contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  -- verify_message \
  --block_height 1000 \
  --leaf "<leaf_hex>" \
  --proof '["<sibling_hex>"]' \
  --proof_flags '[true]'

# Pause verifications
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source admin \
  --network testnet \
  -- set_paused \
  --paused true
```

### Web UI (optional)

```bash
cd web
npm install
npm run dev
# Open http://localhost:3000
```

---

## Upgrade & Compatibility Notes
- Ensure CI and local dev images include `wasm32-unknown-unknown`.
- `cross_chain_verifier` now includes pause support via `PauseType::VERIFY`; update integrations to handle the `VerificationPaused` error.
- Review `core/src/auth.rs` and `error_codes` for changed enums before integrating clients.
- Contracts assume updated gas/lifecycle expectations; test representative flows on testnet before mainnet.

## How to Verify Before Opening PRs
1. Run `cargo test --workspace` and fix regressions.
2. Build all contract WASM artifacts and spot-check a deploy+invoke on a local or public testnet.
3. Add/update unit tests or snapshots for any contract logic changes.

## Reporting Issues & Contribution
- Open issues with the `contract:` or `test:` label. Include: failing command, crate path, Rust version, and minimal failing output.
- Follow `CONTRIBUTING.md` for PR guidance; include tests or snapshots for logic changes.

## Acknowledgements
- Thanks to all contributors and auditors for tests, snapshots, and reviews.
