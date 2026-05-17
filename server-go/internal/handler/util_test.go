package handler_test

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
)

// Tests for clampInt / parseInt edge cases (exercising edits.go helpers)
// and ReadBody error path (exercising auth.go ReadBody).

func TestEdits_QueryEdits_ClampAndParse(t *testing.T) {
	env := newTestEnv(t)

	cases := []struct {
		query string
	}{
		{"/api/v1/ai-track/edits?size=0"},   // clamped to 1
		{"/api/v1/ai-track/edits?size=999"}, // clamped to 100
		{"/api/v1/ai-track/edits?size=abc"}, // default 20
		{"/api/v1/ai-track/edits?page=-5"},  // default 0
		{"/api/v1/ai-track/edits?page=abc"}, // default 0
		{"/api/v1/ai-track/edits"},          // defaults
	}
	for _, c := range cases {
		req := env.signedRequest(http.MethodGet, c.query, nil)
		w := do(env.router, req)
		assertStatus(t, w, http.StatusOK)
	}
}

func TestWriteJSON_ContentType(t *testing.T) {
	env := newTestEnv(t)
	req := env.signedRequest(http.MethodGet, "/api/v1/ai-track/stats", nil)
	w := do(env.router, req)
	ct := w.Header().Get("Content-Type")
	if ct != "application/json" {
		t.Errorf("Content-Type = %q, want application/json", ct)
	}
}

func TestEdits_Submit_ReadBodyError(t *testing.T) {
	// We can't easily trigger io.ReadAll error in httptest,
	// but we can test the successful read-then-bad-JSON path
	// to exercise ReadBody's success branch fully.
	env := newTestEnv(t)
	body := []byte("clearly not json {{{")
	req := env.signedRequest(http.MethodPost, "/api/v1/ai-track/edits", body)
	w := do(env.router, req)
	// Should be 400 bad request (JSON parse error)
	assertStatus(t, w, http.StatusBadRequest)
}

func TestHeartbeat_Submit_BadBody(t *testing.T) {
	env := newTestEnv(t)
	body := []byte("{bad json")
	req := env.signedRequest(http.MethodPost, "/api/v1/ai-track/heartbeat", body)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusBadRequest)
}

func TestAdmin_CreateToken_FullResponse(t *testing.T) {
	env := newTestEnv(t)
	req := env.adminRequest(http.MethodPost, "/admin/tokens",
		map[string]string{"owner": "bob", "note": "my note"})
	w := do(env.router, req)
	assertStatus(t, w, http.StatusOK)

	var resp map[string]interface{}
	json.NewDecoder(w.Body).Decode(&resp)
	if _, ok := resp["token"]; !ok {
		t.Error("response should have 'token' field")
	}
	if _, ok := resp["hmac_secret"]; !ok {
		t.Error("response should have 'hmac_secret' field")
	}
	if _, ok := resp["token_key"]; !ok {
		t.Error("response should have 'token_key' field")
	}
}

func TestRouter_NotFound(t *testing.T) {
	env := newTestEnv(t)
	req := httptest.NewRequest(http.MethodGet, "/nonexistent", bytes.NewReader(nil))
	w := do(env.router, req)
	if w.Code != http.StatusMethodNotAllowed && w.Code != http.StatusNotFound {
		t.Errorf("expected 404 or 405, got %d", w.Code)
	}
}
