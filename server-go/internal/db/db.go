package db

import (
	"database/sql"
	"fmt"
	"os"
	"path/filepath"

	_ "modernc.org/sqlite"
)

// Open opens (or creates) the SQLite database at path and runs schema migrations.
// An optional migrateFn can be injected for testing; production callers omit it.
func Open(path string, migrateFns ...func(*sql.DB) error) (*sql.DB, error) {
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		return nil, fmt.Errorf("create db dir: %w", err)
	}
	db, err := sql.Open("sqlite", path+"?_journal=WAL&_busy_timeout=5000")
	if err != nil {
		return nil, err
	}
	db.SetMaxOpenConns(1) // SQLite write serialization

	fn := migrate
	if len(migrateFns) > 0 {
		fn = migrateFns[0]
	}
	if err := fn(db); err != nil {
		db.Close()
		return nil, fmt.Errorf("migrate: %w", err)
	}
	return db, nil
}

func migrate(db *sql.DB) error {
	_, err := db.Exec(`
CREATE TABLE IF NOT EXISTS tokens (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  token_hash TEXT    NOT NULL UNIQUE,
  token_key  TEXT    NOT NULL,
  hmac_secret TEXT   NOT NULL,
  owner      TEXT    NOT NULL,
  note       TEXT,
  active     INTEGER NOT NULL DEFAULT 1,
  created_at TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_tokens_hash ON tokens(token_hash);

CREATE TABLE IF NOT EXISTS edit_records (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
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
  received_at   TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_edit_records_token_key   ON edit_records(token_key);
CREATE INDEX IF NOT EXISTS idx_edit_records_repo_url    ON edit_records(repo_url);
CREATE INDEX IF NOT EXISTS idx_edit_records_device_id   ON edit_records(device_id);
CREATE INDEX IF NOT EXISTS idx_edit_records_received_at ON edit_records(received_at);

CREATE TABLE IF NOT EXISTS devices (
  id             INTEGER PRIMARY KEY AUTOINCREMENT,
  device_id      TEXT    NOT NULL UNIQUE,
  token_key      TEXT    NOT NULL,
  hostname       TEXT    NOT NULL DEFAULT '',
  client_version TEXT,
  last_heartbeat TEXT,
  hooks_json     TEXT,
  created_at     TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_devices_device_id ON devices(device_id);
`)
	return err
}
