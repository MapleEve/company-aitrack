package handler_test

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/aitrack/server/internal/domain/model"
)

func TestHeartbeat_OK(t *testing.T) {
	env := newTestEnv(t)
	hb := model.HeartbeatRequest{
		DeviceID:      "dev-hooks-arbitrary",
		ClientVersion: "1.0.0",
		TS:            1716000000,
		Hooks: model.HeartbeatHooks{
			"claude":   true,
			"opencode": true,
			"trae":     false,
		},
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

	var hooksJSON string
	if err := env.db.QueryRow("SELECT hooks_json FROM devices WHERE device_id = $1", hb.DeviceID).Scan(&hooksJSON); err != nil {
		t.Fatalf("query hooks_json: %v", err)
	}
	var hooks map[string]bool
	if err := json.Unmarshal([]byte(hooksJSON), &hooks); err != nil {
		t.Fatalf("unmarshal hooks_json %q: %v", hooksJSON, err)
	}
	if !hooks["opencode"] {
		t.Errorf("hooks_json missing opencode=true: %s", hooksJSON)
	}
	if hooks["trae"] {
		t.Errorf("hooks_json trae = true, want false: %s", hooksJSON)
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
