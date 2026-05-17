package handler_test

import (
	"bytes"
	"net/http"
	"net/http/httptest"
	"strconv"
	"testing"
	"time"
)

// Tests targeting auth.go edge-cases not covered by edits_test.go

func TestAuth_MissingBearerPrefix(t *testing.T) {
	env := newTestEnv(t)
	body := []byte(`{"device_id":"d","edits":[]}`)
	req := httptest.NewRequest(http.MethodPost, "/api/v1/ai-track/edits", bytes.NewReader(body))
	req.Header.Set("Authorization", "Token "+env.rawToken) // wrong scheme
	req.Header.Set("X-AiTrack-Timestamp", nowTS())
	req.Header.Set("X-AiTrack-Signature", "sig")

	w := do(env.router, req)
	assertStatus(t, w, http.StatusUnauthorized)
}

func TestAuth_EmptyAuthorization(t *testing.T) {
	env := newTestEnv(t)
	body := []byte(`{}`)
	req := httptest.NewRequest(http.MethodPost, "/api/v1/ai-track/edits", bytes.NewReader(body))
	// No Authorization header at all

	w := do(env.router, req)
	assertStatus(t, w, http.StatusUnauthorized)
}

func TestAuth_MissingSignatureHeader(t *testing.T) {
	env := newTestEnv(t)
	body := []byte(`{"device_id":"d","edits":[]}`)
	ts := strconv.FormatInt(time.Now().Unix(), 10)
	req := httptest.NewRequest(http.MethodPost, "/api/v1/ai-track/edits", bytes.NewReader(body))
	req.Header.Set("Authorization", "Bearer "+env.rawToken)
	req.Header.Set("X-AiTrack-Timestamp", ts)
	// Missing X-AiTrack-Signature

	w := do(env.router, req)
	assertStatus(t, w, http.StatusUnauthorized)
}
