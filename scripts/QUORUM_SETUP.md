# Quorum Node Setup

## Why we run public validators

Every transaction on the Rubix network achieves finality through a quorum of independent validator nodes. When a Fexr trading agent submits an activity batch, that batch is not confirmed by Fexr's infrastructure alone; it is validated by a set of network peers who have no relationship to us and no interest in the outcome.

Running quorum validators is how we participate in that infrastructure as a first-class network member rather than a passive consumer of it. Our validators are registered on the public Rubix network and are eligible to validate any transaction from any node. When we pledge RBT to validate a transaction, we are economically accountable for correct behaviour. Validators that fail to perform are excluded from future selection.

This is the trust guarantee: the on-chain record of an agent's activity exists because independent network participants confirmed it, not because Fexr wrote it.

---

## How QuorumType 1 validator selection works

When a Rubix node initiates a public (QuorumType 1) transaction, it selects validators as follows:

1. It computes `lastChar = hex(SHA3-256(contractBlock))[-1]`
2. It queries its local `DIDPeerTable` for all entries where `did_last_char = lastChar`
3. It selects the first 7 results as the quorum for that transaction

The `did_last_char` value is derived as:

```
lastChar = hex(CID.multihash_bytes)[-1]
```

This is not the last character of the DID string itself. It is the last hex nibble of the raw multihash bytes inside the CID, determined by the hash of the public key at DID creation time. The value cannot be predicted or chosen in advance; DIDs must be generated until one with each of the 16 possible values is obtained.

When `registerdid` is called, the DID is broadcast via pubsub to every node on the Rubix network. Each node adds it to its local `DIDPeerTable`. From that point forward, any public transaction whose last hex character matches a registered DID will include this node as a validator candidate.

---

## The 16-DID requirement

Transaction IDs are SHA3-256 hashes. Their last hex character is uniformly distributed across all 16 values (0 through f). A node with a single DID is eligible to validate approximately 1 in 16 public transactions. A node with one DID for each of the 16 possible suffix values is eligible to validate any public transaction on the network.

The 16-DID configuration is the minimum needed for full participation in the public validator pool.

---

## What the script does

`setup_quorum_dids.py` automates the registration process:

1. `POST /api/createdid`: creates a new LiteDID (type 4, secp256k1) on the node
2. Computes the DID's last hex char using the same logic as the Go node
3. Retains the DID if its suffix value is not yet covered; discards it otherwise
4. For each retained DID:
   - `POST /api/setup-quorum`: enables the DID to co-sign as a quorum member
   - `POST /api/register-did`: broadcasts the DID to the network via pubsub
5. Stops when all 16 suffix values are covered
6. Writes `quorumlist.json` for use with the `addquorum` command

By the coupon collector's problem, covering all 16 values requires generating an expected ~54 DIDs. The script caps at 200 attempts.

---

## Usage

```bash
RUBIX_NODE_URL=http://<quorum-node-ip>:20000 \
PRIV_PWD=mypassword \
QUORUM_PWD=mypassword \
python3 scripts/setup_quorum_dids.py
```

This is a one-time operation per node.

---

## After running

**1. Fund the quorum node with RBT**

The quorum node pledges its own RBT tokens during each validation round. Without sufficient balance it cannot participate. Transfer enough RBT to cover concurrent pledge obligations; each transaction requires a pledge proportional to the smart contract's deployed value (1 RBT for agent_staking).

**2. Register the quorum list with the initiating node**

```bash
./rubixgoplatform addquorum \
  -quorumList quorumlist.json \
  -port 20000 \
  -grpcPort 10500
```

**3. Verify**

```bash
./rubixgoplatform getallquorum -port 20000 -grpcPort 10500
```

Should list 16 DIDs.

---

## Quorum type summary

| Transaction type | quorumType | Validator selection |
|---|---|---|
| Agent activity batches | 2 (configured list) | Validators from the pre-registered quorum list |
| Public network transactions | 1 (public pool) | Node selected when transaction ID suffix matches a registered DID |
