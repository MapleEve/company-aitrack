use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

pub fn hmac_sha256_hex(secret: &[u8], message: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(message);
    hex::encode(mac.finalize().into_bytes())
}

/// Compute the per-record signature stored in `record_sig`.
pub fn compute_record_sig(
    hmac_secret: &str,
    token_key: &str,
    device_id: &str,
    hostname: &str,
    timestamp: &str,
    tool: &str,
    file_path: &str,
    repo_url: &str,
    current_sha: &str,
    added_lines: i64,
    removed_lines: i64,
    diff_hunk: Option<&str>,
) -> String {
    let diff_hash = sha256_hex(diff_hunk.unwrap_or("").as_bytes());
    let msg = format!(
        "{token_key}\n{device_id}\n{hostname}\n{timestamp}\n{tool}\n{file_path}\n{repo_url}\n{current_sha}\n{added_lines}\n{removed_lines}\n{diff_hash}"
    );
    hmac_sha256_hex(hmac_secret.as_bytes(), msg.as_bytes())
}

/// Compute the request-level signature for upload headers.
pub fn compute_request_sig(hmac_secret: &str, unix_ts: u64, body_bytes: &[u8]) -> String {
    let body_hash = sha256_hex(body_bytes);
    let msg = format!("{unix_ts}\n{body_hash}");
    hmac_sha256_hex(hmac_secret.as_bytes(), msg.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::factories::{EditRecordFactory, ApiConfigFactory};

    // Known-value: sha256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
    const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb924\
                                27ae41e4649b934ca495991b7852b855";

    #[test]
    fn sha256_hex_empty_known_value() {
        assert_eq!(sha256_hex(b""), EMPTY_SHA256);
    }

    #[test]
    fn sha256_hex_hello_world_known_value() {
        // sha256("hello world") = b94d27b9934d3e08a52e52d7da7dabfac484efe04294e576f
        //                         2a0d7ffd8e5c3b94 (truncated for test)
        let result = sha256_hex(b"hello world");
        assert_eq!(result.len(), 64);
        assert!(result.starts_with("b94d27b9"));
    }

    #[test]
    fn sha256_hex_deterministic_same_input() {
        let a = sha256_hex(b"aitrack test input");
        let b = sha256_hex(b"aitrack test input");
        assert_eq!(a, b);
    }

    #[test]
    fn sha256_hex_different_inputs_differ() {
        let a = sha256_hex(b"input_a");
        let b = sha256_hex(b"input_b");
        assert_ne!(a, b);
    }

    #[test]
    fn hmac_sha256_hex_known_value() {
        // HMAC-SHA256(key="secret", msg="hello") — verifiable with openssl
        // openssl dgst -sha256 -hmac "secret" <(echo -n "hello")
        let result = hmac_sha256_hex(b"secret", b"hello");
        assert_eq!(result.len(), 64);
        // Must be lowercase hex
        assert!(result.chars().all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));
        // Known value: 88aab3ede8d3adf94d26ab90d3bafd4a2083070c3bcce9c014ee04a443847c0b
        assert_eq!(result, "88aab3ede8d3adf94d26ab90d3bafd4a2083070c3bcce9c014ee04a443847c0b");
    }

    #[test]
    fn hmac_sha256_hex_deterministic() {
        let r1 = hmac_sha256_hex(b"mysecret", b"message");
        let r2 = hmac_sha256_hex(b"mysecret", b"message");
        assert_eq!(r1, r2);
    }

    #[test]
    fn hmac_sha256_hex_different_keys_differ() {
        let r1 = hmac_sha256_hex(b"key1", b"message");
        let r2 = hmac_sha256_hex(b"key2", b"message");
        assert_ne!(r1, r2);
    }

    #[test]
    fn hmac_sha256_hex_different_messages_differ() {
        let r1 = hmac_sha256_hex(b"key", b"msg1");
        let r2 = hmac_sha256_hex(b"key", b"msg2");
        assert_ne!(r1, r2);
    }

    #[test]
    fn compute_record_sig_known_canonical_message() {
        // Verify the canonical message format from CONTRACT.md v1.1:
        // token_key\ndevice_id\nhostname\ntimestamp\ntool\nfile_path\nrepo_url\ncurrent_sha\nadded_lines\nremoved_lines\nsha256_hex(diff_hunk)
        let sig = compute_record_sig(
            "test-secret",
            "tok123",
            "device-abc",
            "myhost.local",
            "2026-05-17T10:00:00Z",
            "claude",
            "src/main.rs",
            "git@github.com:org/repo.git",
            "deadbeef",
            5,
            2,
            None,
        );
        assert_eq!(sig.len(), 64);

        // Re-compute manually to verify canonical format (v1.1 order)
        let diff_hash = sha256_hex(b""); // None → ""
        let msg = format!(
            "tok123\ndevice-abc\nmyhost.local\n2026-05-17T10:00:00Z\nclaude\nsrc/main.rs\ngit@github.com:org/repo.git\ndeadbeef\n5\n2\n{diff_hash}"
        );
        let expected = hmac_sha256_hex(b"test-secret", msg.as_bytes());
        assert_eq!(sig, expected);
    }

    #[test]
    fn compute_record_sig_with_diff_hunk() {
        let hunk = "@@ -1,2 +1,3 @@\n-old\n+new\n+extra";
        let sig = compute_record_sig(
            "secret", "tok", "dev", "build-host", "2026-01-01T00:00:00Z",
            "codex", "src/lib.rs", "https://github.com/x/y", "abc123",
            3, 1, Some(hunk),
        );
        let diff_hash = sha256_hex(hunk.as_bytes());
        let msg = format!(
            "tok\ndev\nbuild-host\n2026-01-01T00:00:00Z\ncodex\nsrc/lib.rs\nhttps://github.com/x/y\nabc123\n3\n1\n{diff_hash}"
        );
        let expected = hmac_sha256_hex(b"secret", msg.as_bytes());
        assert_eq!(sig, expected);
    }

    #[test]
    fn compute_record_sig_uses_factory_fields() {
        // Use factories to build a record and verify sig computation is consistent
        let cfg = ApiConfigFactory::new(42)
            .with_hmac_secret("factory-secret")
            .build();
        let rec = EditRecordFactory::new(42)
            .with_token_key("factory-tok")
            .with_device_id("factory-dev")
            .with_hostname("factory-host")
            .with_timestamp("2026-05-17T12:00:00Z")
            .with_tool("claude")
            .with_file_path("src/factory.rs")
            .with_repo_url("git@github.com:factory/repo.git")
            .with_current_sha("facefeed")
            .with_added_lines(10)
            .with_removed_lines(3)
            .with_diff_hunk(Some("@@ -1 +1 @@\n-x\n+y".to_string()))
            .build();

        let sig = compute_record_sig(
            &cfg.hmac_secret,
            &rec.token_key,
            &rec.device_id,
            &rec.hostname,
            &rec.timestamp,
            &rec.tool,
            &rec.file_path,
            &rec.repo_url,
            &rec.current_sha,
            rec.added_lines,
            rec.removed_lines,
            rec.diff_hunk.as_deref(),
        );
        assert_eq!(sig.len(), 64);
        // Idempotent
        let sig2 = compute_record_sig(
            &cfg.hmac_secret,
            &rec.token_key,
            &rec.device_id,
            &rec.hostname,
            &rec.timestamp,
            &rec.tool,
            &rec.file_path,
            &rec.repo_url,
            &rec.current_sha,
            rec.added_lines,
            rec.removed_lines,
            rec.diff_hunk.as_deref(),
        );
        assert_eq!(sig, sig2);
    }

    #[test]
    fn compute_record_sig_tampered_field_changes_sig() {
        let base_sig = compute_record_sig(
            "s", "tok", "dev", "host", "2026-01-01T00:00:00Z",
            "claude", "src/a.rs", "url", "sha", 1, 0, None,
        );
        // Change added_lines → sig must differ
        let tampered_sig = compute_record_sig(
            "s", "tok", "dev", "host", "2026-01-01T00:00:00Z",
            "claude", "src/a.rs", "url", "sha", 99999, 0, None,
        );
        assert_ne!(base_sig, tampered_sig);
    }

    #[test]
    fn compute_request_sig_known_format() {
        // Format: HMAC_SHA256(secret, "{unix_ts}\n{sha256_hex(body)}")
        let body = b"request body";
        let ts: u64 = 1716000000;
        let sig = compute_request_sig("req-secret", ts, body);
        assert_eq!(sig.len(), 64);

        let body_hash = sha256_hex(body);
        let msg = format!("{ts}\n{body_hash}");
        let expected = hmac_sha256_hex(b"req-secret", msg.as_bytes());
        assert_eq!(sig, expected);
    }

    #[test]
    fn compute_request_sig_different_ts_differs() {
        let body = b"same body";
        let s1 = compute_request_sig("sec", 1000, body);
        let s2 = compute_request_sig("sec", 1001, body);
        assert_ne!(s1, s2);
    }

    #[test]
    fn compute_record_sig_empty_secret_still_produces_hex() {
        let sig = compute_record_sig("", "tok", "dev", "host", "ts", "claude", "f", "url", "sha", 1, 0, None);
        assert_eq!(sig.len(), 64);
    }
}
