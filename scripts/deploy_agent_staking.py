#!/usr/bin/env python3
"""
Deploy agent_staking smart contract to a Rubix testnet node.

Usage:
    RUBIX_NODE_URL=http://localhost:20011 \
    DEPLOYER_DID=bafybmi... \
    PASSPHRASE=mypassword \
    python3 deploy_agent_staking.py

The deployer DID must already exist on the node (created via /api/createdid
or /api/request-did-for-pubkey). The DID must have at least 1 RBT balance.

Prints the deployed contract address on success. Store it as
AGENT_STAKING_CONTRACT_ADDRESS in fexrapi's .env.
"""

import json
import os
import sys
import time

import requests

NODE_URL = os.getenv("RUBIX_NODE_URL", "http://localhost:20011")
DEPLOYER_DID = os.getenv("DEPLOYER_DID", "")
PASSPHRASE = os.getenv("PASSPHRASE", "mypassword")
# AGENT_DID: the DID that will own the contract state and send all future batches.
# This is fexrapi's AGENT_STAKING_EXECUTOR_DID — the one whose private key hex
# fexrapi holds. Defaults to DEPLOYER_DID only if not set, which is wrong for
# production (deployer and executor are always different DIDs).
AGENT_DID = os.getenv("AGENT_DID", "") or DEPLOYER_DID

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WASM_FILE = os.path.join(REPO_ROOT, "artifacts", "agent_staking.wasm")
CODE_FILE = os.path.join(REPO_ROOT, "agent_staking", "src", "lib.rs")


def check_prerequisites():
    if not DEPLOYER_DID:
        print("ERROR: Set DEPLOYER_DID env var to the deployer's Rubix DID (bafybmi...)")
        sys.exit(1)
    if not os.path.exists(WASM_FILE):
        print(f"ERROR: WASM artifact not found at {WASM_FILE}")
        print("Build first: cd agent_staking && cargo build --target wasm32-unknown-unknown --release")
        print("Then copy:   cp target/wasm32-unknown-unknown/release/agent_staking.wasm ../../artifacts/")
        sys.exit(1)


def generate_sc_token(did: str) -> str:
    """
    POST /api/generate-smart-contract (multipart)
    Uploads WASM + source to IPFS, returns the contract address (IPFS CID).
    """
    print("  Uploading WASM + source to node IPFS...")
    with open(WASM_FILE, "rb") as wf, open(CODE_FILE, "rb") as cf:
        resp = requests.post(
            f"{NODE_URL}/api/generate-smart-contract",
            data={"did": did},
            files={
                "binaryCodePath": (os.path.basename(WASM_FILE), wf, "application/octet-stream"),
                "rawCodePath": (os.path.basename(CODE_FILE), cf, "text/plain"),
            },
            timeout=120,
        )
    resp.raise_for_status()
    data = resp.json()
    if not data.get("status"):
        print(f"ERROR: generate-smart-contract failed: {data.get('message')}")
        sys.exit(1)
    contract_address = data["result"]
    print(f"  Contract address: {contract_address}")
    return contract_address


def deploy_sc_token(deployer_did: str, contract_address: str) -> dict:
    """
    POST /api/deploy-smart-contract
    Locks 1 RBT in the genesis block. Uses password-based signing (non-LiteDID mode).
    """
    print("  Deploying contract (locking 1 RBT)...")
    payload = {
        "deployerAddr": deployer_did,
        "smartContractToken": contract_address,
        "rbtAmount": 1.0,
        "quorumType": 2,
        "comment": "agent_staking initial deploy",
    }
    resp = requests.post(
        f"{NODE_URL}/api/deploy-smart-contract",
        json=payload,
        timeout=300,
    )
    resp.raise_for_status()
    data = resp.json()
    # For password-based DID, the node may need a signature response
    if not data.get("status") and "result" not in data:
        print(f"ERROR: deploy failed: {data.get('message')}")
        sys.exit(1)
    return data


def submit_signature(request_id: str) -> dict:
    """Submit password-based signature response (mode 0)."""
    print(f"  Submitting signature for request {request_id}...")
    payload = {
        "id": request_id,
        "mode": 0,
        "password": PASSPHRASE,
        "signature": {"Pixels": "", "Signature": ""},
    }
    resp = requests.post(
        f"{NODE_URL}/api/signature-response",
        json=payload,
        timeout=300,
    )
    resp.raise_for_status()
    return resp.json()


def register_agent(deployer_did: str, contract_address: str, agent_did: str) -> None:
    """
    Initialise the WASM contract state by registering the agent DID.

    deployer_did: signs the transaction (has a node-side key, uses mode-0 signing).
    agent_did:    the DID registered inside the WASM as the contract owner.
                  Must match AGENT_STAKING_EXECUTOR_DID on fexrapi — every future
                  record_activity_batch call checks state.agent_did == req.agent_did.
    """
    print(f"  Registering agent {agent_did}...")
    sc_data = json.dumps({
        "function": "register_agent",
        "params": {
            "agent_did": agent_did,
            "agent_name": "unit1440",
            "agent_type": "trading",
            "timestamp": int(time.time() * 1000),
        }
    })
    payload = {
        "executorAddr": deployer_did,
        "smartContractToken": contract_address,
        "smartContractData": sc_data,
        "quorumType": 2,
        "comment": "register agent",
    }
    resp = requests.post(
        f"{NODE_URL}/api/execute-smart-contract",
        json=payload,
        timeout=300,
    )
    resp.raise_for_status()
    data = resp.json()
    result = data.get("result", {})
    if result.get("id"):
        node_resp = submit_signature(result["id"])
        if not node_resp.get("status"):
            print(f"  WARNING: register_agent execution may have failed: {node_resp.get('message')}")
        else:
            print("  Agent registered successfully.")


def main():
    check_prerequisites()

    print(f"=== Deploying agent_staking contract ===")
    print(f"  Node    : {NODE_URL}")
    print(f"  Deployer: {DEPLOYER_DID}")
    print(f"  Agent   : {AGENT_DID}")
    print(f"  WASM    : {WASM_FILE}")
    print()

    # Step 1: upload to IPFS → get contract address
    contract_address = generate_sc_token(DEPLOYER_DID)

    # Step 2: deploy (lock 1 RBT)
    deploy_resp = deploy_sc_token(DEPLOYER_DID, contract_address)
    result = deploy_resp.get("result", {})
    if isinstance(result, dict) and result.get("id"):
        # Signature required
        final = submit_signature(result["id"])
        if not final.get("status"):
            print(f"ERROR: deploy consensus failed: {final.get('message')}")
            sys.exit(1)
    print("  Deploy committed.")

    # Step 3: execute register_agent to initialise state.
    # DEPLOYER_DID signs (it has a node-side key for mode-0 signing).
    # AGENT_DID is the identity registered in the WASM contract — must match
    # what fexrapi sends in every future record_activity_batch call.
    register_agent(DEPLOYER_DID, contract_address, AGENT_DID)

    print()
    print("✓ Deployment complete!")
    print(f"  Contract address : {contract_address}")
    print(f"  Deployer DID     : {DEPLOYER_DID}")
    print(f"  Agent DID        : {AGENT_DID}")
    print()
    print("Add to fexrapi + unit1440 .env on VM2:")
    print(f"  AGENT_STAKING_CONTRACT_ADDRESS={contract_address}")
    print(f"  AGENT_STAKING_EXECUTOR_DID={AGENT_DID}")
    print(f"  RUBIX_CONTRACT_ADDRESS={contract_address}")
    print(f"  RUBIX_AGENT_DID={AGENT_DID}")


if __name__ == "__main__":
    main()
