# Quorum DID Setup

## Why this exists

Rubix QuorumType 1 (public pool) selects quorum validators by matching the **last hex character of the transaction ID** against the **last hex character of each candidate DID's multihash digest**.

Transaction IDs are SHA3-256 hashes — their last character is uniformly random across all 16 hex values (`0`–`9`, `a`–`f`). If your quorum node only has one DID, it only gets selected for ~1/16 of all public transactions. To be selected for every transaction, your node needs one DID for each of the 16 possible last hex characters.

This is how your node maximises the number of consensus rounds it participates in, which is directly how credits are earned.

## How credits work

Credits are the quorum node's participation record. For every consensus round a quorum node validates:

1. The node pledges some of its RBT tokens for the duration of the transaction
2. After 7 days, those tokens are unpledged and returned
3. A credit entry is stored against the quorum node's DID

Credit score = number of credit entries. More transactions validated = more credits.

Credits come from two sources:

- **Your own transactions** — unit1440 commits activity batches to the agent_staking smart contract. Your quorum node validates every one of these (via QuorumType 2, which routes directly to your node regardless of DID suffix).
- **External network transactions** — every other Rubix node doing QuorumType 1 transactions will select your quorum node when the transaction ID's last char matches one of your registered DIDs. With 16 DIDs covering all hex chars, you are always in the candidate pool.

The 16-DID setup only affects external traffic. Your own unit1440 transactions are already fully captured via QuorumType 2.

## How DID selection works (QuorumType 1)

When a Rubix node initiates a QuorumType 1 transaction:

1. It computes `lastChar = hex(SHA3-256(contractBlock))[-1]`
2. It queries its local `DIDPeerTable` for all entries where `did_last_char = lastChar`
3. It picks the first 7 results as the quorum

The `did_last_char` stored in the table is computed as:
```
lastChar = hex(CID.multihash_bytes)[-1]
```

This is not the last character of the DID string itself — it is the last hex nibble of the raw multihash bytes inside the CID. You cannot choose this value when creating a DID; it is determined by the hash of the public key. You must generate DIDs until you happen to get one for each of the 16 possible values.

When you call `registerdid`, your DID is broadcast via pubsub to every node on the Rubix network. Each node adds it to its local `DIDPeerTable`. From that point on, any node doing a QuorumType 1 transaction whose last char matches yours will include your node as a quorum candidate.

## What the script does

`setup_quorum_dids.py` automates the following loop:

1. `POST /api/createdid` — creates a new LiteDID (type 4, secp256k1) on the quorum node
2. Computes the DID's last hex char using the same logic as the Go node
3. If this hex char is not yet covered, keeps the DID; otherwise discards it
4. For each keeper DID:
   - `POST /api/setup-quorum` — enables the DID to co-sign as a quorum member
   - `POST /api/register-did` — broadcasts the DID to the entire network via pubsub
5. Stops when all 16 hex chars are covered
6. Writes `quorumlist.json` for use with the `addquorum` command on the unit1440 node

By the coupon collector's problem, covering all 16 chars requires generating an expected ~54 DIDs. The script caps at 200 attempts.

## Usage

```bash
RUBIX_NODE_URL=http://<quorum-node-ip>:20000 \
PRIV_PWD=mypassword \
QUORUM_PWD=mypassword \
python3 scripts/setup_quorum_dids.py
```

This is a one-time operation. Run it once when setting up the quorum node.

## After running

**1. Fund the quorum node with RBT**

The quorum node pledges its own RBT tokens to validate each transaction. Without a balance it cannot participate. Transfer enough RBT to cover concurrent pledge obligations — each transaction requires a pledge proportional to the smart contract's deployed value (1 RBT for agent_staking).

**2. Add the quorum list to the unit1440 initiating node**

```bash
./rubixgoplatform addquorum \
  -quorumList quorumlist.json \
  -port 20000 \
  -grpcPort 10500
```

This tells the unit1440 node to use your quorum DIDs for QuorumType 2 transactions. unit1440's `quorumType` stays as `2` — this gives guaranteed routing to your node for all of unit1440's own transactions.

**3. Verify**

```bash
./rubixgoplatform getallquorum -port 20000 -grpcPort 10500
```

Should list the 16 DIDs you registered.

## Quorum type summary

| Transaction source | quorumType | How your node gets selected |
|---|---|---|
| unit1440 batches | 2 (private) | Always — you are explicitly in the quorum list |
| Rest of Rubix network | 1 (public) | When last char of txn ID matches one of your 16 DIDs |
