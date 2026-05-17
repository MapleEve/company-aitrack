package service

import (
	"strings"
	"time"

	"github.com/aitrack/server/internal/config"
	"github.com/aitrack/server/internal/model"
)

type EditOutcome int

const (
	OutcomeAccepted EditOutcome = iota
	OutcomeFlagged
	OutcomeRejected
)

type ValidationResult struct {
	Outcome EditOutcome
	Reasons []string
}

// EditValidator validates required fields before HMAC computation (guards NPE-equivalent panics).
type EditValidator struct{}

func NewEditValidator() *EditValidator { return &EditValidator{} }

// Validate returns "" if valid, or a reason string if malformed.
func (v *EditValidator) Validate(edit *model.EditDTO) string {
	if edit == nil {
		return "malformed"
	}
	if strings.TrimSpace(edit.Tool) == "" {
		return "malformed"
	}
	if strings.TrimSpace(edit.Provider) == "" {
		return "malformed"
	}
	if strings.TrimSpace(edit.SessionID) == "" {
		return "malformed"
	}
	if strings.TrimSpace(edit.FilePath) == "" {
		return "malformed"
	}
	if strings.TrimSpace(edit.Timestamp) == "" {
		return "malformed"
	}
	if strings.TrimSpace(edit.DeviceID) == "" {
		return "malformed"
	}
	if strings.TrimSpace(edit.Hostname) == "" {
		return "malformed"
	}
	if strings.TrimSpace(edit.RepoURL) == "" {
		return "malformed"
	}
	if strings.TrimSpace(edit.Branch) == "" {
		return "malformed"
	}
	if strings.TrimSpace(edit.CurrentSHA) == "" {
		return "malformed"
	}
	if strings.TrimSpace(edit.RecordSig) == "" {
		return "malformed"
	}
	if edit.AddedLines == nil {
		return "malformed"
	}
	if edit.RemovedLines == nil {
		return "malformed"
	}
	return ""
}

// EditRecordCounter is implemented by EditRecordRepository.
type EditRecordCounter interface {
	CountByTokenKeyAndFilePathSince(tokenKey, filePath string, since time.Time) (int64, error)
}

// ValidationService implements the 10-step validation chain (steps 4-10).
type ValidationService struct {
	sig     *SignatureService
	diff    *DiffConsistencyService
	counter EditRecordCounter
	cfg     *config.Config
}

func NewValidationService(sig *SignatureService, diff *DiffConsistencyService, counter EditRecordCounter, cfg *config.Config) *ValidationService {
	return &ValidationService{sig: sig, diff: diff, counter: counter, cfg: cfg}
}

// Validate runs steps 4-10 for a single edit given an active token (with decrypted hmac_secret).
func (s *ValidationService) Validate(token *model.Token, edit *model.EditDTO) ValidationResult {
	var flags []string

	// Step 4: record_sig verification
	expectedSig := s.sig.ComputeRecordSig(
		token.HmacSecret,
		token.TokenKey,
		edit.DeviceID,
		edit.Hostname,
		edit.Timestamp,
		edit.Tool,
		edit.FilePath,
		edit.RepoURL,
		edit.CurrentSHA,
		*edit.AddedLines,
		*edit.RemovedLines,
		edit.DiffHunk,
	)
	if !ConstantTimeEqual(expectedSig, edit.RecordSig) {
		return ValidationResult{Outcome: OutcomeRejected, Reasons: []string{"sig_mismatch"}}
	}

	// Step 5: diff_hunk consistency
	if !s.diff.IsConsistent(edit.DiffHunk, *edit.AddedLines, *edit.RemovedLines) {
		flags = append(flags, "diff_inconsistent")
	}

	// Step 6: repo_url whitelist
	whitelist := s.cfg.RepoWhitelist.URLs
	if len(whitelist) > 0 {
		if !containsStr(whitelist, edit.RepoURL) {
			if s.cfg.RepoWhitelist.Enforce {
				return ValidationResult{Outcome: OutcomeRejected, Reasons: []string{"repo_not_whitelisted"}}
			}
			flags = append(flags, "repo_unknown")
		}
	}

	// Step 7: file_path / repo_url plausibility
	if isPathMismatch(edit.FilePath, edit.RepoURL) {
		flags = append(flags, "path_mismatch")
	}

	// Step 8: oversized
	if *edit.AddedLines > s.cfg.MaxAddedLines {
		flags = append(flags, "oversized")
	}

	// Step 9: rate limiting
	since := time.Now().Add(-1 * time.Hour)
	count, _ := s.counter.CountByTokenKeyAndFilePathSince(token.TokenKey, edit.FilePath, since)
	if count >= s.cfg.RateLimitPerHour {
		return ValidationResult{Outcome: OutcomeRejected, Reasons: []string{"rate_limited"}}
	}

	if len(flags) > 0 {
		return ValidationResult{Outcome: OutcomeFlagged, Reasons: flags}
	}
	return ValidationResult{Outcome: OutcomeAccepted, Reasons: nil}
}

func isPathMismatch(filePath, _ string) bool {
	if filePath == "" {
		return false
	}
	// Flag path traversal attempts.
	if strings.Contains(filePath, "..") {
		return true
	}
	// Flag NUL or ASCII control characters (0x00–0x1F).
	for _, c := range filePath {
		if c < 0x20 {
			return true
		}
	}
	// Absolute paths (e.g. macOS /Users/…) are normal — do not flag them.
	return false
}

func containsStr(slice []string, s string) bool {
	for _, v := range slice {
		if v == s {
			return true
		}
	}
	return false
}
