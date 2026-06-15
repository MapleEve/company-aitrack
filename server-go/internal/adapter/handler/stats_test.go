package handler_test

import (
	"fmt"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/aitrack/server/internal/domain/model"
	"github.com/aitrack/server/internal/testkit"
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

func TestStats_GroupByTool(t *testing.T) {
	env := newTestEnv(t)
	toolName := fmt.Sprintf("tool-%d", time.Now().UnixNano())
	tokenKey := env.resolveTokenKey(t)
	p := testkit.DefaultEditParams()
	p.HmacSecret = env.hmacSecret
	p.TokenKey = tokenKey
	p.Tool = toolName
	p.FilePath = "src/tool_group.go"
	edit := testkit.BuildEditDTO(func(ep *testkit.EditParams) { *ep = p })
	body := env.buildEditBatch(tokenKey, edit)
	postReq := env.signedRequest(http.MethodPost, "/api/v1/ai-track/edits", body)
	postResp := do(env.router, postReq)
	assertStatus(t, postResp, http.StatusOK)

	req := env.signedRequest(http.MethodGet, "/api/v1/ai-track/stats?group_by=tool", nil)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusOK)

	var rows []model.StatsRow
	decodeJSON(t, w, &rows)
	for _, row := range rows {
		if row.Group == toolName {
			if row.Edits != 1 || row.Accepted != 1 {
				t.Fatalf("tool stats = %+v, want edits=1 accepted=1", row)
			}
			return
		}
	}
	t.Fatalf("tool group %q not found in %+v", toolName, rows)
}

func TestStats_WithData(t *testing.T) {
	env := newTestEnv(t)

	// Ingest some edits first
	postReq, _ := env.signedEditRequest(t)
	do(env.router, postReq)

	for _, groupBy := range []string{"token", "repo", "device", "hostname", "tool", "unknown"} {
		req := env.signedRequest(http.MethodGet, "/api/v1/ai-track/stats?group_by="+groupBy, nil)
		w := do(env.router, req)
		assertStatus(t, w, http.StatusOK)
	}
}
