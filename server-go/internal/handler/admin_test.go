package handler_test

import (
	"bytes"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/aitrack/server/internal/model"
)

func TestAdmin_CreateToken_OK(t *testing.T) {
	env := newTestEnv(t)
	req := env.adminRequest(http.MethodPost, "/admin/tokens",
		model.CreateTokenRequest{Owner: "alice", Note: "test token"})

	w := do(env.router, req)
	assertStatus(t, w, http.StatusOK)

	var resp model.CreateTokenResponse
	decodeJSON(t, w, &resp)
	if resp.Token == "" {
		t.Error("token should not be empty")
	}
	if resp.HmacSecret == "" {
		t.Error("hmac_secret should not be empty")
	}
	if resp.TokenKey == "" {
		t.Error("token_key should not be empty")
	}
}

func TestAdmin_CreateToken_MissingAdminKey(t *testing.T) {
	env := newTestEnv(t)
	req := env.adminRequest(http.MethodPost, "/admin/tokens", model.CreateTokenRequest{Owner: "alice"})
	req.Header.Del("X-Admin-Key")

	w := do(env.router, req)
	assertStatus(t, w, http.StatusForbidden)
}

func TestAdmin_CreateToken_WrongAdminKey(t *testing.T) {
	env := newTestEnv(t)
	req := env.adminRequest(http.MethodPost, "/admin/tokens", model.CreateTokenRequest{Owner: "alice"})
	req.Header.Set("X-Admin-Key", "wrong-key")

	w := do(env.router, req)
	assertStatus(t, w, http.StatusForbidden)
}

func TestAdmin_CreateToken_NoAdminKeyConfigured(t *testing.T) {
	env := newTestEnv(t)
	env.cfg.AdminKey = ""
	req := env.adminRequest(http.MethodPost, "/admin/tokens", model.CreateTokenRequest{Owner: "alice"})

	w := do(env.router, req)
	assertStatus(t, w, http.StatusServiceUnavailable)
}

func TestAdmin_CreateToken_MissingOwner(t *testing.T) {
	env := newTestEnv(t)
	req := env.adminRequest(http.MethodPost, "/admin/tokens", model.CreateTokenRequest{Owner: ""})

	w := do(env.router, req)
	assertStatus(t, w, http.StatusBadRequest)
}

func TestAdmin_CreateToken_InvalidJSON(t *testing.T) {
	env := newTestEnv(t)
	req := httptest.NewRequest(http.MethodPost, "/admin/tokens", bytes.NewReader([]byte("not json")))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("X-Admin-Key", env.cfg.AdminKey)

	w := do(env.router, req)
	assertStatus(t, w, http.StatusBadRequest)
}
