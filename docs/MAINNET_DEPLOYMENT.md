# Mainnet Release — Deployment Guide & CLI Installation

This guide covers installing the SoroScope CLI, running a local simulation, and preparing for Stellar Mainnet deployment.

---

## Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Rust (stable) | ≥ 1.75 | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| wasm32 target | — | `rustup target add wasm32-unknown-unknown` |
| Stellar CLI | 22.x | `cargo install soroban-cli --version 22.0.0 --locked` |
| Node.js | ≥ 18 | [nodejs.org](https://nodejs.org) |

---

## CLI Installation

### From Source (Recommended)

```bash
git clone https://github.com/SoroLabs/soroscope
cd soroscope

# Build the core CLI binary
cargo build --release -p soroscope-core

# The binary is at:
./target/release/soroscope-core
```

To make it available system-wide:

```bash
cp target/release/soroscope-core /usr/local/bin/soroscope
soroscope --help
```

### Verify the Build

```bash
cargo check --locked --all-targets
cargo test --locked
```

---

## Running a Local Simulation

The core server exposes an HTTP API on `http://localhost:8080` for profiling contract resource usage.

### Start the Server

```bash
RUST_LOG=info cargo run -p soroscope-core
```

### Build Contract WASMs for Profiling

```bash
cargo build --target wasm32-unknown-unknown --release
```

WASM files are written to `target/wasm32-unknown-unknown/release/`. Upload any `.wasm` file through the web dashboard or POST it directly to the simulation API.

### Start the Web Dashboard

```bash
cd web
npm install
npm run dev
# Open http://localhost:3000
```

---

## Mainnet Deployment Steps

> Complete the [Security Audit Checklist](./SECURITY_AUDIT_CHECKLIST.md) before proceeding.

### 1. Build Release Artifacts

```bash
stellar contract build
```

This compiles all `cdylib` contracts to `target/wasm32-unknown-unknown/release/*.wasm`.

### 2. Configure the Deployer Identity

```bash
# Import your Mainnet deployer key (stored in a secure vault)
stellar keys add deployer --secret-key
# Enter the secret key when prompted — do NOT pass it as a shell argument

# Confirm the public key
stellar keys address deployer
```

### 3. Deploy Each Contract

```bash
# Example: deploy the emergency_guard contract
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/emergency_guard.wasm \
  --source deployer \
  --network mainnet \
  --fee 1000000
```

Record each returned contract ID. The automated CI deploy job (`deploy-testnet.yml`) writes a `deployment_manifest.txt` — replicate this process manually for Mainnet and store the manifest in version control.

### 4. Initialize Contracts

After deployment, call each contract's `initialize` entry point with the production admin multisig address and any required parameters:

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source deployer \
  --network mainnet \
  -- initialize \
  --admin <MULTISIG_ADDRESS> \
  --fee_bps 30
```

### 5. Transfer Admin Authority

Rotate the deployer key out of the admin role immediately after initialization:

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source deployer \
  --network mainnet \
  -- transfer_admin \
  --new_admin <PERMANENT_MULTISIG_ADDRESS>
```

### 6. Verify Deployment

```bash
# Read back the stored admin to confirm the rotation
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network mainnet \
  -- get_admin
```

---

## Network Configuration

| Parameter | Testnet | Mainnet |
|-----------|---------|---------|
| RPC URL | `https://soroban-testnet.stellar.org` | `https://mainnet.sorobanrpc.com` |
| Network passphrase | `Test SDF Network ; September 2015` | `Public Global Stellar Network ; September 2015` |
| Stellar CLI flag | `--network testnet` | `--network mainnet` |

To configure a named network profile locally:

```bash
stellar network add mainnet \
  --rpc-url https://mainnet.sorobanrpc.com \
  --network-passphrase "Public Global Stellar Network ; September 2015"
```

---

## Troubleshooting

**`error: failed to compile` on wasm32 target**
Run `rustup target add wasm32-unknown-unknown` and retry.

**`Insufficient funds` during deploy**
The deployer account must be funded on Mainnet. Transfer XLM to the deployer public key before deploying.

**`Transaction simulation failed`**
Check that the `--fee` value is high enough for current network conditions. Use `stellar fee stats --network mainnet` to check current base fees.

**Proof verification returns `false` after Merkle root update**
See the Troubleshooting section in [`core/MERKLE_TREE_README.md`](../core/MERKLE_TREE_README.md).

---

## Related

- [Security Audit Checklist](./SECURITY_AUDIT_CHECKLIST.md)
- [Testnet Release Notes](../RELEASE_NOTES_TESTNET.md)
- [Merkle Tree CLI Reference](../core/MERKLE_TREE_README.md)
- [EmergencyGuard Setup](./EMERGENCY_GUARD_SETUP.md)
</content>
</invoke>