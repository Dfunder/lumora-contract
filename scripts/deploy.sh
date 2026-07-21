#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
DEPLOYMENTS_FILE="$PROJECT_DIR/deployments/testnet.json"

if [ ! -f "$PROJECT_DIR/.env" ]; then
  echo "Error: .env file not found. Copy .env.example to .env and fill in ADMIN_KEY."
  exit 1
fi

source "$PROJECT_DIR/.env"

if [ -z "${ADMIN_KEY:-}" ]; then
  echo "Error: ADMIN_KEY is not set in .env"
  exit 1
fi

echo "Building contracts..."
cargo build --release --target wasm32-unknown-unknown

echo "Deploying campaign contract..."
CAMPAIGN_ID=$(soroban contract deploy \
  --wasm "$PROJECT_DIR/target/wasm32-unknown-unknown/release/campaign.wasm" \
  --source "$ADMIN_KEY" \
  --rpc-url "${RPC_URL:-https://soroban-testnet.stellar.org}" \
  --network-passphrase "${NETWORK_PASSPHRASE:-Test SDF Network ; September 2015}")

echo "Deployed campaign at: $CAMPAIGN_ID"

echo "Deploying factory contract..."
FACTORY_ID=$(soroban contract deploy \
  --wasm "$PROJECT_DIR/target/wasm32-unknown-unknown/release/factory.wasm" \
  --source "$ADMIN_KEY" \
  --rpc-url "${RPC_URL:-https://soroban-testnet.stellar.org}" \
  --network-passphrase "${NETWORK_PASSPHRASE:-Test SDF Network ; September 2015}")

echo "Deployed factory at: $FACTORY_ID"

TIMESTAMP=$(date -u +%Y-%m-%dT%H:%M:%SZ)

mkdir -p "$(dirname "$DEPLOYMENTS_FILE")"

if [ -f "$DEPLOYMENTS_FILE" ]; then
  EXISTING=$(cat "$DEPLOYMENTS_FILE")
else
  EXISTING="{}"
fi

echo "$EXISTING" | jq \
  --arg cid "$CAMPAIGN_ID" \
  --arg fid "$FACTORY_ID" \
  --arg ts "$TIMESTAMP" \
  '.campaign = $cid | .factory = $fid | .updated_at = $ts' > "$DEPLOYMENTS_FILE"

echo "Deployment record written to $DEPLOYMENTS_FILE"
echo "Done."
