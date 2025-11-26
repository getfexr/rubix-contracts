use rubixwasm_std::{errors::WasmError, contract_fn};
use rubixwasm_std::{call_mint_ft_api, call_transfer_ft_api};
use rubixwasm_std::helpers::{MintFt, TransferFt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Contract state structure
#[derive(Serialize, Deserialize, Clone)]
pub struct StakingPool {
    pub total_staked: u64,
    pub annual_yield_rate: u64, // percentage (e.g., 5 for 5%)
    pub last_update_block: u64,
    pub stakers: HashMap<String, StakeInfo>,
}

// Individual staker information
#[derive(Serialize, Deserialize, Clone)]
pub struct StakeInfo {
    pub amount_staked: u64,
    pub yield_token_id: String,
    pub stake_start_block: u64,
    pub last_claim_block: u64,
}

// Request structures for different operations
#[derive(Serialize, Deserialize)]
pub struct StakeRequest {
    pub staker_did: String,
    pub amount: u64,
    pub current_block: u64,
}

#[derive(Serialize, Deserialize)]
pub struct WithdrawRequest {
    pub staker_did: String,
    pub current_block: u64,
}

#[derive(Serialize, Deserialize)]
pub struct ClaimYieldRequest {
    pub staker_did: String,
    pub current_block: u64,
}

#[derive(Serialize, Deserialize)]
pub struct GetStakeInfoRequest {
    pub staker_did: String,
    pub current_block: u64,
}

#[derive(Serialize, Deserialize)]
pub struct InitializePoolRequest {
    pub annual_yield_rate: u64,
    pub current_block: u64,
}

#[derive(Serialize, Deserialize)]
pub struct GetPoolStatsRequest {
    pub current_block: u64,
}

// Response structure
#[derive(Serialize, Deserialize)]
pub struct StakingResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

// Initialize the staking pool
#[contract_fn]
pub fn initialize_pool(input: InitializePoolRequest) -> Result<String, WasmError> {
    let pool = StakingPool {
        total_staked: 0,
        annual_yield_rate: input.annual_yield_rate,
        last_update_block: input.current_block,
        stakers: HashMap::new(),
    };

    let response = StakingResponse {
        success: true,
        message: format!("Staking pool initialized with {}% annual yield rate", input.annual_yield_rate),
        data: Some(serde_json::to_value(&pool).unwrap()),
    };

    Ok(serde_json::to_string(&response).unwrap())
}

// Stake RBT tokens
#[contract_fn]
pub fn stake_rbt(input: StakeRequest) -> Result<String, WasmError> {
    if input.amount == 0 {
        return Err(WasmError::from("Stake amount must be positive"));
    }

    let mut pool = get_current_pool_state(input.current_block)?;

    // Check if user already has an active stake (following EX2's approach)
    if pool.stakers.contains_key(&input.staker_did) {
        return Err(WasmError::from("User already has an active stake. Please withdraw first."));
    }

    // Generate unique yield token ID (EX2's clever approach)
    let yield_token_id = format!("RBTY_{}_{}_{}",
        input.staker_did,
        input.amount,
        input.current_block
    );

    // Mint yield tokens representing staked RBT + future yield capacity
    // We mint MORE than the stake to account for future yield
    let yield_capacity = input.amount + calculate_max_yield(input.amount, pool.annual_yield_rate);

    let yield_token_mint = MintFt {
        did: input.staker_did.clone(),
        ft_count: yield_capacity as i32,
        ft_name: yield_token_id.clone(),
        ft_num_start_index: 0,
        token_count: yield_capacity as i32,
    };

    // Mint the yield tokens
    match call_mint_ft_api(yield_token_mint) {
        Ok(_) => {
            // Create stake record
            let stake_info = StakeInfo {
                amount_staked: input.amount,
                yield_token_id: yield_token_id.clone(),
                stake_start_block: input.current_block,
                last_claim_block: input.current_block,
            };

            // Update pool state
            pool.stakers.insert(input.staker_did.clone(), stake_info);
            pool.total_staked += input.amount;

            let response = StakingResponse {
                success: true,
                message: format!("Successfully staked {} RBT and minted yield tokens", input.amount),
                data: Some(serde_json::json!({
                    "staker_did": input.staker_did,
                    "amount_staked": input.amount,
                    "yield_token_id": yield_token_id,
                    "yield_capacity": yield_capacity,
                    "total_pool_staked": pool.total_staked
                })),
            };

            Ok(serde_json::to_string(&response).unwrap())
        }
        Err(e) => {
            Err(WasmError::from(format!("Failed to mint yield tokens: {:?}", e)))
        }
    }
}

// Withdraw staked RBT and earned yield
#[contract_fn]
pub fn withdraw_stake(input: WithdrawRequest) -> Result<String, WasmError> {
    let mut pool = get_current_pool_state(input.current_block)?;

    // Get staker info and clone needed values
    let staker_info = pool.stakers.get(&input.staker_did)
        .ok_or_else(|| WasmError::from("No active stake found for this user"))?;

    let amount_staked = staker_info.amount_staked;
    let stake_start_block = staker_info.stake_start_block;
    let yield_token_id = staker_info.yield_token_id.clone();

    // Calculate yield based on time staked
    let blocks_staked = input.current_block - stake_start_block;
    let yield_amount = calculate_yield(amount_staked, blocks_staked, pool.annual_yield_rate);
    let total_amount = amount_staked + yield_amount;

    // Transfer yield tokens back (representing original stake + yield)
    let transfer_info = TransferFt {
        comment: format!("Withdrawing {} RBT stake + {} yield", amount_staked, yield_amount),
        ft_count: total_amount as i32,
        ft_name: yield_token_id.clone(),
        creatorDID: input.staker_did.clone(),
        receiver: input.staker_did.clone(),
        sender: "staking_contract".to_string(),
    };

    match call_transfer_ft_api(transfer_info) {
        Ok(_) => {
            // Remove stake from pool
            pool.stakers.remove(&input.staker_did);
            pool.total_staked -= amount_staked;

            let response = StakingResponse {
                success: true,
                message: format!("Successfully withdrew {} total tokens ({}RBT principal + {} yield)",
                    total_amount, amount_staked, yield_amount),
                data: Some(serde_json::json!({
                    "staker_did": input.staker_did,
                    "principal_withdrawn": amount_staked,
                    "yield_withdrawn": yield_amount,
                    "total_returned": total_amount,
                    "yield_token_id": yield_token_id
                })),
            };

            Ok(serde_json::to_string(&response).unwrap())
        }
        Err(e) => {
            Err(WasmError::from(format!("Failed to transfer tokens: {:?}", e)))
        }
    }
}

// Claim yield tokens without withdrawing principal
#[contract_fn]
pub fn claim_yield(input: ClaimYieldRequest) -> Result<String, WasmError> {
    let mut pool = get_current_pool_state(input.current_block)?;

    let staker_info = pool.stakers.get_mut(&input.staker_did)
        .ok_or_else(|| WasmError::from("Staker not found"))?;

    // Calculate yield earned since last claim
    let blocks_since_last_claim = input.current_block - staker_info.last_claim_block;
    let yield_earned = calculate_yield(staker_info.amount_staked, blocks_since_last_claim, pool.annual_yield_rate);

    if yield_earned == 0 {
        return Err(WasmError::from("No yield tokens to claim"));
    }

    // Transfer only the yield portion using the yield token
    let transfer_info = TransferFt {
        comment: format!("Claiming {} yield tokens", yield_earned),
        ft_count: yield_earned as i32,
        ft_name: staker_info.yield_token_id.clone(),
        creatorDID: input.staker_did.clone(),
        receiver: input.staker_did.clone(),
        sender: "staking_contract".to_string(),
    };

    match call_transfer_ft_api(transfer_info) {
        Ok(_) => {
            // Update last claim block
            staker_info.last_claim_block = input.current_block;

            let response = StakingResponse {
                success: true,
                message: format!("Successfully claimed {} yield tokens", yield_earned),
                data: Some(serde_json::json!({
                    "staker_did": input.staker_did,
                    "yield_claimed": yield_earned,
                    "remaining_stake": staker_info.amount_staked,
                    "yield_token_id": staker_info.yield_token_id
                })),
            };

            Ok(serde_json::to_string(&response).unwrap())
        }
        Err(e) => {
            Err(WasmError::from(format!("Failed to transfer yield tokens: {:?}", e)))
        }
    }
}

// Get staker information
#[contract_fn]
pub fn get_stake_info(input: GetStakeInfoRequest) -> Result<String, WasmError> {
    let pool = get_current_pool_state(input.current_block)?;

    let staker_info = pool.stakers.get(&input.staker_did)
        .ok_or_else(|| WasmError::from("Staker not found"))?;

    // Calculate pending yield since last claim
    let blocks_since_last_claim = input.current_block - staker_info.last_claim_block;
    let pending_yield = calculate_yield(staker_info.amount_staked, blocks_since_last_claim, pool.annual_yield_rate);

    // Calculate total yield if withdrawn now
    let blocks_staked = input.current_block - staker_info.stake_start_block;
    let total_yield_if_withdrawn = calculate_yield(staker_info.amount_staked, blocks_staked, pool.annual_yield_rate);

    let response = StakingResponse {
        success: true,
        message: "Stake info retrieved successfully".to_string(),
        data: Some(serde_json::json!({
            "staker_did": input.staker_did,
            "amount_staked": staker_info.amount_staked,
            "pending_yield_to_claim": pending_yield,
            "total_yield_if_withdrawn": total_yield_if_withdrawn,
            "total_value": staker_info.amount_staked + total_yield_if_withdrawn,
            "stake_start_block": staker_info.stake_start_block,
            "last_claim_block": staker_info.last_claim_block,
            "yield_token_id": staker_info.yield_token_id,
            "blocks_staked": blocks_staked
        })),
    };

    Ok(serde_json::to_string(&response).unwrap())
}

// Get pool statistics
#[contract_fn]
pub fn get_pool_stats(input: GetPoolStatsRequest) -> Result<String, WasmError> {
    let pool = get_current_pool_state(input.current_block)?;

    let response = StakingResponse {
        success: true,
        message: "Pool statistics retrieved successfully".to_string(),
        data: Some(serde_json::json!({
            "total_staked": pool.total_staked,
            "annual_yield_rate": pool.annual_yield_rate,
            "total_stakers": pool.stakers.len(),
            "last_update_block": pool.last_update_block,
        })),
    };

    Ok(serde_json::to_string(&response).unwrap())
}

// Helper functions
fn get_current_pool_state(current_block: u64) -> Result<StakingPool, WasmError> {
    // In a real implementation, this would read from persistent contract storage
    // For now, we'll return a default pool with demo data
    Ok(StakingPool {
        total_staked: 0,  // Start with no stakes
        annual_yield_rate: 10, // 10% default APY
        last_update_block: current_block,
        stakers: HashMap::new(),
    })
}

// Calculate yield for a given stake amount over time
fn calculate_yield(stake_amount: u64, blocks_passed: u64, annual_rate: u64) -> u64 {
    // Assuming ~6 blocks per minute, ~8640 blocks per day, ~3,153,600 blocks per year
    const BLOCKS_PER_YEAR: u64 = 3_153_600;

    if stake_amount == 0 || blocks_passed == 0 || annual_rate == 0 {
        return 0;
    }

    // Calculate yield: (principal * rate * time) / (100 * blocks_per_year)
    let yield_amount = (stake_amount * annual_rate * blocks_passed) / (100 * BLOCKS_PER_YEAR);
    yield_amount
}

// Calculate maximum possible yield for capacity planning
fn calculate_max_yield(stake_amount: u64, annual_rate: u64) -> u64 {
    // Assume maximum 2 years of staking for capacity planning
    const MAX_BLOCKS_STAKED: u64 = 3_153_600 * 2; // 2 years
    calculate_yield(stake_amount, MAX_BLOCKS_STAKED, annual_rate)
}
