package testkit_test

import (
	"strings"
	"testing"

	"github.com/aitrack/server/internal/testkit"
)

func TestBuildToken(t *testing.T) {
	tok := testkit.BuildToken()
	if !tok.Active {
		t.Error("default token should be active")
	}
	if tok.Owner == "" {
		t.Error("default token should have owner")
	}
}

func TestBuildTokenWithSig(t *testing.T) {
	tok := testkit.BuildTokenWithSig()
	if tok.HmacSecret != "testsecret" {
		t.Errorf("HmacSecret = %q, want testsecret", tok.HmacSecret)
	}
	if tok.TokenKey != "abcdef…7890" {
		t.Errorf("TokenKey = %q, want abcdef…7890", tok.TokenKey)
	}
}

func TestBuildEditDTO_ValidSig(t *testing.T) {
	edit := testkit.BuildEditDTO()
	if edit.RecordSig == "" {
		t.Error("record_sig should be non-empty")
	}
	if len(edit.RecordSig) != 64 {
		t.Errorf("record_sig length = %d, want 64", len(edit.RecordSig))
	}
	if edit.AddedLines == nil || *edit.AddedLines <= 0 {
		t.Error("AddedLines should be set")
	}
}

func TestTamperedEditDTO(t *testing.T) {
	edit := testkit.TamperedEditDTO()
	if edit.RecordSig != strings.Repeat("0", 64) {
		t.Errorf("tampered sig = %q, want 64 zeros", edit.RecordSig)
	}
}

func TestExpiredTimestampEditDTO(t *testing.T) {
	edit := testkit.ExpiredTimestampEditDTO()
	if edit.Timestamp == "" {
		t.Error("expired timestamp edit should have a timestamp")
	}
}

func TestOversizedEditDTO(t *testing.T) {
	edit := testkit.OversizedEditDTO()
	if *edit.AddedLines <= 5000 {
		t.Errorf("oversized edit should have added_lines > 5000, got %d", *edit.AddedLines)
	}
}

func TestMalformedEditDTO(t *testing.T) {
	edit := testkit.MalformedEditDTO()
	if edit.Tool != "" {
		t.Error("malformed edit should have blank Tool")
	}
}

func TestBuildEditRecord(t *testing.T) {
	rec := testkit.BuildEditRecord()
	if rec.Status != "ACCEPTED" {
		t.Errorf("default status = %q, want ACCEPTED", rec.Status)
	}
}

func TestBuildHeartbeatRequest(t *testing.T) {
	hb := testkit.BuildHeartbeatRequest()
	if hb.DeviceID == "" {
		t.Error("heartbeat DeviceID should not be empty")
	}
	if hb.Hooks == nil {
		t.Error("default hooks should not be nil")
	}
	if !hb.Hooks.Claude {
		t.Error("default hook claude=true")
	}
}

func TestBuildClaudeHookPayload(t *testing.T) {
	h := testkit.BuildClaudeHookPayload()
	if h.Tool != "claude" {
		t.Errorf("tool = %q, want claude", h.Tool)
	}
}

func TestBuildCodexHookPayload(t *testing.T) {
	h := testkit.BuildCodexHookPayload()
	if h.Tool != "codex" {
		t.Errorf("tool = %q, want codex", h.Tool)
	}
}

func TestBuildCursorHookPayload(t *testing.T) {
	h := testkit.BuildCursorHookPayload()
	if h.Tool != "cursor" {
		t.Errorf("tool = %q, want cursor", h.Tool)
	}
}

func TestBuildUploadResponse(t *testing.T) {
	resp := testkit.BuildUploadResponse(3, nil, nil)
	if resp.Accepted != 3 {
		t.Errorf("accepted = %d, want 3", resp.Accepted)
	}
	if resp.Rejected == nil {
		t.Error("rejected should be non-nil empty slice")
	}
}

func TestDeviceNoHeartbeat(t *testing.T) {
	d := testkit.DeviceNoHeartbeat("dev-1", "key-1")
	if d.DeviceID != "dev-1" {
		t.Errorf("DeviceID = %q, want dev-1", d.DeviceID)
	}
	if d.LastHeartbeat != nil {
		t.Error("LastHeartbeat should be nil")
	}
}

func TestSetSeed(t *testing.T) {
	testkit.SetSeed(42)
	e1 := testkit.BuildEditDTO()
	testkit.SetSeed(42)
	e2 := testkit.BuildEditDTO()
	// session_id uses rng but RecordSig is deterministic given same params
	if e1.RecordSig != e2.RecordSig {
		t.Error("same seed should produce same RecordSig")
	}
}

func TestSigService(t *testing.T) {
	svc := testkit.SigService()
	if svc == nil {
		t.Error("SigService should not be nil")
	}
}

func TestBuildUploadRequest_DefaultEdit(t *testing.T) {
	tok := testkit.BuildTokenWithSig()
	req := testkit.BuildUploadRequest(tok)
	if len(req.Edits) != 1 {
		t.Errorf("expected 1 default edit, got %d", len(req.Edits))
	}
}
