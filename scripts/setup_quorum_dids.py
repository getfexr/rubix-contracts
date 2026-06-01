#!/usr/bin/env python3
"""
Register 16 quorum DIDs (one per hex suffix 0-f) on a Rubix node.

Rubix QuorumType 1 selects quorum members by matching the last hex character
of the transaction ID against the last hex character of each DID's multihash
digest. Covering all 16 hex chars ensures this node is always in the candidate
pool for every public transaction on the network.

Usage:
    RUBIX_NODE_URL=http://<quorum-node>:20000 \
    PRIV_PWD=mypassword \
    QUORUM_PWD=mypassword \
    python3 setup_quorum_dids.py

Outputs a quorumlist.json you can pass to unit1440's addquorum command.
"""

import base64
import json
import os
import sys
import time

import requests

NODE_URL = os.getenv("RUBIX_NODE_URL", "http://localhost:20000")
PRIV_PWD  = os.getenv("PRIV_PWD",  "mypassword")
QUORUM_PWD = os.getenv("QUORUM_PWD", "mypassword")
MAX_ATTEMPTS = 200  # coupon collector: expected ~54 to cover all 16 chars


# ---------------------------------------------------------------------------
# DID last-char computation — replicates rubixgoplatform/core/wallet/did.go
# GetLastChar: decode CIDv1 base32lower → multihash bytes → hex → last char
# ---------------------------------------------------------------------------

def _read_varint(data: bytes, offset: int):
    result, shift = 0, 0
    while True:
        b = data[offset]; offset += 1
        result |= (b & 0x7F) << shift
        shift += 7
        if not (b & 0x80):
            break
    return offset, result


def did_last_char(did: str) -> str:
    # strip multibase prefix 'b', convert base32lower → standard base32
    b32 = did[1:].upper()
    pad = (8 - len(b32) % 8) % 8
    raw = base64.b32decode(b32 + "=" * pad)
    # skip version and codec varints to reach the raw multihash
    offset, _ = _read_varint(raw, 0)   # version
    offset, _ = _read_varint(raw, offset)  # codec
    multihash_hex = raw[offset:].hex()
    return multihash_hex[-1]


# ---------------------------------------------------------------------------
# Rubix node API calls
# ---------------------------------------------------------------------------

def create_did() -> str:
    """POST /api/createdid — creates a new LiteDID on the node."""
    did_config = json.dumps({
        "type": 4,          # LiteDIDMode
        "priv_pwd": PRIV_PWD,
        "config": "",
    })
    resp = requests.post(
        f"{NODE_URL}/api/createdid",
        data={"did_config": did_config},
        timeout=60,
    )
    resp.raise_for_status()
    data = resp.json()
    if not data.get("status"):
        raise RuntimeError(f"createdid failed: {data.get('message')}")
    did = data.get("result", {}).get("did") or data.get("DID") or data.get("did")
    if not did:
        raise RuntimeError(f"no DID in response: {data}")
    return did


def setup_quorum(did: str) -> None:
    """POST /api/setup-quorum — enables the DID to sign as quorum member."""
    resp = requests.post(
        f"{NODE_URL}/api/setup-quorum",
        json={"did": did, "password": QUORUM_PWD, "priv_password": PRIV_PWD},
        timeout=30,
    )
    resp.raise_for_status()
    data = resp.json()
    if not data.get("status"):
        raise RuntimeError(f"setup-quorum failed for {did}: {data.get('message')}")


def register_did(did: str) -> None:
    """POST /api/register-did — broadcasts DID to the network via pubsub."""
    resp = requests.post(
        f"{NODE_URL}/api/register-did",
        json={"did": did},
        timeout=60,
    )
    resp.raise_for_status()
    data = resp.json()
    if not data.get("status"):
        raise RuntimeError(f"register-did failed for {did}: {data.get('message')}")


def get_peer_id() -> str:
    """GET /api/getalldid — also returns peerID in response."""
    resp = requests.get(f"{NODE_URL}/api/getalldid", timeout=10)
    resp.raise_for_status()
    data = resp.json()
    return data.get("peer_id") or data.get("PeerID") or ""


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    print(f"Quorum DID registration — node: {NODE_URL}")
    print()

    # Check node is up
    try:
        requests.get(f"{NODE_URL}/api/getalldid", timeout=5).raise_for_status()
    except Exception as e:
        print(f"ERROR: node unreachable at {NODE_URL}: {e}")
        sys.exit(1)

    peer_id = get_peer_id()

    all_hex = set("0123456789abcdef")
    covered: dict[str, str] = {}   # hex_char → did
    attempts = 0

    print(f"Generating DIDs until all 16 hex suffixes (0-f) are covered...")
    print(f"(Expected ~54 DIDs; max {MAX_ATTEMPTS})")
    print()

    while len(covered) < 16 and attempts < MAX_ATTEMPTS:
        attempts += 1
        try:
            did = create_did()
        except Exception as e:
            print(f"  attempt {attempts}: createdid error: {e}")
            time.sleep(2)
            continue

        char = did_last_char(did)
        if char in covered:
            print(f"  attempt {attempts}: {did} → '{char}' (already covered, skipping)")
            continue

        print(f"  attempt {attempts}: {did} → '{char}' ✓ NEW")
        covered[char] = did

        try:
            setup_quorum(did)
            print(f"    setup-quorum OK")
        except Exception as e:
            print(f"    setup-quorum FAILED: {e}")

        try:
            register_did(did)
            print(f"    register-did OK")
        except Exception as e:
            print(f"    register-did FAILED: {e}")

    print()
    missing = all_hex - set(covered.keys())
    if missing:
        print(f"WARNING: could not cover hex chars: {sorted(missing)} after {MAX_ATTEMPTS} attempts")
    else:
        print(f"All 16 hex chars covered in {attempts} attempts.")

    print()
    print("=== Covered DIDs ===")
    quorum_list = []
    for char in sorted(covered.keys()):
        did = covered[char]
        addr = f"{peer_id}.{did}" if peer_id else did
        print(f"  [{char}]  {did}")
        quorum_list.append({"type": 2, "address": did})

    # Write quorumlist.json for addquorum command on the initiating node
    out_path = os.path.join(os.path.dirname(__file__), "quorumlist.json")
    with open(out_path, "w") as f:
        json.dump(quorum_list, f, indent=2)
    print()
    print(f"quorumlist.json written to: {out_path}")
    print()
    print("Next steps:")
    print(f"  1. Fund the quorum node with RBT (it pledges tokens per transaction)")
    print(f"  2. On the unit1440 initiating node, run:")
    print(f"       ./rubixgoplatform addquorum -quorumList quorumlist.json -port 20000 -grpcPort 10500")
    print(f"  3. unit1440's quorumType stays 2 — it will use these DIDs exclusively")


if __name__ == "__main__":
    main()
