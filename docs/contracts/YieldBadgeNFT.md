# Yield Badge NFT

`contracts/YieldBadgeNFT.rbx` (placeholder)

## Purpose

Mints non-fungible tokens on Rubix representing a club member’s yield contribution harvested from the EVM vault. Each NFT references IPFS metadata generated from `YieldHarvested` events on the vault.

## Key Features

- **Mint-on-demand** when a yield share is credited to Fexr.
- **Metadata Binding** via CID stored in the token URI and echoed in the EVM vault’s `yieldMetadata` mapping.
- **Non-transferable Option** (configurable) to keep proof-of-yield bound to initial recipient.
- **Batch Minting** to handle multiple yield events in a single Rubix transaction.

## Workflow

1. Listener service observes `YieldHarvested` events on the EVM vault.
2. Metadata JSON compiled (club name, period, gross yield, fee paid, tx hash).
3. Upload metadata to IPFS and compute CID hash.
4. Call Rubix contract `mintYieldBadge(recipient, cid, yieldId)` with proof.
5. Optionally send CID hash back to vault via `setYieldMetadata`.

## Events

- `YieldBadgeMinted(address recipient, string cid, bytes32 yieldId)`
- `MetadataUpdated(bytes32 yieldId, string cid)`

## Testing Checklist

- Minting with valid CID and yieldId.
- Preventing duplicate mint for same `yieldId`.
- Optional restriction on transfers (if non-transferable).
- Permissioning for trusted minter service.

> Update file paths and function names once the contract is implemented.

