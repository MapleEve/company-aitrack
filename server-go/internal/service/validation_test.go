package service_test

import (
	"testing"
	"time"

	"github.com/aitrack/server/internal/config"
	"github.com/aitrack/server/internal/model"
	"github.com/aitrack/server/internal/service"
	"github.com/aitrack/server/internal/testkit"
)

// fakeCounter implements EditRecordCounter for tests.
type fakeCounter struct {
	count int64
}

func (f *fakeCounter) CountByTokenKeyAndFilePathSince(_, _ string, _ time.Time) (int64, error) {
	return f.count, nil
}

func defaultValidationSvc(counter service.EditRecordCounter) *service.ValidationService {
	cfg := &config.Config{}
	cfg.TimestampWindowSeconds = 300
	cfg.RateLimitPerHour = 30
	cfg.MaxAddedLines = 5000
	return service.NewValidationService(
		service.NewSignatureService(),
		service.NewDiffConsistencyService(),
		counter,
		cfg,
	)
}

// ── Step 4: record_sig ────────────────────────────────────────────────────────

func TestStep4_ValidSig_Accepted(t *testing.T) {
	svc := defaultValidationSvc(&fakeCounter{0})
	token := testkit.BuildTokenWithSig()
	edit := testkit.BuildEditDTO()

	result := svc.Validate(token, edit)
	if result.Outcome != service.OutcomeAccepted {
		t.Errorf("expected ACCEPTED, got %v reasons=%v", result.Outcome, result.Reasons)
	}
}

func TestStep4_TamperedSig_Rejected(t *testing.T) {
	svc := defaultValidationSvc(&fakeCounter{0})
	token := testkit.BuildTokenWithSig()
	edit := testkit.TamperedEditDTO()

	result := svc.Validate(token, edit)
	if result.Outcome != service.OutcomeRejected {
		t.Errorf("expected REJECTED, got %v", result.Outcome)
	}
	if len(result.Reasons) == 0 || result.Reasons[0] != "sig_mismatch" {
		t.Errorf("expected sig_mismatch, got %v", result.Reasons)
	}
}

// ── Step 5: diff consistency ──────────────────────────────────────────────────

func TestStep5_DiffInconsistent_Flagged(t *testing.T) {
	svc := defaultValidationSvc(&fakeCounter{0})
	token := testkit.BuildTokenWithSig()
	// diff says 1+/1-, but we claim 100 added → sig is recomputed with 100
	diff := "@@ -1,1 +1,1 @@\n-old\n+new\n"
	edit := testkit.BuildEditDTO(func(p *testkit.EditParams) {
		p.AddedLines = 100
		p.DiffHunk = &diff
	})

	result := svc.Validate(token, edit)
	if result.Outcome != service.OutcomeFlagged {
		t.Errorf("expected FLAGGED, got %v", result.Outcome)
	}
	if !containsReason(result.Reasons, "diff_inconsistent") {
		t.Errorf("expected diff_inconsistent in %v", result.Reasons)
	}
}

// ── Step 6: repo_url whitelist ────────────────────────────────────────────────

func TestStep6_RepoNotInWhitelist_SoftFlag(t *testing.T) {
	cfg := &config.Config{}
	cfg.RateLimitPerHour = 30
	cfg.MaxAddedLines = 5000
	cfg.RepoWhitelist.Enforce = false
	cfg.RepoWhitelist.URLs = []string{"git@github.com:allowed/repo.git"}

	svc := service.NewValidationService(
		service.NewSignatureService(),
		service.NewDiffConsistencyService(),
		&fakeCounter{0},
		cfg,
	)
	token := testkit.BuildTokenWithSig()
	edit := testkit.BuildEditDTO(func(p *testkit.EditParams) {
		p.RepoURL = "git@github.com:org/repo.git" // not in whitelist
	})

	result := svc.Validate(token, edit)
	if result.Outcome != service.OutcomeFlagged {
		t.Errorf("expected FLAGGED (soft), got %v", result.Outcome)
	}
	if !containsReason(result.Reasons, "repo_unknown") {
		t.Errorf("expected repo_unknown, got %v", result.Reasons)
	}
}

func TestStep6_RepoNotInWhitelist_HardReject(t *testing.T) {
	cfg := &config.Config{}
	cfg.RateLimitPerHour = 30
	cfg.MaxAddedLines = 5000
	cfg.RepoWhitelist.Enforce = true
	cfg.RepoWhitelist.URLs = []string{"git@github.com:allowed/repo.git"}

	svc := service.NewValidationService(
		service.NewSignatureService(),
		service.NewDiffConsistencyService(),
		&fakeCounter{0},
		cfg,
	)
	token := testkit.BuildTokenWithSig()
	edit := testkit.BuildEditDTO(func(p *testkit.EditParams) {
		p.RepoURL = "git@github.com:org/repo.git" // not in whitelist
	})

	result := svc.Validate(token, edit)
	if result.Outcome != service.OutcomeRejected {
		t.Errorf("expected REJECTED (enforce=true), got %v", result.Outcome)
	}
	if !containsReason(result.Reasons, "repo_not_whitelisted") {
		t.Errorf("expected repo_not_whitelisted, got %v", result.Reasons)
	}
}

func TestStep6_EmptyWhitelist_Passthrough(t *testing.T) {
	svc := defaultValidationSvc(&fakeCounter{0})
	token := testkit.BuildTokenWithSig()
	edit := testkit.BuildEditDTO()

	result := svc.Validate(token, edit)
	if result.Outcome != service.OutcomeAccepted {
		t.Errorf("empty whitelist should not reject, got %v", result.Outcome)
	}
}

// ── Step 7: file_path plausibility ───────────────────────────────────────────

// Absolute paths (e.g. macOS /Users/…) are normal and must NOT trigger path_mismatch.
func TestStep7_AbsolutePathMacOS_NotFlagged(t *testing.T) {
	svc := defaultValidationSvc(&fakeCounter{0})
	token := testkit.BuildTokenWithSig()
	edit := testkit.BuildEditDTO(func(p *testkit.EditParams) {
		p.FilePath = "/Users/developer/projects/myapp/src/main.rs"
		p.RepoURL = "git@github.com:org/repo.git"
	})

	result := svc.Validate(token, edit)
	if containsReason(result.Reasons, "path_mismatch") {
		t.Errorf("macOS absolute path must NOT trigger path_mismatch, got %v", result.Reasons)
	}
}

func TestStep7_RelativePath_NoProblem(t *testing.T) {
	svc := defaultValidationSvc(&fakeCounter{0})
	token := testkit.BuildTokenWithSig()
	edit := testkit.BuildEditDTO(func(p *testkit.EditParams) {
		p.FilePath = "src/main.rs"
	})
	result := svc.Validate(token, edit)
	if result.Outcome == service.OutcomeRejected {
		t.Error("relative path should not be rejected")
	}
}

func TestStep7_PathTraversal_Flagged(t *testing.T) {
	svc := defaultValidationSvc(&fakeCounter{0})
	token := testkit.BuildTokenWithSig()
	edit := testkit.BuildEditDTO(func(p *testkit.EditParams) {
		p.FilePath = "../../etc/passwd"
		p.RepoURL = "git@github.com:org/repo.git"
	})

	result := svc.Validate(token, edit)
	if result.Outcome != service.OutcomeFlagged {
		t.Errorf("expected FLAGGED for path traversal, got %v", result.Outcome)
	}
	if !containsReason(result.Reasons, "path_mismatch") {
		t.Errorf("expected path_mismatch for .. traversal, got %v", result.Reasons)
	}
}

// ── Step 8: oversized ─────────────────────────────────────────────────────────

func TestStep8_Oversized_Flagged(t *testing.T) {
	svc := defaultValidationSvc(&fakeCounter{0})
	token := testkit.BuildTokenWithSig()
	edit := testkit.OversizedEditDTO()

	result := svc.Validate(token, edit)
	if result.Outcome != service.OutcomeFlagged {
		t.Errorf("expected FLAGGED for oversized, got %v", result.Outcome)
	}
	if !containsReason(result.Reasons, "oversized") {
		t.Errorf("expected oversized, got %v", result.Reasons)
	}
}

func TestStep8_ExactMaxLines_Accepted(t *testing.T) {
	svc := defaultValidationSvc(&fakeCounter{0})
	token := testkit.BuildTokenWithSig()
	edit := testkit.BuildEditDTO(func(p *testkit.EditParams) {
		p.AddedLines = 5000
		p.DiffHunk = nil
	})
	result := svc.Validate(token, edit)
	if containsReason(result.Reasons, "oversized") {
		t.Error("exactly max_added_lines should NOT be oversized")
	}
}

// ── Step 9: rate limiting ─────────────────────────────────────────────────────

func TestStep9_RateLimited_Rejected(t *testing.T) {
	svc := defaultValidationSvc(&fakeCounter{30}) // at threshold
	token := testkit.BuildTokenWithSig()
	edit := testkit.BuildEditDTO()

	result := svc.Validate(token, edit)
	if result.Outcome != service.OutcomeRejected {
		t.Errorf("expected REJECTED for rate limit, got %v", result.Outcome)
	}
	if !containsReason(result.Reasons, "rate_limited") {
		t.Errorf("expected rate_limited, got %v", result.Reasons)
	}
}

func TestStep9_UnderRateLimit_NotRejected(t *testing.T) {
	svc := defaultValidationSvc(&fakeCounter{29}) // under threshold
	token := testkit.BuildTokenWithSig()
	edit := testkit.BuildEditDTO()

	result := svc.Validate(token, edit)
	if result.Outcome == service.OutcomeRejected {
		t.Error("under rate limit should not be rejected")
	}
}

// ── Edit field validator (pre-HMAC guard) ─────────────────────────────────────

func TestEditValidator_AllRequired(t *testing.T) {
	v := service.NewEditValidator()
	edit := testkit.BuildEditDTO()
	if reason := v.Validate(edit); reason != "" {
		t.Errorf("valid edit returned reason %q", reason)
	}
}

func TestEditValidator_BlankTool(t *testing.T) {
	v := service.NewEditValidator()
	edit := testkit.MalformedEditDTO() // Tool = ""
	if reason := v.Validate(edit); reason != "malformed" {
		t.Errorf("expected malformed, got %q", reason)
	}
}

func TestEditValidator_NilAddedLines(t *testing.T) {
	v := service.NewEditValidator()
	edit := testkit.BuildEditDTO()
	edit.AddedLines = nil
	if reason := v.Validate(edit); reason != "malformed" {
		t.Errorf("expected malformed for nil AddedLines, got %q", reason)
	}
}

func TestEditValidator_NilRemovedLines(t *testing.T) {
	v := service.NewEditValidator()
	edit := testkit.BuildEditDTO()
	edit.RemovedLines = nil
	if reason := v.Validate(edit); reason != "malformed" {
		t.Errorf("expected malformed for nil RemovedLines, got %q", reason)
	}
}

func TestEditValidator_BlankFields(t *testing.T) {
	v := service.NewEditValidator()
	fields := []struct {
		name string
		set  func(*model.EditDTO)
	}{
		{"Provider", func(e *model.EditDTO) { e.Provider = "" }},
		{"SessionID", func(e *model.EditDTO) { e.SessionID = "" }},
		{"FilePath", func(e *model.EditDTO) { e.FilePath = "" }},
		{"Timestamp", func(e *model.EditDTO) { e.Timestamp = "" }},
		{"DeviceID", func(e *model.EditDTO) { e.DeviceID = "" }},
		{"Hostname", func(e *model.EditDTO) { e.Hostname = "" }},
		{"RepoURL", func(e *model.EditDTO) { e.RepoURL = "" }},
		{"Branch", func(e *model.EditDTO) { e.Branch = "" }},
		{"CurrentSHA", func(e *model.EditDTO) { e.CurrentSHA = "" }},
		{"RecordSig", func(e *model.EditDTO) { e.RecordSig = "" }},
	}
	for _, f := range fields {
		edit := testkit.BuildEditDTO()
		f.set(edit)
		if reason := v.Validate(edit); reason != "malformed" {
			t.Errorf("blank %s: expected malformed, got %q", f.name, reason)
		}
	}
}

func containsReason(reasons []string, want string) bool {
	for _, r := range reasons {
		if r == want {
			return true
		}
	}
	return false
}
