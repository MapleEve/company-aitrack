// Package testkit provides deterministic test factories for all domain objects.
// Each factory builds a fully-valid default instance; closures override specific fields.
// Seed the package-level RNG via SetSeed for reproducible runs.
package testkit

import (
	"crypto/hmac"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"math/rand"
	"strings"
	"time"

	"github.com/aitrack/server/internal/model"
	"github.com/aitrack/server/internal/service"
)

var rng = rand.New(rand.NewSource(42))

// SetSeed resets the factory RNG for reproducibility.
func SetSeed(seed int64) { rng = rand.New(rand.NewSource(seed)) }

func randHex(n int) string {
	b := make([]byte, n)
	rng.Read(b)
	return hex.EncodeToString(b)
}

// ─── Token factories ─────────────────────────────────────────────────────────

type TokenDefaults struct {
	HmacSecret string
	TokenKey   string
	Owner      string
}

func DefaultTokenDefaults() TokenDefaults {
	return TokenDefaults{
		HmacSecret: "testsecret",
		TokenKey:   "abcdef…7890",
		Owner:      "testowner",
	}
}

func BuildToken(opts ...func(*model.Token)) *model.Token {
	d := DefaultTokenDefaults()
	t := &model.Token{
		ID:         1,
		TokenHash:  sha256HexStr("aitrack_" + randHex(16)),
		TokenKey:   d.TokenKey,
		HmacSecret: d.HmacSecret,
		Owner:      d.Owner,
		Note:       "",
		Active:     true,
		CreatedAt:  time.Now().UTC(),
	}
	for _, o := range opts {
		o(t)
	}
	return t
}

// ─── EditRecord / EditDTO factories ──────────────────────────────────────────

type EditParams struct {
	HmacSecret   string
	TokenKey     string
	DeviceID     string
	Hostname     string
	Timestamp    string
	Tool         string
	FilePath     string
	RepoURL      string
	CurrentSHA   string
	AddedLines   int64
	RemovedLines int64
	DiffHunk     *string
}

func DefaultEditParams() EditParams {
	return EditParams{
		HmacSecret:   "testsecret",
		TokenKey:     "abcdef…7890",
		DeviceID:     "device-001",
		Hostname:     "test-host.local",
		Timestamp:    "2026-05-17T10:00:00Z",
		Tool:         "claude",
		FilePath:     "src/main.rs",
		RepoURL:      "git@github.com:org/repo.git",
		CurrentSHA:   "abc123",
		AddedLines:   5,
		RemovedLines: 3,
		DiffHunk:     diffHunkPtr("@@ -1,3 +1,5 @@\n-old1\n-old2\n-old3\n+new1\n+new2\n+new3\n+new4\n+new5\n"),
	}
}

func BuildEditDTO(opts ...func(*EditParams)) *model.EditDTO {
	p := DefaultEditParams()
	for _, o := range opts {
		o(&p)
	}
	sig := computeRecordSig(p)
	provider := "anthropic"
	sessionID := "sess-" + randHex(4)
	branch := "main"
	return &model.EditDTO{
		Tool:         p.Tool,
		ToolVersion:  "claude-code",
		Provider:     provider,
		Model:        nil,
		SessionID:    sessionID,
		RepoURL:      p.RepoURL,
		Branch:       branch,
		CurrentSHA:   p.CurrentSHA,
		FilePath:     p.FilePath,
		AddedLines:   &p.AddedLines,
		RemovedLines: &p.RemovedLines,
		DiffHunk:     p.DiffHunk,
		Metadata:     nil,
		Timestamp:    p.Timestamp,
		DeviceID:     p.DeviceID,
		Hostname:     p.Hostname,
		RecordSig:    sig,
	}
}

// BuildEditRecord builds a full EditRecord (already persisted-style).
func BuildEditRecord(opts ...func(*model.EditRecord)) *model.EditRecord {
	rec := &model.EditRecord{
		ID:           1,
		TokenKey:     "abcdef…7890",
		DeviceID:     "device-001",
		Hostname:     "test-host.local",
		Tool:         "claude",
		ToolVersion:  "claude-code",
		Provider:     "anthropic",
		Model:        "",
		SessionID:    "sess-0001",
		RepoURL:      "git@github.com:org/repo.git",
		Branch:       "main",
		CurrentSHA:   "abc123",
		FilePath:     "src/main.rs",
		AddedLines:   5,
		RemovedLines: 3,
		DiffHunk:     "@@ -1,3 +1,5 @@\n-old1\n-old2\n-old3\n+new1\n+new2\n+new3\n+new4\n+new5\n",
		Metadata:     "",
		Timestamp:    "2026-05-17T10:00:00Z",
		RecordSig:    "deadbeef" + strings.Repeat("0", 56),
		Status:       "ACCEPTED",
		Flags:        "",
		ReceivedAt:   time.Now().UTC(),
	}
	for _, o := range opts {
		o(rec)
	}
	return rec
}

// ─── Heartbeat factories ──────────────────────────────────────────────────────

func BuildHeartbeatRequest(opts ...func(*model.HeartbeatRequest)) *model.HeartbeatRequest {
	hooks := &model.HeartbeatHooks{Claude: true, Codex: false, Cursor: false}
	req := &model.HeartbeatRequest{
		DeviceID:       "device-001",
		Hostname:       "test-host.local",
		TokenKeyMasked: "abcdef…7890",
		ClientVersion:  "1.0.0",
		TS:             time.Now().Unix(),
		Hooks:          hooks,
		PendingCount:   0,
	}
	for _, o := range opts {
		o(req)
	}
	return req
}

// ─── Hook payload factories (Claude / Codex / Cursor) ────────────────────────

type HookPayload struct {
	Tool    string
	Payload map[string]interface{}
}

func BuildClaudeHookPayload(opts ...func(*HookPayload)) *HookPayload {
	h := &HookPayload{
		Tool: "claude",
		Payload: map[string]interface{}{
			"tool_name": "Edit",
			"tool_input": map[string]interface{}{
				"path":       "src/main.rs",
				"old_string": "old content",
				"new_string": "new content",
			},
		},
	}
	for _, o := range opts {
		o(h)
	}
	return h
}

func BuildCodexHookPayload(opts ...func(*HookPayload)) *HookPayload {
	h := &HookPayload{
		Tool: "codex",
		Payload: map[string]interface{}{
			"tool":   "apply_patch",
			"params": map[string]interface{}{"patch": "@@ -1 +1 @@\n-old\n+new\n"},
		},
	}
	for _, o := range opts {
		o(h)
	}
	return h
}

func BuildCursorHookPayload(opts ...func(*HookPayload)) *HookPayload {
	h := &HookPayload{
		Tool: "cursor",
		Payload: map[string]interface{}{
			"file":    "src/main.rs",
			"changes": []string{"+new line"},
		},
	}
	for _, o := range opts {
		o(h)
	}
	return h
}

// ─── Upload request / response factories ─────────────────────────────────────

func BuildUploadRequest(token *model.Token, edits ...*model.EditDTO) *model.EditBatchRequest {
	dtoList := make([]model.EditDTO, len(edits))
	for i, e := range edits {
		dtoList[i] = *e
	}
	if len(dtoList) == 0 {
		dtoList = append(dtoList, *BuildEditDTO())
	}
	return &model.EditBatchRequest{
		DeviceID:      "device-001",
		ClientVersion: "1.0.0",
		Edits:         dtoList,
	}
}

func BuildUploadResponse(accepted int, rejected, flagged []model.IndexedReason) *model.EditBatchResponse {
	if rejected == nil {
		rejected = []model.IndexedReason{}
	}
	if flagged == nil {
		flagged = []model.IndexedReason{}
	}
	return &model.EditBatchResponse{
		Accepted: accepted,
		Rejected: rejected,
		Flagged:  flagged,
	}
}

// ─── Negative (tampered) factories ───────────────────────────────────────────

// TamperedEditDTO returns an EditDTO with a wrong record_sig.
func TamperedEditDTO(opts ...func(*EditParams)) *model.EditDTO {
	edit := BuildEditDTO(opts...)
	edit.RecordSig = strings.Repeat("0", 64)
	return edit
}

// ExpiredTimestampEditDTO returns an EditDTO with a timestamp 10 minutes in the past.
func ExpiredTimestampEditDTO(opts ...func(*EditParams)) *model.EditDTO {
	past := time.Now().Add(-10 * time.Minute).UTC().Format(time.RFC3339)
	return BuildEditDTO(append(opts, func(p *EditParams) {
		p.Timestamp = past
	})...)
}

// OversizedEditDTO returns an EditDTO with added_lines > 5000.
func OversizedEditDTO(opts ...func(*EditParams)) *model.EditDTO {
	return BuildEditDTO(append(opts, func(p *EditParams) {
		p.AddedLines = 5001
		p.DiffHunk = nil // no diff to avoid inconsistency false-positive
	})...)
}

// MalformedEditDTO returns an EditDTO with a blank required field.
func MalformedEditDTO() *model.EditDTO {
	edit := BuildEditDTO()
	edit.Tool = "" // blank required field
	return edit
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

func computeRecordSig(p EditParams) string {
	hunkStr := ""
	if p.DiffHunk != nil {
		hunkStr = *p.DiffHunk
	}
	diffHunkHash := sha256HexStr(hunkStr)
	canonical := p.TokenKey + "\n" +
		p.DeviceID + "\n" +
		p.Hostname + "\n" +
		p.Timestamp + "\n" +
		p.Tool + "\n" +
		p.FilePath + "\n" +
		p.RepoURL + "\n" +
		p.CurrentSHA + "\n" +
		fmt.Sprintf("%d", p.AddedLines) + "\n" +
		fmt.Sprintf("%d", p.RemovedLines) + "\n" +
		diffHunkHash
	return hmacSHA256Hex(p.HmacSecret, canonical)
}

func hmacSHA256Hex(secret, msg string) string {
	mac := hmac.New(sha256.New, []byte(secret))
	mac.Write([]byte(msg))
	return hex.EncodeToString(mac.Sum(nil))
}

func sha256HexStr(s string) string {
	h := sha256.Sum256([]byte(s))
	return hex.EncodeToString(h[:])
}

func diffHunkPtr(s string) *string { return &s }

// BuildTokenWithSig builds a token matching the edit factories' default secrets.
func BuildTokenWithSig() *model.Token {
	return BuildToken(func(t *model.Token) {
		t.HmacSecret = "testsecret"
		t.TokenKey = "abcdef…7890"
	})
}

// SigService returns a real SignatureService for computing test signatures.
func SigService() *service.SignatureService { return service.NewSignatureService() }

// ─── Heartbeat option type (for test ergonomics) ──────────────────────────────

// HeartbeatReq is a convenience alias so test files can write func(*testkit.HeartbeatReq).
type HeartbeatReq = model.HeartbeatRequest

// ─── Device helpers ───────────────────────────────────────────────────────────

// DeviceNoHeartbeat returns a *Device with no LastHeartbeat set (will be marked silent).
func DeviceNoHeartbeat(deviceID, tokenKey string) *model.Device {
	return &model.Device{
		DeviceID:  deviceID,
		TokenKey:  tokenKey,
		CreatedAt: time.Now().UTC(),
	}
}
