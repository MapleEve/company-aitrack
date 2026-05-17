package db_test

import (
	"os"
	"path/filepath"
	"runtime"
	"testing"

	"github.com/aitrack/server/internal/db"
)

func TestOpen_InMemory(t *testing.T) {
	database, err := db.Open(":memory:")
	if err != nil {
		t.Fatalf("Open(:memory:) failed: %v", err)
	}
	defer database.Close()
	if err := database.Ping(); err != nil {
		t.Fatalf("Ping failed: %v", err)
	}
}

func TestOpen_CreatesDirectory(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "sub", "aitrack.db")

	database, err := db.Open(path)
	if err != nil {
		t.Fatalf("Open with sub-directory failed: %v", err)
	}
	defer database.Close()

	if _, err := os.Stat(filepath.Dir(path)); err != nil {
		t.Errorf("directory was not created: %v", err)
	}
}

func TestMigrate_TablesExist(t *testing.T) {
	database, err := db.Open(":memory:")
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()

	tables := []string{"tokens", "edit_records", "devices"}
	for _, table := range tables {
		var name string
		err := database.QueryRow(
			"SELECT name FROM sqlite_master WHERE type='table' AND name=?", table,
		).Scan(&name)
		if err != nil {
			t.Errorf("table %q not found: %v", table, err)
		}
	}
}

func TestMigrate_IndexesExist(t *testing.T) {
	database, err := db.Open(":memory:")
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()

	indexes := []string{
		"idx_tokens_hash",
		"idx_edit_records_token_key",
		"idx_edit_records_repo_url",
		"idx_edit_records_device_id",
		"idx_edit_records_received_at",
		"idx_devices_device_id",
	}
	for _, idx := range indexes {
		var name string
		err := database.QueryRow(
			"SELECT name FROM sqlite_master WHERE type='index' AND name=?", idx,
		).Scan(&name)
		if err != nil {
			t.Errorf("index %q not found: %v", idx, err)
		}
	}
}

func TestMigrate_Idempotent(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "aitrack.db")

	db1, err := db.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	db1.Close()

	// Open same file again — migrate runs again, must not error
	db2, err := db.Open(path)
	if err != nil {
		t.Fatalf("second open failed: %v", err)
	}
	db2.Close()
}

func TestTokensCRUD(t *testing.T) {
	database, err := db.Open(":memory:")
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()

	_, err = database.Exec(`
		INSERT INTO tokens (token_hash, token_key, hmac_secret, owner, note, active, created_at)
		VALUES ('hash1', 'key1', 'secret1', 'alice', 'note', 1, '2026-05-17T00:00:00Z')
	`)
	if err != nil {
		t.Fatalf("insert token: %v", err)
	}

	var hash string
	err = database.QueryRow("SELECT token_hash FROM tokens WHERE token_key = ?", "key1").Scan(&hash)
	if err != nil {
		t.Fatalf("query token: %v", err)
	}
	if hash != "hash1" {
		t.Errorf("got hash %q, want hash1", hash)
	}
}

func TestEditRecordsCRUD(t *testing.T) {
	database, err := db.Open(":memory:")
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()

	_, err = database.Exec(`
		INSERT INTO edit_records
		  (token_key, device_id, tool, provider, session_id, repo_url, branch,
		   current_sha, file_path, added_lines, removed_lines, timestamp, record_sig,
		   status, received_at)
		VALUES ('k1','d1','claude','anthropic','s1','repo','main','sha','f.rs',5,2,
		        '2026-05-17T00:00:00Z','sig1','ACCEPTED','2026-05-17T00:00:00Z')
	`)
	if err != nil {
		t.Fatalf("insert edit_record: %v", err)
	}

	var count int64
	database.QueryRow("SELECT COUNT(*) FROM edit_records WHERE token_key='k1'").Scan(&count)
	if count != 1 {
		t.Errorf("expected 1 edit record, got %d", count)
	}
}

func TestDevicesCRUD(t *testing.T) {
	database, err := db.Open(":memory:")
	if err != nil {
		t.Fatal(err)
	}
	defer database.Close()

	_, err = database.Exec(`
		INSERT INTO devices (device_id, token_key, client_version, created_at)
		VALUES ('dev-1', 'key1', '1.0.0', '2026-05-17T00:00:00Z')
	`)
	if err != nil {
		t.Fatalf("insert device: %v", err)
	}

	// Upsert via ON CONFLICT
	_, err = database.Exec(`
		INSERT INTO devices (device_id, token_key, client_version, created_at)
		VALUES ('dev-1', 'key1', '2.0.0', '2026-05-17T00:00:00Z')
		ON CONFLICT(device_id) DO UPDATE SET client_version = excluded.client_version
	`)
	if err != nil {
		t.Fatalf("upsert device: %v", err)
	}

	var version string
	database.QueryRow("SELECT client_version FROM devices WHERE device_id='dev-1'").Scan(&version)
	if version != "2.0.0" {
		t.Errorf("expected 2.0.0 after upsert, got %s", version)
	}
}

func TestOpen_MkdirAll_Error(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("permission model differs on Windows")
	}
	// Use a path under a file (not a dir) so MkdirAll fails.
	f, err := os.CreateTemp("", "notadir-*.db")
	if err != nil {
		t.Fatal(err)
	}
	f.Close()
	defer os.Remove(f.Name())

	// Try to open a db whose "directory" is actually a regular file.
	// filepath.Dir(f.Name()+"/sub/aitrack.db") will try to mkdir over f.Name()
	badPath := filepath.Join(f.Name(), "sub", "aitrack.db")
	_, err = db.Open(badPath)
	if err == nil {
		t.Error("expected error when parent path is a file")
	}
}

func TestOpen_FileDB_ReadWrite(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "test.db")

	database, err := db.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	_, err = database.Exec(`INSERT INTO tokens (token_hash,token_key,hmac_secret,owner,note,active,created_at) VALUES ('h','k','s','o','',1,'2026-01-01T00:00:00Z')`)
	if err != nil {
		t.Fatalf("insert into file db: %v", err)
	}
	database.Close()

	// Re-open and check data persisted
	database2, err := db.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer database2.Close()
	var count int
	database2.QueryRow("SELECT COUNT(*) FROM tokens").Scan(&count)
	if count != 1 {
		t.Errorf("expected 1 persisted row, got %d", count)
	}
}
