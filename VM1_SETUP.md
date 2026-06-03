# VM1 (Rubix Node) — End-to-End Setup

This runbook covers everything that needs to happen on the Rubix VM to bring
the full on-chain audit pipeline live. Run these steps in order.

**Variables used throughout** — substitute your real values:
```
NODE_PORT=21000
GRPC_PORT=10500
VM2_IP=<fexr vm private ip>
NODE_DIR=<path where rubixgoplatform data lives, e.g. /home/user/node1>
REPO_DIR=<where you clone rbt-contracts, e.g. /home/user/rbt-contracts>
PRIV_PWD=mypassword      # password for node-side DID private key
QUORUM_PWD=mypassword    # password for quorum signing
```

---

## 0. Prerequisites

Install these before anything else:

```bash
# Docker + Docker Compose (for the dapp)
curl -fsSL https://get.docker.com | sh
apt-get install -y docker-compose-plugin   # or: pip install docker-compose

# Python 3 + pip packages (for the scripts)
apt-get install -y python3 python3-pip
pip3 install requests cryptography
```

The `rubixgoplatform` binary must already be present on this VM. If not,
download it from the Rubix releases or build from source.

---

## 1. Start the Rubix node

If the node is not already running, start it now. rubixgoplatform manages its
own embedded IPFS node — there is no separate IPFS process to start.

```bash
cd $NODE_DIR

./rubixgoplatform run \
  -port $NODE_PORT \
  -grpcPort $GRPC_PORT \
  -testNet &

# Wait for it to be ready (IPFS bootstrap takes ~30s on first run)
sleep 30

# Confirm the node is up
curl -s http://localhost:$NODE_PORT/api/ping | python3 -m json.tool
# Expected: {"message":"Node is running","status":true}
```

If `/api/ping` doesn't respond within 60 seconds, check the node logs before
proceeding — all subsequent steps require a live node.

---

## 2. Clone the repo

```bash
git clone git@gitlab.com:fexr.club/dev/rubix-contracts.git $REPO_DIR
cd $REPO_DIR
```

The WASM artifact is already committed at `artifacts/agent_staking.wasm`.
No Rust toolchain needed on this VM.

No Go toolchain needed either — the dapp is built entirely inside Docker
(the Dockerfile clones its own dependencies at build time).

---

## 3. Create the deployer DID (node-side, one-time only)

The deployer DID is used only to deploy the contract and lock 1 RBT. The node
holds its private key so the deploy script can sign automatically (mode 0).

```bash
cd $NODE_DIR

./rubixgoplatform createdid \
  -privPWD $PRIV_PWD \
  -quorumPWD $QUORUM_PWD \
  -port $NODE_PORT \
  -grpcPort $GRPC_PORT
```

Note the DID printed — it starts with `bafybmi`. Call it `DEPLOYER_DID`.

---

## 4. Generate the executor key pair (executor service uses this for ongoing signing)

The executor DID is used by the executor service on VM2 to sign every smart contract
execution. Its private key must never be on the Rubix node — the executor service signs
the hash directly (mode 4 / LiteDID). Generate a fresh secp256k1 key pair:

```bash
python3 - <<'EOF'
from cryptography.hazmat.primitives.asymmetric.ec import generate_private_key, SECP256K1
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat
from cryptography.hazmat.backends import default_backend

key = generate_private_key(SECP256K1(), default_backend())

priv_hex = format(key.private_numbers().private_value, '064x')
pub_hex = key.public_key().public_bytes(Encoding.X962, PublicFormat.UncompressedPoint).hex()

print(f"PRIVATE KEY HEX:\n{priv_hex}\n")
print(f"PUBLIC KEY HEX:\n{pub_hex}\n")
EOF
```

Save both values. The private key hex **goes to VM2 only** — never store it on
this VM. The public key hex is used in the next step.

---

## 5. Register the executor public key as a DID

```bash
curl -s -X POST http://localhost:$NODE_PORT/api/request-did-for-pubkey \
  -H "Content-Type: application/json" \
  -d "{\"public_key\": \"<PUBLIC_KEY_HEX>\"}" | python3 -m json.tool
```

Note the `did` field in the response — call it `EXECUTOR_DID`.

This DID is what the WASM contract stores as its "owner". It must match
`AGENT_STAKING_EXECUTOR_DID` in your executor service `.env` — every future
`record_activity_batch` call is validated against it.

---

## 6. Fund both DIDs with RBT

The deployer DID needs ≥ 1 RBT to lock in the contract genesis block plus
gas for the deploy consensus round.

The executor DID needs enough RBT to cover concurrent pledge obligations.
Each agent batch execution pledges `contract_value / 5 = 0.2 RBT` per
quorum member per transaction, locked for 7 days before returning. At one
batch per 5 minutes, steady-state locked RBT ≈ 2016 × 0.2 = ~400 RBT across
the quorum pool. Fund conservatively — start with at least 50 RBT on the
executor DID and top up as needed.

Transfer RBT to both DIDs from wherever your existing balance is held.

---

## 7. Deploy the smart contract

```bash
cd $REPO_DIR

RUBIX_NODE_URL=http://localhost:$NODE_PORT \
DEPLOYER_DID=<DEPLOYER_DID> \
AGENT_DID=<EXECUTOR_DID> \
PASSPHRASE=$PRIV_PWD \
python3 scripts/deploy_agent_staking.py
```

`AGENT_DID` must be set to `EXECUTOR_DID` (from step 5). This is the DID
the WASM contract stores as its owner. If you omit it, it defaults to
`DEPLOYER_DID`, which will cause every future batch submission to fail with a
DID mismatch error.

This script:
1. Uploads `artifacts/agent_staking.wasm` + source to IPFS via the node
2. Deploys (locks 1 RBT, runs consensus)
3. Calls `register_agent` to initialise on-chain state with `AGENT_DID`
4. Prints the contract address (a `Qm...` IPFS CID)

**Save the contract address** — it goes into your executor service and agent service env vars on VM2.

---

## 8. Build and start the dapp

The dapp is the WASM runtime + SQLite that the Rubix node calls on every
contract execution. Docker builds it without any local Go toolchain or manual
vendoring — the Dockerfile clones the required dependency automatically.

```bash
cd $REPO_DIR

RUBIX_NODE_URL=http://localhost:$NODE_PORT docker compose up -d --build

# Verify it's healthy (allow 30s for startup)
sleep 30
curl -s http://localhost:6001/health | python3 -m json.tool
# Expected: {"status":"ok",...}
```

---

## 9. Register the callback URL

Tell the Rubix node to POST to the dapp on every execution of this contract:

```bash
curl -s -X POST http://localhost:$NODE_PORT/api/register-callback-url \
  -H "Content-Type: application/json" \
  -d "{\"SmartContractToken\": \"<CONTRACT_ADDRESS>\", \"CallBackURL\": \"http://localhost:6001\"}" \
  | python3 -m json.tool
```

---

## 10. Open port 6001 to VM2

The executor service on VM2 reads activity history and state from the dapp directly.
Allow inbound TCP 6001 from VM2's IP:

```bash
# ufw
ufw allow from $VM2_IP to any port 6001 proto tcp

# or iptables
iptables -A INPUT -s $VM2_IP -p tcp --dport 6001 -j ACCEPT
```

Also confirm VM2 can already reach the Rubix node API on port $NODE_PORT.

---

## 11. Register 16 quorum DIDs

This step registers one quorum DID per hex suffix (`0`–`f`) so this node is
always in the QuorumType 1 candidate pool for every transaction on the
Rubix network. See `scripts/QUORUM_SETUP.md` for the full explanation.

```bash
cd $REPO_DIR

RUBIX_NODE_URL=http://localhost:$NODE_PORT \
PRIV_PWD=$PRIV_PWD \
QUORUM_PWD=$QUORUM_PWD \
python3 scripts/setup_quorum_dids.py
```

This generates `scripts/quorumlist.json` when done. Register it with the node:

```bash
cp $REPO_DIR/scripts/quorumlist.json $NODE_DIR/

cd $NODE_DIR
./rubixgoplatform addquorum \
  -quorumList quorumlist.json \
  -port $NODE_PORT \
  -grpcPort $GRPC_PORT
```

Verify:
```bash
./rubixgoplatform getallquorum -port $NODE_PORT -grpcPort $GRPC_PORT
```

Should list 16 DIDs.

---

## 12. Fund the quorum node

The quorum DIDs all live on this node and pledge from its shared RBT pool.
With 16 DIDs across all hex suffixes, this node participates in a large
fraction of public network traffic. Ensure there is enough RBT in the pool
to cover concurrent pledges — start with 100+ RBT and monitor.

---

## Done — values to send to VM2

Once all the above is complete, share the following with whoever manages
the VM2 `.env` files:

```
# executor service .env
AGENT_STAKING_CONTRACT_ADDRESS=<Qm... from step 7>
AGENT_STAKING_EXECUTOR_DID=<EXECUTOR_DID from step 5>
AGENT_STAKING_PRIVATE_KEY_HEX=<64-char hex from step 4>
AGENT_STAKING_DAPP_URL=http://<VM1_IP>:6001
RUBIX_NODE_URL=http://<VM1_IP>:$NODE_PORT

# agent service .env
RUBIX_CONTRACT_ADDRESS=<same contract address>
RUBIX_AGENT_DID=<same EXECUTOR_DID>
```

No rebuilds needed on VM2 — env var changes alone activate everything.
Restart your services after updating `.env`.

---

## Quick-reference: ports that must be reachable from VM2

| Port | Service | Direction |
|------|---------|-----------|
| $NODE_PORT | Rubix node API | VM2 → VM1 |
| 6001 | agent-staking-dapp | VM2 → VM1 |
