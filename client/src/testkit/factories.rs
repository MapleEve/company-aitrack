use crate::config::Config;
use crate::db::Record;

// ---------------------------------------------------------------------------
// Seed-based deterministic helpers
// ---------------------------------------------------------------------------

fn seed_str(seed: u64, prefix: &str) -> String {
    format!("{prefix}-{seed:016x}")
}

fn seed_uuid(seed: u64) -> String {
    // Build a deterministic UUID-v4-shaped string from the seed.
    let a = seed;
    let b = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        (a >> 32) as u32,
        (a >> 16) as u16,
        (a & 0x0fff) as u16,
        (b >> 48) as u16 | 0x8000,
        b & 0x0000_ffff_ffff,
    )
}

// ---------------------------------------------------------------------------
// EditRecord factory
// ---------------------------------------------------------------------------

pub struct EditRecordFactory {
    pub seed: u64,
    pub tool: Option<String>,
    pub tool_version: Option<Option<String>>,
    pub provider: Option<String>,
    pub model: Option<Option<String>>,
    pub session_id: Option<String>,
    pub repo_url: Option<String>,
    pub branch: Option<String>,
    pub current_sha: Option<String>,
    pub file_path: Option<String>,
    pub added_lines: Option<i64>,
    pub removed_lines: Option<i64>,
    pub diff_hunk: Option<Option<String>>,
    pub timestamp: Option<String>,
    pub token_key: Option<String>,
    pub device_id: Option<String>,
    pub hostname: Option<String>,
    pub record_sig: Option<String>,
}

impl EditRecordFactory {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            tool: None,
            tool_version: None,
            provider: None,
            model: None,
            session_id: None,
            repo_url: None,
            branch: None,
            current_sha: None,
            file_path: None,
            added_lines: None,
            removed_lines: None,
            diff_hunk: None,
            timestamp: None,
            token_key: None,
            device_id: None,
            hostname: None,
            record_sig: None,
        }
    }

    pub fn with_tool(mut self, v: &str) -> Self { self.tool = Some(v.to_string()); self }
    pub fn with_provider(mut self, v: &str) -> Self { self.provider = Some(v.to_string()); self }
    pub fn with_file_path(mut self, v: &str) -> Self { self.file_path = Some(v.to_string()); self }
    pub fn with_added_lines(mut self, v: i64) -> Self { self.added_lines = Some(v); self }
    pub fn with_removed_lines(mut self, v: i64) -> Self { self.removed_lines = Some(v); self }
    pub fn with_diff_hunk(mut self, v: Option<String>) -> Self { self.diff_hunk = Some(v); self }
    pub fn with_token_key(mut self, v: &str) -> Self { self.token_key = Some(v.to_string()); self }
    pub fn with_device_id(mut self, v: &str) -> Self { self.device_id = Some(v.to_string()); self }
    pub fn with_hostname(mut self, v: &str) -> Self { self.hostname = Some(v.to_string()); self }
    pub fn with_record_sig(mut self, v: &str) -> Self { self.record_sig = Some(v.to_string()); self }
    pub fn with_timestamp(mut self, v: &str) -> Self { self.timestamp = Some(v.to_string()); self }
    pub fn with_repo_url(mut self, v: &str) -> Self { self.repo_url = Some(v.to_string()); self }
    pub fn with_branch(mut self, v: &str) -> Self { self.branch = Some(v.to_string()); self }
    pub fn with_current_sha(mut self, v: &str) -> Self { self.current_sha = Some(v.to_string()); self }
    pub fn with_session_id(mut self, v: &str) -> Self { self.session_id = Some(v.to_string()); self }

    pub fn build(self) -> Record {
        let s = self.seed;
        Record {
            id: 0,
            tool: self.tool.unwrap_or_else(|| "claude".to_string()),
            tool_version: self.tool_version.unwrap_or_else(|| Some("claude-code".to_string())),
            provider: self.provider.unwrap_or_else(|| "anthropic".to_string()),
            model: self.model.unwrap_or(None),
            session_id: self.session_id.unwrap_or_else(|| seed_uuid(s)),
            repo_url: self.repo_url.unwrap_or_else(|| format!("git@github.com:org/repo-{s}.git")),
            branch: self.branch.unwrap_or_else(|| "main".to_string()),
            current_sha: self.current_sha.unwrap_or_else(|| format!("{s:040x}")),
            file_path: self.file_path.unwrap_or_else(|| format!("src/file_{s}.rs")),
            added_lines: self.added_lines.unwrap_or((s % 50 + 1) as i64),
            removed_lines: self.removed_lines.unwrap_or((s % 20) as i64),
            diff_hunk: self.diff_hunk.unwrap_or_else(|| {
                Some(format!("@@ -1,1 +1,2 @@\n-old line\n+new line {s}\n"))
            }),
            metadata: None,
            synced: 0,
            synced_at: None,
            retry_count: 0,
            timestamp: self.timestamp.unwrap_or_else(|| "2026-05-17T10:00:00Z".to_string()),
            token_key: self.token_key.unwrap_or_else(|| seed_str(s, "tok")),
            device_id: self.device_id.unwrap_or_else(|| seed_uuid(s.wrapping_add(1))),
            hostname: self.hostname.unwrap_or_else(|| format!("host-{s:016x}")),
            record_sig: self.record_sig.unwrap_or_else(|| seed_str(s, "sig")),
        }
    }
}

// ---------------------------------------------------------------------------
// ApiConfig factory
// ---------------------------------------------------------------------------

pub struct ApiConfigFactory {
    pub seed: u64,
    pub api_url: Option<String>,
    pub token: Option<String>,
    pub device_id: Option<String>,
    pub hmac_secret: Option<String>,
}

impl ApiConfigFactory {
    pub fn new(seed: u64) -> Self {
        Self { seed, api_url: None, token: None, device_id: None, hmac_secret: None }
    }

    pub fn with_api_url(mut self, v: &str) -> Self { self.api_url = Some(v.to_string()); self }
    pub fn with_token(mut self, v: &str) -> Self { self.token = Some(v.to_string()); self }
    pub fn with_device_id(mut self, v: &str) -> Self { self.device_id = Some(v.to_string()); self }
    pub fn with_hmac_secret(mut self, v: &str) -> Self { self.hmac_secret = Some(v.to_string()); self }

    pub fn build(self) -> Config {
        let s = self.seed;
        Config {
            api_url: self.api_url.unwrap_or_else(|| format!("https://api-{s}.example.com")),
            token: self.token.unwrap_or_else(|| format!("aitrack_{s:016x}abcdef")),
            device_id: self.device_id.unwrap_or_else(|| seed_uuid(s)),
            hmac_secret: self.hmac_secret.unwrap_or_else(|| format!("secret-{s:016x}")),
        }
    }
}

// ---------------------------------------------------------------------------
// HookPayload factories (claude / codex / cursor)
// ---------------------------------------------------------------------------

pub struct ClaudeHookPayloadFactory {
    pub seed: u64,
    pub session_id: Option<String>,
    pub old_string: Option<String>,
    pub new_string: Option<String>,
    pub file_path: Option<String>,
    pub model: Option<String>,
}

impl ClaudeHookPayloadFactory {
    pub fn new(seed: u64) -> Self {
        Self { seed, session_id: None, old_string: None, new_string: None, file_path: None, model: None }
    }

    pub fn with_session_id(mut self, v: &str) -> Self { self.session_id = Some(v.to_string()); self }
    pub fn with_old_string(mut self, v: &str) -> Self { self.old_string = Some(v.to_string()); self }
    pub fn with_new_string(mut self, v: &str) -> Self { self.new_string = Some(v.to_string()); self }
    pub fn with_file_path(mut self, v: &str) -> Self { self.file_path = Some(v.to_string()); self }
    pub fn with_model(mut self, v: &str) -> Self { self.model = Some(v.to_string()); self }

    pub fn build_json(self) -> String {
        let s = self.seed;
        let session_id = self.session_id.unwrap_or_else(|| seed_uuid(s));
        let old = self.old_string.unwrap_or_else(|| format!("old content {s}"));
        let new = self.new_string.unwrap_or_else(|| format!("new content {s}\nextra line"));
        let file = self.file_path.unwrap_or_else(|| format!("src/file_{s}.rs"));
        let mut val = serde_json::json!({
            "session_id": session_id,
            "tool_version": "claude-code",
            "tool_input": {
                "old_string": old,
                "new_string": new,
                "file_paths": [file]
            }
        });
        if let Some(m) = self.model {
            val["model"] = serde_json::Value::String(m);
        }
        val.to_string()
    }
}

pub struct CodexHookPayloadFactory {
    pub seed: u64,
    pub session_id: Option<String>,
    pub old_string: Option<String>,
    pub new_string: Option<String>,
    pub file_path: Option<String>,
    pub tool_name: Option<String>,
}

impl CodexHookPayloadFactory {
    pub fn new(seed: u64) -> Self {
        Self { seed, session_id: None, old_string: None, new_string: None, file_path: None, tool_name: None }
    }

    pub fn with_session_id(mut self, v: &str) -> Self { self.session_id = Some(v.to_string()); self }
    pub fn with_tool_name(mut self, v: &str) -> Self { self.tool_name = Some(v.to_string()); self }
    pub fn with_old_string(mut self, v: &str) -> Self { self.old_string = Some(v.to_string()); self }
    pub fn with_new_string(mut self, v: &str) -> Self { self.new_string = Some(v.to_string()); self }
    pub fn with_file_path(mut self, v: &str) -> Self { self.file_path = Some(v.to_string()); self }

    pub fn build_json(self) -> String {
        let s = self.seed;
        let session_id = self.session_id.unwrap_or_else(|| seed_uuid(s));
        let old = self.old_string.unwrap_or_else(|| format!("old content {s}"));
        let new = self.new_string.unwrap_or_else(|| format!("new content {s}"));
        let file = self.file_path.unwrap_or_else(|| format!("src/file_{s}.rs"));
        let tool = self.tool_name.unwrap_or_else(|| "Edit".to_string());
        serde_json::json!({
            "hook_event_name": "postToolUse",
            "tool_name": tool,
            "conversation_id": session_id,
            "model": "gpt-4o",
            "tool_input": {
                "old_string": old,
                "new_string": new,
                "file_path": file
            }
        })
        .to_string()
    }
}

pub struct CursorHookPayloadFactory {
    pub seed: u64,
    pub session_id: Option<String>,
    pub old_str: Option<String>,
    pub new_str: Option<String>,
    pub file_path: Option<String>,
}

impl CursorHookPayloadFactory {
    pub fn new(seed: u64) -> Self {
        Self { seed, session_id: None, old_str: None, new_str: None, file_path: None }
    }

    pub fn with_session_id(mut self, v: &str) -> Self { self.session_id = Some(v.to_string()); self }
    pub fn with_old_str(mut self, v: &str) -> Self { self.old_str = Some(v.to_string()); self }
    pub fn with_new_str(mut self, v: &str) -> Self { self.new_str = Some(v.to_string()); self }
    pub fn with_file_path(mut self, v: &str) -> Self { self.file_path = Some(v.to_string()); self }

    pub fn build_json(self) -> String {
        let s = self.seed;
        let session_id = self.session_id.unwrap_or_else(|| seed_uuid(s));
        let old = self.old_str.unwrap_or_else(|| format!("old {s}"));
        let new = self.new_str.unwrap_or_else(|| format!("new {s}"));
        let file = self.file_path.unwrap_or_else(|| format!("src/file_{s}.rs"));
        serde_json::json!({
            "session_id": session_id,
            "cursor_version": "0.40.0",
            "tool_input": {
                "file_path": file,
                "old_str": old,
                "new_str": new
            }
        })
        .to_string()
    }
}

// ---------------------------------------------------------------------------
// Tampered / negative factories
// ---------------------------------------------------------------------------

/// Record with an invalid (tampered) record_sig.
pub fn tampered_record_sig(seed: u64) -> Record {
    EditRecordFactory::new(seed)
        .with_record_sig("00000000000000000000000000000000000000000000000000000000000000ff")
        .build()
}

/// Record with a timestamp far in the past (expired).
pub fn tampered_expired_timestamp(seed: u64) -> Record {
    EditRecordFactory::new(seed)
        .with_timestamp("2000-01-01T00:00:00Z")
        .build()
}

/// Record with oversized added_lines (gaming attempt).
pub fn tampered_oversized_lines(seed: u64) -> Record {
    EditRecordFactory::new(seed)
        .with_added_lines(99_999_999)
        .with_removed_lines(99_999_999)
        .build()
}

/// Malformed JSON that parsers must reject.
pub fn malformed_json() -> String {
    r#"{"session_id": "x", "tool_input": { "file_paths": [1, 2, "#.to_string()
}

/// JSON that is valid but missing required fields (triggers adapter None return).
pub fn json_missing_tool_input(seed: u64) -> String {
    serde_json::json!({
        "session_id": seed_uuid(seed),
        "tool_version": "claude-code"
    })
    .to_string()
}

/// Codex payload with wrong hook_event_name (should be filtered out).
pub fn codex_wrong_event(seed: u64) -> String {
    let mut v: serde_json::Value = serde_json::from_str(&CodexHookPayloadFactory::new(seed).build_json()).unwrap();
    v["hook_event_name"] = serde_json::Value::String("preToolUse".to_string());
    v.to_string()
}

/// Codex payload with non-edit tool_name (should be filtered out).
pub fn codex_non_edit_tool(seed: u64) -> String {
    let mut v: serde_json::Value = serde_json::from_str(&CodexHookPayloadFactory::new(seed).build_json()).unwrap();
    v["tool_name"] = serde_json::Value::String("ListFiles".to_string());
    v.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------------------
    // EditRecordFactory: exercise every builder method
    // ---------------------------------------------------------------------------

    #[test]
    fn edit_record_factory_all_builders() {
        let rec = EditRecordFactory::new(1)
            .with_tool("codex")
            .with_provider("openai")
            .with_file_path("src/foo.rs")
            .with_added_lines(10)
            .with_removed_lines(3)
            .with_diff_hunk(Some("@@ -1 +1 @@\n-old\n+new".to_string()))
            .with_token_key("tok-abc")
            .with_device_id("dev-abc")
            .with_hostname("my-test-host")
            .with_record_sig("sig-abc")
            .with_timestamp("2026-01-01T00:00:00Z")
            .with_repo_url("git@github.com:x/y.git")
            .with_branch("feature/xyz")
            .with_current_sha("deadbeef1234")
            .with_session_id("sess-xyz")
            .build();

        assert_eq!(rec.tool, "codex");
        assert_eq!(rec.provider, "openai");
        assert_eq!(rec.file_path, "src/foo.rs");
        assert_eq!(rec.added_lines, 10);
        assert_eq!(rec.removed_lines, 3);
        assert_eq!(rec.diff_hunk.as_deref(), Some("@@ -1 +1 @@\n-old\n+new"));
        assert_eq!(rec.token_key, "tok-abc");
        assert_eq!(rec.device_id, "dev-abc");
        assert_eq!(rec.hostname, "my-test-host");
        assert_eq!(rec.record_sig, "sig-abc");
        assert_eq!(rec.timestamp, "2026-01-01T00:00:00Z");
        assert_eq!(rec.repo_url, "git@github.com:x/y.git");
        assert_eq!(rec.branch, "feature/xyz");
        assert_eq!(rec.current_sha, "deadbeef1234");
        assert_eq!(rec.session_id, "sess-xyz");
    }

    #[test]
    fn edit_record_factory_seed_determinism() {
        let r1 = EditRecordFactory::new(42).build();
        let r2 = EditRecordFactory::new(42).build();
        assert_eq!(r1.tool, r2.tool);
        assert_eq!(r1.file_path, r2.file_path);
        assert_eq!(r1.added_lines, r2.added_lines);
        assert_eq!(r1.token_key, r2.token_key);
        assert_eq!(r1.device_id, r2.device_id);
        assert_eq!(r1.hostname, r2.hostname);
    }

    #[test]
    fn edit_record_factory_different_seeds_differ() {
        let r1 = EditRecordFactory::new(1).build();
        let r2 = EditRecordFactory::new(2).build();
        assert_ne!(r1.file_path, r2.file_path);
        assert_ne!(r1.repo_url, r2.repo_url);
    }

    #[test]
    fn edit_record_factory_no_diff_hunk() {
        let rec = EditRecordFactory::new(5)
            .with_diff_hunk(None)
            .build();
        assert!(rec.diff_hunk.is_none());
    }

    // ---------------------------------------------------------------------------
    // ApiConfigFactory: exercise every builder method
    // ---------------------------------------------------------------------------

    #[test]
    fn api_config_factory_all_builders() {
        let cfg = ApiConfigFactory::new(10)
            .with_api_url("https://custom.example.com")
            .with_token("aitrack_customtoken12345")
            .with_device_id("custom-device-id")
            .with_hmac_secret("custom-hmac-secret")
            .build();

        assert_eq!(cfg.api_url, "https://custom.example.com");
        assert_eq!(cfg.token, "aitrack_customtoken12345");
        assert_eq!(cfg.device_id, "custom-device-id");
        assert_eq!(cfg.hmac_secret, "custom-hmac-secret");
    }

    #[test]
    fn api_config_factory_defaults() {
        let cfg = ApiConfigFactory::new(99).build();
        assert!(cfg.api_url.contains("99"));
        assert!(cfg.token.starts_with("aitrack_"));
        assert!(!cfg.device_id.is_empty());
        assert!(!cfg.hmac_secret.is_empty());
    }

    // ---------------------------------------------------------------------------
    // Tampered factories: verify each produces the expected tampered value
    // ---------------------------------------------------------------------------

    #[test]
    fn tampered_record_sig_has_known_bad_sig() {
        let rec = tampered_record_sig(1);
        assert_eq!(
            rec.record_sig,
            "00000000000000000000000000000000000000000000000000000000000000ff"
        );
    }

    #[test]
    fn tampered_expired_timestamp_has_year_2000() {
        let rec = tampered_expired_timestamp(1);
        assert_eq!(rec.timestamp, "2000-01-01T00:00:00Z");
        // Confirm it differs from the default timestamp
        let normal = EditRecordFactory::new(1).build();
        assert_ne!(rec.timestamp, normal.timestamp);
    }

    #[test]
    fn tampered_oversized_lines_has_inflated_counts() {
        let rec = tampered_oversized_lines(1);
        assert_eq!(rec.added_lines, 99_999_999);
        assert_eq!(rec.removed_lines, 99_999_999);
    }

    #[test]
    fn malformed_json_is_invalid() {
        let json = malformed_json();
        let result = serde_json::from_str::<serde_json::Value>(&json);
        assert!(result.is_err(), "malformed_json() must produce invalid JSON");
    }

    #[test]
    fn json_missing_tool_input_is_valid_but_incomplete() {
        let json = json_missing_tool_input(1);
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(val.get("tool_input").is_none(), "must not have tool_input");
        assert!(val.get("session_id").is_some());
    }

    #[test]
    fn codex_wrong_event_has_pre_tool_use() {
        let json = codex_wrong_event(1);
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(val["hook_event_name"].as_str().unwrap(), "preToolUse");
    }

    #[test]
    fn codex_non_edit_tool_has_list_files() {
        let json = codex_non_edit_tool(1);
        let val: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(val["tool_name"].as_str().unwrap(), "ListFiles");
    }

    // ---------------------------------------------------------------------------
    // Payload factory coverage
    // ---------------------------------------------------------------------------

    #[test]
    fn claude_factory_seed_determinism() {
        let j1 = ClaudeHookPayloadFactory::new(7).build_json();
        let j2 = ClaudeHookPayloadFactory::new(7).build_json();
        assert_eq!(j1, j2);
    }

    #[test]
    fn codex_factory_seed_determinism() {
        let j1 = CodexHookPayloadFactory::new(8).build_json();
        let j2 = CodexHookPayloadFactory::new(8).build_json();
        assert_eq!(j1, j2);
    }

    #[test]
    fn cursor_factory_seed_determinism() {
        let j1 = CursorHookPayloadFactory::new(9).build_json();
        let j2 = CursorHookPayloadFactory::new(9).build_json();
        assert_eq!(j1, j2);
    }

    #[test]
    fn tampered_expired_timestamp_sig_vs_real() {
        // The expired record's sig was computed over the tampered timestamp,
        // so it will differ from a sig computed over a fresh timestamp.
        use crate::crypto::compute_record_sig;
        let rec = tampered_expired_timestamp(5);
        let fresh_rec = EditRecordFactory::new(5).build();
        let sig_expired = compute_record_sig(
            "secret", &rec.token_key, &rec.device_id, &rec.hostname, &rec.timestamp,
            &rec.tool, &rec.file_path, &rec.repo_url, &rec.current_sha,
            rec.added_lines, rec.removed_lines, rec.diff_hunk.as_deref(),
        );
        let sig_fresh = compute_record_sig(
            "secret", &fresh_rec.token_key, &fresh_rec.device_id, &fresh_rec.hostname, &fresh_rec.timestamp,
            &fresh_rec.tool, &fresh_rec.file_path, &fresh_rec.repo_url, &fresh_rec.current_sha,
            fresh_rec.added_lines, fresh_rec.removed_lines, fresh_rec.diff_hunk.as_deref(),
        );
        // Expired and normal differ because timestamp differs
        assert_ne!(sig_expired, sig_fresh, "expired timestamp changes the sig");
    }
}
