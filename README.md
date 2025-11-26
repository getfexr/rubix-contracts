# Rubix Smart Contracts

Production-ready smart contracts for the Rubix blockchain ecosystem, featuring validator social staking and token management capabilities.

## 📁 **Contracts**

### **🏛️ Validator Social (`validator-social/`)**
A sophisticated RBT staking contract enabling users to stake tokens in support of validators and earn rewards through fungible yield tokens.

**Key Features:**
- ✅ **Real Token Integration**: Uses authentic Rubix FT APIs (`call_mint_ft_api`, `call_transfer_ft_api`)
- ✅ **Smart Capacity Planning**: Mints sufficient RBTY tokens to cover future validator rewards
- ✅ **Flexible Reward Claims**: Harvest validator earnings while keeping stake active
- ✅ **Production-Ready Economics**: Proper token minting and transfer mechanics

**Functions:**
- `initialize_pool` - Set up validator staking pool with reward rate
- `stake_rbt` - Stake RBT to support validators and mint RBTY tokens
- `claim_yield` - Claim accumulated validator rewards
- `withdraw_stake` - Withdraw stake + all earned rewards
- `get_stake_info` - View individual staker position
- `get_pool_stats` - View overall pool statistics

## 🚀 **Quick Start**

### **Build Contracts**
```bash
# Build validator social contract
cd validator-social
cargo build --target wasm32-unknown-unknown
```

### **Deploy Contracts**
```bash
# Deploy using rubix-nexus
rubix-nexus contract deploy \
  --contract-dir validator-social \
  --deployer-did <your-did> \
  --deploy-amt 0.001
```

### **Execute Contract Functions**
```bash
# Execute contract with message file
rubix-nexus contract execute \
  --contract-dir validator-social \
  --contract-hash <contract-hash> \
  --contract-msg-file validator-social/examples/stake_request.json \
  --executor-did <your-did>
```

## 🛠️ **Development**

### **Prerequisites**
- Rust toolchain with `wasm32-unknown-unknown` target
- `rubix-nexus` CLI tool
- Valid Rubix DID for deployment

### **Build All Contracts**
```bash
# Build all contracts in the repository
for contract in */; do
  if [ -f "$contract/Cargo.toml" ]; then
    echo "Building $contract..."
    cd "$contract"
    cargo build --target wasm32-unknown-unknown
    cd ..
  fi
done
```

## 📊 **Economics & Tokenomics**

### **Validator Social Staking**
- **Reward Mechanism**: Block-based reward calculation (~3.15M blocks/year)
- **Token Model**: Unique RBTY tokens per stake with capacity planning
- **Yield Distribution**: Configurable annual percentage yield (APY)
- **Capacity Planning**: Mints 2x expected rewards to prevent token shortage

### **Security Features**
- Input validation and error handling
- Safe state management without unsafe operations
- Proper borrowing and ownership in Rust
- Comprehensive testing with example JSON files

## 🔗 **Integration**

### **With Rubix Node**
All contracts integrate with Rubix blockchain nodes via:
- `rubix-nexus` CLI for deployment and execution
- JSON-based message passing
- Standard Rubix DID authentication

### **Token Standards**
- Uses Rubix native fungible token (FT) APIs
- Follows standard `MintFt` and `TransferFt` patterns
- Compatible with existing Rubix wallet infrastructure

## 📋 **Examples**

Each contract includes comprehensive examples in `/examples/` directories:
- Request message templates
- Response format documentation
- Usage flow demonstrations

## 🔧 **Architecture**

### **Contract Structure**
```
validator-social/
├── src/lib.rs           # Main contract logic
├── examples/            # JSON request examples
├── Cargo.toml          # Rust dependencies
└── README.md           # Contract documentation
```

### **State Management**
- Persistent storage design (demo implementation included)
- HashMap-based staker tracking
- Block-based time management
- Capacity-aware token minting

## 📚 **Documentation**

- **Contract-specific docs**: See individual `/README.md` files
- **API Reference**: Function signatures and examples in source code
- **Integration guides**: JSON message formats and response structures

## 🤝 **Contributing**

1. **Fork and Clone**: Create your development environment
2. **Build and Test**: Ensure all contracts compile successfully
3. **Documentation**: Update relevant docs for any changes
4. **Submit PR**: Include clear description of changes and rationale

## 📄 **License**

MIT License - see individual contract directories for specific terms.

---

**Built for the Rubix Blockchain Ecosystem** - Enabling decentralized validator economics and token staking mechanisms.

