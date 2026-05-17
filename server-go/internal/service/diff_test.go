package service_test

import (
	"testing"

	"github.com/aitrack/server/internal/service"
)

func TestParseDiffHunk(t *testing.T) {
	svc := service.NewDiffConsistencyService()

	diff := "@@ -1,3 +1,5 @@\n-old1\n-old2\n-old3\n+new1\n+new2\n+new3\n+new4\n+new5\n"
	counts := svc.ParseDiffHunk(diff)
	if counts.Added != 5 {
		t.Errorf("added = %d, want 5", counts.Added)
	}
	if counts.Removed != 3 {
		t.Errorf("removed = %d, want 3", counts.Removed)
	}
}

func TestParseDiffHunkSkipsHeaders(t *testing.T) {
	svc := service.NewDiffConsistencyService()
	diff := "--- a/file.rs\n+++ b/file.rs\n@@ -1 +1 @@\n-old\n+new\n"
	counts := svc.ParseDiffHunk(diff)
	if counts.Added != 1 {
		t.Errorf("added = %d, want 1 (skipped +++ header)", counts.Added)
	}
	if counts.Removed != 1 {
		t.Errorf("removed = %d, want 1 (skipped --- header)", counts.Removed)
	}
}

func TestParseDiffHunkDeletedDashContent(t *testing.T) {
	svc := service.NewDiffConsistencyService()
	// A line whose content is "---" appears as "----" in the diff (- prefix + --- content).
	// It must NOT be treated as a file header and must be counted as a removed line.
	diff := "--- a/file.rs\n+++ b/file.rs\n@@ -1,2 +1,1 @@\n----\n+replacement\n"
	counts := svc.ParseDiffHunk(diff)
	if counts.Removed != 1 {
		t.Errorf("removed = %d, want 1 (---- is a content line, not a header)", counts.Removed)
	}
	if counts.Added != 1 {
		t.Errorf("added = %d, want 1", counts.Added)
	}
}

func TestIsConsistentNilHunk(t *testing.T) {
	svc := service.NewDiffConsistencyService()
	if !svc.IsConsistent(nil, 100, 50) {
		t.Error("nil hunk should always be consistent")
	}
}

func TestIsConsistentBlankHunk(t *testing.T) {
	svc := service.NewDiffConsistencyService()
	blank := "   "
	if !svc.IsConsistent(&blank, 100, 50) {
		t.Error("blank hunk should always be consistent")
	}
}

func TestIsConsistentWithinDelta(t *testing.T) {
	svc := service.NewDiffConsistencyService()
	// diff: 1 added, 1 removed; claimed: 1 added, 2 removed (delta 1 for removed)
	diff := "@@ -1 +1 @@\n-old\n+new\n"
	if !svc.IsConsistent(&diff, 1, 2) {
		t.Error("within delta=1 should be consistent")
	}
}

func TestIsConsistentExceedsDelta(t *testing.T) {
	svc := service.NewDiffConsistencyService()
	diff := "@@ -1,1 +1,1 @@\n-old\n+new\n"
	// diff says 1 added, claimed 100
	if svc.IsConsistent(&diff, 100, 1) {
		t.Error("delta > 1 should be inconsistent")
	}
}
