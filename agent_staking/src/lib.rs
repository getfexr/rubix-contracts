use rubixwasm_std::errors::WasmError;
use rubixwasm_std::contract_fn;
use serde::{Deserialize, Serialize};
use std::slice;
use std::str;

// ── State types ──────────────────────────────────────────────────────────────

/// Lightweight summary of a committed activity batch.
/// Only hashes and counters live in WASM state — full records are in dapp SQLite.
#[derive(Serialize, Deserialize, Clone)]
pub struct BatchRoot {
    pub batch_hash: String,       // SHA-256 hex of full batch JSON
    pub activity_count: u64,      // number of actions in this batch
    pub period_start: u64,        // earliest action timestamp (unix ms)
    pub period_end: u64,          // latest action timestamp (unix ms)
    pub committed_at: u64,        // when record_activity_batch was called
    pub dominant_action: String,  // most frequent action_type in the batch
}

/// Single-activity record written to dapp SQLite via append_activity_to_storage.
/// NOT stored in WASM state — only used as a wire type for the host function.
#[derive(Serialize, Deserialize, Clone)]
pub struct ActivityRecord {
    pub activity_type: String,
    pub activity_data: String, // JSON-encoded payload
    pub timestamp: u64,
    pub tx_ref: String,
}

/// Agent state kept in WASM — stays lean regardless of activity volume.
#[derive(Serialize, Deserialize, Clone)]
pub struct AgentState {
    pub agent_did: String,
    pub agent_name: String,
    pub agent_type: String,         // "trading" | "advisory" | "monitoring" | "general"
    pub status: String,             // "active" | "suspended"
    pub stake_amount: f64,          // always 1.0
    pub registered_at: u64,
    pub activity_count: u64,        // total across all batches and single records
    pub last_activity_at: u64,
    pub reputation_score: f64,      // 0.0–100.0
    pub batch_count: u64,
    pub batch_roots: Vec<BatchRoot>, // lightweight — only hashes + metadata
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
            reputation_score: 0.0,
            batch_count: 0,
            batch_roots: Vec::new(),
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

/// Batch commit request — primary write path for the agent activity committer.
/// The full batch JSON is stored in dapp SQLite via append_batch_to_storage;
/// only the hash + metadata enter WASM state.
#[derive(Serialize, Deserialize)]
pub struct RecordActivityBatchReq {
    pub agent_did: String,
    pub batch_hash: String,       // SHA-256 hex of full batch JSON (computed off-chain)
    pub activity_count: u64,      // number of actions in batch
    pub period_start: u64,        // earliest action timestamp (unix ms)
    pub period_end: u64,          // latest action timestamp (unix ms)
    pub dominant_action: String,  // most frequent action_type
    pub full_batch_json: String,  // full JSON — passed to dapp host fn, not stored in WASM state
}

/// Single-activity request — kept for low-volume / test usage.
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

/// Register an AI agent — called once after deploying with 1 RBT.
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

/// Commit a batch of activities on-chain. One consensus round covers the entire batch.
/// Full batch JSON is stored in dapp SQLite; only the hash enters WASM state.
/// Reputation score is recomputed after each batch commit.
#[contract_fn]
pub fn record_activity_batch(req: RecordActivityBatchReq) -> Result<String, WasmError> {
    if req.agent_did.is_empty() {
        return Err(WasmError::from("agent_did must not be empty"));
    }
    if req.batch_hash.is_empty() {
        return Err(WasmError::from("batch_hash must not be empty"));
    }
    if req.activity_count == 0 {
        return Err(WasmError::from("activity_count must be > 0"));
    }
    if req.full_batch_json.is_empty() {
        return Err(WasmError::from("full_batch_json must not be empty"));
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
        return Err(WasmError::from(
            "Agent is suspended and cannot record activities",
        ));
    }

    // Persist full batch JSON to dapp SQLite via host function
    host_ffi::call_append_batch_to_storage(&req.batch_hash, &req.full_batch_json)?;

    // Record lightweight root in WASM state — cap at 500 to bound state size.
    // Older roots are preserved permanently in dapp SQLite; only recent history
    // needs to be in the on-chain state for reputation queries.
    const MAX_BATCH_ROOTS: usize = 500;
    state.batch_roots.push(BatchRoot {
        batch_hash: req.batch_hash.clone(),
        activity_count: req.activity_count,
        period_start: req.period_start,
        period_end: req.period_end,
        committed_at: req.period_end,
        dominant_action: req.dominant_action.clone(),
    });
    if state.batch_roots.len() > MAX_BATCH_ROOTS {
        state.batch_roots.remove(0); // drop oldest; O(n) but n≤500 is fine
    }
    state.activity_count += req.activity_count;
    state.batch_count += 1;
    state.last_activity_at = req.period_end;

    // Recompute reputation after each batch
    let recent_count = host_ffi::call_get_activity_count_in_range(
        req.period_end.saturating_sub(7 * 24 * 3600 * 1000),
        req.period_end,
    )
    .unwrap_or(0);
    state.reputation_score = compute_reputation(&state, req.period_end, recent_count);

    save_state(&state)?;

    let resp = ContractResponse {
        success: true,
        message: format!(
            "Batch '{}' committed for agent {}. Total activities: {}. Reputation: {:.1}",
            req.batch_hash, req.agent_did, state.activity_count, state.reputation_score
        ),
        data: Some(serde_json::json!({
            "batch_hash": req.batch_hash,
            "batch_count": state.batch_count,
            "activity_count": state.activity_count,
            "reputation_score": state.reputation_score,
            "last_activity_at": state.last_activity_at,
        })),
    };
    Ok(serde_json::to_string(&resp)
        .map_err(|e| WasmError::from(format!("serialization error: {}", e)))?)
}

/// Record a single activity. Kept for low-volume / testing use.
/// Writes to dapp SQLite via append_activity_to_storage; updates counters in state.
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
        return Err(WasmError::from(
            "Agent is suspended and cannot record activities",
        ));
    }

    let record = ActivityRecord {
        activity_type: req.activity_type.clone(),
        activity_data: req.activity_data,
        timestamp: req.timestamp,
        tx_ref: req.tx_ref.unwrap_or_default(),
    };

    let record_json = serde_json::to_string(&record)
        .map_err(|e| WasmError::from(format!("failed to serialize activity: {}", e)))?;

    host_ffi::call_append_activity_to_storage(&record_json)?;

    state.activity_count += 1;
    state.last_activity_at = req.timestamp;

    // Recompute reputation periodically (every 10 single records)
    if state.activity_count % 10 == 0 {
        let recent_count = host_ffi::call_get_activity_count_in_range(
            req.timestamp.saturating_sub(7 * 24 * 3600 * 1000),
            req.timestamp,
        )
        .unwrap_or(0);
        state.reputation_score = compute_reputation(&state, req.timestamp, recent_count);
    }

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
            "reputation_score": state.reputation_score,
        })),
    };
    Ok(serde_json::to_string(&resp)
        .map_err(|e| WasmError::from(format!("serialization error: {}", e)))?)
}

/// Read-only query — returns full agent state including all batch roots.
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

/// Update agent status (active/suspended).
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
        return Err(WasmError::from(
            "Only the registered agent can update its status",
        ));
    }

    state.status = req.status.clone();
    save_state(&state)?;

    let resp = ContractResponse {
        success: true,
        message: format!(
            "Agent {} status updated to {}",
            req.agent_did, req.status
        ),
        data: None,
    };
    Ok(serde_json::to_string(&resp)
        .map_err(|e| WasmError::from(format!("serialization error: {}", e)))?)
}

// ── Reputation scoring ────────────────────────────────────────────────────────

/// Pure computation — uses only state fields + recent_count from host.
/// No external calls, no HTTP, no oracle. Self-contained.
fn compute_reputation(state: &AgentState, now_ts: u64, recent_7d_count: u64) -> f64 {
    // Velocity: actions per day over last 7 days, target = 50/day
    let velocity = (recent_7d_count as f64 / 7.0 / 50.0).min(1.0) * 30.0;

    // Recency: exponential decay, half-life = 3 days (ln2/3 ≈ 0.231)
    let days_idle = (now_ts.saturating_sub(state.last_activity_at)) as f64 / 86_400_000.0;
    let recency = (-days_idle * 0.231_f64).exp() * 20.0;

    // Tenure: saturates at 30 days
    let age_days = (now_ts.saturating_sub(state.registered_at)) as f64 / 86_400_000.0;
    let tenure = (age_days / 30.0).min(1.0) * 10.0;

    // Volume: log₂ scale, saturates at ~10k actions (log₂(10000) ≈ 13.3)
    let volume = if state.activity_count > 0 {
        (state.activity_count as f64).log2().min(13.3) / 13.3 * 40.0
    } else {
        0.0
    };

    (velocity + recency + tenure + volume).min(100.0)
}

// ── Host function FFI ─────────────────────────────────────────────────────────

mod host_ffi {
    use super::*;

    extern "C" {
        // Existing state persistence
        pub fn get_agent_state_from_storage(
            out_state_ptr: *mut *const u8,
            out_state_len: *mut usize,
        ) -> i32;

        pub fn save_agent_state_to_storage(
            in_state_ptr: *const u8,
            in_state_len: usize,
        ) -> i32;

        // New: write full batch JSON to dapp SQLite (never enters WASM state)
        pub fn append_batch_to_storage(
            batch_hash_ptr: *const u8,
            batch_hash_len: usize,
            batch_json_ptr: *const u8,
            batch_json_len: usize,
        ) -> i32;

        // New: write single activity record to dapp SQLite
        pub fn append_activity_to_storage(
            activity_json_ptr: *const u8,
            activity_json_len: usize,
        ) -> i32;

        // New: count activity records in a time range (for reputation velocity)
        pub fn get_activity_count_in_range(from_ts: u64, to_ts: u64) -> i64;
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

    pub fn call_append_batch_to_storage(
        batch_hash: &str,
        batch_json: &str,
    ) -> Result<(), WasmError> {
        unsafe {
            let hash_bytes = batch_hash.as_bytes();
            let json_bytes = batch_json.as_bytes();
            let rc = append_batch_to_storage(
                hash_bytes.as_ptr(),
                hash_bytes.len(),
                json_bytes.as_ptr(),
                json_bytes.len(),
            );
            if rc != 0 {
                return Err(WasmError::from(format!(
                    "append_batch_to_storage returned error code {}",
                    rc
                )));
            }
            Ok(())
        }
    }

    pub fn call_append_activity_to_storage(activity_json: &str) -> Result<(), WasmError> {
        unsafe {
            let bytes = activity_json.as_bytes();
            let rc = append_activity_to_storage(bytes.as_ptr(), bytes.len());
            if rc != 0 {
                return Err(WasmError::from(format!(
                    "append_activity_to_storage returned error code {}",
                    rc
                )));
            }
            Ok(())
        }
    }

    pub fn call_get_activity_count_in_range(from_ts: u64, to_ts: u64) -> Result<u64, WasmError> {
        unsafe {
            let count = get_activity_count_in_range(from_ts, to_ts);
            if count < 0 {
                return Err(WasmError::from("get_activity_count_in_range failed"));
            }
            Ok(count as u64)
        }
    }
}

// ── State helpers ─────────────────────────────────────────────────────────────

fn load_state() -> Result<AgentState, WasmError> {
    let json = host_ffi::call_get_agent_state()?;
    if json.trim().is_empty() || json == "null" {
        return Ok(AgentState::default());
    }
    serde_json::from_str(&json)
        .map_err(|e| WasmError::from(format!("failed to parse agent state: {}", e)))
}

fn save_state(state: &AgentState) -> Result<(), WasmError> {
    let json = serde_json::to_string(state)
        .map_err(|e| WasmError::from(format!("failed to serialize agent state: {}", e)))?;
    host_ffi::call_save_agent_state(&json)
}
