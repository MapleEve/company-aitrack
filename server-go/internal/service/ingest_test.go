package service_test

import (
	"testing"

	"github.com/aitrack/server/internal/config"
	"github.com/aitrack/server/internal/service"
	"github.com/aitrack/server/internal/testkit"
)

func newIngestSvc(t *testing.T, counter service.EditRecordCounter) (*service.IngestService, *service.EditRecordRepository) {
	t.Helper()
	database := openTestDB(t)
	cfg := &config.Config{}
	cfg.RateLimitPerHour = 30
	cfg.MaxAddedLines = 5000

	sig := service.NewSignatureService()
	diff := service.NewDiffConsistencyService()
	editRepo := service.NewEditRecordRepository(database)

	var c service.EditRecordCounter = editRepo
	if counter != nil {
		c = counter
	}

	validation := service.NewValidationService(sig, diff, c, cfg)
	validator := service.NewEditValidator()
	ingest := service.NewIngestService(validation, validator, editRepo)
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

	edits := []*testkit.EditParams{}
	_ = edits
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
