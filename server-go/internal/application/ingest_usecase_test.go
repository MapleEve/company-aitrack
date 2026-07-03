package application_test

import (
	"fmt"
	"strings"
	"testing"
	"time"

	dbadapter "github.com/aitrack/server/internal/adapter/db"
	"github.com/aitrack/server/internal/application"
	"github.com/aitrack/server/internal/domain/model"
	"github.com/aitrack/server/internal/domain/port"
	"github.com/aitrack/server/internal/domain/service"
	"github.com/aitrack/server/internal/testkit"
)

func newIngestSvc(t *testing.T, counter port.EditRecordCounter) (*application.IngestService, *dbadapter.EditRecordAdapter) {
	t.Helper()
	database := openTestDB(t)
	policy := service.ValidationPolicy{RateLimitPerHour: 30, MaxAddedLines: 5000}

	sig := service.NewSignatureService()
	diff := service.NewDiffConsistencyService()
	editRepo := dbadapter.NewEditRecordAdapter(database)

	var c port.EditRecordCounter = editRepo
	if counter != nil {
		c = counter
	}

	validation := service.NewValidationService(sig, diff, c, policy)
	validator := service.NewEditValidator()
	ingest := application.NewIngestService(validation, validator, editRepo)
	return ingest, editRepo
}

func TestIngest_AllAccepted(t *testing.T) {
	ingest, _ := newIngestSvc(t, nil)
	token := testkit.BuildTokenWithSig()
	req := testkit.BuildUploadRequest(token, testkit.BuildEditDTO())

	resp := ingest.Ingest(token, req)
	if resp.Accepted != 1 {
		t.Errorf("expected 1 accepted, got %d", resp.Accepted)
	}
	if len(resp.Rejected) != 0 {
		t.Errorf("expected 0 rejected, got %v", resp.Rejected)
	}
}

func TestIngest_TamperedSig_Rejected(t *testing.T) {
	ingest, _ := newIngestSvc(t, nil)
	token := testkit.BuildTokenWithSig()
	req := testkit.BuildUploadRequest(token, testkit.TamperedEditDTO())

	resp := ingest.Ingest(token, req)
	if resp.Accepted != 0 {
		t.Error("tampered sig should not be accepted")
	}
	if len(resp.Rejected) != 1 {
		t.Errorf("expected 1 rejected, got %v", resp.Rejected)
	}
	if resp.Rejected[0].Reason != "sig_mismatch" {
		t.Errorf("expected sig_mismatch, got %s", resp.Rejected[0].Reason)
	}
}

func TestIngest_Malformed_Rejected(t *testing.T) {
	ingest, _ := newIngestSvc(t, nil)
	token := testkit.BuildTokenWithSig()
	req := testkit.BuildUploadRequest(token, testkit.MalformedEditDTO())

	resp := ingest.Ingest(token, req)
	if len(resp.Rejected) != 1 {
		t.Errorf("expected 1 rejected, got %v", resp.Rejected)
	}
	if resp.Rejected[0].Reason != "malformed" {
		t.Errorf("expected malformed, got %s", resp.Rejected[0].Reason)
	}
}

func TestIngest_Oversized_Flagged(t *testing.T) {
	ingest, _ := newIngestSvc(t, nil)
	token := testkit.BuildTokenWithSig()
	req := testkit.BuildUploadRequest(token, testkit.OversizedEditDTO())

	resp := ingest.Ingest(token, req)
	if len(resp.Flagged) != 1 {
		t.Errorf("expected 1 flagged, got %v", resp.Flagged)
	}
}

func TestIngest_MixedBatch(t *testing.T) {
	ingest, _ := newIngestSvc(t, nil)
	token := testkit.BuildTokenWithSig()

	req := testkit.BuildUploadRequest(
		token,
		testkit.BuildEditDTO(),     // accepted
		testkit.TamperedEditDTO(),  // rejected
		testkit.OversizedEditDTO(), // flagged
	)

	resp := ingest.Ingest(token, req)
	if resp.Accepted != 1 {
		t.Errorf("expected 1 accepted, got %d", resp.Accepted)
	}
	if len(resp.Rejected) != 1 {
		t.Errorf("expected 1 rejected, got %v", resp.Rejected)
	}
	if len(resp.Flagged) != 1 {
		t.Errorf("expected 1 flagged, got %v", resp.Flagged)
	}
	// Check indices
	if resp.Rejected[0].Index != 1 {
		t.Errorf("rejected index should be 1, got %d", resp.Rejected[0].Index)
	}
	if resp.Flagged[0].Index != 2 {
		t.Errorf("flagged index should be 2, got %d", resp.Flagged[0].Index)
	}
}

func TestIngest_EmptyEdits_ResponseHasEmptySlices(t *testing.T) {
	ingest, _ := newIngestSvc(t, nil)
	token := testkit.BuildTokenWithSig()
	req := testkit.BuildUploadRequest(token)
	resp := ingest.Ingest(token, req)
	if resp.Rejected == nil {
		t.Error("Rejected should not be nil")
	}
	if resp.Flagged == nil {
		t.Error("Flagged should not be nil")
	}
}

func TestIngest_QueryEdits(t *testing.T) {
	ingest, _ := newIngestSvc(t, nil)
	token := testkit.BuildTokenWithSig()
	req := testkit.BuildUploadRequest(token, testkit.BuildEditDTO())
	ingest.Ingest(token, req)

	result, err := ingest.QueryEdits("", "", 0, 20)
	if err != nil {
		t.Fatal(err)
	}
	if result.Total < 1 {
		t.Errorf("expected at least 1 record, got %d", result.Total)
	}
}

func TestIngest_TruncatesLargeStoredTextFields(t *testing.T) {
	ingest, _ := newIngestSvc(t, nil)
	token := testkit.BuildTokenWithSig()
	diffHunk := "@@ -1,0 +1,0 @@\n" + strings.Repeat(" context\n", 9000)
	metadata := strings.Repeat("m", 5000)
	promptSummary := strings.Repeat("p", 5000)
	edit := testkit.BuildEditDTO(func(p *testkit.EditParams) {
		p.AddedLines = 0
		p.RemovedLines = 0
		p.DiffHunk = &diffHunk
	})
	edit.Metadata = &metadata
	edit.PromptSummary = &promptSummary

	resp := ingest.Ingest(token, testkit.BuildUploadRequest(token, edit))
	if resp.Accepted != 1 {
		t.Fatalf("expected accepted edit, got %+v", resp)
	}

	result, err := ingest.QueryEdits("", "", 0, 20)
	if err != nil {
		t.Fatal(err)
	}
	if len(result.Records) != 1 {
		t.Fatalf("expected one stored record, got %d", len(result.Records))
	}
	record := result.Records[0]
	if len([]rune(record.DiffHunk)) != 8192 {
		t.Fatalf("diff_hunk length = %d, want 8192", len([]rune(record.DiffHunk)))
	}
	if len([]rune(record.Metadata)) != 4096 {
		t.Fatalf("metadata length = %d, want 4096", len([]rune(record.Metadata)))
	}
	if record.PromptSummary == nil || len([]rune(*record.PromptSummary)) != 4096 {
		t.Fatalf("prompt_summary length mismatch")
	}
}

func TestIngest_DuplicateRecordSigIsIdempotent(t *testing.T) {
	ingest, _ := newIngestSvc(t, nil)
	token := testkit.BuildTokenWithSig()
	edit := testkit.BuildEditDTO()
	req := testkit.BuildUploadRequest(token, edit)

	first := ingest.Ingest(token, req)
	second := ingest.Ingest(token, req)
	if first.Accepted != 1 || second.Accepted != 1 {
		t.Fatalf("expected idempotent accepted responses, got first=%+v second=%+v", first, second)
	}

	result, err := ingest.QueryEdits("", "", 0, 20)
	if err != nil {
		t.Fatal(err)
	}
	if result.Total != 1 {
		t.Fatalf("duplicate record_sig should store one record, got %d", result.Total)
	}
}

func TestIngest_AppliesRawRetentionAfterSuccessfulSave(t *testing.T) {
	ingest, editRepo := newIngestSvc(t, nil)
	token := testkit.BuildTokenWithSig()
	now := time.Now().UTC()
	oldSigs := make(map[string]bool)

	for i := 0; i < 12; i++ {
		promptSummary := fmt.Sprintf("old prompt summary %02d", i)
		recordSig := fmt.Sprintf("old-retention-%02d-%046d", i, i)
		oldSigs[recordSig] = true
		rec := testkit.BuildEditRecord(func(r *model.EditRecord) {
			r.RecordSig = recordSig
			r.DeviceID = fmt.Sprintf("old-device-%02d", i)
			r.DiffHunk = fmt.Sprintf("@@ -1 +1 @@\n-old-%02d\n+new-%02d\n", i, i)
			r.Metadata = fmt.Sprintf(`{"old":%d}`, i)
			r.PromptSummary = &promptSummary
			r.ReceivedAt = now.AddDate(0, 0, -31).Add(time.Duration(i) * time.Minute)
		})
		if err := editRepo.Save(rec); err != nil {
			t.Fatalf("seed old record: %v", err)
		}
	}

	diffHunk := "@@ -1,0 +1,0 @@\n" + strings.Repeat(" context\n", 9000)
	metadata := strings.Repeat("m", 5000)
	promptSummary := strings.Repeat("p", 5000)
	newEdit := testkit.BuildEditDTO(func(p *testkit.EditParams) {
		p.AddedLines = 0
		p.RemovedLines = 0
		p.DiffHunk = &diffHunk
	})
	newEdit.Metadata = &metadata
	newEdit.PromptSummary = &promptSummary

	resp := ingest.Ingest(token, testkit.BuildUploadRequest(token, newEdit))
	if resp.Accepted != 1 || len(resp.Rejected) != 0 {
		t.Fatalf("expected accepted edit without retention error, got %+v", resp)
	}
	duplicate := ingest.Ingest(token, testkit.BuildUploadRequest(token, newEdit))
	if duplicate.Accepted != 1 || len(duplicate.Rejected) != 0 {
		t.Fatalf("expected duplicate record_sig to remain idempotent, got %+v", duplicate)
	}

	result, err := ingest.QueryEdits("", "", 0, 50)
	if err != nil {
		t.Fatal(err)
	}
	if result.Total != 13 {
		t.Fatalf("expected 12 old rows plus one deduped new row, got %d", result.Total)
	}

	var oldCount, newCount int
	for _, record := range result.Records {
		if oldSigs[record.RecordSig] {
			oldCount++
			if record.DiffHunk != dbadapter.RawFieldStrippedMarker {
				t.Fatalf("old diff_hunk was not stripped: %q", record.DiffHunk)
			}
			if record.Metadata != dbadapter.RawFieldStrippedMarker {
				t.Fatalf("old metadata was not stripped: %q", record.Metadata)
			}
			if record.PromptSummary == nil || *record.PromptSummary != dbadapter.RawFieldStrippedMarker {
				t.Fatalf("old prompt_summary was not stripped: %v", record.PromptSummary)
			}
			if record.RecordSig == "" || record.TokenKey == "" || record.DeviceID == "" || record.FilePath == "" {
				t.Fatalf("retention removed scalar identity fields: %+v", record)
			}
			if record.AddedLines == 0 && record.RemovedLines == 0 {
				t.Fatalf("retention removed line counts: %+v", record)
			}
			continue
		}
		if record.RecordSig == newEdit.RecordSig {
			newCount++
			if len([]rune(record.DiffHunk)) != 8192 {
				t.Fatalf("recent diff_hunk length = %d, want 8192", len([]rune(record.DiffHunk)))
			}
			if len([]rune(record.Metadata)) != 4096 {
				t.Fatalf("recent metadata length = %d, want 4096", len([]rune(record.Metadata)))
			}
			if record.PromptSummary == nil || len([]rune(*record.PromptSummary)) != 4096 {
				t.Fatalf("recent prompt_summary length mismatch")
			}
		}
	}
	if oldCount != 12 {
		t.Fatalf("expected 12 stripped old rows, got %d", oldCount)
	}
	if newCount != 1 {
		t.Fatalf("expected one retained recent row, got %d", newCount)
	}
}
