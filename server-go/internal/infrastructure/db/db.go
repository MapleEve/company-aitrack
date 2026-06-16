// Package db opens a PostgreSQL/ParadeDB connection and runs schema migrations.
package db

import (
	"database/sql"
	"fmt"

	_ "github.com/jackc/pgx/v5/stdlib"
)

// Open connects to PostgreSQL/ParadeDB using the given databaseURL and runs
// schema migrations. An optional func(*sql.DB) error may be passed as the
// second argument to override the default migration (used in tests).
func Open(databaseURL string, opts ...interface{}) (*sql.DB, error) {
	var customMigrateFn func(*sql.DB) error

	for _, opt := range opts {
		if fn, ok := opt.(func(*sql.DB) error); ok {
			customMigrateFn = fn
		}
	}

	database, err := sql.Open("pgx", databaseURL)
	if err != nil {
		return nil, fmt.Errorf("open postgres: %w", err)
	}

	fn := func(db *sql.DB) error { return migrate(db) }
	if customMigrateFn != nil {
		fn = customMigrateFn
	}
	if err := fn(database); err != nil {
		database.Close()
		return nil, fmt.Errorf("migrate: %w", err)
	}
	return database, nil
}

// migrate runs PostgreSQL/ParadeDB schema DDL.
func migrate(db *sql.DB) error {
	// Serialize concurrent DDL across parallel test packages.
	if _, err := db.Exec(`SELECT pg_advisory_lock(8675309)`); err != nil {
		return fmt.Errorf("advisory lock: %w", err)
	}
	defer db.Exec(`SELECT pg_advisory_unlock(8675309)`) //nolint:errcheck
	for _, stmt := range postgresDDL() {
		if _, err := db.Exec(stmt); err != nil {
			truncated := stmt
			if len(truncated) > 60 {
				truncated = truncated[:60]
			}
			return fmt.Errorf("migrate stmt %q: %w", truncated, err)
		}
	}
	return nil
}

func postgresDDL() []string {
	return []string{
		`CREATE TABLE IF NOT EXISTS tokens (
  id          BIGSERIAL PRIMARY KEY,
  token_hash  TEXT NOT NULL UNIQUE,
  token_key   TEXT NOT NULL,
  hmac_secret TEXT NOT NULL,
  owner       TEXT NOT NULL,
  note        TEXT,
  active      INTEGER NOT NULL DEFAULT 1,
  created_at  TEXT NOT NULL
)`,
		`CREATE INDEX IF NOT EXISTS idx_tokens_hash ON tokens(token_hash)`,

		`CREATE TABLE IF NOT EXISTS edit_records (
  id            BIGSERIAL PRIMARY KEY,
  token_key     TEXT    NOT NULL,
  device_id     TEXT    NOT NULL,
  hostname      TEXT    NOT NULL DEFAULT '',
  tool          TEXT    NOT NULL,
  tool_version  TEXT,
  provider      TEXT    NOT NULL,
  model         TEXT,
  session_id    TEXT    NOT NULL,
  repo_url      TEXT    NOT NULL,
  branch        TEXT    NOT NULL,
  current_sha   TEXT    NOT NULL,
  file_path     TEXT    NOT NULL,
  added_lines   INTEGER NOT NULL,
  removed_lines INTEGER NOT NULL,
  diff_hunk     TEXT,
  metadata      TEXT,
  timestamp     TEXT    NOT NULL,
  record_sig    TEXT    NOT NULL,
  status        TEXT    NOT NULL,
  flags         TEXT,
  received_at   TEXT    NOT NULL,
  prompt_summary TEXT,
  embedding      BYTEA
)`,
		`CREATE INDEX IF NOT EXISTS idx_edit_records_token_key   ON edit_records(token_key)`,
		`CREATE INDEX IF NOT EXISTS idx_edit_records_repo_url    ON edit_records(repo_url)`,
		`CREATE INDEX IF NOT EXISTS idx_edit_records_device_id   ON edit_records(device_id)`,
		`CREATE INDEX IF NOT EXISTS idx_edit_records_received_at ON edit_records(received_at)`,

		`CREATE TABLE IF NOT EXISTS devices (
  id             BIGSERIAL PRIMARY KEY,
  device_id      TEXT NOT NULL UNIQUE,
  token_key      TEXT NOT NULL,
  hostname       TEXT NOT NULL DEFAULT '',
  client_version TEXT,
  last_heartbeat TEXT,
  hooks_json     TEXT,
  created_at     TEXT NOT NULL
)`,
		`CREATE INDEX IF NOT EXISTS idx_devices_device_id ON devices(device_id)`,

		`CREATE TABLE IF NOT EXISTS usage_daily_rollups (
  id                  BIGSERIAL PRIMARY KEY,
  token_key           TEXT NOT NULL,
  device_id           TEXT NOT NULL,
  day                 TEXT NOT NULL,
  agent               TEXT NOT NULL,
  model               TEXT NOT NULL,
  account             TEXT NOT NULL DEFAULT '',
  tokens_in           BIGINT NOT NULL DEFAULT 0,
  tokens_out          BIGINT NOT NULL DEFAULT 0,
  tokens_cache_read   BIGINT NOT NULL DEFAULT 0,
  tokens_cache_write  BIGINT NOT NULL DEFAULT 0,
  tokens_reasoning    BIGINT NOT NULL DEFAULT 0,
  message_count       BIGINT NOT NULL DEFAULT 0,
  source_cost         DOUBLE PRECISION NOT NULL DEFAULT 0,
  updated_at          TEXT NOT NULL,
  UNIQUE(token_key, device_id, day, agent, model, account)
)`,
		`CREATE INDEX IF NOT EXISTS idx_usage_daily_rollups_token_key ON usage_daily_rollups(token_key)`,
		`CREATE INDEX IF NOT EXISTS idx_usage_daily_rollups_day ON usage_daily_rollups(day)`,
		`CREATE INDEX IF NOT EXISTS idx_usage_daily_rollups_agent ON usage_daily_rollups(agent)`,

		`CREATE TABLE IF NOT EXISTS usage_subscription_snapshots (
  id                        BIGSERIAL PRIMARY KEY,
  token_key                 TEXT NOT NULL,
  device_id                 TEXT NOT NULL,
  agent                     TEXT NOT NULL,
  account                   TEXT NOT NULL DEFAULT '',
  plan                      TEXT,
  quota_session_remaining   BIGINT,
  quota_weekly_remaining    BIGINT,
  quota_reset_at            TEXT,
  reader_status             TEXT NOT NULL,
  snapshotted_at            TEXT NOT NULL,
  updated_at                TEXT NOT NULL,
  UNIQUE(token_key, device_id, agent, account)
)`,
		`CREATE INDEX IF NOT EXISTS idx_usage_subscription_token_key ON usage_subscription_snapshots(token_key)`,

		// Additive migrations for existing PostgreSQL databases.
		`ALTER TABLE edit_records ADD COLUMN IF NOT EXISTS prompt_summary TEXT`,
		`ALTER TABLE edit_records ADD COLUMN IF NOT EXISTS embedding BYTEA`,
		`ALTER TABLE usage_daily_rollups ADD COLUMN IF NOT EXISTS message_count BIGINT NOT NULL DEFAULT 0`,
		`ALTER TABLE usage_daily_rollups ADD COLUMN IF NOT EXISTS source_cost DOUBLE PRECISION NOT NULL DEFAULT 0`,
	}
}
