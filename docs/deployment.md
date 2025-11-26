# Deployment Guide

## Prerequisites

- Rubix CLI installed and authenticated.
- Minter service keypair available and funded.
- IPFS pinning service configured (Pinata, web3.storage, etc.).
- Mapping between EVM vault addresses and Rubix minter contract recorded.

## Steps

1. **Compile Contracts**
   ```bash
   rubix compile
   ```
2. **Deploy to Test Network**
   ```bash
   rubix deploy --network testnet contracts/YieldBadgeNFT.rbx
   ```
3. **Initialize**
   - Set trusted minter address (Fexr service).
   - Configure base URI if needed.
4. **Smoke Test**
   - Mint a badge to a test wallet.
   - Verify metadata loads via IPFS.
5. **Deploy to Mainnet**
   ```bash
   rubix deploy --network mainnet contracts/YieldBadgeNFT.rbx
   ```
6. **Record**
   - Update `docs/changelog.md`.
   - Sync address with backend configuration and `FexrClubVault` metadata store.

## Post-Deployment

- Monitor mint events.
- Ensure cross-chain listener writes CID hashes back to EVM vault.
- Rotate minter keys when operational policies require.

