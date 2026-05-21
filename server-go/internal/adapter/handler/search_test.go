package handler_test

import (
	"bytes"
	"database/sql"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/aitrack/server/internal/adapter/handler"
)

// newSearchTestRouter builds a minimal chi router that only has the two
// search/similar routes, backed by an in-memory SQLite db.
func newSearchTestRouter(t *testing.T, db *sql.DB, adminKey string, isPostgres bool) http.Handler {
	t.Helper()
	searchH := handler.NewSearchHandler(db, adminKey, isPostgres)
	similarH := handler.NewSimilarHandler(db, adminKey, isPostgres)
	// Wire into a full router via the shared testEnv (which always uses isPostgres=false),
	// but for custom isPostgres=true tests we use the handlers directly.
	_ = searchH
	_ = similarH

	// Build a standalone router for these handlers only.
	mux := http.NewServeMux()
	mux.HandleFunc("/api/v1/ai-track/edits/search", searchH.Search)
	mux.HandleFunc("/api/v1/ai-track/edits/similar", similarH.Similar)
	return mux
}

// ─── SearchHandler tests ────────────────────────────────────────────────────

// Test 1: Search 403 — no admin key
func TestSearch_NoAdminKey(t *testing.T) {
	env := newTestEnv(t)
	req := httptest.NewRequest(http.MethodGet, "/api/v1/ai-track/edits/search?q=hello", nil)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusForbidden)
}

// Test 2: Search 400 — missing q parameter
func TestSearch_MissingQ(t *testing.T) {
	env := newTestEnv(t)
	req := httptest.NewRequest(http.MethodGet, "/api/v1/ai-track/edits/search", nil)
	req.Header.Set("X-Admin-Key", env.cfg.AdminKey)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusBadRequest)

	var body map[string]string
	decodeJSON(t, w, &body)
	if body["error"] == "" {
		t.Error("expected non-empty error message")
	}
}

// Test 3: Search 501 — SQLite mode (isPostgres=false)
func TestSearch_SQLiteMode_Returns501(t *testing.T) {
	env := newTestEnv(t)
	sqliteRouter := newSearchTestRouter(t, env.db, env.cfg.AdminKey, false)
	req := httptest.NewRequest(http.MethodGet, "/api/v1/ai-track/edits/search?q=foo", nil)
	req.Header.Set("X-Admin-Key", env.cfg.AdminKey)
	w := do(sqliteRouter, req)
	assertStatus(t, w, http.StatusNotImplemented)

	var body map[string]string
	decodeJSON(t, w, &body)
	if body["error"] == "" {
		t.Error("expected error message for SQLite mode")
	}
}

// Test 4: Search 400 — q is URL-encoded whitespace only (trimmed to empty)
func TestSearch_EmptyQ_WhitespaceOnly(t *testing.T) {
	env := newTestEnv(t)
	// Use %20 (encoded space) so httptest.NewRequest doesn't panic on raw spaces.
	req := httptest.NewRequest(http.MethodGet, "/api/v1/ai-track/edits/search?q=%20%20%20", nil)
	req.Header.Set("X-Admin-Key", env.cfg.AdminKey)
	w := do(env.router, req)
	// q is whitespace-only → trimmed to "" → 400
	assertStatus(t, w, http.StatusBadRequest)
}

// Test 5: Search — admin key not configured returns 503
func TestSearch_AdminKeyNotConfigured(t *testing.T) {
	env := newTestEnv(t)
	env.cfg.AdminKey = ""

	// Rebuild handler with empty admin key pointing to same router infra.
	searchH := handler.NewSearchHandler(nil, "", false)
	mux := http.NewServeMux()
	mux.HandleFunc("/api/v1/ai-track/edits/search", searchH.Search)

	req := httptest.NewRequest(http.MethodGet, "/api/v1/ai-track/edits/search?q=test", nil)
	req.Header.Set("X-Admin-Key", "anything")
	w := httptest.NewRecorder()
	mux.ServeHTTP(w, req)
	assertStatus(t, w, http.StatusServiceUnavailable)
}

// ─── SimilarHandler tests ───────────────────────────────────────────────────

// Test 6: Similar 403 — no admin key
func TestSimilar_NoAdminKey(t *testing.T) {
	env := newTestEnv(t)
	body := similarRequestBody(t, 384)
	req := httptest.NewRequest(http.MethodPost, "/api/v1/ai-track/edits/similar",
		bytes.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	w := do(env.router, req)
	assertStatus(t, w, http.StatusForbidden)
}

// Test 7: Similar 400 — embedding wrong dimension (100 instead of 384)
func TestSimilar_WrongEmbeddingDimension(t *testing.T) {
	env := newTestEnv(t)
	body := similarRequestBody(t, 100)
	req := httptest.NewRequest(http.MethodPost, "/api/v1/ai-track/edits/similar",
		bytes.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("X-Admin-Key", env.cfg.AdminKey)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusBadRequest)

	var resp map[string]string
	decodeJSON(t, w, &resp)
	if resp["error"] == "" {
		t.Error("expected error message for wrong dimension")
	}
}

// Test 8: Similar 501 — SQLite mode
func TestSimilar_SQLiteMode_Returns501(t *testing.T) {
	env := newTestEnv(t)
	sqliteRouter := newSearchTestRouter(t, env.db, env.cfg.AdminKey, false)
	body := similarRequestBody(t, 384)
	req := httptest.NewRequest(http.MethodPost, "/api/v1/ai-track/edits/similar",
		bytes.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("X-Admin-Key", env.cfg.AdminKey)
	w := do(sqliteRouter, req)
	assertStatus(t, w, http.StatusNotImplemented)
}

// Test 9: Similar 400 — malformed JSON
func TestSimilar_MalformedJSON(t *testing.T) {
	env := newTestEnv(t)
	req := httptest.NewRequest(http.MethodPost, "/api/v1/ai-track/edits/similar",
		bytes.NewReader([]byte("not-json")))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("X-Admin-Key", env.cfg.AdminKey)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusBadRequest)
}

// Test 10: Similar — admin key not configured returns 503
func TestSimilar_AdminKeyNotConfigured(t *testing.T) {
	similarH := handler.NewSimilarHandler(nil, "", false)
	mux := http.NewServeMux()
	mux.HandleFunc("/api/v1/ai-track/edits/similar", similarH.Similar)

	body := similarRequestBody(t, 384)
	req := httptest.NewRequest(http.MethodPost, "/api/v1/ai-track/edits/similar",
		bytes.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("X-Admin-Key", "anything")
	w := httptest.NewRecorder()
	mux.ServeHTTP(w, req)
	assertStatus(t, w, http.StatusServiceUnavailable)
}

// Test 11: Similar 400 — zero-length embedding
func TestSimilar_EmptyEmbedding(t *testing.T) {
	env := newTestEnv(t)
	payload := map[string]interface{}{
		"embedding": []float32{},
		"limit":     10,
	}
	body, _ := json.Marshal(payload)
	req := httptest.NewRequest(http.MethodPost, "/api/v1/ai-track/edits/similar",
		bytes.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("X-Admin-Key", env.cfg.AdminKey)
	w := do(env.router, req)
	assertStatus(t, w, http.StatusBadRequest)
}

// Test 12: Search — wrong admin key returns 403
func TestSearch_WrongAdminKey(t *testing.T) {
	env := newTestEnv(t)
	req := httptest.NewRequest(http.MethodGet, "/api/v1/ai-track/edits/search?q=foo", nil)
	req.Header.Set("X-Admin-Key", "wrong-key")
	w := do(env.router, req)
	assertStatus(t, w, http.StatusForbidden)
}

// ─── helpers ────────────────────────────────────────────────────────────────

// similarRequestBody builds a JSON body with an embedding of the given dimension.
func similarRequestBody(t *testing.T, dim int) []byte {
	t.Helper()
	embedding := make([]float32, dim)
	for i := range embedding {
		embedding[i] = float32(i) * 0.001
	}
	payload := map[string]interface{}{
		"embedding": embedding,
		"limit":     10,
	}
	b, err := json.Marshal(payload)
	if err != nil {
		t.Fatalf("marshal similar body: %v", err)
	}
	return b
}
