use rubixwasm_std::errors::WasmError;
use rubixwasm_std::contract_fn;
use serde::{Deserialize, Serialize};
use std::slice;
use std::str;

// ── State types ──────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
pub struct ActivityRecord {
    pub activity_type: String,
    pub activity_data: String, // JSON-encoded payload
    pub timestamp: u64,
    pub tx_ref: String, // optional caller reference / tx ID
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AgentState {
    pub agent_did: String,
    pub agent_name: String,
    pub agent_type: String,      // "trading" | "advisory" | "monitoring" | "general"
    pub status: String,          // "active" | "suspended"
    pub stake_amount: f64,       // always 1.0
    pub registered_at: u64,
    pub activity_count: u64,
    pub last_activity_at: u64,
    pub activities: Vec<ActivityRecord>,
}

impl Default for AgentState {
    fn default() -> Self {
        AgentState {
            agent_did: String::new(),
            agent_name: String::new(),
            agent_type: String::new(),
            status: String::new(),
            stake_amount: 0.0,
            registered_at: 0,
            activity_count: 0,
            last_activity_at: 0,
            activities: Vec::new(),
        }
    }
}

// ── Request types ─────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct RegisterAgentReq {
    pub agent_did: String,
    pub agent_name: String,
    pub agent_type: String,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize)]
pub struct RecordActivityReq {
    pub agent_did: String,
    pub activity_type: String,
    pub activity_data: String,
    pub timestamp: u64,
    pub tx_ref: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct GetAgentInfoReq {
    pub agent_did: String,
}

#[derive(Serialize, Deserialize)]
pub struct UpdateStatusReq {
    pub agent_did: String,
    pub status: String, // "active" | "suspended"
}

// ── Response type ─────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct ContractResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

// ── Contract functions ────────────────────────────────────────────────────────

/// Register an AI agent — called on first execute after deploying with 1 RBT.
/// Idempotent: re-registering with the same DID is a no-op.
#[contract_fn]
pub fn register_agent(req: RegisterAgentReq) -> Result<String, WasmError> {
    if req.agent_did.is_empty() {
        return Err(WasmError::from("agent_did must not be empty"));
    }
    if req.agent_name.is_empty() {
        return Err(WasmError::from("agent_name must not be empty"));
    }
    let valid_types = ["trading", "advisory", "monitoring", "general"];
    if !valid_types.contains(&req.agent_type.as_str()) {
        return Err(WasmError::from(
            "agent_type must be one of: trading, advisory, monitoring, general",
        ));
    }

    let mut state = load_state()?;

    // Already registered — idempotent
    if !state.agent_did.is_empty() {
        let resp = ContractResponse {
            success: true,
            message: format!("Agent {} already registered", state.agent_did),
            data: Some(serde_json::to_value(&state).unwrap_or(serde_json::Value::Null)),
        };
        return Ok(serde_json::to_string(&resp)
            .map_err(|e| WasmError::from(format!("serialization error: {}", e)))?);
    }

    state.agent_did = req.agent_did.clone();
    state.agent_name = req.agent_name;
    state.agent_type = req.agent_type;
    state.status = "active".to_string();
    state.stake_amount = 1.0;
    state.registered_at = req.timestamp;
    state.activity_count = 0;
    state.last_activity_at = req.timestamp;

    save_state(&state)?;

    let resp = ContractResponse {
        success: true,
        message: format!("Agent {} registered with 1 RBT stake", req.agent_did),
        data: Some(serde_json::to_value(&state).unwrap_or(serde_json::Value::Null)),
    };
    Ok(serde_json::to_string(&resp)
        .map_err(|e| WasmError::from(format!("serialization error: {}", e)))?)
}

/// Record any agent activity on-chain. Each call creates a new block in the
/// smart contract token chain, permanently linking the activity to the agent.
#[contract_fn]
pub fn record_activity(req: RecordActivityReq) -> Result<String, WasmError> {
    if req.agent_did.is_empty() {
        return Err(WasmError::from("agent_did must not be empty"));
    }
    if req.activity_type.is_empty() {
        return Err(WasmError::from("activity_type must not be empty"));
    }

    let mut state = load_state()?;

    if state.agent_did.is_empty() {
        return Err(WasmError::from(
            "Agent not registered. Call register_agent first.",
        ));
    }
    if state.agent_did != req.agent_did {
        return Err(WasmError::from(format!(
            "DID mismatch: contract owned by {}, got {}",
            state.agent_did, req.agent_did
        )));
    }
    if state.status == "suspended" {
        return Err(WasmError::from("Agent is suspended and cannot record activities"));
    }

    let record = ActivityRecord {
        activity_type: req.activity_type.clone(),
        activity_data: req.activity_data,
        timestamp: req.timestamp,
        tx_ref: req.tx_ref.unwrap_or_default(),
    };

    state.activities.push(record);
    state.activity_count += 1;
    state.last_activity_at = req.timestamp;

    save_state(&state)?;

    let resp = ContractResponse {
        success: true,
        message: format!(
            "Activity '{}' recorded for agent {}. Total: {}",
            req.activity_type, req.agent_did, state.activity_count
        ),
        data: Some(serde_json::json!({
            "activity_count": state.activity_count,
            "last_activity_at": state.last_activity_at,
            "activity_type": req.activity_type,
        })),
    };
    Ok(serde_json::to_string(&resp)
        .map_err(|e| WasmError::from(format!("serialization error: {}", e)))?)
}

/// Read-only query — returns full agent state.
#[contract_fn]
pub fn get_agent_info(req: GetAgentInfoReq) -> Result<String, WasmError> {
    let state = load_state()?;

    if state.agent_did.is_empty() {
        return Ok(serde_json::to_string(&ContractResponse {
            success: false,
            message: "Agent not registered".to_string(),
            data: None,
        })
        .unwrap_or_default());
    }
    if !req.agent_did.is_empty() && state.agent_did != req.agent_did {
        return Err(WasmError::from(format!(
            "No agent found for DID {}",
            req.agent_did
        )));
    }

    let resp = ContractResponse {
        success: true,
        message: "Agent info retrieved".to_string(),
        data: Some(serde_json::to_value(&state).unwrap_or(serde_json::Value::Null)),
    };
    Ok(serde_json::to_string(&resp)
        .map_err(|e| WasmError::from(format!("serialization error: {}", e)))?)
}

/// Update agent status (active/suspended). Only the registered agent's DID can call.
#[contract_fn]
pub fn update_status(req: UpdateStatusReq) -> Result<String, WasmError> {
    if req.status != "active" && req.status != "suspended" {
        return Err(WasmError::from("status must be 'active' or 'suspended'"));
    }

    let mut state = load_state()?;

    if state.agent_did.is_empty() {
        return Err(WasmError::from("Agent not registered"));
    }
    if state.agent_did != req.agent_did {
        return Err(WasmError::from("Only the registered agent can update its status"));
    }

    state.status = req.status.clone();
    save_state(&state)?;

    let resp = ContractResponse {
        success: true,
        message: format!("Agent {} status updated to {}", req.agent_did, req.status),
        data: None,
    };
    Ok(serde_json::to_string(&resp)
        .map_err(|e| WasmError::from(format!("serialization error: {}", e)))?)
}

// ── Host function FFI ─────────────────────────────────────────────────────────

mod state_storage {
    use super::*;

    extern "C" {
        pub fn get_agent_state_from_storage(
            out_state_ptr: *mut *const u8,
            out_state_len: *mut usize,
        ) -> i32;

        pub fn save_agent_state_to_storage(
            in_state_ptr: *const u8,
            in_state_len: usize,
        ) -> i32;
    }

    pub fn call_get_agent_state() -> Result<String, WasmError> {
        unsafe {
            let mut out_ptr: *const u8 = std::ptr::null();
            let mut out_len: usize = 0;

            let rc = get_agent_state_from_storage(&mut out_ptr, &mut out_len);
            if rc != 0 {
                return Err(WasmError::from(format!(
                    "get_agent_state_from_storage returned error code {}",
                    rc
                )));
            }
            if out_ptr.is_null() {
                return Err(WasmError::from("state pointer is null"));
            }
            let slice = slice::from_raw_parts(out_ptr, out_len);
            str::from_utf8(slice)
                .map(|s| s.to_string())
                .map_err(|_| WasmError::from("invalid UTF-8 in state"))
        }
    }

    pub fn call_save_agent_state(state_json: &str) -> Result<(), WasmError> {
        unsafe {
            let bytes = state_json.as_bytes();
            let rc = save_agent_state_to_storage(bytes.as_ptr(), bytes.len());
            if rc != 0 {
                return Err(WasmError::from(format!(
                    "save_agent_state_to_storage returned error code {}",
                    rc
                )));
            }
            Ok(())
        }
    }
}

// ── State helpers ─────────────────────────────────────────────────────────────

fn load_state() -> Result<AgentState, WasmError> {
    let json = state_storage::call_get_agent_state()?;
    if json.trim().is_empty() || json == "null" {
        return Ok(AgentState::default());
    }
    serde_json::from_str(&json)
        .map_err(|e| WasmError::from(format!("failed to parse agent state: {}", e)))
}

fn save_state(state: &AgentState) -> Result<(), WasmError> {
    let json = serde_json::to_string(state)
        .map_err(|e| WasmError::from(format!("failed to serialize agent state: {}", e)))?;
    state_storage::call_save_agent_state(&json)
}
