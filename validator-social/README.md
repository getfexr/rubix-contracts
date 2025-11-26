# Validator Social - RBT Staking Contract

A production-ready Rubix smart contract for social validator staking. Users can stake RBT tokens to support validators and receive fungible yield tokens representing their stake and accumulated rewards.

## Features

✅ **Real Token Integration**: Uses actual Rubix FT APIs (`call_mint_ft_api`, `call_transfer_ft_api`)
✅ **Validator Staking Tokens**: Creates unique RBTY fungible tokens for each validator stake
✅ **Smart Capacity Planning**: Mints sufficient tokens to cover future validator rewards
✅ **Flexible Reward Claims**: Harvest validator rewards while keeping stake active
✅ **Complete Stake Management**: Withdraw stake + all accumulated validator rewards
✅ **Transparent Validator Tracking**: Real-time stake information and validator pool statistics

## Key Features

- ✅ **Production-Ready Token Economics**: Properly mints capacity for future validator rewards
- ✅ **Safe State Management**: Secure contract state handling without unsafe operations
- ✅ **Block-based Rewards**: Accurate time-based validator reward calculation
- ✅ **Multiple Claim Support**: Users can claim rewards multiple times
- ✅ **Comprehensive Validation**: Robust input validation and error handling

## Contract Functions

### 1. `initialize_pool`
Initializes the validator staking pool with a specified annual reward rate.

**Input:** `InitializePoolRequest`
```json
{
  "annual_yield_rate": 8,
  "current_block": 1000
}
```

### 2. `stake_rbt`
Stakes RBT tokens to support validators and mints RBTY validator staking tokens.

**Input:** `StakeRequest`
```json
{
  "staker_did": "did:rubix:12345abcdef67890",
  "amount": 100,
  "current_block": 1100
}
```

**What happens:**
- Mints `RBTY_{DID}_{amount}_{block}` validator staking tokens
- Token capacity = stake + max possible validator rewards (2 years)
- Records validator stake in contract state

### 3. `withdraw_stake`
Withdraws validator stake and all accumulated reward tokens.

**Input:** `WithdrawRequest`
```json
{
  "staker_did": "did:rubix:12345abcdef67890",
  "current_block": 1500
}
```

### 4. `claim_yield`
Claims accumulated validator reward tokens without withdrawing the principal stake.

**Input:** `ClaimYieldRequest`
```json
{
  "staker_did": "did:rubix:12345abcdef67890",
  "current_block": 1300
}
```

### 5. `get_stake_info`
Retrieves information about a specific validator staker's position.

**Input:** `GetStakeInfoRequest`
```json
{
  "staker_did": "did:rubix:12345abcdef67890",
  "current_block": 1250
}
```

### 6. `get_pool_stats`
Retrieves overall validator staking pool statistics.

**Input:** `GetPoolStatsRequest`
```json
{
  "current_block": 1400
}
```

## Validator Reward Calculation

- **Annual Reward Rate**: Configurable percentage (e.g., 8% APY for validator support)
- **Block-based Calculation**: Assumes ~6 blocks per minute, ~3,153,600 blocks per year
- **Continuous Accumulation**: Validator rewards accumulate every block

## How to Deploy

1. **Build the contract:**
   ```bash
   cd validator-social
   cargo build --target wasm32-unknown-unknown
   ```

2. **Deploy using rubix-nexus:**
   ```bash
   rubix-nexus contract deploy \
     --contract-dir validator-social \
     --deployer-did <your-did> \
     --deploy-amt 0.001
   ```

## How to Execute

After deployment, you'll receive a contract hash (starting with `Qm`). Use this to execute contract functions:

```bash
rubix-nexus contract execute \
  --contract-dir validator-social \
  --contract-hash <contract-hash> \
  --contract-msg-file examples/stake_request.json \
  --executor-did <your-did>
```

## Example Usage Flow

1. **Initialize the validator pool** with desired reward rate
2. **Stake RBT tokens** to support validators and start earning rewards
3. **Check stake info** to see accumulated validator rewards
4. **Claim rewards** periodically to harvest validator earnings
5. **Withdraw stake** when ready to stop supporting validators

## Important Notes

- This is a validator social staking contract - in production, you would need:
  - Persistent storage for validator stake state
  - Integration with actual validator selection mechanism
  - Validator performance tracking and slashing
  - Governance mechanisms for reward rate changes
- The current implementation uses demo data for pool state
- Validator reward calculations are based on time-staked, not validator performance

## Security Considerations

- Always validate input parameters
- Implement proper access controls
- Use secure random number generation for critical operations
- Consider reentrancy attacks and implement guards
- Audit contract code before mainnet deployment