package db

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"time"

	_ "modernc.org/sqlite"
)

var (
	globalDB *DB
	initOnce sync.Once
)

// DB wraps sql.DB with the agent-staking schema and a write mutex.
type DB struct {
	sql *sql.DB
	mu  sync.Mutex
}

// Open opens (or creates) the SQLite database at dbPath and runs migrations.
// The first successful call also sets the package-level singleton.
func Open(dbPath string) (*DB, error) {
	if err := os.MkdirAll(filepath.Dir(dbPath), 0755); err != nil {
		return nil, fmt.Errorf("create db dir: %w", err)
	}

	sqlDB, err := sql.Open("sqlite", dbPath)
	if err != nil {
		return nil, fmt.Errorf("open sqlite: %w", err)
	}
	sqlDB.SetMaxOpenConns(1) // SQLite: serialise writes via mutex

	d := &DB{sql: sqlDB}
	if err := d.migrate(); err != nil {
		sqlDB.Close()
		return nil, fmt.Errorf("migrate: %w", err)
	}

	initOnce.Do(func() { globalDB = d })
	return d, nil
}

// GetGlobal returns the singleton opened by the first Open call.
func GetGlobal() *DB { return globalDB }

// Close shuts down the database connection.
func (d *DB) Close() error { return d.sql.Close() }

// ── Schema ────────────────────────────────────────────────────────────────────

func (d *DB) migrate() error {
	d.mu.Lock()
	defer d.mu.Unlock()

	if _, err := d.sql.Exec(`PRAGMA journal_mode = WAL;`); err != nil {
		return err
	}

	_, err := d.sql.Exec(`
		CREATE TABLE IF NOT EXISTS agent_state (
			id         INTEGER PRIMARY KEY DEFAULT 1,
			state_json TEXT    NOT NULL DEFAULT '{}',
			updated_at INTEGER NOT NULL DEFAULT 0
		);

		CREATE TABLE IF NOT EXISTS activity_batches (
			batch_hash     TEXT    PRIMARY KEY,
			activity_count INTEGER NOT NULL DEFAULT 0,
			period_start   INTEGER NOT NULL DEFAULT 0,
			period_end     INTEGER NOT NULL DEFAULT 0,
			committed_at   INTEGER NOT NULL DEFAULT 0,
			full_json      TEXT    NOT NULL DEFAULT '{}'
		);

		CREATE TABLE IF NOT EXISTS activity_records (
			id            INTEGER PRIMARY KEY AUTOINCREMENT,
			activity_type TEXT    NOT NULL,
			activity_data TEXT    NOT NULL DEFAULT '{}',
			timestamp     INTEGER NOT NULL,
			tx_ref        TEXT    NOT NULL DEFAULT '',
			batch_hash    TEXT
		);

		CREATE INDEX IF NOT EXISTS idx_records_ts    ON activity_records(timestamp);
		CREATE INDEX IF NOT EXISTS idx_records_type  ON activity_records(activity_type);
		CREATE INDEX IF NOT EXISTS idx_records_batch ON activity_records(batch_hash);
	`)
	return err
}

// ── Agent state ───────────────────────────────────────────────────────────────

// GetStateJSON returns the current lean AgentState JSON, or "{}" if not yet set.
func (d *DB) GetStateJSON() (string, error) {
	d.mu.Lock()
	defer d.mu.Unlock()

	var s string
	err := d.sql.QueryRow(`SELECT state_json FROM agent_state WHERE id = 1`).Scan(&s)
	if err == sql.ErrNoRows {
		return "{}", nil
	}
	return s, err
}

// SaveStateJSON upserts the agent state JSON.
func (d *DB) SaveStateJSON(stateJSON string) error {
	d.mu.Lock()
	defer d.mu.Unlock()

	now := time.Now().UnixMilli()
	_, err := d.sql.Exec(`
		INSERT INTO agent_state (id, state_json, updated_at)
		VALUES (1, ?, ?)
		ON CONFLICT(id) DO UPDATE SET
			state_json = excluded.state_json,
			updated_at = excluded.updated_at
	`, stateJSON, now)
	return err
}

// ── Batch storage ─────────────────────────────────────────────────────────────

// AppendBatch stores a full batch JSON blob keyed by its SHA-256 hash.
// Silently ignores duplicate hashes (idempotent).
func (d *DB) AppendBatch(batchHash, fullJSON string) error {
	d.mu.Lock()
	defer d.mu.Unlock()

	var meta struct {
		ActivityCount int64 `json:"activity_count"`
		PeriodStart   int64 `json:"period_start"`
		PeriodEnd     int64 `json:"period_end"`
	}
	_ = json.Unmarshal([]byte(fullJSON), &meta) // best-effort

	now := time.Now().UnixMilli()
	_, err := d.sql.Exec(`
		INSERT OR IGNORE INTO activity_batches
			(batch_hash, activity_count, period_start, period_end, committed_at, full_json)
		VALUES (?, ?, ?, ?, ?, ?)
	`, batchHash, meta.ActivityCount, meta.PeriodStart, meta.PeriodEnd, now, fullJSON)
	return err
}

// GetBatch retrieves full batch JSON by hash. Returns ("", nil) if not found.
func (d *DB) GetBatch(batchHash string) (string, error) {
	d.mu.Lock()
	defer d.mu.Unlock()

	var s string
	err := d.sql.QueryRow(
		`SELECT full_json FROM activity_batches WHERE batch_hash = ?`, batchHash,
	).Scan(&s)
	if err == sql.ErrNoRows {
		return "", nil
	}
	return s, err
}

// ── Single-record storage ─────────────────────────────────────────────────────

// AppendActivity parses and inserts a single activity-record JSON.
func (d *DB) AppendActivity(activityJSON string) error {
	d.mu.Lock()
	defer d.mu.Unlock()

	var r struct {
		ActivityType string `json:"activity_type"`
		ActivityData string `json:"activity_data"`
		Timestamp    int64  `json:"timestamp"`
		TxRef        string `json:"tx_ref"`
	}
	if err := json.Unmarshal([]byte(activityJSON), &r); err != nil {
		return fmt.Errorf("parse activity JSON: %w", err)
	}

	_, err := d.sql.Exec(`
		INSERT INTO activity_records (activity_type, activity_data, timestamp, tx_ref)
		VALUES (?, ?, ?, ?)
	`, r.ActivityType, r.ActivityData, r.Timestamp, r.TxRef)
	return err
}

// ── Query helpers ─────────────────────────────────────────────────────────────

// GetActivityCountInRange returns the count of activity_records with timestamp
// in [fromTs, toTs] (unix ms). Used by WASM for reputation velocity scoring.
func (d *DB) GetActivityCountInRange(fromTs, toTs uint64) (int64, error) {
	d.mu.Lock()
	defer d.mu.Unlock()

	var count int64
	err := d.sql.QueryRow(`
		SELECT COUNT(*) FROM activity_records
		WHERE timestamp >= ? AND timestamp <= ?
	`, int64(fromTs), int64(toTs)).Scan(&count)
	return count, err
}

// ActivityRow is returned by QueryActivities.
type ActivityRow struct {
	ID           int64
	ActivityType string
	ActivityData string
	Timestamp    int64
	TxRef        string
	BatchHash    string
}

// QueryActivities returns activity_records in a time range, newest-first.
// limit is clamped to [1, 1000].
func (d *DB) QueryActivities(fromTs, toTs int64, limit int) ([]ActivityRow, error) {
	d.mu.Lock()
	defer d.mu.Unlock()

	if limit <= 0 || limit > 1000 {
		limit = 100
	}
	if toTs <= 0 {
		toTs = time.Now().UnixMilli()
	}

	rows, err := d.sql.Query(`
		SELECT id, activity_type, activity_data, timestamp, tx_ref,
		       COALESCE(batch_hash, '') AS batch_hash
		FROM   activity_records
		WHERE  timestamp >= ? AND timestamp <= ?
		ORDER  BY timestamp DESC
		LIMIT  ?
	`, fromTs, toTs, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var out []ActivityRow
	for rows.Next() {
		var r ActivityRow
		if err := rows.Scan(&r.ID, &r.ActivityType, &r.ActivityData,
			&r.Timestamp, &r.TxRef, &r.BatchHash); err != nil {
			return nil, err
		}
		out = append(out, r)
	}
	return out, rows.Err()
}
