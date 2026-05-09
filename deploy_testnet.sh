#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONTRACT_DIR="$REPO_ROOT/agent_staking"
ARTIFACTS_DIR="$REPO_ROOT/artifacts"

echo "=== Building agent_staking WASM contract ==="
cd "$CONTRACT_DIR"
cargo build --target wasm32-unknown-unknown --release

WASM_SRC="$CONTRACT_DIR/target/wasm32-unknown-unknown/release/agent_staking.wasm"
mkdir -p "$ARTIFACTS_DIR"
cp "$WASM_SRC" "$ARTIFACTS_DIR/agent_staking.wasm"
echo "✓ Artifact: $ARTIFACTS_DIR/agent_staking.wasm"

echo ""
echo "=== Deploying to testnet ==="
cd "$REPO_ROOT"
python3 scripts/deploy_agent_staking.py

echo ""
echo "=== Starting dapp server (background) ==="
echo "Run manually: cd agent_staking/dapp && go run main.go"
echo ""
echo "Dapp env vars:"
echo "  WASM_PATH=$ARTIFACTS_DIR/agent_staking.wasm"
echo "  RUBIX_NODE_URL=${RUBIX_NODE_URL:-http://localhost:20011}"
echo "  DAPP_PORT=:6001"
