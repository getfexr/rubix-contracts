# Testing Guide

Rubix contracts should be accompanied by automated tests using the Rubix SDK or any available local runtime.

## Minimum Tests

- Unit tests covering minting, metadata updates, and permission checks.
- Integration tests that simulate the off-chain listener calling Rubix after an EVM harvest.
- Serialization tests to ensure CIDs and yield identifiers are encoded consistently.

## Suggested Workflow

```bash
# Install dependencies
npm install

# Run unit tests (placeholder command)
npm test
```

Adjust the commands once the actual toolchain is finalised.

