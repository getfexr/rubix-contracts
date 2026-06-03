# rbt-contracts

Smart contracts for the [Fexr](https://getfexr.com) platform — on-chain infrastructure for human + agent governance on Rubix.

## What this repo does

Each contract in this repo represents a deployable unit of on-chain logic. Currently:

| Contract | Purpose |
|---|---|
| `agent_staking` | Registers AI agents on-chain, stakes 1 RBT per agent, and creates an immutable activity log for every action the agent takes |

Contracts are written in Rust, compiled to WASM, and executed on the Rubix blockchain. A Go dapp runs alongside the Rubix node to handle execution callbacks and persist contract state.

---

## Architecture

```
VM1 (Rubix VM)                         VM2 (Executor VM)
├── rubixgoplatform (node :20011)        └── executor service
├── artifacts/agent_staking.wasm              └── calls VM1:20011 for execute-smart-contract
├── agent_staking/dapp (Go, :6001)
│    └── receives callbacks from node
│    └── runs WASM, reads/writes state
└── state/agent_state.json
```

- The Rubix node and Go dapp live on the same VM — node callbacks to dapp are localhost-to-localhost
- The executor service on VM2 calls the Rubix node API directly; it never talks to the dapp
- For other chains (Polygon, Solana): contract code lives with the executor service on VM2, connects to external RPC nodes

---

## Repo structure

```
rbt-contracts/
├── artifacts/
│   └── agent_staking.wasm          pre-built WASM binary
├── agent_staking/
│   ├── src/lib.rs                  Rust contract (compile target)
│   ├── Cargo.toml
│   └── dapp/
│       ├── main.go                 Go dapp server (callback handler)
│       ├── host/
│       │   ├── host_get_state.go   host fn: load state into WASM memory
│       │   └── host_save_state.go  host fn: write state from WASM memory
│       ├── state/
│       │   ├── state.go            file-based state persistence
│       │   └── agent_state.json    live state file (runtime)
│       └── go.mod
├── scripts/
│   └── deploy_agent_staking.py     deploy script (generate → deploy → register)
└── deploy_testnet.sh               one-shot build + deploy
```

---

## Contract functions — agent_staking

| Function | Description |
|---|---|
| `register_agent` | Registers the agent DID, name, type. Idempotent — safe to call again on an already-registered agent. Requires 1 RBT staked at deploy time. |
| `record_activity` | Appends an activity record (type, data, timestamp, tx ref) to the agent's immutable history. Rejected if agent is suspended. |
| `get_agent_info` | Returns full agent state: DID, name, type, status, stake, registration time, activity count, full activity history. |
| `update_status` | Sets agent status to `active` or `suspended`. Only the registered DID can call this. A suspended agent cannot record activity. |

Agent types: `trading` | `advisory` | `monitoring` | `general`

Payload format (passed as `smartContractData` to `/api/execute-smart-contract`):
```json
{ "register_agent": { "agent_did": "bafybmi...", "agent_name": "MyAgent", "agent_type": "trading", "timestamp": 1700000000 } }
{ "record_activity": { "agent_did": "bafybmi...", "activity_type": "trade", "activity_data": "{\"pair\":\"BTC/USDT\"}", "timestamp": 1700000120, "tx_ref": "txn_abc" } }
{ "get_agent_info": { "agent_did": "bafybmi..." } }
{ "update_status": { "agent_did": "bafybmi...", "status": "suspended" } }
```

---

## VM1 setup — Rubix VM

### Prerequisites

- Rubix node (`rubixgoplatform`) running on port `20011`
- Deployer DID created on the node with at least **1 RBT** balance
- Rust with `wasm32-unknown-unknown` target
- Go 1.21+
- Python 3 + `requests` (`pip install requests`)
- `rubix-wasm` repo cloned as a sibling to this repo (required by `go.mod` replace directive)

Required directory layout on VM1:

```
/home/ubuntu/                        (or any common parent)
├── rbt-contracts/                   this repo
└── Rubix/
    └── rubix-wasm/
        └── go-wasm-bridge/          required by agent_staking/dapp/go.mod
```

Clone rubix-wasm if not present:
```bash
git clone https://github.com/rubixchain/rubix-wasm Rubix/rubix-wasm
```

### Step 1 — Install Rust target

```bash
rustup target add wasm32-unknown-unknown
```

### Step 2 — Build the WASM binary

```bash
cd rbt-contracts/agent_staking
cargo build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/agent_staking.wasm ../artifacts/
```

### Step 3 — Build the Go dapp

```bash
cd rbt-contracts/agent_staking/dapp
go build -o agent-staking-dapp .
```

### Step 4 — Start the dapp

Start this **before** deploying the contract. The Rubix node will attempt a callback on the first execute call.

```bash
cd rbt-contracts/agent_staking/dapp

WASM_PATH=/home/ubuntu/rbt-contracts/artifacts/agent_staking.wasm \
RUBIX_NODE_URL=http://localhost:20011 \
DAPP_PORT=:6001 \
./agent-staking-dapp
```

To run as a background service, use `systemd` or `screen`:
```bash
screen -S agent-dapp
# run the command above, then Ctrl+A D to detach
```

### Step 5 — Deploy the contract

```bash
cd rbt-contracts

RUBIX_NODE_URL=http://localhost:20011 \
DEPLOYER_DID=bafybmi... \
PASSPHRASE=yourpassword \
python3 scripts/deploy_agent_staking.py
```

This runs three steps automatically:
1. Uploads WASM + source to node IPFS → returns contract address (IPFS CID)
2. Deploys the contract, locking 1 RBT in the genesis block via quorum consensus
3. Calls `register_agent` to initialise on-chain state

**Save the output.** You will need `AGENT_STAKING_CONTRACT_ADDRESS`.

### Step 6 — Register dapp URL with the node

The Rubix node must know where to POST execution callbacks. Run this once after deploy:

```bash
curl -X POST http://localhost:20011/api/subscribe-smart-contract \
  -H "Content-Type: application/json" \
  -d '{
    "contract": "<AGENT_STAKING_CONTRACT_ADDRESS>",
    "dappURL": "http://localhost:6001"
  }'
```

Verify with Rubix documentation if the endpoint or field names differ for your node version.

---

## VM2 setup — Executor VM

No contract code or dapp runs here. The executor service talks to the Rubix node on VM1 via HTTP.

Add to your executor service `.env`:
```env
RUBIX_NODE_URL=http://<VM1-IP>:20011
AGENT_STAKING_CONTRACT_ADDRESS=<from deploy step 5>
AGENT_STAKING_EXECUTOR_DID=<EXECUTOR_DID from deploy output>
```

Ensure VM2 can reach VM1 on port `20011` (outbound). No inbound ports required on VM2 for Rubix.

---

## Rebuilding after contract changes

```bash
# 1. rebuild WASM
cd agent_staking
cargo build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/agent_staking.wasm ../artifacts/

# 2. redeploy (new contract address each time)
cd ..
RUBIX_NODE_URL=http://localhost:20011 \
DEPLOYER_DID=bafybmi... \
PASSPHRASE=yourpassword \
python3 scripts/deploy_agent_staking.py

# 3. update AGENT_STAKING_CONTRACT_ADDRESS in your executor service .env
# 4. re-register dapp URL (step 6 above)
# 5. restart dapp (new WASM_PATH if changed)
```

Each deploy produces a new contract address. The old contract and its state remain on-chain but your executor service must be pointed to the new address.

---

## Common issues

### `go build` fails with missing module

```
cannot find module providing github.com/rubixchain/rubix-wasm/go-wasm-bridge
```

The `replace` directive in `agent_staking/dapp/go.mod` points to a local path:
```
replace github.com/rubixchain/rubix-wasm/go-wasm-bridge => ../../../../Rubix/rubix-wasm/go-wasm-bridge
```

Fix: clone `rubix-wasm` so it sits at `../../../Rubix/rubix-wasm` relative to `rbt-contracts`. See VM1 Prerequisites above for the required layout.

---

### Deploy script fails — WASM not found

```
ERROR: WASM artifact not found at .../artifacts/agent_staking.wasm
```

Run Step 2 (build WASM) first.

---

### Deploy script fails — DID not set

```
ERROR: Set DEPLOYER_DID env var
```

The deployer DID must already exist on the Rubix node and have at least 1 RBT. Create one via `/api/createdid` or `/api/request-did-for-pubkey` on the node before deploying.

---

### Contract executes but state is not updated

The dapp is not running or not reachable. The Rubix node ran consensus but the callback to `:6001` failed silently.

- Confirm the dapp is running: `curl -X POST http://localhost:6001/` — should return a JSON error (not connection refused)
- Confirm dapp URL is registered with the node (Step 6)
- Check dapp logs for callback errors

---

### State resets between dapp restarts

The dapp resolves `agent_state.json` relative to the binary's directory:
```
<binary dir>/state/agent_state.json
```

If you move the binary or run it from a different directory, it won't find the existing state file. Always run the dapp from the same directory, or verify the `state/` path is consistent.

---

### `execute-smart-contract` returns success but register_agent has no effect

The dapp may have started after the deploy script already sent the execute call. Stop the deploy mid-run, ensure the dapp is running, then re-run from Step 5.

---

### Quorum consensus times out

The Rubix node needs quorum peers to confirm the transaction. On a testnet with limited peers this can be slow or fail. Check node logs on VM1:

```bash
# rubixgoplatform logs (path depends on your setup)
tail -f /path/to/rubix/node.log
```

---

## Environment variable reference

| Variable | Used by | Description |
|---|---|---|
| `RUBIX_NODE_URL` | dapp, deploy script, executor service | Rubix node API base URL. Default: `http://localhost:20011` |
| `DEPLOYER_DID` | deploy script | Rubix DID of the deployer. Must have ≥ 1 RBT. |
| `PASSPHRASE` | deploy script | Password for the deployer DID (password-based signing). |
| `WASM_PATH` | dapp | Absolute path to `agent_staking.wasm`. Default: `../../../artifacts/agent_staking.wasm` |
| `DAPP_PORT` | dapp | Port the dapp listens on. Default: `:6001` |
| `AGENT_STAKING_CONTRACT_ADDRESS` | executor service | Contract address returned by the deploy script. |
| `AGENT_STAKING_EXECUTOR_DID` | executor service | Executor DID, used to sign contract calls. |
