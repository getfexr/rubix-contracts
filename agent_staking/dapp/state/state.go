package state

import (
	"encoding/json"
	"os"
	"path/filepath"
)

const stateFileName = "agent_state.json"

type ActivityRecord struct {
	ActivityType string `json:"activity_type"`
	ActivityData string `json:"activity_data"`
	Timestamp    uint64 `json:"timestamp"`
	TxRef        string `json:"tx_ref"`
}

type AgentState struct {
	AgentDID        string           `json:"agent_did"`
	AgentName       string           `json:"agent_name"`
	AgentType       string           `json:"agent_type"`
	Status          string           `json:"status"`
	StakeAmount     float64          `json:"stake_amount"`
	RegisteredAt    uint64           `json:"registered_at"`
	ActivityCount   uint64           `json:"activity_count"`
	LastActivityAt  uint64           `json:"last_activity_at"`
	Activities      []ActivityRecord `json:"activities"`
}

func stateFilePath() string {
	dir, _ := filepath.Abs(filepath.Dir(os.Args[0]))
	return filepath.Join(dir, "state", stateFileName)
}

func LoadState() (*AgentState, error) {
	data, err := os.ReadFile(stateFilePath())
	if err != nil {
		if os.IsNotExist(err) {
			return &AgentState{Activities: []ActivityRecord{}}, nil
		}
		return nil, err
	}
	var s AgentState
	if err := json.Unmarshal(data, &s); err != nil {
		return nil, err
	}
	if s.Activities == nil {
		s.Activities = []ActivityRecord{}
	}
	return &s, nil
}

func SaveState(s *AgentState) error {
	data, err := json.MarshalIndent(s, "", "  ")
	if err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(stateFilePath()), 0755); err != nil {
		return err
	}
	return os.WriteFile(stateFilePath(), data, 0644)
}

func GetStateJSON() (string, error) {
	s, err := LoadState()
	if err != nil {
		return "", err
	}
	data, err := json.Marshal(s)
	if err != nil {
		return "", err
	}
	return string(data), nil
}

func SaveStateFromJSON(jsonStr string) error {
	var s AgentState
	if err := json.Unmarshal([]byte(jsonStr), &s); err != nil {
		return err
	}
	return SaveState(&s)
}
