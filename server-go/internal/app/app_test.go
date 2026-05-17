package app_test

import (
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/aitrack/server/internal/app"
	"github.com/aitrack/server/internal/config"
)

func TestBuild_OK(t *testing.T) {
	cfg := &config.Config{}
	cfg.Server.Port = 8080
	cfg.DB.Path = ":memory:"
	cfg.TimestampWindowSeconds = 300
	cfg.RateLimitPerHour = 30
	cfg.MaxAddedLines = 5000
	cfg.AdminKey = "admin"

	handler, cleanup, err := app.Build(cfg)
	if err != nil {
		t.Fatalf("Build failed: %v", err)
	}
	defer cleanup()

	if handler == nil {
		t.Error("handler should not be nil")
	}

	// Smoke test: hit a route
	req := httptest.NewRequest(http.MethodGet, "/api/v1/ai-track/stats", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)
	// No auth → 401
	if w.Code != http.StatusUnauthorized {
		t.Errorf("expected 401 without auth, got %d", w.Code)
	}
}

func TestBuild_BadEncryptorKey(t *testing.T) {
	cfg := &config.Config{}
	cfg.DB.Path = ":memory:"
	cfg.SecretKey = "not-valid-base64!!!" // will fail

	_, _, err := app.Build(cfg)
	if err == nil {
		t.Error("expected error for bad secret_key")
	}
}

func TestBuild_BadDBPath(t *testing.T) {
	cfg := &config.Config{}
	// A path where MkdirAll will fail (parent is a file, not a dir)
	// Use /dev/null/sub which can't be created on macOS/Linux
	cfg.DB.Path = "/dev/null/sub/aitrack.db"

	_, _, err := app.Build(cfg)
	if err == nil {
		t.Error("expected error for bad db path")
	}
}
