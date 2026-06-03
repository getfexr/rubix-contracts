# Quorum Node Setup

## Why we run public validators

Every transaction on the Rubix network achieves finality through a quorum of independent validator nodes. When a Fexr trading agent submits an activity batch, that batch is not confirmed by Fexr's infrastructure alone; it is validated by a set of network peers who have no relationship to us and no interest in the outcome.

Running quorum validators is how we participate in that infrastructure as a first-class network member rather than a passive consumer of it. Our validators are registered on the public Rubix network and are eligible to validate transactions from any node. When we pledge RBT to validate a transaction, we are economically accountable for correct behaviour. Validators that fail to perform are excluded from future selection.

This is the trust guarantee: the on-chain record of an agent's activity exists because independent network participants confirmed it, not because Fexr wrote it.

---

## What the script does

`setup_quorum_dids.py` registers validator DIDs on the node and broadcasts them to the Rubix network:

1. `POST /api/createdid`: creates a new LiteDID (type 4, secp256k1) on the node
2. `POST /api/setup-quorum`: enables the DID to co-sign as a quorum member
3. `POST /api/register-did`: broadcasts the DID to the network via pubsub
4. Writes `quorumlist.json` for use with the `addquorum` command

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

---

## Quorum type summary

| Transaction type | quorumType | Validator selection |
|---|---|---|
| Agent activity batches | 2 (configured list) | Validators from the pre-registered quorum list |
| Public network transactions | 1 (public pool) | Node selected based on DID registration in the network peer table |
