# Fexr Rubix Contracts

Rubix-chain contracts backing the Fexr yield badge NFT workflow.

👉 Detailed documentation lives in [`docs/index.md`](docs/index.md).

## Repository Goals

- Maintain NFT minting contracts that mirror yield events from the EVM vault.
- Provide deployment scripts/configuration for Rubix testnet and mainnet.
- Keep documentation and metadata schemas aligned with off-chain services.

## Quick Start

```bash
# Install dependencies (placeholder)
npm install

# Compile contracts
rubix compile

# Run tests
npm test
```

Update the commands once the tooling is finalised.

## Related Repositories

- `../evm-contracts` – EVM vaults emitting `YieldHarvested` events.
- `../solana-contracts` – Solana programs for proof infrastructure.

## Contributing

1. Create a feature branch.
2. Add/adjust documentation under `docs/`.
3. Provide tests for contract changes.
4. Open a merge request referencing relevant tasks.

## License

MIT (unless superseded by project-wide license).

