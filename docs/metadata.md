# Metadata Schema

Yield badge NFTs reference IPFS metadata describing the harvest event that triggered the mint.

## JSON Structure

```json
{
  "name": "Fexr Yield Badge #<yieldId>",
  "description": "Represents 10% yield share for Club <clubId> harvested on <timestamp>.",
  "external_url": "https://app.fexr.club/clubs/<clubId>/yield/<yieldId>",
  "attributes": [
    { "trait_type": "Club", "value": "<clubName>" },
    { "trait_type": "Vault", "value": "<vaultAddress>" },
    { "trait_type": "Gross Yield", "value": "<grossYieldUSDC>" },
    { "trait_type": "Fee Paid", "value": "<feePaidUSDC>" },
    { "trait_type": "Harvest Tx", "value": "<txHash>" }
  ]
}
```

## Encoding Rules

- Use fixed decimal strings for USDC amounts (6 decimals).
- Lowercase addresses.
- CID (IPFS) stored in vault via keccak256 hash (fits in `bytes32`) and replicated here as full string.

## Cross-Chain Linkage

1. EVM vault emits `YieldHarvested`.
2. Listener builds metadata JSON, uploads to IPFS.
3. Rubix contract mints NFT with CID.
4. Listener calls `setYieldMetadata` on the EVM vault with hashed CID.

Maintain a registry linking `yieldId` to `(cid, rubixTokenId)` in the backend database for audit.

