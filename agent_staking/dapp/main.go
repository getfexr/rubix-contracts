package main

import (
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"strconv"
	"time"

	agentDB "agent-staking/db"
	agentHost "agent-staking/host"

	wasmbridge "github.com/rubixchain/rubix-wasm/go-wasm-bridge"
)

const (
	defaultWasmPath   = "../../../artifacts/agent_staking.wasm"
	defaultNodeAddr   = "http://localhost:20011"
	defaultDappPort   = ":6001"
	defaultDBPath     = "./state/agent_activity.db"
	defaultQuorumType = 2
)

// callbackPayload matches what rubixgoplatform POSTs to the dapp URL on contract execution.
type callbackPayload struct {
	SmartContractHash string `json:"smart_contract_hash"`
	Port              int    `json:"port"`
	SmartContractData string `json:"smart_contract_data"`
	InitiatorDID      string `json:"initiator_did"`
}

func main() {
	wasmPath := envOr("WASM_PATH", defaultWasmPath)
	nodeAddr := envOr("RUBIX_NODE_URL", defaultNodeAddr)
	dappPort := envOr("DAPP_PORT", defaultDappPort)
	dbPath := envOr("DB_PATH", defaultDBPath)

	// ── Open SQLite ───────────────────────────────────────────────────────────
	database, err := agentDB.Open(dbPath)
	if err != nil {
		log.Fatalf("failed to open SQLite at %s: %v", dbPath, err)
	}
	defer database.Close()
	log.Printf("[dapp] SQLite opened at %s", dbPath)

	// ── Build WASM module with all host functions ─────────────────────────────
	registry := wasmbridge.NewHostFunctionRegistry()
	registry.Register(agentHost.NewGetAgentState())
	registry.Register(agentHost.NewSaveAgentState())
	registry.Register(agentHost.NewAppendBatch())
	registry.Register(agentHost.NewAppendActivity())
	registry.Register(agentHost.NewGetActivityCount())

	wasmModule, err := wasmbridge.NewWasmModule(
		wasmPath,
		registry,
		wasmbridge.WithRubixNodeAddress(nodeAddr),
		wasmbridge.WithQuorumType(defaultQuorumType),
	)
	if err != nil {
		log.Fatalf("failed to initialise WASM module: %v", err)
	}

	// ── HTTP routes ───────────────────────────────────────────────────────────
	mux := http.NewServeMux()

	// Rubix node → dapp contract-execution callback
	mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/" {
			http.NotFound(w, r)
			return
		}
		if r.Method != http.MethodPost {
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}

		body, err := io.ReadAll(r.Body)
		if err != nil {
			http.Error(w, "failed to read body", http.StatusBadRequest)
			return
		}
		defer r.Body.Close()

		var payload callbackPayload
		if err := json.Unmarshal(body, &payload); err != nil {
			http.Error(w, "invalid JSON payload", http.StatusBadRequest)
			return
		}

		log.Printf("[dapp] callback: contract=%s initiator=%s data_len=%d",
			payload.SmartContractHash, payload.InitiatorDID, len(payload.SmartContractData))

		result, err := wasmModule.CallFunction(payload.SmartContractData)
		if err != nil {
			log.Printf("[dapp] contract execution error: %v", err)
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusOK) // node expects 200 even on contract errors
			json.NewEncoder(w).Encode(map[string]any{
				"success": false,
				"message": fmt.Sprintf("contract error: %v", err),
			})
			return
		}

		log.Printf("[dapp] contract result: %s", result)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		fmt.Fprint(w, result)
	})

	// GET /activities?from_ts=<ms>&to_ts=<ms>&limit=<n>
	mux.HandleFunc("/activities", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodGet {
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}

		q := r.URL.Query()
		fromTs := parseIntOrDefault(q.Get("from_ts"), 0)
		toTs := parseIntOrDefault(q.Get("to_ts"), time.Now().UnixMilli())
		limit := int(parseIntOrDefault(q.Get("limit"), 100))

		rows, err := database.QueryActivities(fromTs, toTs, limit)
		if err != nil {
			http.Error(w, fmt.Sprintf("query error: %v", err), http.StatusInternalServerError)
			return
		}

		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]any{
			"activities": rows,
			"count":      len(rows),
		})
	})

	// GET /batch/{hash}
	mux.HandleFunc("/batch/", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodGet {
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}

		hash := r.URL.Path[len("/batch/"):]
		if hash == "" {
			http.Error(w, "batch hash required", http.StatusBadRequest)
			return
		}

		fullJSON, err := database.GetBatch(hash)
		if err != nil {
			http.Error(w, fmt.Sprintf("db error: %v", err), http.StatusInternalServerError)
			return
		}
		if fullJSON == "" {
			http.Error(w, "batch not found", http.StatusNotFound)
			return
		}

		w.Header().Set("Content-Type", "application/json")
		fmt.Fprint(w, fullJSON)
	})

	// GET /health
	mux.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		stateJSON, _ := database.GetStateJSON()
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]any{
			"status":  "ok",
			"db_path": dbPath,
			"state":   json.RawMessage(stateJSON),
		})
	})

	log.Printf("[dapp] agent_staking dapp listening on %s (node: %s, db: %s)",
		dappPort, nodeAddr, dbPath)
	if err := http.ListenAndServe(dappPort, mux); err != nil {
		log.Fatalf("dapp server failed: %v", err)
	}
}

// ── Helpers ───────────────────────────────────────────────────────────────────

func envOr(key, fallback string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return fallback
}

func parseIntOrDefault(s string, def int64) int64 {
	if s == "" {
		return def
	}
	v, err := strconv.ParseInt(s, 10, 64)
	if err != nil {
		return def
	}
	return v
}
