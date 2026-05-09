package main

import (
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"

	agentHost "agent-staking/host"

	wasmbridge "github.com/rubixchain/rubix-wasm/go-wasm-bridge"
)

const (
	defaultWasmPath   = "../../../artifacts/agent_staking.wasm"
	defaultNodeAddr   = "http://localhost:20011"
	defaultDappPort   = ":6001"
	defaultQuorumType = 2
)

// callbackPayload matches what rubixgoplatform POSTs to the dapp URL
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

	registry := wasmbridge.NewHostFunctionRegistry()
	registry.Register(agentHost.NewGetAgentState())
	registry.Register(agentHost.NewSaveAgentState())

	wasmModule, err := wasmbridge.NewWasmModule(
		wasmPath,
		registry,
		wasmbridge.WithRubixNodeAddress(nodeAddr),
		wasmbridge.WithQuorumType(defaultQuorumType),
	)
	if err != nil {
		log.Fatalf("failed to initialise WASM module: %v", err)
	}

	http.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
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

		log.Printf("[dapp] callback received: contract=%s initiator=%s data=%s",
			payload.SmartContractHash, payload.InitiatorDID, payload.SmartContractData)

		result, err := wasmModule.CallFunction(payload.SmartContractData)
		if err != nil {
			log.Printf("[dapp] contract execution error: %v", err)
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusOK) // node expects 200 even on contract errors
			json.NewEncoder(w).Encode(map[string]interface{}{
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

	log.Printf("[dapp] agent_staking dapp listening on %s (node: %s)", dappPort, nodeAddr)
	if err := http.ListenAndServe(dappPort, nil); err != nil {
		log.Fatalf("dapp server failed: %v", err)
	}
}

func envOr(key, fallback string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return fallback
}
