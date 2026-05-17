// Package factory provides seed-deterministic builders for all e2e test payloads.
// All data derives from the local prompt fixtures in e2e/fixtures/prompts/ —
// no hand-written fake strings.
package factory

import (
	"crypto/hmac"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"math/rand"
	"os"
	"path/filepath"
	"strings"
	"time"
)

// ─── Fixture content cache ────────────────────────────────────────────────────

var fixtureCache = map[string]string{}

func LoadFixture(name string) string {
	if v, ok := fixtureCache[name]; ok {
		return v
	}
	dirs := []string{
		"e2e/fixtures/prompts",
		"../e2e/fixtures/prompts",
		"/e2e/fixtures/prompts",
	}
	for _, dir := range dirs {
		b, err := os.ReadFile(filepath.Join(dir, name))
		if err == nil {
			fixtureCache[name] = string(b)
			return fixtureCache[name]
		}
	}
	return "// fixture not found: " + name
}

// ─── Deterministic seed helpers ──────────────────────────────────────────────

func newRNG(seed int64) *rand.Rand {
	return rand.New(rand.NewSource(seed))
}

func randHex(rng *rand.Rand, n int) string {
	b := make([]byte, n)
	rng.Read(b)
	return hex.EncodeToString(b)
}

// SeedUUIDExport is the exported version for use by scenario code.
func SeedUUIDExport(seed int64) string { return seedUUID(seed) }

func seedUUID(seed int64) string {
	rng := newRNG(seed)
	b := make([]byte, 16)
	rng.Read(b)
	b[6] = (b[6] & 0x0f) | 0x40
	b[8] = (b[8] & 0x3f) | 0x80
	return fmt.Sprintf("%x-%x-%x-%x-%x", b[0:4], b[4:6], b[6:8], b[8:10], b[10:])
}

// ─── HMAC helpers (mirrors CONTRACT.md) ──────────────────────────────────────

func sha256HexStr(s string) string {
	h := sha256.Sum256([]byte(s))
	return hex.EncodeToString(h[:])
}

func hmacSHA256Hex(secret, msg string) string {
	mac := hmac.New(sha256.New, []byte(secret))
	mac.Write([]byte(msg))
	return hex.EncodeToString(mac.Sum(nil))
}

// ComputeRecordSig implements the canonical record_sig from CONTRACT.md v1.1.
// Field order: token_key, device_id, hostname, timestamp, tool, file_path, repo_url, current_sha, added_lines, removed_lines, sha256(diff_hunk)
func ComputeRecordSig(secret, tokenKey, deviceID, hostname, timestamp, tool, filePath, repoURL, currentSHA string, addedLines, removedLines int64, diffHunk string) string {
	diffHash := sha256HexStr(diffHunk)
	canonical := fmt.Sprintf("%s\n%s\n%s\n%s\n%s\n%s\n%s\n%s\n%d\n%d\n%s",
		tokenKey, deviceID, hostname, timestamp, tool, filePath, repoURL, currentSHA,
		addedLines, removedLines, diffHash)
	return hmacSHA256Hex(secret, canonical)
}

// ComputeRequestSig computes the X-AiTrack-Signature header value.
func ComputeRequestSig(secret string, unixTS int64, bodyBytes []byte) string {
	bodyHash := sha256HexStr(string(bodyBytes))
	msg := fmt.Sprintf("%d\n%s", unixTS, bodyHash)
	return hmacSHA256Hex(secret, msg)
}

// ─── Token provisioning ───────────────────────────────────────────────────────

type TokenBundle struct {
	Token      string `json:"token"`
	HmacSecret string `json:"hmac_secret"`
	TokenKey   string `json:"token_key"`
}

// ─── Edit payload builders ────────────────────────────────────────────────────

type EditParams struct {
	Seed         int64
	HmacSecret   string
	TokenKey     string
	DeviceID     string
	Hostname     string
	Timestamp    string
	Tool         string
	ToolVersion  string
	Provider     string
	SessionID    string
	RepoURL      string
	Branch       string
	CurrentSHA   string
	FilePath     string
	AddedLines   int64
	RemovedLines int64
	DiffHunk     string
}

func DefaultEditParams(seed int64, tok TokenBundle) EditParams {
	rng := newRNG(seed)
	// The fixture is a plain code snippet, not a diff. Build a realistic unified
	// diff from it: first half becomes removed (-) lines, second half added (+).
	// added_lines / removed_lines are then derived from the diff itself so the
	// payload is internally consistent (server step-5 diff consistency check).
	diffContent := LoadFixture("claude_edit_snippet.txt")
	srcLines := strings.Split(strings.TrimRight(diffContent, "\n"), "\n")
	if len(srcLines) < 2 {
		srcLines = []string{"fn placeholder() {}", "fn added() {}"}
	}
	mid := len(srcLines) / 2
	var removed, added []string
	for _, l := range srcLines[:mid] {
		removed = append(removed, "-"+l)
	}
	for _, l := range srcLines[mid:] {
		added = append(added, "+"+l)
	}
	removedCount := int64(len(removed))
	addedCount := int64(len(added))
	diffHunk := fmt.Sprintf("@@ -1,%d +1,%d @@\n", removedCount, addedCount) +
		strings.Join(removed, "\n") + "\n" + strings.Join(added, "\n") + "\n"

	return EditParams{
		Seed:         seed,
		HmacSecret:   tok.HmacSecret,
		TokenKey:     tok.TokenKey,
		DeviceID:     seedUUID(seed + 100),
		Hostname:     fmt.Sprintf("e2e-host-%d", seed),
		Timestamp:    time.Now().UTC().Format(time.RFC3339),
		Tool:         "claude",
		ToolVersion:  "claude-code",
		Provider:     "anthropic",
		SessionID:    "sess-" + randHex(rng, 4),
		RepoURL:      "git@github.com:aitrack-e2e/repo.git",
		Branch:       "main",
		CurrentSHA:   randHex(rng, 20),
		FilePath:     fmt.Sprintf("src/file_%d.rs", seed),
		AddedLines:   addedCount,
		RemovedLines: removedCount,
		DiffHunk:     diffHunk,
	}
}

func (p EditParams) BuildDTO() map[string]interface{} {
	sig := ComputeRecordSig(p.HmacSecret, p.TokenKey, p.DeviceID, p.Hostname, p.Timestamp,
		p.Tool, p.FilePath, p.RepoURL, p.CurrentSHA, p.AddedLines, p.RemovedLines, p.DiffHunk)
	addedLines := p.AddedLines
	removedLines := p.RemovedLines
	return map[string]interface{}{
		"tool":          p.Tool,
		"tool_version":  p.ToolVersion,
		"provider":      p.Provider,
		"model":         nil,
		"session_id":    p.SessionID,
		"repo_url":      p.RepoURL,
		"branch":        p.Branch,
		"current_sha":   p.CurrentSHA,
		"file_path":     p.FilePath,
		"added_lines":   addedLines,
		"removed_lines": removedLines,
		"diff_hunk":     p.DiffHunk,
		"metadata":      nil,
		"timestamp":     p.Timestamp,
		"device_id":     p.DeviceID,
		"hostname":      p.Hostname,
		"record_sig":    sig,
	}
}

// BuildBatchRequest builds the full upload request body as JSON bytes.
func BuildBatchRequest(deviceID string, edits ...map[string]interface{}) []byte {
	req := map[string]interface{}{
		"device_id":      deviceID,
		"client_version": "1.0.0",
		"edits":          edits,
	}
	b, _ := json.Marshal(req)
	return b
}

// BuildHeartbeatRequest builds a heartbeat request body as JSON bytes.
func BuildHeartbeatRequest(deviceID, hostname, tokenKeyMasked string, pendingCount int) []byte {
	req := map[string]interface{}{
		"device_id":        deviceID,
		"hostname":         hostname,
		"token_key_masked": tokenKeyMasked,
		"client_version":   "1.0.0",
		"ts":               time.Now().Unix(),
		"hooks":            map[string]bool{"claude": true, "codex": false, "cursor": false},
		"pending_count":    pendingCount,
	}
	b, _ := json.Marshal(req)
	return b
}

// ─── Hook payload builders (per-tool, mirrors Rust factories) ─────────────────

func BuildClaudeHookPayload(seed int64, filePath string) map[string]interface{} {
	rng := newRNG(seed)
	content := LoadFixture("claude_edit_snippet.txt")
	lines := strings.Split(content, "\n")
	mid := len(lines) / 2
	if mid == 0 {
		mid = 1
	}
	return map[string]interface{}{
		"session_id":   seedUUID(seed),
		"tool_version": "claude-code",
		"tool_input": map[string]interface{}{
			"old_string": strings.Join(lines[:mid], "\n"),
			"new_string": strings.Join(lines[mid:], "\n") + "\n// added by e2e seed=" + randHex(rng, 2),
			"file_paths": []string{filePath},
		},
	}
}

func BuildCodexHookPayload(seed int64, filePath string) map[string]interface{} {
	rng := newRNG(seed)
	content := LoadFixture("codex_edit_snippet.txt")
	lines := strings.Split(content, "\n")
	mid := len(lines) / 2
	if mid == 0 {
		mid = 1
	}
	return map[string]interface{}{
		"hook_event_name": "postToolUse",
		"tool_name":       "Edit",
		"conversation_id": seedUUID(seed),
		"model":           "gpt-4o",
		"tool_input": map[string]interface{}{
			"old_string": strings.Join(lines[:mid], "\n"),
			"new_string": strings.Join(lines[mid:], "\n") + "\n// codex e2e seed=" + randHex(rng, 2),
			"file_path":  filePath,
		},
	}
}

func BuildCursorHookPayload(seed int64, filePath string) map[string]interface{} {
	rng := newRNG(seed)
	content := LoadFixture("cursor_edit_snippet.txt")
	lines := strings.Split(content, "\n")
	mid := len(lines) / 2
	if mid == 0 {
		mid = 1
	}
	return map[string]interface{}{
		"session_id":     seedUUID(seed),
		"cursor_version": "0.40.0",
		"tool_input": map[string]interface{}{
			"file_path": filePath,
			"old_str":   strings.Join(lines[:mid], "\n"),
			"new_str":   strings.Join(lines[mid:], "\n") + "\n// cursor e2e seed=" + randHex(rng, 2),
		},
	}
}

// ─── Tampered / negative builders ────────────────────────────────────────────

// TamperedRecordSig returns an edit DTO with a zeroed-out record_sig.
func TamperedRecordSig(p EditParams) map[string]interface{} {
	dto := p.BuildDTO()
	dto["record_sig"] = strings.Repeat("0", 64)
	return dto
}

// ExpiredTimestampEdit returns an edit DTO with a timestamp far in the past.
func ExpiredTimestampEdit(p EditParams) map[string]interface{} {
	p.Timestamp = "2000-01-01T00:00:00Z"
	return p.BuildDTO()
}

// OversizedEdit returns an edit DTO with added_lines > max threshold.
func OversizedEdit(p EditParams) map[string]interface{} {
	p.AddedLines = 99_999_999
	p.RemovedLines = 0
	p.DiffHunk = ""
	return p.BuildDTO()
}

// MissingFieldEdit returns an edit DTO missing the required "tool" field.
func MissingFieldEdit(p EditParams) map[string]interface{} {
	dto := p.BuildDTO()
	delete(dto, "tool")
	return dto
}
