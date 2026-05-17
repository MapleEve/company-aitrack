package handler_test

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/aitrack/server/internal/model"
)

func TestHeartbeat_OK(t *testing.T) {
	env := newTestEnv(t)
	hb := model.HeartbeatRequest{
		DeviceID:      "dev-001",
		ClientVersion: "1.0.0",
		TS:            1716000000,
		Hooks:         &model.HeartbeatHooks{Claude: true},
	}
	body, _ := json.Marshal(hb)
	req := env.signedRequest(http.MethodPost, "/api/v1/ai-track/heartbeat", body)

	w := do(env.router, req)
	assertStatus(t, w, http.StatusOK)

	var resp map[string]bool
	decodeJSON(t, w, &resp)
	if !resp["ok"] {
		t.Error("expected ok=true")
	}
}

func TestHeartbeat_NoAuth(t *testing.T) {
	env := newTestEnv(t)
	hb := model.HeartbeatRequest{DeviceID: "dev-001"}
	body, _ := json.Marshal(hb)
	req := httptest.NewRequest(http.MethodPost, "/api/v1/ai-track/heartbeat", bytes.NewReader(body))
	req.Header.Set("Content-Type", "application/json")

	w := do(env.router, req)
	assertStatus(t, w, http.StatusUnauthorized)
}

func TestHeartbeat_MissingDeviceID(t *testing.T) {
	env := newTestEnv(t)
	hb := model.HeartbeatRequest{DeviceID: ""}
	body, _ := json.Marshal(hb)
	req := env.signedRequest(http.MethodPost, "/api/v1/ai-track/heartbeat", body)

	w := do(env.router, req)
	assertStatus(t, w, http.StatusBadRequest)
}

func TestHeartbeat_InvalidJSON(t *testing.T) {
	env := newTestEnv(t)
	body := []byte("not json")
	req := env.signedRequest(http.MethodPost, "/api/v1/ai-track/heartbeat", body)

	w := do(env.router, req)
	assertStatus(t, w, http.StatusBadRequest)
}

func TestHeartbeat_NilHooks(t *testing.T) {
	env := newTestEnv(t)
	hb := model.HeartbeatRequest{
		DeviceID:      "dev-002",
		ClientVersion: "1.0.0",
		Hooks:         nil,
	}
	body, _ := json.Marshal(hb)
	req := env.signedRequest(http.MethodPost, "/api/v1/ai-track/heartbeat", body)

	w := do(env.router, req)
	assertStatus(t, w, http.StatusOK)
}

func TestHeartbeat_Upsert(t *testing.T) {
	env := newTestEnv(t)
	for i := 0; i < 2; i++ {
		hb := model.HeartbeatRequest{
			DeviceID:      "dev-upsert",
			ClientVersion: "1.0.0",
		}
		body, _ := json.Marshal(hb)
		req := env.signedRequest(http.MethodPost, "/api/v1/ai-track/heartbeat", body)
		w := do(env.router, req)
		assertStatus(t, w, http.StatusOK)
	}
}
