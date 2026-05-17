package handler_test

import (
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestStats_OK(t *testing.T) {
	env := newTestEnv(t)
	req := env.signedRequest(http.MethodGet, "/api/v1/ai-track/stats", nil)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusOK)
}

func TestStats_GroupByRepo(t *testing.T) {
	env := newTestEnv(t)
	req := env.signedRequest(http.MethodGet, "/api/v1/ai-track/stats?group_by=repo", nil)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusOK)
}

func TestStats_GroupByDevice(t *testing.T) {
	env := newTestEnv(t)
	req := env.signedRequest(http.MethodGet, "/api/v1/ai-track/stats?group_by=device", nil)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusOK)
}

func TestStats_NoAuth(t *testing.T) {
	env := newTestEnv(t)
	req := httptest.NewRequest(http.MethodGet, "/api/v1/ai-track/stats", nil)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusUnauthorized)
}

func TestDevices_OK(t *testing.T) {
	env := newTestEnv(t)

	// Add a heartbeat first
	import_ := env.signedRequest(http.MethodPost, "/api/v1/ai-track/heartbeat", []byte(`{"device_id":"dev-001","client_version":"1.0.0"}`))
	do(env.router, import_)

	req := env.signedRequest(http.MethodGet, "/api/v1/ai-track/devices", nil)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusOK)
}

func TestDevices_NoAuth(t *testing.T) {
	env := newTestEnv(t)
	req := httptest.NewRequest(http.MethodGet, "/api/v1/ai-track/devices", nil)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusUnauthorized)
}

func TestStats_GroupByHostname(t *testing.T) {
	env := newTestEnv(t)
	req := env.signedRequest(http.MethodGet, "/api/v1/ai-track/stats?group_by=hostname", nil)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusOK)
}

func TestStats_WithData(t *testing.T) {
	env := newTestEnv(t)

	// Ingest some edits first
	postReq, _ := env.signedEditRequest(t)
	do(env.router, postReq)

	for _, groupBy := range []string{"token", "repo", "device", "hostname", "unknown"} {
		req := env.signedRequest(http.MethodGet, "/api/v1/ai-track/stats?group_by="+groupBy, nil)
		w := do(env.router, req)
		assertStatus(t, w, http.StatusOK)
	}
}
