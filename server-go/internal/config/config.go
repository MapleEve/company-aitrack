package config

import (
	"os"
	"strconv"
	"strings"

	"gopkg.in/yaml.v3"
)

type Config struct {
	Server struct {
		Port int `yaml:"port"`
	} `yaml:"server"`

	DB struct {
		Path string `yaml:"path"`
	} `yaml:"db"`

	SecretKey string `yaml:"secret_key"`
	AdminKey  string `yaml:"admin_key"`

	TimestampWindowSeconds int64 `yaml:"timestamp_window_seconds"`
	RateLimitPerHour       int64 `yaml:"rate_limit_per_hour"`
	MaxAddedLines          int64 `yaml:"max_added_lines"`

	RepoWhitelist struct {
		Enforce bool     `yaml:"enforce"`
		URLs    []string `yaml:"urls"`
	} `yaml:"repo_whitelist"`
}

func Load(path string) (*Config, error) {
	cfg := defaults()

	if path != "" {
		f, err := os.Open(path)
		if err == nil {
			defer f.Close()
			dec := yaml.NewDecoder(f)
			dec.KnownFields(true)
			if err := dec.Decode(cfg); err != nil {
				return nil, err
			}
		}
	}

	applyEnv(cfg)
	return cfg, nil
}

func defaults() *Config {
	cfg := &Config{}
	cfg.Server.Port = 8080
	cfg.DB.Path = "./data/aitrack.db"
	cfg.TimestampWindowSeconds = 300
	cfg.RateLimitPerHour = 30
	cfg.MaxAddedLines = 5000
	cfg.RepoWhitelist.Enforce = false
	return cfg
}

func applyEnv(cfg *Config) {
	if v := os.Getenv("AITRACK_SECRET_KEY"); v != "" {
		cfg.SecretKey = v
	}
	if v := os.Getenv("AITRACK_ADMIN_KEY"); v != "" {
		cfg.AdminKey = v
	}
	if v := os.Getenv("AITRACK_PORT"); v != "" {
		if p, err := strconv.Atoi(v); err == nil {
			cfg.Server.Port = p
		}
	}
	if v := os.Getenv("AITRACK_DB_PATH"); v != "" {
		cfg.DB.Path = v
	}
	if v := os.Getenv("AITRACK_TIMESTAMP_WINDOW"); v != "" {
		if n, err := strconv.ParseInt(v, 10, 64); err == nil {
			cfg.TimestampWindowSeconds = n
		}
	}
	if v := os.Getenv("AITRACK_RATE_LIMIT_PER_HOUR"); v != "" {
		if n, err := strconv.ParseInt(v, 10, 64); err == nil {
			cfg.RateLimitPerHour = n
		}
	}
	if v := os.Getenv("AITRACK_MAX_ADDED_LINES"); v != "" {
		if n, err := strconv.ParseInt(v, 10, 64); err == nil {
			cfg.MaxAddedLines = n
		}
	}
	if v := os.Getenv("AITRACK_REPO_WHITELIST_ENFORCE"); v != "" {
		cfg.RepoWhitelist.Enforce = strings.EqualFold(v, "true") || v == "1"
	}
	if v := os.Getenv("AITRACK_REPO_WHITELIST_URLS"); v != "" {
		cfg.RepoWhitelist.URLs = strings.Split(v, ",")
	}
}
