package db

import (
	"database/sql"
	"errors"
	"testing"
)

// TestMigrateOnClosedDB exercises the migrate function directly with a closed db.
func TestMigrateOnClosedDB(t *testing.T) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	db.Close()

	if err := migrate(db); err == nil {
		t.Error("expected migrate to fail on closed db")
	}
}

// TestOpen_MigrateError exercises the branch in Open where migrate returns an error.
// We inject a failing migrateFn to hit the db.Close() + return error path.
func TestOpen_MigrateError(t *testing.T) {
	failMigrate := func(*sql.DB) error {
		return errors.New("injected migrate failure")
	}
	_, err := Open(":memory:", failMigrate)
	if err == nil {
		t.Fatal("expected Open to return error when migrate fails")
	}
	if err.Error() == "" {
		t.Error("error should have a message")
	}
}
