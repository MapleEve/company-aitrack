package handler_test

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strconv"
	"strings"
	"testing"
	"time"

	"github.com/aitrack/server/internal/model"
	"github.com/aitrack/server/internal/testkit"
)

func TestEdits_Submit_OK(t *testing.T) {
	env := newTestEnv(t)
	req, _ := env.signedEditRequest(t)

	w := do(env.router, req)
	assertStatus(t, w, http.StatusOK)

	var resp model.EditBatchResponse
	decodeJSON(t, w, &resp)
	if resp.Accepted != 1 {
		t.Errorf("expected 1 accepted, got %d; rejected=%v flagged=%v",
			resp.Accepted, resp.Rejected, resp.Flagged)
	}
}

func TestEdits_Submit_NoAuth(t *testing.T) {
	env := newTestEnv(t)
	edit := env.buildValidEditDTO(t)
	body := env.buildEditBatch(env.resolveTokenKey(t), edit)
	req := httptest.NewRequest(http.MethodPost, "/api/v1/ai-track/edits", bytes.NewReader(body))
	req.Header.Set("Content-Type", "application/json")

	w := do(env.router, req)
	assertStatus(t, w, http.StatusUnauthorized)
}

func TestEdits_Submit_WrongToken(t *testing.T) {
	env := newTestEnv(t)
	edit := env.buildValidEditDTO(t)
	body := env.buildEditBatch(env.resolveTokenKey(t), edit)
	ts := nowTS()
	sig := env.sig.ComputeRequestSignature(env.hmacSecret, ts, body)
	req := httptest.NewRequest(http.MethodPost, "/api/v1/ai-track/edits", bytes.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer aitrack_badtoken00000000000000000000000")
	req.Header.Set("X-AiTrack-Timestamp", ts)
	req.Header.Set("X-AiTrack-Signature", sig)

	w := do(env.router, req)
	assertStatus(t, w, http.StatusUnauthorized)
}

func TestEdits_Submit_MissingTimestamp(t *testing.T) {
	env := newTestEnv(t)
	edit := env.buildValidEditDTO(t)
	body := env.buildEditBatch(env.resolveTokenKey(t), edit)
	req := httptest.NewRequest(http.MethodPost, "/api/v1/ai-track/edits", bytes.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+env.rawToken)

	w := do(env.router, req)
	assertStatus(t, w, http.StatusUnauthorized)
}

func TestEdits_Submit_InvalidTimestamp(t *testing.T) {
	env := newTestEnv(t)
	edit := env.buildValidEditDTO(t)
	body := env.buildEditBatch(env.resolveTokenKey(t), edit)
	req := httptest.NewRequest(http.MethodPost, "/api/v1/ai-track/edits", bytes.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+env.rawToken)
	req.Header.Set("X-AiTrack-Timestamp", "not-a-number")
	req.Header.Set("X-AiTrack-Signature", "sig")

	w := do(env.router, req)
	assertStatus(t, w, http.StatusUnauthorized)
}

func TestEdits_Submit_ExpiredTimestamp(t *testing.T) {
	env := newTestEnv(t)
	edit := env.buildValidEditDTO(t)
	body := env.buildEditBatch(env.resolveTokenKey(t), edit)
	oldTS := strconv.FormatInt(time.Now().Add(-10*time.Minute).Unix(), 10)
	sig := env.sig.ComputeRequestSignature(env.hmacSecret, oldTS, body)
	req := httptest.NewRequest(http.MethodPost, "/api/v1/ai-track/edits", bytes.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+env.rawToken)
	req.Header.Set("X-AiTrack-Timestamp", oldTS)
	req.Header.Set("X-AiTrack-Signature", sig)

	w := do(env.router, req)
	assertStatus(t, w, http.StatusUnauthorized)
}

func TestEdits_Submit_WrongSignature(t *testing.T) {
	env := newTestEnv(t)
	edit := env.buildValidEditDTO(t)
	body := env.buildEditBatch(env.resolveTokenKey(t), edit)
	ts := nowTS()
	req := httptest.NewRequest(http.MethodPost, "/api/v1/ai-track/edits", bytes.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+env.rawToken)
	req.Header.Set("X-AiTrack-Timestamp", ts)
	req.Header.Set("X-AiTrack-Signature", strings.Repeat("0", 64))

	w := do(env.router, req)
	assertStatus(t, w, http.StatusUnauthorized)
}

func TestEdits_Submit_EmptyEdits(t *testing.T) {
	env := newTestEnv(t)
	body, _ := json.Marshal(model.EditBatchRequest{
		DeviceID: "dev-001",
		Edits:    []model.EditDTO{},
	})
	req := env.signedRequest(http.MethodPost, "/api/v1/ai-track/edits", body)

	w := do(env.router, req)
	assertStatus(t, w, http.StatusBadRequest)
}

func TestEdits_Submit_InvalidJSON(t *testing.T) {
	env := newTestEnv(t)
	body := []byte("not json")
	req := env.signedRequest(http.MethodPost, "/api/v1/ai-track/edits", body)

	w := do(env.router, req)
	assertStatus(t, w, http.StatusBadRequest)
}

func TestEdits_Submit_TamperedSig(t *testing.T) {
	env := newTestEnv(t)
	tokenKey := env.resolveTokenKey(t)

	// Build edit with wrong record_sig
	p := testkit.DefaultEditParams()
	p.HmacSecret = env.hmacSecret
	p.TokenKey = tokenKey
	edit := testkit.BuildEditDTO(func(ep *testkit.EditParams) { *ep = p })
	edit.RecordSig = strings.Repeat("0", 64)

	body := env.buildEditBatch(tokenKey, edit)
	req := env.signedRequest(http.MethodPost, "/api/v1/ai-track/edits", body)

	w := do(env.router, req)
	assertStatus(t, w, http.StatusOK)

	var resp model.EditBatchResponse
	decodeJSON(t, w, &resp)
	if len(resp.Rejected) != 1 || resp.Rejected[0].Reason != "sig_mismatch" {
		t.Errorf("expected sig_mismatch rejection, got %+v", resp)
	}
}

func TestEdits_Submit_MalformedEdit(t *testing.T) {
	env := newTestEnv(t)
	tokenKey := env.resolveTokenKey(t)
	p := testkit.DefaultEditParams()
	p.HmacSecret = env.hmacSecret
	p.TokenKey = tokenKey
	edit := testkit.BuildEditDTO(func(ep *testkit.EditParams) { *ep = p })
	edit.Tool = "" // malformed

	body := env.buildEditBatch(tokenKey, edit)
	req := env.signedRequest(http.MethodPost, "/api/v1/ai-track/edits", body)

	w := do(env.router, req)
	assertStatus(t, w, http.StatusOK)

	var resp model.EditBatchResponse
	decodeJSON(t, w, &resp)
	if len(resp.Rejected) != 1 || resp.Rejected[0].Reason != "malformed" {
		t.Errorf("expected malformed rejection, got %+v", resp)
	}
}

func TestEdits_Query_OK(t *testing.T) {
	env := newTestEnv(t)
	// First ingest something
	req, _ := env.signedEditRequest(t)
	do(env.router, req)

	// Query
	getReq := env.signedRequest(http.MethodGet, "/api/v1/ai-track/edits?page=0&size=10", nil)
	w := do(env.router, getReq)
	assertStatus(t, w, http.StatusOK)

	var result model.EditQueryResult
	decodeJSON(t, w, &result)
	if result.Total < 1 {
		t.Errorf("expected at least 1 record, got total=%d", result.Total)
	}
}

func TestEdits_Query_NoAuth(t *testing.T) {
	env := newTestEnv(t)
	req := httptest.NewRequest(http.MethodGet, "/api/v1/ai-track/edits", nil)

	w := do(env.router, req)
	assertStatus(t, w, http.StatusUnauthorized)
}

func TestEdits_Query_WithFilters(t *testing.T) {
	env := newTestEnv(t)
	req, _ := env.signedEditRequest(t)
	do(env.router, req)

	// Query with token_key and repo filter
	tk := env.resolveTokenKey(t)
	path := "/api/v1/ai-track/edits?token_key=" + tk + "&repo=git@github.com:org/repo.git&size=5"
	getReq := env.signedRequest(http.MethodGet, path, nil)
	w := do(env.router, getReq)
	assertStatus(t, w, http.StatusOK)
}
