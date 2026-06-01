// Package state adapts the dapp/db SQLite layer to the simple get/save
// interface used by the existing host-function callbacks.
package state

import (
	"agent-staking/db"
	"fmt"
)

// GetStateJSON returns the lean AgentState JSON from SQLite.
func GetStateJSON() (string, error) {
	d := db.GetGlobal()
	if d == nil {
		return "", fmt.Errorf("db not initialised — call db.Open before host functions are invoked")
	}
	return d.GetStateJSON()
}

// SaveStateFromJSON upserts the AgentState JSON in SQLite.
func SaveStateFromJSON(jsonStr string) error {
	d := db.GetGlobal()
	if d == nil {
		return fmt.Errorf("db not initialised")
	}
	return d.SaveStateJSON(jsonStr)
}
