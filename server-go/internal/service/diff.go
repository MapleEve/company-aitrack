package service

import "strings"

const allowedDelta = 1

// DiffConsistencyService parses unified diff hunks to verify claimed line counts.
type DiffConsistencyService struct{}

func NewDiffConsistencyService() *DiffConsistencyService { return &DiffConsistencyService{} }

type DiffCounts struct {
	Added   int64
	Removed int64
}

// IsConsistent returns true if diff_hunk line counts match claimed values (within delta 1).
// Nil or blank diff_hunk skips the check.
func (d *DiffConsistencyService) IsConsistent(diffHunk *string, claimedAdded, claimedRemoved int64) bool {
	if diffHunk == nil || strings.TrimSpace(*diffHunk) == "" {
		return true
	}
	counts := d.ParseDiffHunk(*diffHunk)
	return abs64(counts.Added-claimedAdded) <= allowedDelta &&
		abs64(counts.Removed-claimedRemoved) <= allowedDelta
}

// ParseDiffHunk counts added/removed lines in a unified diff, skipping headers.
func (d *DiffConsistencyService) ParseDiffHunk(diffHunk string) DiffCounts {
	var added, removed int64
	for _, line := range strings.Split(diffHunk, "\n") {
		if strings.HasPrefix(line, "+++ ") || strings.HasPrefix(line, "--- ") {
			continue
		}
		if strings.HasPrefix(line, "@@") {
			continue
		}
		if strings.HasPrefix(line, "+") {
			added++
		} else if strings.HasPrefix(line, "-") {
			removed++
		}
	}
	return DiffCounts{Added: added, Removed: removed}
}

func abs64(x int64) int64 {
	if x < 0 {
		return -x
	}
	return x
}
