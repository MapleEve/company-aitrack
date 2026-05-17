package service_test

import (
	"testing"
	"time"

	"github.com/aitrack/server/internal/model"
	"github.com/aitrack/server/internal/service"
)

// Tests for AggregateByRepo, AggregateByDevice, and Query edge-cases.

func insertEditRecord(t *testing.T, repo *service.EditRecordRepository, status, tokenKey, repoURL, deviceID string) {
	t.Helper()
	insertEditRecordWithHostname(t, repo, status, tokenKey, repoURL, deviceID, "test-host.local")
}

func insertEditRecordWithHostname(t *testing.T, repo *service.EditRecordRepository, status, tokenKey, repoURL, deviceID, hostname string) {
	t.Helper()
	n := int64(5)
	rec := &model.EditRecord{
		TokenKey:     tokenKey,
		DeviceID:     deviceID,
		Hostname:     hostname,
		Tool:         "claude",
		Provider:     "anthropic",
		SessionID:    "sess-001",
		RepoURL:      repoURL,
		Branch:       "main",
		CurrentSHA:   "abc123",
		FilePath:     "src/main.rs",
		AddedLines:   n,
		RemovedLines: 2,
		Timestamp:    "2026-05-17T10:00:00Z",
		RecordSig:    "sig123" + status,
		Status:       status,
		ReceivedAt:   time.Now().UTC(),
	}
	if err := repo.Save(rec); err != nil {
		t.Fatalf("insert edit_record: %v", err)
	}
}

func TestAggregateByRepo(t *testing.T) {
	db := openTestDB(t)
	repo := service.NewEditRecordRepository(db)
	insertEditRecord(t, repo, "ACCEPTED", "k1", "git@github.com:org/repo.git", "dev1")
	insertEditRecord(t, repo, "FLAGGED", "k1", "git@github.com:org/repo.git", "dev1")
	insertEditRecord(t, repo, "REJECTED", "k2", "git@github.com:org/other.git", "dev2")

	rows, err := repo.AggregateByRepo()
	if err != nil {
		t.Fatal(err)
	}
	if len(rows) < 2 {
		t.Errorf("expected 2 repos, got %d", len(rows))
	}
}

func TestAggregateByDevice(t *testing.T) {
	db := openTestDB(t)
	repo := service.NewEditRecordRepository(db)
	insertEditRecord(t, repo, "ACCEPTED", "k1", "git@github.com:org/repo.git", "dev-A")
	insertEditRecord(t, repo, "ACCEPTED", "k1", "git@github.com:org/repo.git", "dev-B")

	rows, err := repo.AggregateByDevice()
	if err != nil {
		t.Fatal(err)
	}
	if len(rows) < 2 {
		t.Errorf("expected 2 devices, got %d", len(rows))
	}
}

func TestAggregateByHostname(t *testing.T) {
	db := openTestDB(t)
	repo := service.NewEditRecordRepository(db)
	insertEditRecordWithHostname(t, repo, "ACCEPTED", "k1", "git@github.com:org/repo.git", "dev1", "host-A.local")
	insertEditRecordWithHostname(t, repo, "ACCEPTED", "k1", "git@github.com:org/repo.git", "dev2", "host-B.local")
	insertEditRecordWithHostname(t, repo, "FLAGGED", "k1", "git@github.com:org/repo.git", "dev3", "host-A.local")

	rows, err := repo.AggregateByHostname()
	if err != nil {
		t.Fatal(err)
	}
	if len(rows) < 2 {
		t.Errorf("expected 2 hostname groups, got %d", len(rows))
	}
}

func TestAggregateByTokenKey(t *testing.T) {
	db := openTestDB(t)
	repo := service.NewEditRecordRepository(db)
	insertEditRecord(t, repo, "ACCEPTED", "tok1", "git@github.com:org/repo.git", "dev1")
	insertEditRecord(t, repo, "REJECTED", "tok2", "git@github.com:org/repo.git", "dev2")

	rows, err := repo.AggregateByTokenKey()
	if err != nil {
		t.Fatal(err)
	}
	if len(rows) < 2 {
		t.Errorf("expected 2 token rows, got %d", len(rows))
	}
}

func TestQuery_WithFilters(t *testing.T) {
	db := openTestDB(t)
	repo := service.NewEditRecordRepository(db)
	insertEditRecord(t, repo, "ACCEPTED", "tok-filter", "git@github.com:org/repo.git", "dev1")
	insertEditRecord(t, repo, "ACCEPTED", "tok-other", "git@github.com:org/other.git", "dev2")

	// Filter by tokenKey
	records, total, err := repo.Query("tok-filter", "", 0, 10)
	if err != nil {
		t.Fatal(err)
	}
	if total != 1 {
		t.Errorf("token filter: expected 1, got %d", total)
	}
	if len(records) != 1 {
		t.Errorf("token filter records: expected 1, got %d", len(records))
	}

	// Filter by repoURL
	records2, total2, err := repo.Query("", "git@github.com:org/other.git", 0, 10)
	if err != nil {
		t.Fatal(err)
	}
	if total2 != 1 {
		t.Errorf("repo filter: expected 1, got %d", total2)
	}
	_ = records2

	// No filters
	_, totalAll, err := repo.Query("", "", 0, 10)
	if err != nil {
		t.Fatal(err)
	}
	if totalAll != 2 {
		t.Errorf("no filter: expected 2, got %d", totalAll)
	}
}

func TestQuery_Pagination(t *testing.T) {
	db := openTestDB(t)
	repo := service.NewEditRecordRepository(db)
	for i := 0; i < 5; i++ {
		insertEditRecord(t, repo, "ACCEPTED", "k1", "git@github.com:org/repo.git", "dev1")
	}

	records, total, err := repo.Query("", "", 0, 2)
	if err != nil {
		t.Fatal(err)
	}
	if total != 5 {
		t.Errorf("total = %d, want 5", total)
	}
	if len(records) != 2 {
		t.Errorf("page size = %d, want 2", len(records))
	}
}

func TestCountByTokenKeyAndFilePath(t *testing.T) {
	db := openTestDB(t)
	repo := service.NewEditRecordRepository(db)
	insertEditRecord(t, repo, "ACCEPTED", "k1", "git@github.com:org/repo.git", "dev1")
	insertEditRecord(t, repo, "ACCEPTED", "k1", "git@github.com:org/repo.git", "dev1")

	count, err := repo.CountByTokenKeyAndFilePathSince("k1", "src/main.rs", time.Now().Add(-1*time.Hour))
	if err != nil {
		t.Fatal(err)
	}
	if count != 2 {
		t.Errorf("count = %d, want 2", count)
	}
}

func TestStatsService_AllGroupBys(t *testing.T) {
	db := openTestDB(t)
	editRepo := service.NewEditRecordRepository(db)
	deviceRepo := service.NewDeviceRepository(db)
	statsSvc := service.NewStatsService(editRepo, deviceRepo)

	insertEditRecord(t, editRepo, "ACCEPTED", "tok1", "git@github.com:org/repo.git", "dev1")
	insertEditRecord(t, editRepo, "FLAGGED", "tok2", "git@github.com:org/other.git", "dev2")
	insertEditRecord(t, editRepo, "REJECTED", "tok1", "git@github.com:org/repo.git", "dev1")

	for _, groupBy := range []string{"token", "repo", "device", "hostname", "other"} {
		rows, err := statsSvc.GetStats(groupBy)
		if err != nil {
			t.Fatalf("GetStats(%q): %v", groupBy, err)
		}
		if rows == nil {
			t.Errorf("GetStats(%q) returned nil", groupBy)
		}
	}
}
