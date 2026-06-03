# rbt-contracts

On-chain agent staking and activity auditing for the [Fexr](https://getfexr.com) platform, built on the [Rubix](https://rubix.network) blockchain.

Contracts are written in Rust, compiled to WASM, and executed by the Rubix node. A Go dapp runs alongside the node to handle execution callbacks and provide a query API over the persisted activity history.

---

## What this repo contains

| Path | Description |
|---|---|
| `agent_staking/src/lib.rs` | Rust smart contract — WASM execution target |
| `agent_staking/dapp/` | Go dapp — callback handler, SQLite persistence, query API |
| `artifacts/agent_staking.wasm` | Pre-built WASM binary (deploy without Rust toolchain) |
| `scripts/deploy_agent_staking.py` | One-shot deploy + register script |
| `scripts/setup_quorum_dids.py` | 16-DID quorum coverage script |
| `docker-compose.yml` | Runs the dapp on the Rubix VM |
| `VM1_SETUP.md` | Full end-to-end Rubix VM setup runbook |
| `scripts/QUORUM_SETUP.md` | Quorum DID strategy explanation |

---

## Architecture

```
Rubix VM (VM1)                               Executor VM (VM2)
├── rubixgoplatform                           └── executor service
│    ├── node API  :20000                          ├── POST /api/execute-smart-contract
│    └── embedded IPFS                             └── GET  <VM1>:6001/activities
│                                                       GET  <VM1>:6001/batch/{hash}
├── agent-staking-dapp  :6001
│    ├── POST /          ← Rubix node callback on every execution
│    ├── GET  /activities
│    ├── GET  /batch/{hash}
│    └── GET  /health
│
└── SQLite  /data/agent_activity.db
```

**How a contract execution flows:**

1. The executor service calls `POST /api/execute-smart-contract` on the Rubix node
2. The node runs quorum consensus and calls `POST http://localhost:6001/` on the dapp
3. The dapp dispatches the call to the WASM module, which reads/writes state via host functions
4. The dapp writes full batch JSON to SQLite; only compact roots (hash + metadata) enter WASM state
5. The node records the result in the blockchain and returns to the executor service

The executor service never talks to the dapp directly for writes — everything goes through the Rubix node. The dapp's `/activities` and `/batch/{hash}` endpoints are read-only and used directly by the executor service for audit queries.

---

## Contract — `agent_staking`

### State model

WASM state is kept lean regardless of activity volume. Full activity records live in the dapp's SQLite database and are accessible via the query API.

| Field | Type | Stored in |
|---|---|---|
| `agent_did` | string | WASM state |
| `agent_name` | string | WASM state |
| `agent_type` | string | WASM state |
| `status` | `"active"` \| `"suspended"` | WASM state |
| `stake_amount` | float (always 1.0) | WASM state |
| `registered_at` | unix ms | WASM state |
| `activity_count` | uint64 | WASM state |
| `last_activity_at` | unix ms | WASM state |
| `reputation_score` | float 0–100 | WASM state |
| `batch_count` | uint64 | WASM state |
| `batch_roots` | array (capped at 500) | WASM state |
| Full batch JSON | blob | SQLite only |
| Individual activity records | rows | SQLite only |

### Contract functions

All calls go through `POST /api/execute-smart-contract` on the Rubix node as `smartContractData` JSON with the shape:

```json
{ "function": "<name>", "params": { ... } }
```

---

#### `register_agent`

Registers the agent DID, name, and type. One-time setup called during deployment. Idempotent — re-calling with the same DID is a no-op and returns current state.

**Request:**
```json
{
  "function": "register_agent",
  "params": {
    "agent_did":   "bafybmi...",
    "agent_name":  "MyAgent",
    "agent_type":  "trading",
    "timestamp":   1700000000000
  }
}
```

`agent_type` must be one of: `trading` | `advisory` | `monitoring` | `general`

`agent_did` becomes the contract owner. Every subsequent write call must supply this same DID — mismatches are rejected with a hard error.

**Response:**
```json
{
  "success": true,
  "message": "Agent bafybmi... registered with 1 RBT stake",
  "data": { "<full AgentState>" }
}
```

---

#### `record_activity_batch`

Primary write path. Commits a batch of activities in a single consensus round. The full batch JSON is stored in dapp SQLite; only a compact root (hash + counters) enters WASM state. Reputation score is recomputed after each commit.

**Request:**
```json
{
  "function": "record_activity_batch",
  "params": {
    "agent_did":        "bafybmi...",
    "batch_hash":       "sha256hex...",
    "activity_count":   42,
    "period_start":     1700000000000,
    "period_end":       1700003600000,
    "dominant_action":  "trade",
    "full_batch_json":  "{\"activities\":[...]}"
  }
}
```

`batch_hash` is the SHA-256 hex of `full_batch_json`, computed by the caller before submission. The contract does not re-hash; it trusts and stores what it receives. Verifiers can recompute the hash from SQLite to confirm integrity.

`dominant_action` is the most frequent `activity_type` string in the batch — used in lightweight on-chain summaries.

**Response:**
```json
{
  "success": true,
  "message": "Batch 'sha256hex...' committed for agent bafybmi.... Total activities: 1042. Reputation: 73.4",
  "data": {
    "batch_hash":       "sha256hex...",
    "batch_count":      25,
    "activity_count":   1042,
    "reputation_score": 73.4,
    "last_activity_at": 1700003600000
  }
}
```

WASM state holds at most 500 batch roots. Older roots roll off the in-memory window but remain permanently in SQLite and are queryable via `/batch/{hash}`.

---

#### `record_activity`

Records a single activity. Intended for low-volume or testing use. For production throughput, prefer `record_activity_batch`.

**Request:**
```json
{
  "function": "record_activity",
  "params": {
    "agent_did":      "bafybmi...",
    "activity_type":  "trade",
    "activity_data":  "{\"pair\":\"BTC/USDT\",\"side\":\"buy\"}",
    "timestamp":      1700000120000,
    "tx_ref":         "txn_abc123"
  }
}
```

`activity_data` is a free-form JSON string. `tx_ref` is optional.

Reputation is recomputed every 10 single records.

---

#### `get_agent_info`

Read-only. Returns full WASM state including all batch roots currently in the 500-entry window.

**Request:**
```json
{
  "function": "get_agent_info",
  "params": {
    "agent_did": "bafybmi..."
  }
}
```

`agent_did` is optional — if omitted, returns state for the registered agent.

**Response:**
```json
{
  "success": true,
  "message": "Agent info retrieved",
  "data": {
    "agent_did":        "bafybmi...",
    "agent_name":       "MyAgent",
    "agent_type":       "trading",
    "status":           "active",
    "stake_amount":     1.0,
    "registered_at":    1700000000000,
    "activity_count":   1042,
    "last_activity_at": 1700003600000,
    "reputation_score": 73.4,
    "batch_count":      25,
    "batch_roots": [
      {
        "batch_hash":      "sha256hex...",
        "activity_count":  42,
        "period_start":    1700000000000,
        "period_end":      1700003600000,
        "committed_at":    1700003600000,
        "dominant_action": "trade"
      }
    ]
  }
}
```

---

#### `update_status`

Sets agent status to `active` or `suspended`. Only the registered agent DID can call this. A suspended agent cannot record activity.

**Request:**
```json
{
  "function": "update_status",
  "params": {
    "agent_did": "bafybmi...",
    "status":    "suspended"
  }
}
```

---

### Reputation scoring

Reputation (0–100) is recomputed after every `record_activity_batch` call and every 10th `record_activity` call. It is a pure function of WASM state plus a single host call for recent activity count — no external oracles.

| Component | Weight | Description |
|---|---|---|
| Velocity | 30 | Actions/day over last 7 days, target 50/day |
| Volume | 40 | log₂ scale, saturates at ~10 000 total actions |
| Recency | 20 | Exponential decay, half-life 3 days |
| Tenure | 10 | Linear ramp, saturates at 30 days |

The score is visible on-chain in WASM state and returned in every write response.

---

## Dapp query API

The dapp exposes three read-only HTTP endpoints used by the executor service for audit queries. These bypass consensus — they read directly from SQLite.

### `GET /activities`

Returns individual activity records stored by `record_activity` calls.

**Query parameters:**

| Parameter | Default | Description |
|---|---|---|
| `from_ts` | `0` | Start of time range (unix ms) |
| `to_ts` | now | End of time range (unix ms) |
| `limit` | `100` | Max rows returned |

**Response:**
```json
{
  "activities": [
    {
      "activity_type": "trade",
      "activity_data": "{\"pair\":\"BTC/USDT\"}",
      "timestamp":     1700000120000,
      "tx_ref":        "txn_abc123"
    }
  ],
  "count": 1
}
```

### `GET /batch/{hash}`

Returns the full JSON payload of a committed batch by its SHA-256 hash.

**Response:** The raw JSON string passed as `full_batch_json` at commit time, or `404` if not found.

### `GET /health`

Returns dapp liveness and current WASM state.

**Response:**
```json
{
  "status":  "ok",
  "db_path": "/data/agent_activity.db",
  "state":   { "<current AgentState or null>" }
}
```

---

## Deployment

See **[VM1_SETUP.md](./VM1_SETUP.md)** for the complete end-to-end Rubix VM setup runbook, covering:

- Node startup and health check
- Deployer DID creation (node-side, password signing)
- Executor key pair generation (secp256k1, self-custody)
- Executor DID registration via `/api/request-did-for-pubkey`
- RBT funding requirements
- Contract deploy + `register_agent` initialisation
- Dapp build and startup via Docker
- Callback URL registration with the Rubix node
- Port firewall rules
- 16-DID quorum coverage setup

### Quick deploy reference

```bash
# On VM1, after node is running and DIDs are funded:
cd $REPO_DIR

RUBIX_NODE_URL=http://localhost:<NODE_PORT> \
DEPLOYER_DID=<deployer bafybmi...> \
AGENT_DID=<executor bafybmi...> \
AGENT_NAME=MyAgent \
AGENT_TYPE=trading \
PASSPHRASE=<deployer password> \
python3 scripts/deploy_agent_staking.py
```

`AGENT_DID` is the executor service's DID — the identity the WASM contract stores as its owner. Every future `record_activity_batch` call must supply this same DID. It must match `AGENT_STAKING_EXECUTOR_DID` in the executor service config.

The script prints the contract address on success:

```
✓ Deployment complete!
  Contract address : Qm...
```

---

## Building from source

The pre-built WASM binary at `artifacts/agent_staking.wasm` is committed to this repo. You only need to rebuild if you modify `agent_staking/src/lib.rs`.

### Prerequisites

```bash
# Rust with WASM target
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add wasm32-unknown-unknown

# rubixwasm-std (local dependency — clone alongside this repo)
git clone https://github.com/rubixchain/rubix-wasm ../rubix-wasm
```

The `Cargo.toml` references `rubixwasm-std` via a relative path. The expected layout:

```
parent/
├── rbt-contracts/       ← this repo
└── rubix-wasm/
    └── packages/
        └── std/
```

### Build

```bash
cd agent_staking
cargo build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/agent_staking.wasm ../artifacts/
```

Commit the updated `artifacts/agent_staking.wasm` and redeploy — each deploy produces a new contract address.

---

## Quorum node setup

See **[scripts/QUORUM_SETUP.md](./scripts/QUORUM_SETUP.md)** for a full explanation of the 16-DID quorum coverage strategy and how Rubix QuorumType 1 DID selection works.

```bash
# Register one quorum DID per hex suffix (0–f)
RUBIX_NODE_URL=http://localhost:<NODE_PORT> \
PRIV_PWD=<password> \
QUORUM_PWD=<password> \
python3 scripts/setup_quorum_dids.py
```

---

## Environment variable reference

### Dapp (`agent_staking/dapp`)

| Variable | Default | Description |
|---|---|---|
| `RUBIX_NODE_URL` | `http://localhost:20000` | Rubix node API base URL |
| `WASM_PATH` | `../../../artifacts/agent_staking.wasm` | Path to WASM binary |
| `DB_PATH` | `./state/agent_activity.db` | SQLite database path |
| `DAPP_PORT` | `:6001` | Port the dapp listens on |

### Deploy script (`scripts/deploy_agent_staking.py`)

| Variable | Required | Description |
|---|---|---|
| `RUBIX_NODE_URL` | no | Node API URL. Default: `http://localhost:20011` |
| `DEPLOYER_DID` | **yes** | Node-side DID that signs the deploy. Must have ≥ 1 RBT. |
| `AGENT_DID` | **yes** | Executor DID registered as contract owner. Must match executor service config. |
| `PASSPHRASE` | no | Deployer DID password. Default: `mypassword` |
| `AGENT_NAME` | no | Agent name written to on-chain state. Default: `agent` |
| `AGENT_TYPE` | no | Agent type. One of: `trading`, `advisory`, `monitoring`, `general`. Default: `general` |

### Executor service (VM2)

| Variable | Description |
|---|---|
| `AGENT_STAKING_CONTRACT_ADDRESS` | Contract address returned by the deploy script |
| `AGENT_STAKING_EXECUTOR_DID` | Executor DID — must match the DID registered via `register_agent` |
| `AGENT_STAKING_PRIVATE_KEY_HEX` | Executor secp256k1 private key (hex, 64 chars) — signs contract calls |
| `AGENT_STAKING_DAPP_URL` | Dapp base URL for read queries, e.g. `http://<VM1-IP>:6001` |
| `RUBIX_NODE_URL` | Rubix node API URL, e.g. `http://<VM1-IP>:20000` |
