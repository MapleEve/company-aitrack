package dbadapter_test

import (
	"database/sql"
	"fmt"
	"os"
	"testing"
	"time"

	dbadapter "github.com/aitrack/server/internal/adapter/db"
	dbpkg "github.com/aitrack/server/internal/infrastructure/db"
	"github.com/aitrack/server/internal/testkit"

	_ "github.com/jackc/pgx/v5/stdlib"
)

func retentionTestDBURL() string {
	if u := os.Getenv("TEST_DATABASE_URL"); u != "" {
		return u
	}
	return "postgres://aitrack:aitrack_secret@localhost:5432/aitrack_test?sslmode=disable"
}

func TestMain(m *testing.M) {
	conn, err := sql.Open("pgx", retentionTestDBURL())
	if err != nil || conn.Ping() != nil {
		fmt.Println("SKIP: TEST_DATABASE_URL not reachable, skipping adapter/db tests")
		os.Exit(0)
	}
	conn.Close()
	os.Exit(m.Run())
}

func openRetentionTestDB(t *testing.T) *sql.DB {
	t.Helper()
	database, err := dbpkg.Open(retentionTestDBURL())
	if err != nil {
		t.Fatalf("open test db: %v", err)
	}
	if _, err := database.Exec(`TRUNCATE TABLE edit_records RESTART IDENTITY CASCADE`); err != nil {
		database.Close()
		t.Fatalf("truncate edit_records: %v", err)
	}
	t.Cleanup(func() { database.Close() })
	return database
}

func TestApplyRawRetentionWithPolicyStripsOverflowRows(t *testing.T) {
	database := openRetentionTestDB(t)
	repo := dbadapter.NewEditRecordAdapter(database)
	now := time.Date(2026, 6, 29, 12, 0, 0, 0, time.UTC)

	for i := 0; i < 5; i++ {
		promptSummary := fmt.Sprintf("prompt summary %d", i)
		rec := testkit.BuildEditRecord()
		rec.RecordSig = fmt.Sprintf("overflow-retention-%02d-%041d", i, i)
		rec.DiffHunk = fmt.Sprintf("@@ -1 +1 @@\n-old-%d\n+new-%d\n", i, i)
		rec.Metadata = fmt.Sprintf(`{"i":%d}`, i)
		rec.PromptSummary = &promptSummary
		rec.ReceivedAt = now.Add(-time.Duration(i) * time.Minute)
		if err := repo.Save(rec); err != nil {
			t.Fatalf("seed record %d: %v", i, err)
		}
	}

	stripped, err := repo.ApplyRawRetentionWithPolicy(now, dbadapter.EditRawRetentionPolicy{
		RawRetentionWindowDays: 365,
		MaxRawRows:             2,
	})
	if err != nil {
		t.Fatal(err)
	}
	if stripped != 3 {
		t.Fatalf("stripped rows = %d, want 3", stripped)
	}

	records, total, err := repo.Query("", "", 0, 10)
	if err != nil {
		t.Fatal(err)
	}
	if total != 5 {
		t.Fatalf("retention should not delete rows, got total=%d", total)
	}
	for i, record := range records {
		if i < 2 {
			if record.DiffHunk == dbadapter.RawFieldStrippedMarker {
				t.Fatalf("recent overflow-retained record was stripped: %+v", record)
			}
			continue
		}
		if record.DiffHunk != dbadapter.RawFieldStrippedMarker {
			t.Fatalf("overflow record diff_hunk was not stripped: %+v", record)
		}
		if record.Metadata != dbadapter.RawFieldStrippedMarker {
			t.Fatalf("overflow record metadata was not stripped: %+v", record)
		}
		if record.PromptSummary == nil || *record.PromptSummary != dbadapter.RawFieldStrippedMarker {
			t.Fatalf("overflow record prompt_summary was not stripped: %+v", record)
		}
		if record.RecordSig == "" || record.TokenKey == "" || record.FilePath == "" {
			t.Fatalf("retention removed scalar identity fields: %+v", record)
		}
	}
}
