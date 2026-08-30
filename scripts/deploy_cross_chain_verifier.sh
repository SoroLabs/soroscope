#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Deploy and initialize the cross-chain verifier contract.

Usage:
  scripts/deploy_cross_chain_verifier.sh --source-account <identity-or-secret> \
    --root-validator-key <G...> [--root-validator-key <G...> ...]

Options:
  --source-account, --source <value>   Stellar identity, secret key, or signing source account.
  --root-validator-key <G...>         Root validator public key. May be repeated or comma-separated.
  --root-validator-keys-file <path>   File of root validator public keys, one per line. Blank lines
                                      and lines starting with # are ignored.
  --admin <address>                   Address passed to initialize. Defaults to the first root
                                      validator key because the verifier ABI currently stores one admin.
  --network <name>                    Stellar network alias. Defaults to STELLAR_NETWORK or testnet.
  --rpc-url <url>                     RPC URL, useful when not using a configured network alias.
  --network-passphrase <value>        Network passphrase for --rpc-url deployments.
  --wasm <path>                       WASM path. Defaults to target/wasm32v1-none/release/cross_chain_verifier.wasm.
  --no-build                          Skip cargo build and deploy the existing WASM.
  --dry-run                           Print the commands that would run.
  -h, --help                          Show this help.

Environment:
  STELLAR_SOURCE_ACCOUNT              Default --source-account.
  STELLAR_NETWORK                     Default --network.
  STELLAR_RPC_URL                     Default --rpc-url.
  STELLAR_NETWORK_PASSPHRASE          Default --network-passphrase.
USAGE
}

die() {
  echo "error: $*" >&2
  exit 1
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || die "required command '$1' was not found"
}

append_validator_keys() {
  local raw="$1"
  local key

  IFS=',' read -ra split_keys <<<"$raw"
  for key in "${split_keys[@]}"; do
    key="${key#"${key%%[![:space:]]*}"}"
    key="${key%"${key##*[![:space:]]}"}"
    [[ -n "$key" ]] && ROOT_VALIDATOR_KEYS+=("$key")
  done
}

load_validator_keys_file() {
  local file="$1"
  local line

  [[ -f "$file" ]] || die "root validator keys file not found: $file"

  while IFS= read -r line || [[ -n "$line" ]]; do
    line="${line%%#*}"
    append_validator_keys "$line"
  done <"$file"
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
WASM="$REPO_ROOT/target/wasm32v1-none/release/cross_chain_verifier.wasm"
BUILD=1
DRY_RUN=0
ADMIN=""
ROOT_VALIDATOR_KEYS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --source-account|--source)
      SOURCE_ACCOUNT="${2:-}"
      shift 2
      ;;
    --root-validator-key|--validator-key)
      append_validator_keys "${2:-}"
      shift 2
      ;;
    --root-validator-keys-file|--validator-keys-file)
      load_validator_keys_file "${2:-}"
      shift 2
      ;;
    --admin)
      ADMIN="${2:-}"
      shift 2
      ;;
    --network)
      NETWORK="${2:-}"
      shift 2
      ;;
    --rpc-url)
      RPC_URL="${2:-}"
      shift 2
      ;;
    --network-passphrase)
      NETWORK_PASSPHRASE="${2:-}"
      shift 2
      ;;
    --wasm)
      WASM="${2:-}"
      shift 2
      ;;
    --no-build)
      BUILD=0
      shift
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

[[ -n "$SOURCE_ACCOUNT" ]] || die "--source-account is required"
[[ ${#ROOT_VALIDATOR_KEYS[@]} -gt 0 || -n "$ADMIN" ]] || die "provide --root-validator-key, --root-validator-keys-file, or --admin"

if [[ -z "$ADMIN" ]]; then
  ADMIN="${ROOT_VALIDATOR_KEYS[0]}"
fi

if [[ ${#ROOT_VALIDATOR_KEYS[@]} -gt 1 && "$ADMIN" == "${ROOT_VALIDATOR_KEYS[0]}" ]]; then
  echo "warning: verifier initialize currently accepts one admin address; using the first root validator key as admin" >&2
fi

NETWORK_ARGS=()
if [[ -n "$NETWORK" ]]; then
  NETWORK_ARGS+=(--network "$NETWORK")
fi
if [[ -n "$RPC_URL" ]]; then
  NETWORK_ARGS+=(--rpc-url "$RPC_URL")
fi
if [[ -n "$NETWORK_PASSPHRASE" ]]; then
  NETWORK_ARGS+=(--network-passphrase "$NETWORK_PASSPHRASE")
fi

if [[ "$DRY_RUN" != "1" ]]; then
  require_command stellar
fi

if [[ "$BUILD" == "1" ]]; then
  if [[ "$DRY_RUN" != "1" ]]; then
    require_command cargo
  fi
  run_cmd cargo build --manifest-path "$REPO_ROOT/Cargo.toml" --target wasm32v1-none --release -p cross-chain-verifier
fi

if [[ "$DRY_RUN" != "1" && ! -f "$WASM" ]]; then
  die "WASM not found: $WASM"
fi

echo "Deploying cross-chain verifier..."
CONTRACT_ID="$(capture_cmd stellar contract deploy --wasm "$WASM" --source-account "$SOURCE_ACCOUNT" "${NETWORK_ARGS[@]}")"

echo "Initializing verifier with admin: $ADMIN"
run_cmd stellar contract invoke --id "$CONTRACT_ID" --source-account "$SOURCE_ACCOUNT" "${NETWORK_ARGS[@]}" -- initialize --admin "$ADMIN"

echo "Cross-chain verifier deployed and initialized."
echo "Contract ID: $CONTRACT_ID"
echo "Root validator keys supplied: ${#ROOT_VALIDATOR_KEYS[@]}"
