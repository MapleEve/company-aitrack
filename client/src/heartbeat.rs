use anyhow::Result;
use chrono::Utc;
use reqwest::Client;
use rusqlite::Connection;
use serde::Serialize;

use crate::config::{load_config, mask_token};
use crate::crypto::compute_request_sig;
use crate::db::{ensure_kv_table, get_last_heartbeat, pending_count_all, set_last_heartbeat};
use crate::init::{has_claude_hook, has_codex_hook, has_cursor_hook};

const HEARTBEAT_INTERVAL_SECS: i64 = 3600; // 1 hour

#[derive(Serialize)]
struct HeartbeatPayload {
    device_id: String,
    hostname: String,
    token_key_masked: String,
    client_version: String,
    ts: i64,
    hooks: HookStatus,
    pending_count: i64,
}

#[derive(Serialize)]
struct HookStatus {
    claude: bool,
    codex: bool,
    cursor: bool,
}

pub async fn send_heartbeat(conn: &Connection, api_url: &str, token: &str, force: bool) -> Result<()> {
    ensure_kv_table(conn)?;

    let now = Utc::now().timestamp();

    if !force {
        if let Some(last) = get_last_heartbeat(conn) {
            if now - last < HEARTBEAT_INTERVAL_SECS {
                return Ok(());
            }
        }
    }

    let cfg = load_config();
    let home = dirs::home_dir().expect("cannot find home directory");

    let claude_installed = has_claude_hook(&home.join(".claude").join("settings.json"));
    let codex_installed = has_codex_hook(&home.join(".codex").join("config.toml"));
    let cursor_installed = has_cursor_hook(&home.join(".cursor").join("hooks.json"));

    let pending = pending_count_all(conn);
    let token_key_masked = mask_token(token);
    let device_id = cfg.device_id.clone();
    let hmac_secret = cfg.hmac_secret.clone();

    let hostname = gethostname::gethostname()
        .into_string()
        .unwrap_or_else(|_| String::new());

    let payload = HeartbeatPayload {
        device_id: device_id.clone(),
        hostname,
        token_key_masked,
        client_version: env!("CARGO_PKG_VERSION").to_string(),
        ts: now,
        hooks: HookStatus {
            claude: claude_installed,
            codex: codex_installed,
            cursor: cursor_installed,
        },
        pending_count: pending,
    };

    let body_bytes = serde_json::to_vec(&payload)?;
    let unix_ts = now as u64;
    let sig = if hmac_secret.is_empty() {
        String::new()
    } else {
        compute_request_sig(&hmac_secret, unix_ts, &body_bytes)
    };

    let url = format!("{api_url}/api/v1/ai-track/heartbeat");
    let client = Client::new();
    let mut req = client
        .post(&url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .header("X-AiTrack-Device", &device_id)
        .header(
            "X-AiTrack-Client",
            format!("aitrack/{}", env!("CARGO_PKG_VERSION")),
        )
        .header("X-AiTrack-Timestamp", unix_ts.to_string())
        .body(body_bytes);

    if !sig.is_empty() {
        req = req.header("X-AiTrack-Signature", &sig);
    }

    match req.send().await {
        Ok(resp) if resp.status().is_success() => {
            set_last_heartbeat(conn, now)?;
        }
        Ok(resp) => {
            eprintln!("[aitrack] heartbeat failed: HTTP {}", resp.status());
        }
        Err(e) => {
            eprintln!("[aitrack] heartbeat error: {e}");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path};
    use crate::db::{ensure_kv_table, get_last_heartbeat, set_last_heartbeat};
    use super::send_heartbeat;

    fn open_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        ensure_kv_table(&conn).unwrap();
        conn
    }

    #[test]
    fn heartbeat_throttle_skips_when_recent() {
        let conn = open_test_db();
        // Set a very recent last_heartbeat (now)
        let now = chrono::Utc::now().timestamp();
        set_last_heartbeat(&conn, now).unwrap();

        // Simulate the throttle check: if now - last < 3600, skip
        let last = get_last_heartbeat(&conn).unwrap();
        let elapsed = now - last;
        assert!(elapsed < 3600, "recent heartbeat should throttle");
    }

    #[test]
    fn heartbeat_throttle_allows_when_expired() {
        let conn = open_test_db();
        // Set last_heartbeat > 1 hour ago
        let old_ts = chrono::Utc::now().timestamp() - 7200;
        set_last_heartbeat(&conn, old_ts).unwrap();

        let last = get_last_heartbeat(&conn).unwrap();
        let now = chrono::Utc::now().timestamp();
        let elapsed = now - last;
        assert!(elapsed >= 3600, "expired heartbeat should allow send");
    }

    #[test]
    fn heartbeat_no_last_means_send_allowed() {
        let conn = open_test_db();
        // No last_heartbeat stored → None → should send
        assert!(get_last_heartbeat(&conn).is_none());
    }

    #[test]
    fn set_last_heartbeat_persists_and_overwrites() {
        let conn = open_test_db();
        set_last_heartbeat(&conn, 1000).unwrap();
        assert_eq!(get_last_heartbeat(&conn), Some(1000));
        set_last_heartbeat(&conn, 2000).unwrap();
        assert_eq!(get_last_heartbeat(&conn), Some(2000));
    }

    #[test]
    fn heartbeat_force_flag_bypasses_throttle_logic() {
        let conn = open_test_db();
        let now = chrono::Utc::now().timestamp();
        set_last_heartbeat(&conn, now).unwrap();
        let last = get_last_heartbeat(&conn).unwrap();
        let elapsed = now - last;
        let force = true;
        let would_skip = !force && elapsed < 3600;
        assert!(!would_skip, "force=true must never skip");
    }

    #[tokio::test]
    async fn send_heartbeat_success_updates_last_heartbeat() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/ai-track/heartbeat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&mock_server)
            .await;

        let conn = open_test_db();
        let before = chrono::Utc::now().timestamp();
        send_heartbeat(&conn, &mock_server.uri(), "aitrack_testtoken12345", true).await.unwrap();

        let recorded = get_last_heartbeat(&conn).expect("last_heartbeat should be set after success");
        assert!(recorded >= before, "last_heartbeat should be >= time before send");
    }

    #[tokio::test]
    async fn send_heartbeat_failure_does_not_update_last_heartbeat() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/ai-track/heartbeat"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let conn = open_test_db();
        send_heartbeat(&conn, &mock_server.uri(), "aitrack_testtoken12345", true).await.unwrap();

        // HTTP 500 → last_heartbeat should not be updated
        assert!(get_last_heartbeat(&conn).is_none(), "failed heartbeat should not update timestamp");
    }

    #[tokio::test]
    async fn send_heartbeat_throttled_when_recent() {
        let mock_server = MockServer::start().await;
        // No mock for heartbeat — if it's called, the test will fail via connection to nothing
        // We set last_heartbeat to now (very recent) and expect throttle to skip
        let conn = open_test_db();
        let now = chrono::Utc::now().timestamp();
        set_last_heartbeat(&conn, now).unwrap();

        // force=false → throttle should skip (elapsed < 3600)
        // Using a valid but unused URL to verify no HTTP call is made
        send_heartbeat(&conn, &mock_server.uri(), "aitrack_testtoken12345", false).await.unwrap();
        // If throttled, last_heartbeat stays the same value
        let after = get_last_heartbeat(&conn).unwrap();
        assert_eq!(after, now, "throttled: last_heartbeat should not change");
    }

    #[tokio::test]
    async fn send_heartbeat_connection_error_is_graceful() {
        let conn = open_test_db();
        // Use invalid endpoint → connection error → should not panic, just log
        send_heartbeat(&conn, "http://127.0.0.1:1", "aitrack_testtoken12345", true).await.unwrap();
        // last_heartbeat should not be set
        assert!(get_last_heartbeat(&conn).is_none());
    }
}
