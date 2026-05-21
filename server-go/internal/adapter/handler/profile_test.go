package handler_test

import (
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/aitrack/server/internal/adapter/handler"
	"github.com/aitrack/server/internal/domain/model"
)

// insertToken inserts a token row directly into the test DB.
func insertToken(t *testing.T, env *testEnv, tokenKey, owner string) {
	t.Helper()
	_, err := env.db.Exec(
		`INSERT INTO tokens (token_hash, token_key, hmac_secret, owner, note, active, created_at)
		 VALUES ($1, $2, $3, $4, $5, 1, $6)`,
		"hash-"+tokenKey, tokenKey, "secret", owner, "test", time.Now().UTC().Format(time.RFC3339),
	)
	if err != nil {
		t.Fatalf("insertToken: %v", err)
	}
}

// insertEditRecord inserts an edit_record row directly into the test DB.
func insertEditRecord(t *testing.T, env *testEnv, tokenKey, tool, filePath string, added, removed int64, epochTs int64) {
	t.Helper()
	_, err := env.db.Exec(
		`INSERT INTO edit_records
		 (token_key, device_id, hostname, tool, tool_version, provider, model,
		  session_id, repo_url, branch, current_sha, file_path,
		  added_lines, removed_lines, diff_hunk, metadata,
		  timestamp, record_sig, status, flags, received_at)
		 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21)`,
		tokenKey, "device-1", "host-1", tool, "1.0", "openai", "gpt-4",
		"sess-1", "https://github.com/test/repo", "main", "abc123", filePath,
		added, removed, "diff hunk", "{}",
		fmt.Sprintf("%d", epochTs), "sig-abc", "ACCEPTED", "",
		time.Now().UTC().Format(time.RFC3339),
	)
	if err != nil {
		t.Fatalf("insertEditRecord: %v", err)
	}
}

// ─── Auth guard tests ──────────────────────────────────────────────────────

// Test 1: No admin key → 403.
func TestProfile_NoAdminKey(t *testing.T) {
	env := newTestEnv(t)
	req := httptest.NewRequest(http.MethodGet, "/api/v1/ai-track/profiles/aitrack_test", nil)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusForbidden)
}

// Test 2: Wrong admin key → 403.
func TestProfile_WrongAdminKey(t *testing.T) {
	env := newTestEnv(t)
	req := httptest.NewRequest(http.MethodGet, "/api/v1/ai-track/profiles/aitrack_test", nil)
	req.Header.Set("X-Admin-Key", "wrong-key")
	w := do(env.router, req)
	assertStatus(t, w, http.StatusForbidden)
}

// Test 3: Admin key not configured → 503.
func TestProfile_AdminKeyNotConfigured(t *testing.T) {
	profileH := handler.NewProfileHandler(nil, "")
	mux := http.NewServeMux()
	mux.HandleFunc("/api/v1/ai-track/profiles/", profileH.Profile)

	req := httptest.NewRequest(http.MethodGet, "/api/v1/ai-track/profiles/aitrack_test", nil)
	req.Header.Set("X-Admin-Key", "anything")
	w := httptest.NewRecorder()
	mux.ServeHTTP(w, req)
	assertStatus(t, w, http.StatusServiceUnavailable)
}

// ─── 404 test ──────────────────────────────────────────────────────────────

// Test 4: Token does not exist in DB → 404.
func TestProfile_NotFound(t *testing.T) {
	env := newTestEnv(t)
	req := httptest.NewRequest(http.MethodGet, "/api/v1/ai-track/profiles/nonexistent_token", nil)
	req.Header.Set("X-Admin-Key", env.cfg.AdminKey)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusNotFound)
}

// ─── Empty edits test ──────────────────────────────────────────────────────

// Test 5: Token exists but no edit records → 200 with zero totals.
func TestProfile_EmptyEdits(t *testing.T) {
	env := newTestEnv(t)
	insertToken(t, env, "aitrack_empty", "empty-owner")

	req := httptest.NewRequest(http.MethodGet, "/api/v1/ai-track/profiles/aitrack_empty", nil)
	req.Header.Set("X-Admin-Key", env.cfg.AdminKey)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusOK)

	var resp model.ProfileDto
	decodeJSON(t, w, &resp)

	if resp.TotalEdits != 0 {
		t.Errorf("total_edits = %d, want 0", resp.TotalEdits)
	}
	if resp.Owner != "empty-owner" {
		t.Errorf("owner = %q, want %q", resp.Owner, "empty-owner")
	}
	if resp.TokenKey != "aitrack_empty" {
		t.Errorf("token_key = %q, want %q", resp.TokenKey, "aitrack_empty")
	}
	if resp.Frequency == nil {
		t.Error("frequency should not be nil")
	}
	if resp.Depth == nil {
		t.Error("depth should not be nil")
	}
	if resp.FirstSeen != nil {
		t.Errorf("first_seen should be nil for empty edits, got %v", resp.FirstSeen)
	}
}

// ─── Full profile test ─────────────────────────────────────────────────────

// Test 6: Token exists with 3 edit records — verify aggregations.
func TestProfile_WithEdits(t *testing.T) {
	env := newTestEnv(t)
	insertToken(t, env, "aitrack_full", "dev-owner")

	now := time.Now().Unix()
	// Insert 3 records with different tools and file paths.
	insertEditRecord(t, env, "aitrack_full", "cursor", "src/main.go", 10, 5, now-100)
	insertEditRecord(t, env, "aitrack_full", "cursor", "src/main_test.go", 20, 2, now-200)
	insertEditRecord(t, env, "aitrack_full", "copilot", "docs/README.md", 5, 1, now-300)

	req := httptest.NewRequest(http.MethodGet, "/api/v1/ai-track/profiles/aitrack_full", nil)
	req.Header.Set("X-Admin-Key", env.cfg.AdminKey)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusOK)

	var resp model.ProfileDto
	decodeJSON(t, w, &resp)

	if resp.TotalEdits != 3 {
		t.Errorf("total_edits = %d, want 3", resp.TotalEdits)
	}
	if resp.TotalAddedLines != 35 {
		t.Errorf("total_added_lines = %d, want 35", resp.TotalAddedLines)
	}
	if resp.TotalRemovedLines != 8 {
		t.Errorf("total_removed_lines = %d, want 8", resp.TotalRemovedLines)
	}
	if resp.Owner != "dev-owner" {
		t.Errorf("owner = %q, want %q", resp.Owner, "dev-owner")
	}
	if resp.Languages == nil {
		t.Fatal("languages map should not be nil")
	}
	if resp.Tools == nil {
		t.Fatal("tools map should not be nil")
	}
	// src/main.go → Go, src/main_test.go → Go, docs/README.md → Docs.
	if resp.Languages["Go"] != 2 {
		t.Errorf("languages[Go] = %d, want 2", resp.Languages["Go"])
	}
	if resp.Languages["Docs"] != 1 {
		t.Errorf("languages[Docs] = %d, want 1", resp.Languages["Docs"])
	}
	// Two different tools: cursor (x2) and copilot (x1).
	if resp.Tools["cursor"] != 2 {
		t.Errorf("tools[cursor] = %d, want 2", resp.Tools["cursor"])
	}
	if resp.Tools["copilot"] != 1 {
		t.Errorf("tools[copilot] = %d, want 1", resp.Tools["copilot"])
	}
	if resp.Frequency == nil {
		t.Error("frequency should not be nil")
	}
	if resp.Depth == nil {
		t.Error("depth should not be nil")
	}
	if resp.FirstSeen == nil {
		t.Error("first_seen should not be nil when records exist")
	}
	if resp.LastSeen == nil {
		t.Error("last_seen should not be nil when records exist")
	}
	if resp.GeneratedAt == "" {
		t.Error("generated_at should not be empty")
	}
}

// Test 7: REJECTED records are excluded from aggregation.
func TestProfile_RejectedRecordsExcluded(t *testing.T) {
	env := newTestEnv(t)
	insertToken(t, env, "aitrack_reject", "owner-r")

	now := time.Now().Unix()
	insertEditRecord(t, env, "aitrack_reject", "cursor", "src/main.go", 10, 5, now-100)

	// Insert a REJECTED record directly.
	_, err := env.db.Exec(
		`INSERT INTO edit_records
		 (token_key, device_id, hostname, tool, tool_version, provider, model,
		  session_id, repo_url, branch, current_sha, file_path,
		  added_lines, removed_lines, diff_hunk, metadata,
		  timestamp, record_sig, status, flags, received_at)
		 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21)`,
		"aitrack_reject", "device-1", "host-1", "cursor", "1.0", "openai", "gpt-4",
		"sess-2", "https://github.com/test/repo", "main", "abc123", "src/bad.go",
		999, 999, "diff", "{}",
		fmt.Sprintf("%d", now-50), "sig-rejected", "REJECTED", "",
		time.Now().UTC().Format(time.RFC3339),
	)
	if err != nil {
		t.Fatalf("insert rejected record: %v", err)
	}

	req := httptest.NewRequest(http.MethodGet, "/api/v1/ai-track/profiles/aitrack_reject", nil)
	req.Header.Set("X-Admin-Key", env.cfg.AdminKey)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusOK)

	var resp model.ProfileDto
	decodeJSON(t, w, &resp)

	// Only 1 accepted record should be counted.
	if resp.TotalEdits != 1 {
		t.Errorf("total_edits = %d, want 1 (REJECTED should be excluded)", resp.TotalEdits)
	}
	if resp.TotalAddedLines != 10 {
		t.Errorf("total_added_lines = %d, want 10", resp.TotalAddedLines)
	}
}

// ─── JSON shape test ───────────────────────────────────────────────────────

// Test 8: Verify full JSON structure with raw map decode.
func TestProfile_JSONShape(t *testing.T) {
	env := newTestEnv(t)
	insertToken(t, env, "aitrack_shape", "shape-owner")
	insertEditRecord(t, env, "aitrack_shape", "claude", "src/feature.go", 5, 3, time.Now().Unix()-50)

	req := httptest.NewRequest(http.MethodGet, "/api/v1/ai-track/profiles/aitrack_shape", nil)
	req.Header.Set("X-Admin-Key", env.cfg.AdminKey)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusOK)

	var raw map[string]json.RawMessage
	decodeJSON(t, w, &raw)

	requiredKeys := []string{
		"token_key", "owner", "total_edits", "total_added_lines",
		"total_removed_lines", "generated_at", "frequency", "depth",
		"languages", "tools", "prompt_patterns",
	}
	for _, k := range requiredKeys {
		if _, ok := raw[k]; !ok {
			t.Errorf("missing key %q in response", k)
		}
	}

	// Verify depth sub-struct contains comment_density.
	var depth map[string]json.RawMessage
	if err := json.Unmarshal(raw["depth"], &depth); err != nil {
		t.Fatalf("unmarshal depth: %v", err)
	}
	if _, ok := depth["comment_density"]; !ok {
		t.Error("missing key \"comment_density\" in depth object")
	}
}
