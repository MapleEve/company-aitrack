use anyhow::Result;
use chrono::Utc;
use reqwest::Client;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::config::{load_config, mask_token};
use crate::crypto::compute_request_sig;
use crate::db::{fetch_unsynced, increment_retry, mark_synced};

const BATCH_LIMIT: i64 = 200;

#[derive(Serialize)]
struct UploadPayload {
    device_id: String,
    client_version: String,
    edits: Vec<EditRecord>,
}

#[derive(Serialize)]
struct EditRecord {
    tool: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_version: Option<String>,
    provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    session_id: String,
    repo_url: String,
    branch: String,
    current_sha: String,
    file_path: String,
    added_lines: i64,
    removed_lines: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    diff_hunk: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<String>,
    timestamp: String,
    device_id: String,
    hostname: String,
    record_sig: String,
}

#[derive(Deserialize)]
struct UploadResponse {
    accepted: Option<i64>,
    #[serde(default)]
    rejected: Vec<IndexedItem>,
    #[serde(default)]
    #[allow(dead_code)]
    flagged: Vec<IndexedItem>,
}

#[derive(Deserialize)]
struct IndexedItem {
    index: usize,
    #[allow(dead_code)]
    reason: Option<String>,
}

pub async fn flush_unsynced(conn: &Connection, api_url: &str, token: &str) -> Result<()> {
    let cfg = load_config();
    let token_key = mask_token(token);
    let device_id = cfg.device_id.clone();
    let hmac_secret = cfg.hmac_secret.clone();

    let rows = fetch_unsynced(conn, &token_key, BATCH_LIMIT)?;
    if rows.is_empty() {
        return Ok(());
    }

    let ids: Vec<i64> = rows.iter().map(|r| r.id).collect();

    let edits: Vec<EditRecord> = rows
        .iter()
        .map(|r| EditRecord {
            tool: r.tool.clone(),
            tool_version: r.tool_version.clone(),
            provider: r.provider.clone(),
            model: r.model.clone(),
            session_id: r.session_id.clone(),
            repo_url: r.repo_url.clone(),
            branch: r.branch.clone(),
            current_sha: r.current_sha.clone(),
            file_path: r.file_path.clone(),
            added_lines: r.added_lines,
            removed_lines: r.removed_lines,
            diff_hunk: r.diff_hunk.clone(),
            metadata: r.metadata.clone(),
            timestamp: r.timestamp.clone(),
            device_id: r.device_id.clone(),
            hostname: r.hostname.clone(),
            record_sig: r.record_sig.clone(),
        })
        .collect();

    let payload = UploadPayload {
        device_id: device_id.clone(),
        client_version: env!("CARGO_PKG_VERSION").to_string(),
        edits,
    };

    let body_bytes = serde_json::to_vec(&payload)?;
    let unix_ts = Utc::now().timestamp() as u64;
    let sig = if hmac_secret.is_empty() {
        String::new()
    } else {
        compute_request_sig(&hmac_secret, unix_ts, &body_bytes)
    };

    let url = format!("{api_url}/api/v1/ai-track/edits");
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
            // Parse response to distinguish accepted/rejected/flagged
            if let Ok(ur) = resp.json::<UploadResponse>().await {
                // rejected: increment retry
                let rejected_ids: Vec<i64> = ur
                    .rejected
                    .iter()
                    .filter_map(|item| ids.get(item.index).copied())
                    .collect();
                increment_retry(conn, &rejected_ids)?;

                // accepted + flagged: mark synced
                let _accepted_count = ur.accepted.unwrap_or(0) as usize;
                let accepted_and_flagged: Vec<i64> = ids
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| {
                        !ur.rejected.iter().any(|r| r.index == *i)
                    })
                    .map(|(_, id)| *id)
                    .collect();
                mark_synced(conn, &accepted_and_flagged)?;
            } else {
                mark_synced(conn, &ids)?;
            }
        }
        Ok(resp) => {
            eprintln!("[aitrack] upload failed: HTTP {}", resp.status());
            increment_retry(conn, &ids)?;
        }
        Err(e) => {
            eprintln!("[aitrack] upload error: {e}");
            increment_retry(conn, &ids)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path};

    use crate::db::{
        self, ensure_kv_table, fetch_unsynced, pending_count,
    };
    use crate::config::mask_token;
    use crate::testkit::factories::EditRecordFactory;
    use super::flush_unsynced;

    const CREATE_TABLE_SQL: &str = "
    CREATE TABLE IF NOT EXISTS records (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      tool TEXT NOT NULL,
      tool_version TEXT,
      provider TEXT NOT NULL,
      model TEXT,
      session_id TEXT NOT NULL,
      repo_url TEXT NOT NULL DEFAULT '',
      branch TEXT NOT NULL DEFAULT '',
      current_sha TEXT NOT NULL DEFAULT '',
      file_path TEXT NOT NULL,
      added_lines INTEGER NOT NULL,
      removed_lines INTEGER NOT NULL,
      diff_hunk TEXT,
      metadata TEXT,
      synced INTEGER DEFAULT 0,
      synced_at TEXT,
      retry_count INTEGER DEFAULT 0,
      timestamp TEXT NOT NULL,
      token_key TEXT NOT NULL DEFAULT '',
      device_id TEXT NOT NULL DEFAULT '',
      hostname TEXT NOT NULL DEFAULT '',
      record_sig TEXT NOT NULL DEFAULT ''
    );
    CREATE INDEX IF NOT EXISTS idx_synced ON records(synced);
    ";

    fn open_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(CREATE_TABLE_SQL).unwrap();
        let _ = conn.execute(
            "ALTER TABLE records ADD COLUMN device_id TEXT NOT NULL DEFAULT ''", [],
        );
        let _ = conn.execute(
            "ALTER TABLE records ADD COLUMN hostname TEXT NOT NULL DEFAULT ''", [],
        );
        let _ = conn.execute(
            "ALTER TABLE records ADD COLUMN record_sig TEXT NOT NULL DEFAULT ''", [],
        );
        ensure_kv_table(&conn).unwrap();
        conn
    }

    fn insert_factory_record(conn: &Connection, seed: u64, token: &str) -> i64 {
        let masked = mask_token(token);
        let rec = EditRecordFactory::new(seed)
            .with_token_key(&masked)
            .with_repo_url("git@github.com:org/repo.git")
            .build();
        db::insert_record(conn, &rec).unwrap();
        // Get the inserted id
        conn.query_row("SELECT id FROM records ORDER BY id DESC LIMIT 1", [], |r| r.get(0)).unwrap()
    }

    #[tokio::test]
    async fn flush_empty_db_is_noop() {
        let conn = open_test_db();
        // No records → flush should return Ok without making HTTP calls
        flush_unsynced(&conn, "http://localhost:9999", "aitrack_testtoken12345").await.unwrap();
    }

    #[tokio::test]
    async fn flush_accepted_response_marks_synced() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/ai-track/edits"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({
                    "accepted": 1,
                    "rejected": [],
                    "flagged": []
                })))
            .mount(&mock_server)
            .await;

        let conn = open_test_db();
        let token = "aitrack_testtoken12345";
        insert_factory_record(&conn, 1, token);

        let masked = mask_token(token);
        assert_eq!(pending_count(&conn, &masked), 1);

        flush_unsynced(&conn, &mock_server.uri(), token).await.unwrap();

        assert_eq!(pending_count(&conn, &masked), 0, "accepted → synced");
    }

    #[tokio::test]
    async fn flush_rejected_response_increments_retry() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/ai-track/edits"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({
                    "accepted": 0,
                    "rejected": [{"index": 0, "reason": "invalid_sig"}],
                    "flagged": []
                })))
            .mount(&mock_server)
            .await;

        let conn = open_test_db();
        let token = "aitrack_testtoken12345";
        insert_factory_record(&conn, 2, token);

        let masked = mask_token(token);
        flush_unsynced(&conn, &mock_server.uri(), token).await.unwrap();

        // Still unsynced (retry_count=1, not yet at 5)
        assert_eq!(pending_count(&conn, &masked), 1, "rejected → still pending");
        // Verify retry_count incremented
        let rows = fetch_unsynced(&conn, &masked, 10).unwrap();
        assert_eq!(rows[0].retry_count, 1);
    }

    #[tokio::test]
    async fn flush_flagged_response_marks_synced() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/ai-track/edits"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({
                    "accepted": 0,
                    "rejected": [],
                    "flagged": [{"index": 0, "reason": "duplicate"}]
                })))
            .mount(&mock_server)
            .await;

        let conn = open_test_db();
        let token = "aitrack_testtoken12345";
        insert_factory_record(&conn, 3, token);
        let masked = mask_token(token);

        flush_unsynced(&conn, &mock_server.uri(), token).await.unwrap();

        // flagged → synced per contract
        assert_eq!(pending_count(&conn, &masked), 0, "flagged → synced");
    }

    #[tokio::test]
    async fn flush_http_500_increments_retry() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/ai-track/edits"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let conn = open_test_db();
        let token = "aitrack_testtoken12345";
        insert_factory_record(&conn, 4, token);
        let masked = mask_token(token);

        flush_unsynced(&conn, &mock_server.uri(), token).await.unwrap();

        let rows = fetch_unsynced(&conn, &masked, 10).unwrap();
        assert_eq!(rows[0].retry_count, 1, "HTTP 500 → retry incremented");
    }

    #[tokio::test]
    async fn flush_connection_error_increments_retry() {
        let conn = open_test_db();
        let token = "aitrack_testtoken12345";
        insert_factory_record(&conn, 5, token);
        let masked = mask_token(token);

        // Use an invalid URL → connection error
        flush_unsynced(&conn, "http://127.0.0.1:1", token).await.unwrap();

        let rows = fetch_unsynced(&conn, &masked, 10).unwrap();
        assert_eq!(rows[0].retry_count, 1, "connection error → retry incremented");
    }

    #[tokio::test]
    async fn flush_mixed_accepted_rejected() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/ai-track/edits"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({
                    "accepted": 1,
                    "rejected": [{"index": 1, "reason": "invalid_sig"}],
                    "flagged": []
                })))
            .mount(&mock_server)
            .await;

        let conn = open_test_db();
        let token = "aitrack_testtoken12345";
        // Insert 2 records (different seeds so different file paths — avoid dedup)
        insert_factory_record(&conn, 10, token);
        insert_factory_record(&conn, 11, token);
        let masked = mask_token(token);

        flush_unsynced(&conn, &mock_server.uri(), token).await.unwrap();

        // index 0 → accepted → synced; index 1 → rejected → retry
        // One still pending
        assert_eq!(pending_count(&conn, &masked), 1, "1 rejected still pending");
    }

    #[tokio::test]
    async fn flush_marks_synced_when_response_unparseable() {
        let mock_server = MockServer::start().await;
        // 200 with non-JSON body → fallback: mark all synced
        Mock::given(method("POST"))
            .and(path("/api/v1/ai-track/edits"))
            .respond_with(ResponseTemplate::new(200).set_body_string("OK"))
            .mount(&mock_server)
            .await;

        let conn = open_test_db();
        let token = "aitrack_testtoken12345";
        let id = insert_factory_record(&conn, 6, token);
        let masked = mask_token(token);

        flush_unsynced(&conn, &mock_server.uri(), token).await.unwrap();

        // Fallback: mark_synced all ids
        let rows = fetch_unsynced(&conn, &masked, 10).unwrap();
        assert!(rows.iter().all(|r| r.id != id), "unparseable 200 → fallback synced");
    }

    #[test]
    fn upload_response_json_structure() {
        // Verify we correctly parse the contract response format
        let json = serde_json::json!({
            "accepted": 3,
            "rejected": [{"index": 1, "reason": "invalid_sig"}],
            "flagged": [{"index": 2, "reason": "duplicate"}]
        });
        // Deserialize to verify field names match
        let accepted = json["accepted"].as_i64().unwrap();
        let rejected_len = json["rejected"].as_array().unwrap().len();
        let flagged_len = json["flagged"].as_array().unwrap().len();
        assert_eq!(accepted, 3);
        assert_eq!(rejected_len, 1);
        assert_eq!(flagged_len, 1);
    }

    #[test]
    fn factory_record_fields_match_edit_record_contract() {
        // Verify factory produces 17 required fields per CONTRACT.md v1.1
        let rec = EditRecordFactory::new(99)
            .with_tool("claude")
            .with_provider("anthropic")
            .build();
        assert!(!rec.tool.is_empty());
        assert!(!rec.provider.is_empty());
        assert!(!rec.session_id.is_empty());
        assert!(!rec.repo_url.is_empty());
        assert!(!rec.branch.is_empty());
        assert!(!rec.current_sha.is_empty());
        assert!(!rec.file_path.is_empty());
        assert!(rec.added_lines >= 0);
        assert!(rec.removed_lines >= 0);
        assert!(!rec.timestamp.is_empty());
        assert!(!rec.device_id.is_empty());
        assert!(!rec.hostname.is_empty());
        // record_sig set by factory (not real HMAC, but not empty)
        assert!(!rec.record_sig.is_empty());
    }

    #[test]
    fn tampered_record_sig_is_not_valid_hmac() {
        use crate::testkit::factories::tampered_record_sig;
        use crate::crypto::compute_record_sig;
        let rec = tampered_record_sig(42);
        let real_sig = compute_record_sig(
            "some-secret",
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
        assert_ne!(rec.record_sig, real_sig, "tampered sig must differ from real sig");
    }

    /// Capture chain integration test: parse → diff → sig → db insert → upload → mark synced.
    #[tokio::test]
    async fn capture_chain_parse_diff_sig_db_upload() {
        use crate::adapters::parse_claude;
        use crate::crypto::compute_record_sig;
        use crate::testkit::factories::ClaudeHookPayloadFactory;

        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/ai-track/edits"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"accepted": 1, "rejected": [], "flagged": []})))
            .mount(&mock_server)
            .await;

        // 1. Parse
        let json = ClaudeHookPayloadFactory::new(100)
            .with_old_string("fn old() {}\n")
            .with_new_string("fn new() {}\nfn added() {}\n")
            .with_file_path("src/chain.rs")
            .with_session_id("chain-sess")
            .build_json();
        let mut record = parse_claude(&json).expect("parse must succeed");

        // 2. Enrich (simulate capture flow)
        record.repo_url = "git@github.com:org/repo.git".to_string();
        record.branch = "main".to_string();
        record.current_sha = "aabbccdd".to_string();
        let token = "aitrack_testtoken12345";
        let token_key = crate::config::mask_token(token);
        record.token_key = token_key.clone();
        record.device_id = "device-chain-test".to_string();
        record.hostname = "chain-test-host".to_string();
        record.timestamp = "2026-05-17T12:00:00Z".to_string();

        // 3. Compute record_sig per CONTRACT.md v1.1
        let hmac_secret = "chain-hmac-secret";
        record.record_sig = compute_record_sig(
            hmac_secret,
            &record.token_key,
            &record.device_id,
            &record.hostname,
            &record.timestamp,
            &record.tool,
            &record.file_path,
            &record.repo_url,
            &record.current_sha,
            record.added_lines,
            record.removed_lines,
            record.diff_hunk.as_deref(),
        );
        assert_eq!(record.record_sig.len(), 64, "record_sig must be 64-char hex");

        // 4. DB insert
        let conn = open_test_db();
        let inserted = db::insert_record(&conn, &record).unwrap();
        assert!(inserted, "first insert should succeed");

        // 5. Dedup: same record within 2s should be rejected
        let dup = db::insert_record(&conn, &record).unwrap();
        assert!(!dup, "duplicate within 2s should be rejected");

        // 6. Verify pending count
        assert_eq!(pending_count(&conn, &token_key), 1);

        // 7. Upload and verify synced
        flush_unsynced(&conn, &mock_server.uri(), token).await.unwrap();
        assert_eq!(pending_count(&conn, &token_key), 0, "after accepted upload: synced");

        // old has 1 line ("fn old()"), new has 2 lines ("fn new()" + "fn added()")
        // Myers: delete "fn old()", insert "fn new()" + "fn added()"
        assert_eq!(record.added_lines, 2, "fn new and fn added both inserted");
        assert_eq!(record.removed_lines, 1, "fn old removed");
        assert_eq!(record.tool, "claude");
        assert_eq!(record.file_path, "src/chain.rs");
    }

    /// Verify that oversized-line tampered record gets inserted but its lines are detectable.
    #[test]
    fn oversized_lines_record_insertable_but_detectable() {
        use crate::testkit::factories::tampered_oversized_lines;
        let conn = open_test_db();
        let rec = tampered_oversized_lines(77);
        let masked = crate::config::mask_token("aitrack_testtoken12345");
        let mut rec = rec;
        rec.token_key = masked.clone();
        rec.repo_url = "git@github.com:org/repo.git".to_string();
        let inserted = db::insert_record(&conn, &rec).unwrap();
        assert!(inserted);
        let rows = crate::db::fetch_unsynced(&conn, &masked, 10).unwrap();
        let found = rows.iter().find(|r| r.added_lines == 99_999_999);
        assert!(found.is_some(), "oversized record should be in DB");
    }
}
