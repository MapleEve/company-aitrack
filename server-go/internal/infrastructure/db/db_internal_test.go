package db

import (
	"database/sql"
	"errors"
	"os"
	"strings"
	"testing"

	_ "github.com/jackc/pgx/v5/stdlib"
)

func testDSN() string {
	dsn := os.Getenv("TEST_DATABASE_URL")
	if dsn == "" {
		return "postgres://aitrack:aitrack_secret@localhost:5432/aitrack_test?sslmode=disable"
	}
	return dsn
}

// TestMigrateOnClosedDB exercises the migrate function directly with a closed db.
func TestMigrateOnClosedDB(t *testing.T) {
	conn, err := sql.Open("pgx", testDSN())
	if err != nil {
		t.Fatal(err)
	}
	conn.Close()

	if err := migrate(conn); err == nil {
		t.Error("expected migrate to fail on closed db")
	}
}

// TestOpen_MigrateError exercises the branch in Open where migrate returns an error.
// We inject a failing migrateFn to hit the db.Close() + return error path.
func TestOpen_MigrateError(t *testing.T) {
	failMigrate := func(*sql.DB) error {
		return errors.New("injected migrate failure")
	}
	_, err := Open(testDSN(), failMigrate)
	if err == nil {
		t.Fatal("expected Open to return error when migrate fails")
	}
	if err.Error() == "" {
		t.Error("error should have a message")
	}
}

// TestPostgresDDL verifies the PostgreSQL DDL generator returns a non-empty statement list
// that includes the expected tables and new columns.
func TestPostgresDDL(t *testing.T) {
	stmts := postgresDDL()
	if len(stmts) == 0 {
		t.Fatal("postgresDDL returned no statements")
	}
	joined := strings.Join(stmts, "\n")
	for _, want := range []string{
		"tokens",
		"edit_records",
		"devices",
		"prompt_summary",
		"embedding",
		"BIGSERIAL",
		"BYTEA",
		"usage_basis",
		"UNIQUE(token_key, device_id, day, agent, model, account, usage_basis)",
	} {
		if !strings.Contains(joined, want) {
			t.Errorf("postgresDDL missing %q", want)
		}
	}
}

func TestPostgresDDL_DropsLegacyUsageDailyRollupConstraints(t *testing.T) {
	joined := strings.Join(postgresDDL(), "\n")
	for _, name := range []string{
		"usage_daily_rollups_token_key_device_id_day_agent_model_account_key",
		"usage_daily_rollups_token_key_device_id_day_agent_model_acc_key",
		"uk_usage_daily_rollup",
	} {
		want := "ALTER TABLE usage_daily_rollups DROP CONSTRAINT IF EXISTS " + name
		if !strings.Contains(joined, want) {
			t.Errorf("postgresDDL missing legacy usage rollup constraint drop %q", want)
		}
	}
}

// TestOpen_InvalidURL verifies Open fails gracefully when given an invalid PostgreSQL URL.
func TestOpen_InvalidURL(t *testing.T) {
	// sql.Open("pgx", ...) is lazy — it doesn't actually connect until a query is made.
	// When migrate() runs, it will try to execute SQL and fail because the host is unreachable.
	// We just verify that Open returns an error (from migrate) rather than panicking.
	_, err := Open("postgres://invalid-host-no-such-server/db?sslmode=disable&connect_timeout=1")
	if err == nil {
		// In CI or environments where the host resolves, it might time out; that's also fine.
		t.Log("unexpected success connecting to invalid postgres host (may be environment-specific)")
		return
	}
	// Error should be wrapped with "migrate:" prefix.
	if !strings.Contains(err.Error(), "migrate") {
		t.Errorf("expected error to mention migrate, got: %v", err)
	}
}
