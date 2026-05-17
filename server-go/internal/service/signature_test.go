package service_test

import (
	"testing"

	"github.com/aitrack/server/internal/service"
)

func TestSHA256Hex(t *testing.T) {
	svc := service.NewSignatureService()
	// empty string sha256
	got := svc.SHA256HexStr("")
	want := "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
	if got != want {
		t.Errorf("SHA256('') = %s, want %s", got, want)
	}
}

func TestHmacSHA256Hex(t *testing.T) {
	svc := service.NewSignatureService()
	// Known HMAC-SHA256 value
	got := svc.HmacSHA256Hex("key", "message")
	want := "6e9ef29b75fffc5b7abae527d58fdadb2fe42e7219011976917343065f58ed4a"
	if got != want {
		t.Errorf("HMAC('key','message') = %s, want %s", got, want)
	}
}

func TestComputeRequestSignature(t *testing.T) {
	svc := service.NewSignatureService()
	body := []byte(`{"hello":"world"}`)
	ts := "1716000000"
	sig := svc.ComputeRequestSignature("mysecret", ts, body)
	if len(sig) != 64 {
		t.Errorf("expected 64-char hex, got %d", len(sig))
	}
	// Deterministic: same inputs must give same output
	sig2 := svc.ComputeRequestSignature("mysecret", ts, body)
	if sig != sig2 {
		t.Error("request signature not deterministic")
	}
	// Different secret gives different result
	sig3 := svc.ComputeRequestSignature("other", ts, body)
	if sig == sig3 {
		t.Error("different secret should give different signature")
	}
}

func TestComputeRecordSig_FieldOrder(t *testing.T) {
	svc := service.NewSignatureService()
	// Canonical string field order from CONTRACT.md v1.1:
	// token_key, device_id, hostname, timestamp, tool, file_path, repo_url, current_sha,
	// added_lines, removed_lines, sha256(diff_hunk or "")
	hunk := "@@ -1 +1 @@\n-old\n+new\n"
	sig := svc.ComputeRecordSig(
		"secret", "tok123…0000", "dev-001", "host.local", "2026-05-17T10:00:00Z",
		"claude", "src/main.rs", "git@github.com:org/repo.git", "abc123",
		1, 0, &hunk,
	)
	if len(sig) != 64 {
		t.Errorf("expected 64-char hex, got %d", len(sig))
	}
	// Nil diff_hunk uses sha256("")
	sigNil := svc.ComputeRecordSig(
		"secret", "tok123…0000", "dev-001", "host.local", "2026-05-17T10:00:00Z",
		"claude", "src/main.rs", "git@github.com:org/repo.git", "abc123",
		1, 0, nil,
	)
	emptyHunk := ""
	sigEmpty := svc.ComputeRecordSig(
		"secret", "tok123…0000", "dev-001", "host.local", "2026-05-17T10:00:00Z",
		"claude", "src/main.rs", "git@github.com:org/repo.git", "abc123",
		1, 0, &emptyHunk,
	)
	if sigNil != sigEmpty {
		t.Error("nil diff_hunk and empty string diff_hunk should produce same sig")
	}
	// Different line counts produce different sigs
	sig2 := svc.ComputeRecordSig(
		"secret", "tok123…0000", "dev-001", "host.local", "2026-05-17T10:00:00Z",
		"claude", "src/main.rs", "git@github.com:org/repo.git", "abc123",
		2, 0, nil,
	)
	if sig2 == sigNil {
		t.Error("different added_lines should produce different sig")
	}
	// Different hostname produces different sig
	sigDiffHost := svc.ComputeRecordSig(
		"secret", "tok123…0000", "dev-001", "other-host.local", "2026-05-17T10:00:00Z",
		"claude", "src/main.rs", "git@github.com:org/repo.git", "abc123",
		1, 0, nil,
	)
	if sigDiffHost == sigNil {
		t.Error("different hostname should produce different sig")
	}
}

func TestConstantTimeEqual(t *testing.T) {
	cases := []struct {
		a, b string
		want bool
	}{
		{"abc", "abc", true},
		{"abc", "abd", false},
		{"abc", "ab", false},
		{"", "", true},
	}
	for _, c := range cases {
		got := service.ConstantTimeEqual(c.a, c.b)
		if got != c.want {
			t.Errorf("ConstantTimeEqual(%q, %q) = %v, want %v", c.a, c.b, got, c.want)
		}
	}
}
