package handler_test

import (
	"bytes"
	"compress/gzip"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strconv"
	"testing"
	"time"
)

func (e *testEnv) signedGzipUsageRequest(method, path string, body []byte) *http.Request {
	var buf bytes.Buffer
	gz := gzip.NewWriter(&buf)
	_, _ = gz.Write(body)
	_ = gz.Close()
	compressed := buf.Bytes()

	req := httptest.NewRequest(method, path, bytes.NewReader(compressed))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Content-Encoding", "gzip")
	req.Header.Set("Authorization", "Bearer "+e.rawToken)
	sum := sha256.Sum256(compressed)
	req.Header.Set("X-AiTrack-Body-Sha256", hex.EncodeToString(sum[:]))
	req.Header.Set("X-AiTrack-Body-Timestamp", time.Now().UTC().Format(time.RFC3339))
	ts := strconv.FormatInt(time.Now().Unix(), 10)
	req.Header.Set("X-AiTrack-Timestamp", ts)
	req.Header.Set("X-AiTrack-Signature", e.sig.ComputeRequestSignature(e.hmacSecret, ts, compressed))
	return req
}

func (e *testEnv) signedUsageRequest(method, path string, body []byte) *http.Request {
	req := httptest.NewRequest(method, path, bytes.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+e.rawToken)
	sum := sha256.Sum256(body)
	req.Header.Set("X-AiTrack-Body-Sha256", hex.EncodeToString(sum[:]))
	req.Header.Set("X-AiTrack-Body-Timestamp", time.Now().UTC().Format(time.RFC3339))
	ts := strconv.FormatInt(time.Now().Unix(), 10)
	req.Header.Set("X-AiTrack-Timestamp", ts)
	req.Header.Set("X-AiTrack-Signature", e.sig.ComputeRequestSignature(e.hmacSecret, ts, body))
	return req
}

func TestUsageRollupIngestAndSummary(t *testing.T) {
	env := newTestEnv(t)
	body := []byte(`{"items":[{"device_id":"usage-device-1","day":"2026-06-16","agent":"codex","model":"gpt-5","account":"openai","tokens_in":10,"tokens_out":20,"tokens_cache_read":3,"tokens_cache_write":4,"tokens_reasoning":5,"message_count":2,"source_cost":0.12}]}`)

	req := env.signedGzipUsageRequest(http.MethodPost, "/api/v1/ai-track/usage/rollup", body)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusOK)

	// Idempotent overwrite: same natural key with different values should not double count.
	body = []byte(`{"items":[{"device_id":"usage-device-1","day":"2026-06-16","agent":"codex","model":"gpt-5","account":"openai","tokens_in":11,"tokens_out":22,"tokens_cache_read":0,"tokens_cache_write":0,"tokens_reasoning":7,"message_count":3,"source_cost":0.25}]}`)
	req = env.signedGzipUsageRequest(http.MethodPost, "/api/v1/ai-track/usage/rollup", body)
	w = do(env.router, req)
	assertStatus(t, w, http.StatusOK)

	req = env.signedRequest(http.MethodGet, "/api/v1/ai-track/usage/summary?agent=codex", nil)
	w = do(env.router, req)
	assertStatus(t, w, http.StatusOK)
	var resp map[string]interface{}
	decodeJSON(t, w, &resp)
	if got := int(resp["total_tokens"].(float64)); got != 40 {
		t.Fatalf("total_tokens = %d, want 40", got)
	}
	if got := int(resp["message_count"].(float64)); got != 3 {
		t.Fatalf("message_count = %d, want 3", got)
	}
	if got := resp["source_cost"].(float64); got != 0.25 {
		t.Fatalf("source_cost = %f, want 0.25", got)
	}
}

func TestUsageRollupBasisKeepsNativeAndLocalDerivedSeparate(t *testing.T) {
	env := newTestEnv(t)
	body := []byte(`{"items":[{"device_id":"usage-device-basis","day":"2026-06-17","agent":"codex","model":"gpt-5","account":"openai","usage_basis":"native","tokens_in":10,"tokens_out":5,"tokens_cache_read":0,"tokens_cache_write":0,"tokens_reasoning":0,"message_count":1},{"device_id":"usage-device-basis","day":"2026-06-17","agent":"codex","model":"gpt-5","account":"openai","usage_basis":"local_derived","tokens_in":20,"tokens_out":6,"tokens_cache_read":0,"tokens_cache_write":0,"tokens_reasoning":0,"message_count":2}]}`)

	req := env.signedGzipUsageRequest(http.MethodPost, "/api/v1/ai-track/usage/rollup", body)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusOK)

	// Same basis still overwrites idempotently.
	body = []byte(`{"items":[{"device_id":"usage-device-basis","day":"2026-06-17","agent":"codex","model":"gpt-5","account":"openai","usage_basis":"local_derived","tokens_in":30,"tokens_out":7,"tokens_cache_read":0,"tokens_cache_write":0,"tokens_reasoning":0,"message_count":3}]}`)
	req = env.signedGzipUsageRequest(http.MethodPost, "/api/v1/ai-track/usage/rollup", body)
	w = do(env.router, req)
	assertStatus(t, w, http.StatusOK)

	req = env.signedRequest(http.MethodGet, "/api/v1/ai-track/usage/summary?agent=codex&limit=10", nil)
	w = do(env.router, req)
	assertStatus(t, w, http.StatusOK)

	var resp struct {
		TotalTokens  int64 `json:"total_tokens"`
		MessageCount int64 `json:"message_count"`
		Items        []struct {
			UsageBasis   string `json:"usage_basis"`
			TotalTokens  int64  `json:"total_tokens"`
			MessageCount int64  `json:"message_count"`
		} `json:"items"`
	}
	decodeJSON(t, w, &resp)
	if resp.TotalTokens != 52 {
		t.Fatalf("total_tokens = %d, want 52", resp.TotalTokens)
	}
	if resp.MessageCount != 4 {
		t.Fatalf("message_count = %d, want 4", resp.MessageCount)
	}
	got := map[string]struct {
		totalTokens  int64
		messageCount int64
	}{}
	for _, item := range resp.Items {
		got[item.UsageBasis] = struct {
			totalTokens  int64
			messageCount int64
		}{totalTokens: item.TotalTokens, messageCount: item.MessageCount}
	}
	if got["native"].totalTokens != 15 || got["native"].messageCount != 1 {
		t.Fatalf("native summary item = %+v, want total_tokens=15 message_count=1", got["native"])
	}
	if got["local_derived"].totalTokens != 37 || got["local_derived"].messageCount != 3 {
		t.Fatalf("local_derived summary item = %+v, want total_tokens=37 message_count=3", got["local_derived"])
	}
}

func TestUsageRollupDefaultsMissingBasisToNative(t *testing.T) {
	env := newTestEnv(t)
	body := []byte(`{"items":[{"device_id":"usage-device-default-basis","day":"2026-06-18","agent":"codex","model":"gpt-5","account":"openai","tokens_in":1,"tokens_out":0,"tokens_cache_read":0,"tokens_cache_write":0,"tokens_reasoning":0}]}`)

	req := env.signedGzipUsageRequest(http.MethodPost, "/api/v1/ai-track/usage/rollup", body)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusOK)

	req = env.signedRequest(http.MethodGet, "/api/v1/ai-track/usage/summary?agent=codex&limit=10", nil)
	w = do(env.router, req)
	assertStatus(t, w, http.StatusOK)

	var resp struct {
		Items []struct {
			UsageBasis string `json:"usage_basis"`
		} `json:"items"`
	}
	decodeJSON(t, w, &resp)
	for _, item := range resp.Items {
		if item.UsageBasis == "native" {
			return
		}
	}
	t.Fatalf("summary items did not include default usage_basis native: %+v", resp.Items)
}

func TestUsageRollupRejectsInvalidBasis(t *testing.T) {
	env := newTestEnv(t)
	body := []byte(`{"items":[{"device_id":"usage-device-invalid-basis","day":"2026-06-18","agent":"codex","model":"gpt-5","account":"openai","usage_basis":"estimated","tokens_in":1,"tokens_out":0,"tokens_cache_read":0,"tokens_cache_write":0,"tokens_reasoning":0}]}`)

	req := env.signedGzipUsageRequest(http.MethodPost, "/api/v1/ai-track/usage/rollup", body)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusBadRequest)
}

func TestUsageRollupAcceptsIdentityJson(t *testing.T) {
	env := newTestEnv(t)
	body := []byte(`{"items":[{"device_id":"usage-device-identity","day":"2026-06-16","agent":"codex","model":"gpt-5","account":"local","tokens_in":1,"tokens_out":2,"tokens_cache_read":0,"tokens_cache_write":0,"tokens_reasoning":0}]}`)
	req := env.signedUsageRequest(http.MethodPost, "/api/v1/ai-track/usage/rollup", body)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusOK)
}

func TestUsageRollupRejectsDigestMismatch(t *testing.T) {
	env := newTestEnv(t)
	req := env.signedGzipUsageRequest(http.MethodPost, "/api/v1/ai-track/usage/rollup", []byte(`{"items":[]}`))
	req.Header.Set("X-AiTrack-Body-Sha256", "deadbeef")
	w := do(env.router, req)
	assertStatus(t, w, http.StatusBadRequest)
}

func TestUsageRollupRejectsGzipBodyAboveDecodedLimit(t *testing.T) {
	env := newTestEnv(t)
	body := bytes.Repeat([]byte("x"), 8<<20+1)

	req := env.signedGzipUsageRequest(http.MethodPost, "/api/v1/ai-track/usage/rollup", body)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusRequestEntityTooLarge)
}

func TestUsageSubscriptionIngest(t *testing.T) {
	env := newTestEnv(t)
	body, _ := json.Marshal(map[string]interface{}{
		"device_id":               "usage-device-1",
		"agent":                   "codex",
		"account":                 "local",
		"plan":                    "Pro",
		"quota_session_remaining": 70,
		"quota_weekly_remaining":  80,
		"quota_reset_at":          "2026-06-16T10:00:00Z",
		"reader_status":           "ok",
		"snapshotted_at":          "2026-06-16T09:00:00Z",
	})
	req := env.signedGzipUsageRequest(http.MethodPost, "/api/v1/ai-track/usage/subscription", body)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusOK)

	var count int
	if err := env.db.QueryRow("SELECT COUNT(*) FROM usage_subscription_snapshots WHERE agent = 'codex'").Scan(&count); err != nil {
		t.Fatalf("count subscription snapshots: %v", err)
	}
	if count != 1 {
		t.Fatalf("subscription rows = %d, want 1", count)
	}
}
