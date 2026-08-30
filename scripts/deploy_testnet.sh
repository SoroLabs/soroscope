#!/usr/bin/env bash
set -euo pipefail

# Automates building and deploying all workspace contracts to testnet.
# Usage: scripts/deploy_testnet.sh --source-account <identity-or-secret>

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

SOURCE_ACCOUNT="${STELLAR_SOURCE_ACCOUNT:-}"
NETWORK="${STELLAR_NETWORK:-testnet}"
WASM_DIR="$REPO_ROOT/target/wasm32v1-none/release"

die() {
  echo "error: $*" >&2
  exit 1
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || die "required command '$1' not found"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --source-account|--source)
      SOURCE_ACCOUNT="${2:-}"
      shift 2
      ;;
    --network)
      NETWORK="${2:-}"
      shift 2
      ;;
    -h|--help)
      echo "Usage: $0 --source-account <identity-or-secret> [--network <name>]"
      exit 0
      ;;
    *)
      die "unknown option: $1"
      ;;
  esac
done

[[ -n "$SOURCE_ACCOUNT" ]] || die "--source-account is required"

require_command stellar
require_command cargo

echo "Building all workspace contracts..."
cargo build \
  --manifest-path "$REPO_ROOT/Cargo.toml" \
  --target wasm32v1-none \
  --release

echo "Deploying and initializing contracts to $NETWORK..."

deployed=0
for wasm in "$WASM_DIR"/*.wasm; do
  [[ -f "$wasm" ]] || continue

  contract_name=$(basename "$wasm" .wasm)
  echo "Deploying $contract_name..."

  CONTRACT_ID=$(stellar contract deploy \
    --wasm "$wasm" \
    --source-account "$SOURCE_ACCOUNT" \
    --network "$NETWORK")

  echo "Deployed $contract_name at $CONTRACT_ID"

  if stellar contract invoke \
      --id "$CONTRACT_ID" \
      --source-account "$SOURCE_ACCOUNT" \
      --network "$NETWORK" \
      -- initialize >/dev/null 2>&1; then
    echo "Initialized $contract_name successfully."
  else
    echo "$contract_name does not require initialization or requires arguments."
  fi

  deployed=$((deployed + 1))
done

echo "All deployments finished. Total contracts deployed: $deployed"
