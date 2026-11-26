#!/usr/bin/env bash
set -euo pipefail

# Automates building and deploying all workspace contracts to a Stellar network.
# Network settings can be overridden with flags or environment variables.

usage() {
  cat <<'USAGE'
Automate building and deploying all workspace contracts to a Stellar network.

Usage:
  scripts/deploy_testnet.sh [--source-account <identity-or-secret>] [options]

Options:
  --source-account, --source <value>   Stellar identity, secret key, or signing source.
  --network <name>                     Stellar network alias. Defaults to STELLAR_NETWORK or testnet.
  --rpc-url <url>                      RPC URL, useful when not using a configured alias. Defaults to STELLAR_RPC_URL.
  --network-passphrase <value>         Network passphrase for --rpc-url deployments. Defaults to STELLAR_NETWORK_PASSPHRASE.
  --cli <binary>                       Stellar CLI binary. Defaults to STELLAR_CLI, then stellar or soroban.
  --dry-run                            Print the commands that would run; no funded keys or CLI required.
  -h, --help                           Show this help.

Environment:
  STELLAR_SOURCE_ACCOUNT              Default --source-account.
  STELLAR_NETWORK                     Default --network.
  STELLAR_RPC_URL                     Default --rpc-url.
  STELLAR_NETWORK_PASSPHRASE          Default --network-passphrase.
  STELLAR_CLI                         Default --cli.
USAGE
}

die() {
  echo "error: $*" >&2
  exit 1
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || die "required command '$1' was not found"
}

run_cmd() {
  if [[ "$DRY_RUN" == "1" ]]; then
    printf '+'
    printf ' %q' "$@"
    printf '\n'
    return 0
  fi

  "$@"
}

run_quiet() {
  if [[ "$DRY_RUN" == "1" ]]; then
    run_cmd "$@"
  else
    "$@" >/dev/null 2>&1
  fi
}

capture_cmd() {
  if [[ "$DRY_RUN" == "1" ]]; then
    printf '+' >&2
    printf ' %q' "$@" >&2
    printf '\n' >&2
    echo "DRY_RUN_CONTRACT_ID"
    return 0
  fi

  "$@"
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

SOURCE_ACCOUNT="${STELLAR_SOURCE_ACCOUNT:-}"
NETWORK="${STELLAR_NETWORK:-testnet}"
RPC_URL="${STELLAR_RPC_URL:-}"
NETWORK_PASSPHRASE="${STELLAR_NETWORK_PASSPHRASE:-}"
CLI="${STELLAR_CLI:-}"
CLI_MISSING=0
WASM_DIR="$REPO_ROOT/target/wasm32v1-none/release"
DRY_RUN=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --source-account|--source)
      [[ $# -ge 2 ]] || die "$1 requires a value"
      SOURCE_ACCOUNT="$2"
      shift 2
      ;;
    --network)
      [[ $# -ge 2 ]] || die "$1 requires a value"
      NETWORK="$2"
      shift 2
      ;;
    --rpc-url)
      [[ $# -ge 2 ]] || die "$1 requires a value"
      RPC_URL="$2"
      shift 2
      ;;
    --network-passphrase)
      [[ $# -ge 2 ]] || die "$1 requires a value"
      NETWORK_PASSPHRASE="$2"
      shift 2
      ;;
    --cli)
      [[ $# -ge 2 ]] || die "$1 requires a value"
      CLI="$2"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      die "unknown option: $1"
      ;;
  esac
done

if [[ -z "$CLI" ]]; then
  if command -v stellar >/dev/null 2>&1; then
    CLI="stellar"
  elif command -v soroban >/dev/null 2>&1; then
    CLI="soroban"
  else
    CLI="stellar"
    CLI_MISSING=1
  fi
fi

if [[ "$DRY_RUN" == "1" ]]; then
  [[ -n "$SOURCE_ACCOUNT" ]] || SOURCE_ACCOUNT="<source-account>"
else
  [[ "$CLI_MISSING" != "1" ]] || die "neither 'stellar' nor 'soroban' CLI was found; install one or set STELLAR_CLI"
  require_command "$CLI"
  require_command cargo
  [[ -n "$SOURCE_ACCOUNT" ]] || die "--source-account (or STELLAR_SOURCE_ACCOUNT) is required"
fi

NETWORK_ARGS=()
[[ -z "$NETWORK" ]] || NETWORK_ARGS+=(--network "$NETWORK")
[[ -z "$RPC_URL" ]] || NETWORK_ARGS+=(--rpc-url "$RPC_URL")
[[ -z "$NETWORK_PASSPHRASE" ]] || NETWORK_ARGS+=(--network-passphrase "$NETWORK_PASSPHRASE")

echo "Building all workspace contracts..."
run_cmd cargo build \
  --manifest-path "$REPO_ROOT/Cargo.toml" \
  --target wasm32v1-none \
  --release

echo "Deploying and initializing contracts to ${NETWORK:-configured RPC}..."

deployed=0
for wasm in "$WASM_DIR"/*.wasm; do
  [[ -f "$wasm" ]] || continue

  contract_name="$(basename "$wasm" .wasm)"
  echo "Deploying $contract_name..."

  CONTRACT_ID="$(capture_cmd "$CLI" contract deploy \
    --wasm "$wasm" \
    --source-account "$SOURCE_ACCOUNT" \
    "${NETWORK_ARGS[@]}")"

  echo "Deployed $contract_name at $CONTRACT_ID"

  if run_quiet "$CLI" contract invoke \
      --id "$CONTRACT_ID" \
      --source-account "$SOURCE_ACCOUNT" \
      "${NETWORK_ARGS[@]}" \
      -- initialize; then
    echo "Initialized $contract_name successfully."
  else
    echo "$contract_name does not require initialization or requires arguments."
  fi

  deployed=$((deployed + 1))
done

echo "All deployments finished. Total contracts deployed: $deployed"
