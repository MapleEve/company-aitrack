package config_test

import (
	"os"
	"testing"

	"github.com/aitrack/server/internal/config"
)

func TestDefaults(t *testing.T) {
	cfg, err := config.Load("")
	if err != nil {
		t.Fatal(err)
	}
	if cfg.Server.Port != 8080 {
		t.Errorf("default port = %d, want 8080", cfg.Server.Port)
	}
	if cfg.TimestampWindowSeconds != 300 {
		t.Errorf("timestamp_window = %d, want 300", cfg.TimestampWindowSeconds)
	}
	if cfg.RateLimitPerHour != 30 {
		t.Errorf("rate_limit = %d, want 30", cfg.RateLimitPerHour)
	}
	if cfg.MaxAddedLines != 5000 {
		t.Errorf("max_added_lines = %d, want 5000", cfg.MaxAddedLines)
	}
	if cfg.RepoWhitelist.Enforce {
		t.Error("enforce should default to false")
	}
}

func TestEnvOverride(t *testing.T) {
	t.Setenv("AITRACK_ADMIN_KEY", "mykey")
	t.Setenv("AITRACK_SECRET_KEY", "mysecret")
	t.Setenv("AITRACK_PORT", "9090")
	t.Setenv("AITRACK_RATE_LIMIT_PER_HOUR", "100")
	t.Setenv("AITRACK_MAX_ADDED_LINES", "9999")
	t.Setenv("AITRACK_TIMESTAMP_WINDOW", "60")
	t.Setenv("AITRACK_REPO_WHITELIST_ENFORCE", "true")
	t.Setenv("AITRACK_REPO_WHITELIST_URLS", "git@github.com:a/b.git,git@github.com:c/d.git")

	cfg, err := config.Load("")
	if err != nil {
		t.Fatal(err)
	}
	if cfg.AdminKey != "mykey" {
		t.Errorf("admin_key = %q, want mykey", cfg.AdminKey)
	}
	if cfg.SecretKey != "mysecret" {
		t.Errorf("secret_key = %q, want mysecret", cfg.SecretKey)
	}
	if cfg.Server.Port != 9090 {
		t.Errorf("port = %d, want 9090", cfg.Server.Port)
	}
	if cfg.RateLimitPerHour != 100 {
		t.Errorf("rate_limit = %d, want 100", cfg.RateLimitPerHour)
	}
	if cfg.MaxAddedLines != 9999 {
		t.Errorf("max_added_lines = %d, want 9999", cfg.MaxAddedLines)
	}
	if cfg.TimestampWindowSeconds != 60 {
		t.Errorf("timestamp_window = %d, want 60", cfg.TimestampWindowSeconds)
	}
	if !cfg.RepoWhitelist.Enforce {
		t.Error("enforce should be true")
	}
	if len(cfg.RepoWhitelist.URLs) != 2 {
		t.Errorf("whitelist urls = %v, want 2", cfg.RepoWhitelist.URLs)
	}
}

func TestEnvEnforceOne(t *testing.T) {
	t.Setenv("AITRACK_REPO_WHITELIST_ENFORCE", "1")
	cfg, _ := config.Load("")
	if !cfg.RepoWhitelist.Enforce {
		t.Error("enforce '1' should be true")
	}
}

func TestYAMLLoad(t *testing.T) {
	f, err := os.CreateTemp("", "config-*.yaml")
	if err != nil {
		t.Fatal(err)
	}
	defer os.Remove(f.Name())
	f.WriteString(`
server:
  port: 7777
rate_limit_per_hour: 50
max_added_lines: 1000
repo_whitelist:
  enforce: true
  urls:
    - "git@github.com:myorg/repo.git"
`)
	f.Close()

	cfg, err := config.Load(f.Name())
	if err != nil {
		t.Fatal(err)
	}
	if cfg.Server.Port != 7777 {
		t.Errorf("yaml port = %d, want 7777", cfg.Server.Port)
	}
	if cfg.RateLimitPerHour != 50 {
		t.Errorf("yaml rate_limit = %d, want 50", cfg.RateLimitPerHour)
	}
	if cfg.MaxAddedLines != 1000 {
		t.Errorf("yaml max_added_lines = %d, want 1000", cfg.MaxAddedLines)
	}
	if !cfg.RepoWhitelist.Enforce {
		t.Error("yaml enforce should be true")
	}
	if len(cfg.RepoWhitelist.URLs) != 1 {
		t.Errorf("yaml urls = %v, want 1", cfg.RepoWhitelist.URLs)
	}
}

func TestMissingYAML_UsesDefaults(t *testing.T) {
	cfg, err := config.Load("/nonexistent/path/config.yaml")
	if err != nil {
		t.Fatal(err)
	}
	if cfg.Server.Port != 8080 {
		t.Errorf("missing yaml should fall back to default port, got %d", cfg.Server.Port)
	}
}

func TestBadYAML_ReturnsError(t *testing.T) {
	f, err := os.CreateTemp("", "config-bad-*.yaml")
	if err != nil {
		t.Fatal(err)
	}
	defer os.Remove(f.Name())
	// KnownFields(true) causes the decoder to reject unknown field names.
	f.WriteString("this_field_does_not_exist_in_config: true\n")
	f.Close()

	_, loadErr := config.Load(f.Name())
	if loadErr == nil {
		t.Error("expected error for YAML with unknown fields (KnownFields strict mode)")
	}
}
