// coverage_test.go covers remaining uncovered branches across service package.
package service_test

import (
	"testing"

	"github.com/aitrack/server/internal/config"
	"github.com/aitrack/server/internal/model"
	"github.com/aitrack/server/internal/service"
	"github.com/aitrack/server/internal/testkit"
)

// ── Token boolToInt / inactive branch ────────────────────────────────────────

func TestBoolToInt_ViaActiveToken(t *testing.T) {
	// boolToInt(false) is exercised when active=false is stored.
	// We test it indirectly by creating an inactive token and checking it returns nil.
	database := openTestDB(t)
	enc, _ := service.NewHmacSecretEncryptor("")
	sig := service.NewSignatureService()
	repo := service.NewTokenRepository(database)
	svc := service.NewTokenService(repo, sig, enc)

	resp, _ := svc.CreateToken(&model.CreateTokenRequest{Owner: "test"})
	// Force active=false
	database.Exec("UPDATE tokens SET active=0")
	found, err := svc.FindActiveToken(resp.Token)
	if err != nil {
		t.Fatal(err)
	}
	if found != nil {
		t.Error("inactive token should return nil")
	}
}

// ── Ingest saveEdit with optional fields ─────────────────────────────────────

func TestIngest_SaveEditWithAllOptionalFields(t *testing.T) {
	database := openTestDB(t)
	cfg := &config.Config{}
	cfg.RateLimitPerHour = 30
	cfg.MaxAddedLines = 5000

	sig := service.NewSignatureService()
	diff := service.NewDiffConsistencyService()
	editRepo := service.NewEditRecordRepository(database)
	validation := service.NewValidationService(sig, diff, editRepo, cfg)
	validator := service.NewEditValidator()
	ingest := service.NewIngestService(validation, validator, editRepo)

	token := testkit.BuildTokenWithSig()

	// Build edit with model, diffhunk, and metadata set
	modelStr := "claude-3-5-sonnet"
	meta := `{"key":"value"}`
	diff_ := "@@ -1 +1 @@\n-old\n+new\n"
	edit := testkit.BuildEditDTO(func(p *testkit.EditParams) {
		p.DiffHunk = &diff_
		p.AddedLines = 1
		p.RemovedLines = 1
	})
	edit.Model = &modelStr
	edit.Metadata = &meta

	req := &model.EditBatchRequest{
		DeviceID: "dev-001",
		Edits:    []model.EditDTO{*edit},
	}
	resp := ingest.Ingest(token, req)
	if resp.Accepted != 1 {
		t.Errorf("expected 1 accepted, got %d; rejected=%v flagged=%v",
			resp.Accepted, resp.Rejected, resp.Flagged)
	}
}

// ── EditValidator nil edit ────────────────────────────────────────────────────

func TestEditValidator_NilEdit(t *testing.T) {
	v := service.NewEditValidator()
	if reason := v.Validate(nil); reason != "malformed" {
		t.Errorf("nil edit: expected malformed, got %q", reason)
	}
}

// ── isPathMismatch edge cases ─────────────────────────────────────────────────

func TestValidation_PathMismatch_LocalRepo(t *testing.T) {
	// Absolute path with local repo URL → not a mismatch
	svc := defaultValidationSvc(&fakeCounter{0})
	token := testkit.BuildTokenWithSig()
	edit := testkit.BuildEditDTO(func(p *testkit.EditParams) {
		p.FilePath = "/home/user/project/src/main.rs"
		p.RepoURL = "/home/user/project"
	})
	result := svc.Validate(token, edit)
	// Should not flag path_mismatch for local repo
	for _, r := range result.Reasons {
		if r == "path_mismatch" {
			t.Error("local repo with absolute path should not flag path_mismatch")
		}
	}
}

func TestValidation_PathMismatch_FileScheme(t *testing.T) {
	svc := defaultValidationSvc(&fakeCounter{0})
	token := testkit.BuildTokenWithSig()
	edit := testkit.BuildEditDTO(func(p *testkit.EditParams) {
		p.FilePath = "/home/user/project/src/main.rs"
		p.RepoURL = "file:///home/user/project"
	})
	result := svc.Validate(token, edit)
	for _, r := range result.Reasons {
		if r == "path_mismatch" {
			t.Error("file:// repo with absolute path should not flag path_mismatch")
		}
	}
}

func TestValidation_PathMismatch_EmptyFields(t *testing.T) {
	svc := defaultValidationSvc(&fakeCounter{0})
	token := testkit.BuildTokenWithSig()
	// relative path → no mismatch possible
	edit := testkit.BuildEditDTO(func(p *testkit.EditParams) {
		p.FilePath = "src/main.rs"
		p.RepoURL = "git@github.com:org/repo.git"
	})
	result := svc.Validate(token, edit)
	for _, r := range result.Reasons {
		if r == "path_mismatch" {
			t.Error("relative path should never flag path_mismatch")
		}
	}
}

// ── containsStr (whitelist present, URL in list) ──────────────────────────────

func TestValidation_RepoInWhitelist_NoFlag(t *testing.T) {
	cfg := &config.Config{}
	cfg.RateLimitPerHour = 30
	cfg.MaxAddedLines = 5000
	cfg.RepoWhitelist.Enforce = true
	cfg.RepoWhitelist.URLs = []string{"git@github.com:org/repo.git"}

	svc := service.NewValidationService(
		service.NewSignatureService(),
		service.NewDiffConsistencyService(),
		&fakeCounter{0},
		cfg,
	)
	token := testkit.BuildTokenWithSig()
	edit := testkit.BuildEditDTO() // repoURL = "git@github.com:org/repo.git" (in whitelist)

	result := svc.Validate(token, edit)
	if result.Outcome == service.OutcomeRejected {
		t.Errorf("repo in whitelist should not be rejected, got %v", result.Reasons)
	}
}

// ── Ingest QueryEdits with no results ────────────────────────────────────────

func TestIngest_QueryEdits_Empty(t *testing.T) {
	database := openTestDB(t)
	cfg := &config.Config{RateLimitPerHour: 30, MaxAddedLines: 5000}
	sig := service.NewSignatureService()
	diff := service.NewDiffConsistencyService()
	editRepo := service.NewEditRecordRepository(database)
	validation := service.NewValidationService(sig, diff, editRepo, cfg)
	validator := service.NewEditValidator()
	ingest := service.NewIngestService(validation, validator, editRepo)

	result, err := ingest.QueryEdits("no-such-token", "", 0, 10)
	if err != nil {
		t.Fatal(err)
	}
	if result.Total != 0 {
		t.Errorf("expected 0, got %d", result.Total)
	}
	if result.Records == nil {
		t.Error("Records should be non-nil empty slice")
	}
}
