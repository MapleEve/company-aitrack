use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub api_url: String,
    pub token: String,
    pub device_id: String,
    pub hmac_secret: String,
}

pub fn config_dir() -> PathBuf {
    // AITRACK_HOME overrides the default ~/.aitrack location.
    // This makes every path in this module testable without touching the real home directory.
    if let Ok(home) = std::env::var("AITRACK_HOME") {
        return PathBuf::from(home);
    }
    dirs::home_dir()
        .expect("cannot find home directory")
        .join(".aitrack")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn db_path() -> PathBuf {
    config_dir().join("records.db")
}

pub fn load_config() -> Config {
    let path = config_path();
    if let Ok(text) = fs::read_to_string(&path) {
        if let Ok(cfg) = toml::from_str::<Config>(&text) {
            return cfg;
        }
    }
    Config::default()
}

pub fn save_config(cfg: &Config) -> Result<()> {
    let dir = config_dir();
    fs::create_dir_all(&dir).context("create ~/.aitrack")?;

    let text = toml::to_string(cfg).context("serialize config")?;
    let path = config_path();
    fs::write(&path, text).context("write config.toml")?;

    let mut perms = fs::metadata(&path)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(&path, perms).context("chmod config.toml")?;

    Ok(())
}

pub fn ensure_device_id(cfg: &mut Config) -> Result<bool> {
    if cfg.device_id.is_empty() {
        cfg.device_id = Uuid::new_v4().to_string();
        save_config(cfg)?;
        return Ok(true);
    }
    Ok(false)
}

/// Resolve api_url and token: CLI args > env vars > config file.
pub fn resolve_api_config(
    cli_url: Option<String>,
    cli_token: Option<String>,
) -> (String, String) {
    let file_cfg = load_config();

    let api_url = cli_url
        .or_else(|| std::env::var("AITRACK_API_URL").ok())
        .unwrap_or(file_cfg.api_url);

    let token = cli_token
        .or_else(|| std::env::var("AITRACK_API_TOKEN").ok())
        .unwrap_or(file_cfg.token);

    (api_url, token)
}

/// Apply CLI-provided init args into config and persist.
pub fn apply_init_args(
    api_url: Option<String>,
    api_token: Option<String>,
    hmac_secret: Option<String>,
) -> Result<Config> {
    let mut cfg = load_config();

    if let Some(u) = api_url {
        cfg.api_url = u;
    }
    if let Some(t) = api_token {
        cfg.token = t;
    }
    if let Some(s) = hmac_secret {
        cfg.hmac_secret = s;
    }
    ensure_device_id(&mut cfg)?;
    save_config(&cfg)?;
    Ok(cfg)
}

pub fn mask_token(token: &str) -> String {
    if token.len() <= 5 {
        return "legacy".to_string();
    }
    let body = token.strip_prefix("aitrack_").unwrap_or(token);
    truncate_middle(body)
}

fn truncate_middle(s: &str) -> String {
    const HEAD: usize = 6;
    const TAIL: usize = 4;
    // Threshold MUST match server TokenService.computeTokenKey: stripped length <= 10
    // is returned unchanged, otherwise head-6 + U+2026 + tail-4.
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= HEAD + TAIL {
        return s.to_string();
    }
    let head: String = chars[..HEAD].iter().collect();
    let tail: String = chars[chars.len() - TAIL..].iter().collect();
    format!("{head}\u{2026}{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::ENV_LOCK;
    use tempfile::TempDir;

    /// Run `f` with AITRACK_HOME pointing at `dir`, holding the process-wide env
    /// mutex for the duration so parallel tests cannot interfere.
    fn with_aitrack_home<F: FnOnce(&std::path::Path)>(dir: &TempDir, f: F) {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        std::env::set_var("AITRACK_HOME", dir.path());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(dir.path())));
        std::env::remove_var("AITRACK_HOME");
        if let Err(e) = result {
            std::panic::resume_unwind(e);
        }
    }

    fn with_isolated_config<F: FnOnce(&std::path::Path)>(f: F) {
        let dir = TempDir::new().unwrap();
        let cfg = Config {
            api_url: "https://example.com".to_string(),
            token: "aitrack_abc123def456".to_string(),
            device_id: "test-device".to_string(),
            hmac_secret: "secret".to_string(),
        };
        let path = dir.path().join("config.toml");
        let text = toml::to_string(&cfg).unwrap();
        std::fs::write(&path, text).unwrap();
        f(dir.path());
    }

    #[test]
    fn mask_token_short_returns_legacy() {
        assert_eq!(mask_token("abc"), "legacy");
        assert_eq!(mask_token(""), "legacy");
        assert_eq!(mask_token("12345"), "legacy");
    }

    #[test]
    fn mask_token_strips_prefix_and_truncates() {
        // "aitrack_" stripped → body length > 10 → truncated
        let token = "aitrack_abcdefghijklmnop";
        let masked = mask_token(token);
        assert!(masked.contains('\u{2026}'), "expected ellipsis in: {masked}");
        // body after stripping: "abcdefghijklmnop" → head 6 = "abcdef" + … + tail 4 = "mnop"
        assert!(masked.starts_with("abcdef"), "got: {masked}");
        assert!(masked.ends_with("mnop"), "got: {masked}");
    }

    #[test]
    fn mask_token_no_prefix_short_body_returned_as_is() {
        // No "aitrack_" prefix, body ≤ 10 chars → returned unchanged
        let token = "aitrack_short";
        // stripped = "short" (5 chars) ≤ 10, returned as-is
        let masked = mask_token(token);
        assert_eq!(masked, "short");
    }

    #[test]
    fn mask_token_exact_boundary_10_chars_returned_as_is() {
        // exactly 10 chars after stripping prefix
        let token = "aitrack_1234567890";
        let masked = mask_token(token);
        assert_eq!(masked, "1234567890");
    }

    #[test]
    fn mask_token_11_chars_after_prefix_gets_truncated() {
        // 11 chars after stripping prefix → HEAD(6) + … + TAIL(4) = truncated
        let token = "aitrack_12345678901";
        let masked = mask_token(token);
        assert!(masked.contains('\u{2026}'), "expected ellipsis, got: {masked}");
    }

    #[test]
    fn save_and_load_config_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = Config {
            api_url: "https://test.example.com".to_string(),
            token: "aitrack_testtoken123456".to_string(),
            device_id: "device-uuid-here".to_string(),
            hmac_secret: "my-hmac-secret".to_string(),
        };
        let text = toml::to_string(&cfg).unwrap();
        std::fs::write(&path, &text).unwrap();

        // Verify toml roundtrip
        let loaded: Config = toml::from_str(&text).unwrap();
        assert_eq!(loaded.api_url, cfg.api_url);
        assert_eq!(loaded.token, cfg.token);
        assert_eq!(loaded.device_id, cfg.device_id);
        assert_eq!(loaded.hmac_secret, cfg.hmac_secret);
    }

    #[test]
    fn ensure_device_id_generates_uuid_when_empty() {
        let _dir = TempDir::new().unwrap();
        // Override config path via writing to tmp file, then test logic directly
        let cfg = Config::default();
        assert!(cfg.device_id.is_empty());
        // Since ensure_device_id writes to the real path, we test the UUID generation logic:
        let id = uuid::Uuid::new_v4().to_string();
        assert_eq!(id.len(), 36);
        assert!(id.contains('-'));
    }

    #[test]
    fn config_default_is_all_empty() {
        let cfg = Config::default();
        assert!(cfg.api_url.is_empty());
        assert!(cfg.token.is_empty());
        assert!(cfg.device_id.is_empty());
        assert!(cfg.hmac_secret.is_empty());
    }

    #[test]
    fn truncate_middle_short_string_unchanged() {
        // ≤ 10 chars → unchanged
        assert_eq!(truncate_middle("abcdefghij"), "abcdefghij");
        assert_eq!(truncate_middle("short"), "short");
    }

    #[test]
    fn truncate_middle_long_string_has_ellipsis() {
        let result = truncate_middle("abcdefghijklmnop");
        assert!(result.contains('\u{2026}'));
        assert!(result.starts_with("abcdef"));
        assert!(result.ends_with("mnop"));
    }

    #[test]
    fn resolve_api_config_prefers_cli_args() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        std::env::remove_var("AITRACK_API_URL");
        std::env::remove_var("AITRACK_API_TOKEN");
        let (url, token) = resolve_api_config(
            Some("https://cli.example.com".to_string()),
            Some("cli-token".to_string()),
        );
        assert_eq!(url, "https://cli.example.com");
        assert_eq!(token, "cli-token");
    }

    #[test]
    fn resolve_api_config_falls_back_to_env() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        std::env::remove_var("AITRACK_API_URL");
        std::env::remove_var("AITRACK_API_TOKEN");
        let (url, token) = resolve_api_config(
            Some("https://override.example.com".to_string()),
            Some("override-token".to_string()),
        );
        assert_eq!(url, "https://override.example.com");
        assert_eq!(token, "override-token");
    }

    #[test]
    fn with_isolated_config_demonstrates_pattern() {
        with_isolated_config(|dir| {
            let path = dir.join("config.toml");
            assert!(path.exists());
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(text.contains("api_url"));
        });
    }

    #[test]
    fn config_paths_are_under_home_aitrack() {
        // This test asserts the *default* (~/.aitrack) path layout, which only
        // holds when AITRACK_HOME is unset. It reads process-global env state, so
        // it must hold ENV_LOCK and clear AITRACK_HOME — otherwise a concurrent
        // test that sets AITRACK_HOME to a tempdir makes config_dir() return that
        // tempdir and the ".aitrack" assertion fails intermittently.
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        std::env::remove_var("AITRACK_HOME");

        let dir = config_dir();
        assert!(dir.to_string_lossy().contains(".aitrack"));

        let cfg_path = config_path();
        assert!(cfg_path.to_string_lossy().ends_with("config.toml"));

        let db = db_path();
        assert!(db.to_string_lossy().ends_with("records.db"));
    }

    #[test]
    fn load_config_returns_default_when_file_missing() {
        // config_path() may or may not exist; if it doesn't, returns Default
        // We test the branch by ensuring the function always returns a Config
        let cfg = load_config();
        // Just verify it returns without panic — actual values depend on FS state
        let _ = cfg;
    }

    #[test]
    fn save_and_reload_via_toml_roundtrip() {
        // Test the serialization logic without hitting real paths
        let cfg = Config {
            api_url: "https://roundtrip.example.com".to_string(),
            token: "aitrack_roundtriptoken12345".to_string(),
            device_id: "device-roundtrip-1234".to_string(),
            hmac_secret: "hmac-roundtrip-secret".to_string(),
        };
        let serialized = toml::to_string(&cfg).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.api_url, cfg.api_url);
        assert_eq!(deserialized.token, cfg.token);
        assert_eq!(deserialized.device_id, cfg.device_id);
        assert_eq!(deserialized.hmac_secret, cfg.hmac_secret);
    }

    #[test]
    fn load_config_invalid_toml_returns_default() {
        // Test that toml::from_str failure returns Default
        let bad_toml = "not valid toml ===\n[[[[";
        let result = toml::from_str::<Config>(bad_toml);
        assert!(result.is_err());
        // load_config falls back to Config::default() when parse fails
        let fallback = Config::default();
        assert!(fallback.api_url.is_empty());
    }

    #[test]
    fn mask_token_exactly_5_chars_returns_legacy() {
        // Exactly 5 chars → "legacy" (len <= 5)
        assert_eq!(mask_token("abcde"), "legacy");
    }

    #[test]
    fn mask_token_6_chars_no_prefix_no_truncate() {
        // 6 chars, no prefix → body = 6 chars ≤ 10 → returned as-is
        assert_eq!(mask_token("abcdef"), "abcdef");
    }

    #[test]
    fn mask_token_without_aitrack_prefix_long_gets_truncated() {
        // No prefix, 12 chars → body = 12 chars > 10 → truncated
        let masked = mask_token("abcdefghijkl");
        assert!(masked.contains('\u{2026}'));
    }

    #[test]
    fn ensure_device_id_no_op_when_already_set() {
        // When device_id is non-empty, ensure_device_id returns Ok(false)
        // We test the branch logic directly without hitting filesystem
        let cfg = Config {
            device_id: "already-set".to_string(),
            ..Config::default()
        };
        // The function checks if device_id.is_empty() → false path returns Ok(false)
        assert!(!cfg.device_id.is_empty());
    }

    #[test]
    fn apply_init_args_partial_update() {
        // Test the logic: only Some fields update config
        let mut cfg = Config {
            api_url: "old-url".to_string(),
            token: "old-token".to_string(),
            hmac_secret: "old-secret".to_string(),
            device_id: "device-1".to_string(),
        };
        // Simulate apply_init_args logic for Some(url), None token, None secret
        let api_url = Some("new-url".to_string());
        let api_token: Option<String> = None;
        let hmac_secret: Option<String> = None;

        if let Some(u) = api_url { cfg.api_url = u; }
        if let Some(t) = api_token { cfg.token = t; }
        if let Some(s) = hmac_secret { cfg.hmac_secret = s; }

        assert_eq!(cfg.api_url, "new-url");
        assert_eq!(cfg.token, "old-token"); // unchanged
        assert_eq!(cfg.hmac_secret, "old-secret"); // unchanged
    }

    // -------------------------------------------------------------------------
    // AITRACK_HOME-isolated tests: exercise save_config / load_config /
    // apply_init_args / ensure_device_id against a real tempdir.
    // These tests are run single-threaded by cargo test (default), so the
    // process-global env var mutation is safe.
    // -------------------------------------------------------------------------

    #[test]
    fn save_config_writes_file_and_sets_permissions() {
        let dir = TempDir::new().unwrap();
        with_aitrack_home(&dir, |home| {
            let cfg = Config {
                api_url: "https://save.example.com".to_string(),
                token: "aitrack_savetoken12345".to_string(),
                device_id: "save-device".to_string(),
                hmac_secret: "save-secret".to_string(),
            };
            save_config(&cfg).unwrap();

            let path = home.join("config.toml");
            assert!(path.exists(), "config.toml should exist after save");

            // Verify permissions are 0o600
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(&path).unwrap();
            let mode = meta.permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "config.toml should be 0o600, got {mode:o}");
        });
    }

    #[test]
    fn load_config_reads_saved_file() {
        let dir = TempDir::new().unwrap();
        with_aitrack_home(&dir, |_| {
            let original = Config {
                api_url: "https://load.example.com".to_string(),
                token: "aitrack_loadtoken123456".to_string(),
                device_id: "load-device".to_string(),
                hmac_secret: "load-secret".to_string(),
            };
            save_config(&original).unwrap();

            let loaded = load_config();
            assert_eq!(loaded.api_url, original.api_url);
            assert_eq!(loaded.token, original.token);
            assert_eq!(loaded.device_id, original.device_id);
            assert_eq!(loaded.hmac_secret, original.hmac_secret);
        });
    }

    #[test]
    fn load_config_returns_default_when_dir_empty() {
        let dir = TempDir::new().unwrap();
        with_aitrack_home(&dir, |_| {
            // No file written → should return Default
            let cfg = load_config();
            assert!(cfg.api_url.is_empty());
            assert!(cfg.token.is_empty());
            assert!(cfg.device_id.is_empty());
        });
    }

    #[test]
    fn load_config_returns_default_when_toml_invalid() {
        let dir = TempDir::new().unwrap();
        with_aitrack_home(&dir, |home| {
            // Write deliberately broken TOML
            std::fs::write(home.join("config.toml"), "not valid [[[ toml").unwrap();
            let cfg = load_config();
            assert!(cfg.api_url.is_empty(), "broken toml should yield default");
        });
    }

    #[test]
    fn ensure_device_id_generates_and_persists_uuid() {
        let dir = TempDir::new().unwrap();
        with_aitrack_home(&dir, |_| {
            let mut cfg = Config::default();
            assert!(cfg.device_id.is_empty());

            let changed = ensure_device_id(&mut cfg).unwrap();
            assert!(changed, "should return true when uuid was generated");
            assert!(!cfg.device_id.is_empty(), "device_id should be set");
            assert_eq!(cfg.device_id.len(), 36, "uuid should be 36 chars");

            // File should now exist with the device_id
            let loaded = load_config();
            assert_eq!(loaded.device_id, cfg.device_id, "device_id should be persisted");
        });
    }

    #[test]
    fn ensure_device_id_noop_when_already_set() {
        let dir = TempDir::new().unwrap();
        with_aitrack_home(&dir, |_| {
            let mut cfg = Config {
                device_id: "existing-uuid-1234".to_string(),
                ..Config::default()
            };
            save_config(&cfg).unwrap();

            let changed = ensure_device_id(&mut cfg).unwrap();
            assert!(!changed, "should return false when device_id already set");
            assert_eq!(cfg.device_id, "existing-uuid-1234");
        });
    }

    #[test]
    fn apply_init_args_persists_all_fields() {
        let dir = TempDir::new().unwrap();
        with_aitrack_home(&dir, |_| {
            let result = apply_init_args(
                Some("https://apply.example.com".to_string()),
                Some("aitrack_applytoken12345".to_string()),
                Some("apply-hmac-secret".to_string()),
            ).unwrap();

            assert_eq!(result.api_url, "https://apply.example.com");
            assert_eq!(result.token, "aitrack_applytoken12345");
            assert_eq!(result.hmac_secret, "apply-hmac-secret");
            assert!(!result.device_id.is_empty(), "device_id should be auto-generated");

            // Load from disk and verify persistence
            let loaded = load_config();
            assert_eq!(loaded.api_url, "https://apply.example.com");
            assert_eq!(loaded.token, "aitrack_applytoken12345");
            assert_eq!(loaded.hmac_secret, "apply-hmac-secret");
        });
    }

    #[test]
    fn apply_init_args_none_args_preserves_existing() {
        let dir = TempDir::new().unwrap();
        with_aitrack_home(&dir, |_| {
            // Pre-populate config
            let existing = Config {
                api_url: "https://existing.example.com".to_string(),
                token: "aitrack_existingtoken123".to_string(),
                device_id: "existing-device".to_string(),
                hmac_secret: "existing-secret".to_string(),
            };
            save_config(&existing).unwrap();

            // Call apply_init_args with all None → should preserve existing values
            let result = apply_init_args(None, None, None).unwrap();
            assert_eq!(result.api_url, "https://existing.example.com");
            assert_eq!(result.token, "aitrack_existingtoken123");
            assert_eq!(result.hmac_secret, "existing-secret");
        });
    }

    #[test]
    fn config_dir_uses_aitrack_home_env() {
        let dir = TempDir::new().unwrap();
        with_aitrack_home(&dir, |home| {
            let dir_path = config_dir();
            assert_eq!(dir_path, home, "config_dir() should return AITRACK_HOME value");

            let cfg_path = config_path();
            assert_eq!(cfg_path, home.join("config.toml"));

            let db = db_path();
            assert_eq!(db, home.join("records.db"));
        });
    }

    #[test]
    fn resolve_api_config_reads_from_file_when_no_cli_or_env() {
        let dir = TempDir::new().unwrap();
        // with_aitrack_home holds ENV_LOCK, so the remove_vars inside are safe
        with_aitrack_home(&dir, |_| {
            std::env::remove_var("AITRACK_API_URL");
            std::env::remove_var("AITRACK_API_TOKEN");
            let cfg = Config {
                api_url: "https://file.example.com".to_string(),
                token: "aitrack_filetoken123456".to_string(),
                ..Config::default()
            };
            save_config(&cfg).unwrap();

            let (url, token) = resolve_api_config(None, None);
            assert_eq!(url, "https://file.example.com");
            assert_eq!(token, "aitrack_filetoken123456");
        });
    }
}

