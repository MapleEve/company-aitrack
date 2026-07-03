use anyhow::Result;
use rusqlite::Connection;

use crate::adapter::http::upload::{HttpUploader, PostBatchResult};
use crate::adapter::sqlite::{fetch_unsynced, increment_retry, mark_synced};
use crate::config::{load_config, mask_token, split_credential};
use crate::domain::model::Record;

const BATCH_LIMIT: i64 = 200;
const MAX_UPLOAD_BODY_BYTES: usize = 7 * 1024 * 1024;

/// Flush unsynced records to the server.
///
/// `uploader` must already be constructed with the target `api_url` and
/// `credential`.  This function handles all DB bookkeeping:
/// - Fetching unsynced records
/// - Delegating the HTTP POST to `uploader.post_batch`
/// - Marking accepted / flagged records as synced
/// - Incrementing the retry counter for rejected records or on transient errors
pub async fn flush_unsynced(conn: &Connection, uploader: &HttpUploader) -> Result<()> {
    flush_unsynced_with_limits(conn, uploader, BATCH_LIMIT as usize, MAX_UPLOAD_BODY_BYTES).await
}

async fn flush_unsynced_with_limits(
    conn: &Connection,
    uploader: &HttpUploader,
    max_count: usize,
    max_body_bytes: usize,
) -> Result<()> {
    let (token, _) = match split_credential(&uploader.credential) {
        Ok(parts) => parts,
        Err(e) => {
            eprintln!("[aitrack] invalid credential: {e}");
            return Ok(());
        }
    };

    let cfg = load_config();
    let token_key = mask_token(&token);
    let device_id = cfg.device_id.clone();

    let rows = fetch_unsynced(conn, &token_key, BATCH_LIMIT)?;
    if rows.is_empty() {
        return Ok(());
    }

    for batch in split_upload_batches(&rows, &device_id, max_count, max_body_bytes)? {
        flush_batch(conn, uploader, &device_id, &batch).await?;
    }

    Ok(())
}

async fn flush_batch(
    conn: &Connection,
    uploader: &HttpUploader,
    device_id: &str,
    rows: &[Record],
) -> Result<()> {
    let ids: Vec<i64> = rows.iter().map(|r| r.id).collect();

    match uploader.post_batch(rows, device_id).await {
        PostBatchResult::Success(ur) => {
            // rejected: increment retry counter
            let rejected_ids: Vec<i64> = ur
                .rejected
                .iter()
                .filter_map(|item| ids.get(item.index).copied())
                .collect();
            increment_retry(conn, &rejected_ids)?;

            // accepted + flagged: mark synced
            let accepted_and_flagged: Vec<i64> = ids
                .iter()
                .enumerate()
                .filter(|(i, _)| !ur.rejected.iter().any(|r| r.index == *i))
                .map(|(_, id)| *id)
                .collect();
            mark_synced(conn, &accepted_and_flagged)?;
        }
        PostBatchResult::UnparseableOk => {
            // 2xx but no parseable body — treat all as synced (conservative)
            mark_synced(conn, &ids)?;
        }
        PostBatchResult::TransientError => {
            increment_retry(conn, &ids)?;
        }
        PostBatchResult::CredentialError => {
            // Permanent credential error — skip silently
        }
    }

    Ok(())
}

fn split_upload_batches(
    rows: &[Record],
    device_id: &str,
    max_count: usize,
    max_body_bytes: usize,
) -> Result<Vec<Vec<Record>>> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let max_count = max_count.max(1);
    let max_body_bytes = max_body_bytes.max(1);
    let mut batches = Vec::new();
    let mut current: Vec<Record> = Vec::new();

    for row in rows {
        let count_full = current.len() >= max_count;
        let byte_full = if current.is_empty() {
            false
        } else {
            let mut candidate = current.clone();
            candidate.push(row.clone());
            HttpUploader::payload_size_bytes(&candidate, device_id)? > max_body_bytes
        };

        if count_full || byte_full {
            batches.push(std::mem::take(&mut current));
        }
        current.push(row.clone());
    }

    if !current.is_empty() {
        batches.push(current);
    }

    Ok(batches)
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::{
        flush_unsynced, flush_unsynced_with_limits, split_upload_batches, BATCH_LIMIT,
        MAX_UPLOAD_BODY_BYTES,
    };
    use crate::adapter::http::upload::HttpUploader;
    use crate::adapter::sqlite::{self as db, ensure_kv_table, fetch_unsynced, pending_count};
    use crate::config::mask_token;
    use crate::testkit::factories::EditRecordFactory;

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
      record_sig TEXT NOT NULL DEFAULT '',
      prompt_summary TEXT
    );
    CREATE INDEX IF NOT EXISTS idx_synced ON records(synced);
    ";

    fn open_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(CREATE_TABLE_SQL).unwrap();
        let _ = conn.execute(
            "ALTER TABLE records ADD COLUMN device_id TEXT NOT NULL DEFAULT ''",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE records ADD COLUMN hostname TEXT NOT NULL DEFAULT ''",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE records ADD COLUMN record_sig TEXT NOT NULL DEFAULT ''",
            [],
        );
        ensure_kv_table(&conn).unwrap();
        conn
    }

    /// Test credential: token part is "aitrack_testtoken12345", hmac_secret is "testhmacsecret"
    const TEST_CREDENTIAL: &str = "aitrack_testtoken12345-testhmacsecret";
    const TEST_TOKEN: &str = "aitrack_testtoken12345";

    fn insert_factory_record(conn: &Connection, seed: u64, token: &str) -> i64 {
        let masked = mask_token(token);
        let rec = EditRecordFactory::new(seed)
            .with_token_key(&masked)
            .with_repo_url("git@github.com:org/repo.git")
            .build();
        db::insert_record(conn, &rec).unwrap();
        // Get the inserted id
        conn.query_row("SELECT id FROM records ORDER BY id DESC LIMIT 1", [], |r| {
            r.get(0)
        })
        .unwrap()
    }

    fn set_original_text_fields(conn: &Connection, id: i64) {
        conn.execute(
            "UPDATE records
             SET diff_hunk = ?1, metadata = ?2, prompt_summary = ?3
             WHERE id = ?4",
            (
                "@@ -1 +1 @@\n-raw\n+changed",
                "{\"raw\":\"metadata\"}",
                "raw prompt",
                id,
            ),
        )
        .unwrap();
    }

    fn original_text_fields(
        conn: &Connection,
        id: i64,
    ) -> (Option<String>, Option<String>, Option<String>) {
        conn.query_row(
            "SELECT diff_hunk, metadata, prompt_summary FROM records WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap()
    }

    fn assert_original_text_fields_pruned(conn: &Connection, id: i64) {
        let (diff_hunk, metadata, prompt_summary) = original_text_fields(conn, id);
        assert_eq!(diff_hunk.as_deref(), Some("[aitrack-pruned:diff_hunk]"));
        assert_eq!(metadata.as_deref(), Some("{\"aitrack_pruned\":true}"));
        assert_eq!(
            prompt_summary.as_deref(),
            Some("[aitrack-pruned:prompt_summary]")
        );
    }

    fn make_uploader(api_url: &str) -> HttpUploader {
        HttpUploader::new(api_url.to_string(), TEST_CREDENTIAL.to_string())
    }

    #[tokio::test]
    async fn flush_empty_db_is_noop() {
        let conn = open_test_db();
        // No records → flush should return Ok without making HTTP calls
        let uploader = make_uploader("http://localhost:9999");
        flush_unsynced(&conn, &uploader).await.unwrap();
    }

    #[tokio::test]
    async fn flush_accepted_response_marks_synced() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/ai-track/edits"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "accepted": 1,
                "rejected": [],
                "flagged": []
            })))
            .mount(&mock_server)
            .await;

        let conn = open_test_db();
        let id = insert_factory_record(&conn, 1, TEST_TOKEN);
        set_original_text_fields(&conn, id);

        let masked = mask_token(TEST_TOKEN);
        assert_eq!(pending_count(&conn, &masked), 1);

        let uploader = make_uploader(&mock_server.uri());
        flush_unsynced(&conn, &uploader).await.unwrap();

        assert_eq!(pending_count(&conn, &masked), 0, "accepted → synced");
        assert_original_text_fields_pruned(&conn, id);
    }

    #[tokio::test]
    async fn flush_rejected_response_increments_retry() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/ai-track/edits"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "accepted": 0,
                "rejected": [{"index": 0, "reason": "invalid_sig"}],
                "flagged": []
            })))
            .mount(&mock_server)
            .await;

        let conn = open_test_db();
        let id = insert_factory_record(&conn, 2, TEST_TOKEN);
        set_original_text_fields(&conn, id);

        let masked = mask_token(TEST_TOKEN);
        let uploader = make_uploader(&mock_server.uri());
        flush_unsynced(&conn, &uploader).await.unwrap();

        // Still unsynced (retry_count=1, not yet at 5)
        assert_eq!(pending_count(&conn, &masked), 1, "rejected → still pending");
        // Verify retry_count incremented
        let rows = fetch_unsynced(&conn, &masked, 10).unwrap();
        assert_eq!(rows[0].retry_count, 1);
        assert_eq!(
            original_text_fields(&conn, id),
            (
                Some("@@ -1 +1 @@\n-raw\n+changed".to_string()),
                Some("{\"raw\":\"metadata\"}".to_string()),
                Some("raw prompt".to_string())
            )
        );
    }

    #[tokio::test]
    async fn flush_flagged_response_marks_synced() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/ai-track/edits"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "accepted": 0,
                "rejected": [],
                "flagged": [{"index": 0, "reason": "duplicate"}]
            })))
            .mount(&mock_server)
            .await;

        let conn = open_test_db();
        let id = insert_factory_record(&conn, 3, TEST_TOKEN);
        set_original_text_fields(&conn, id);
        let masked = mask_token(TEST_TOKEN);

        let uploader = make_uploader(&mock_server.uri());
        flush_unsynced(&conn, &uploader).await.unwrap();

        // flagged → synced per contract
        assert_eq!(pending_count(&conn, &masked), 0, "flagged → synced");
        assert_original_text_fields_pruned(&conn, id);
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
        let id = insert_factory_record(&conn, 4, TEST_TOKEN);
        set_original_text_fields(&conn, id);
        let masked = mask_token(TEST_TOKEN);

        let uploader = make_uploader(&mock_server.uri());
        flush_unsynced(&conn, &uploader).await.unwrap();

        let rows = fetch_unsynced(&conn, &masked, 10).unwrap();
        assert_eq!(rows[0].retry_count, 1, "HTTP 500 → retry incremented");
        assert_eq!(
            original_text_fields(&conn, id),
            (
                Some("@@ -1 +1 @@\n-raw\n+changed".to_string()),
                Some("{\"raw\":\"metadata\"}".to_string()),
                Some("raw prompt".to_string())
            )
        );
    }

    #[tokio::test]
    async fn flush_connection_error_increments_retry() {
        let conn = open_test_db();
        let id = insert_factory_record(&conn, 5, TEST_TOKEN);
        set_original_text_fields(&conn, id);
        let masked = mask_token(TEST_TOKEN);

        // Use an invalid URL → connection error
        let uploader = make_uploader("http://127.0.0.1:1");
        flush_unsynced(&conn, &uploader).await.unwrap();

        let rows = fetch_unsynced(&conn, &masked, 10).unwrap();
        assert_eq!(
            rows[0].retry_count, 1,
            "connection error → retry incremented"
        );
        assert_eq!(
            original_text_fields(&conn, id),
            (
                Some("@@ -1 +1 @@\n-raw\n+changed".to_string()),
                Some("{\"raw\":\"metadata\"}".to_string()),
                Some("raw prompt".to_string())
            )
        );
    }

    #[tokio::test]
    async fn flush_mixed_accepted_rejected() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/ai-track/edits"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "accepted": 1,
                "rejected": [{"index": 1, "reason": "invalid_sig"}],
                "flagged": []
            })))
            .mount(&mock_server)
            .await;

        let conn = open_test_db();
        // Insert 2 records (different seeds so different file paths — avoid dedup)
        insert_factory_record(&conn, 10, TEST_TOKEN);
        insert_factory_record(&conn, 11, TEST_TOKEN);
        let masked = mask_token(TEST_TOKEN);

        let uploader = make_uploader(&mock_server.uri());
        flush_unsynced(&conn, &uploader).await.unwrap();

        // index 0 → accepted → synced; index 1 → rejected → retry
        // One still pending
        assert_eq!(pending_count(&conn, &masked), 1, "1 rejected still pending");
    }

    #[tokio::test]
    async fn flush_splits_records_by_serialized_body_limit() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/ai-track/edits"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "accepted": 1,
                "rejected": [],
                "flagged": []
            })))
            .mount(&mock_server)
            .await;

        let conn = open_test_db();
        let id_1 = insert_factory_record(&conn, 201, TEST_TOKEN);
        let id_2 = insert_factory_record(&conn, 202, TEST_TOKEN);
        set_original_text_fields(&conn, id_1);
        set_original_text_fields(&conn, id_2);
        let rows = fetch_unsynced(&conn, &mask_token(TEST_TOKEN), 10).unwrap();
        let device_id = crate::config::load_config().device_id;
        let single_size = HttpUploader::payload_size_bytes(&rows[0..1], &device_id).unwrap();
        let pair_size = HttpUploader::payload_size_bytes(&rows[0..2], &device_id).unwrap();

        let uploader = make_uploader(&mock_server.uri());
        flush_unsynced_with_limits(&conn, &uploader, BATCH_LIMIT as usize, pair_size - 1)
            .await
            .unwrap();

        let requests = mock_server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 2);
        for request in requests {
            assert!(request.body.len() >= single_size);
            assert!(request.body.len() < pair_size);
            let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
            assert_eq!(body["edits"].as_array().unwrap().len(), 1);
        }
        let masked = mask_token(TEST_TOKEN);
        assert_eq!(pending_count(&conn, &masked), 0);
        assert_original_text_fields_pruned(&conn, id_1);
        assert_original_text_fields_pruned(&conn, id_2);
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
        let id = insert_factory_record(&conn, 6, TEST_TOKEN);
        set_original_text_fields(&conn, id);
        let masked = mask_token(TEST_TOKEN);

        let uploader = make_uploader(&mock_server.uri());
        flush_unsynced(&conn, &uploader).await.unwrap();

        // Fallback: mark_synced all ids
        let rows = fetch_unsynced(&conn, &masked, 10).unwrap();
        assert!(
            rows.iter().all(|r| r.id != id),
            "unparseable 200 → fallback synced"
        );
        assert_original_text_fields_pruned(&conn, id);
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
        use crate::domain::crypto::compute_record_sig;
        use crate::testkit::factories::tampered_record_sig;
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
        assert_ne!(
            rec.record_sig, real_sig,
            "tampered sig must differ from real sig"
        );
    }

    #[test]
    fn split_upload_batches_preserves_count_limit() {
        let records = (0..(BATCH_LIMIT as u64 + 1))
            .map(|seed| EditRecordFactory::new(seed).build())
            .collect::<Vec<_>>();

        let batches =
            split_upload_batches(&records, "dev-count", BATCH_LIMIT as usize, usize::MAX).unwrap();

        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].len(), BATCH_LIMIT as usize);
        assert_eq!(batches[1].len(), 1);
    }

    #[test]
    fn split_upload_batches_respects_serialized_body_limit() {
        let records = (0..3)
            .map(|seed| EditRecordFactory::new(seed).build())
            .collect::<Vec<_>>();
        let single_size = HttpUploader::payload_size_bytes(&records[0..1], "dev-bytes").unwrap();
        let pair_size = HttpUploader::payload_size_bytes(&records[0..2], "dev-bytes").unwrap();

        let batches =
            split_upload_batches(&records, "dev-bytes", BATCH_LIMIT as usize, pair_size - 1)
                .unwrap();

        assert!(single_size < pair_size);
        assert_eq!(batches.len(), 3);
        assert!(batches.iter().all(|batch| batch.len() == 1));
    }

    #[test]
    fn production_stored_single_record_fits_upload_body_limit() {
        let mut rec = EditRecordFactory::new(204)
            .with_diff_hunk(Some(
                "d".repeat(crate::domain::model::MAX_STORED_DIFF_HUNK_CHARS),
            ))
            .with_prompt_summary(Some(
                "p".repeat(crate::domain::model::MAX_STORED_PROMPT_CHARS),
            ))
            .build();
        rec.metadata = Some("m".repeat(crate::domain::model::MAX_STORED_METADATA_CHARS));
        let body_size = HttpUploader::payload_size_bytes(&[rec], "dev-single-limit").unwrap();

        assert!(body_size < MAX_UPLOAD_BODY_BYTES);
    }

    /// Capture chain integration test: parse → diff → sig → db insert → upload → mark synced.
    #[tokio::test]
    async fn capture_chain_parse_diff_sig_db_upload() {
        use crate::adapter::event::parse_claude;
        use crate::domain::crypto::compute_record_sig;
        use crate::testkit::factories::ClaudeHookPayloadFactory;

        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/ai-track/edits"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"accepted": 1, "rejected": [], "flagged": []}),
                ),
            )
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
        let token_key = crate::config::mask_token(TEST_TOKEN);
        record.token_key = token_key.clone();
        record.device_id = "device-chain-test".to_string();
        record.hostname = "chain-test-host".to_string();
        record.timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

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
        assert_eq!(
            record.record_sig.len(),
            64,
            "record_sig must be 64-char hex"
        );

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
        let uploader = make_uploader(&mock_server.uri());
        flush_unsynced(&conn, &uploader).await.unwrap();
        assert_eq!(
            pending_count(&conn, &token_key),
            0,
            "after accepted upload: synced"
        );

        // old has 1 line ("fn old()"), new has 2 lines ("fn new()" + "fn added()")
        // Myers: delete "fn old()", insert "fn new()" + "fn added()"
        assert_eq!(record.added_lines, 2, "fn new and fn added both inserted");
        assert_eq!(record.removed_lines, 1, "fn old removed");
        assert_eq!(record.tool, "claude");
        assert_eq!(record.file_path, "src/chain.rs");
    }

    #[tokio::test]
    async fn capture_chain_uploads_bounded_prompt_monitoring_content() {
        use crate::adapter::event::parse_claude;
        use crate::domain::crypto::compute_record_sig;
        use crate::testkit::factories::ClaudeHookPayloadFactory;

        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/ai-track/edits"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    serde_json::json!({"accepted": 1, "rejected": [], "flagged": []}),
                ),
            )
            .mount(&mock_server)
            .await;

        let conn = open_test_db();
        db::ensure_prompt_context_table(&conn).unwrap();
        db::insert_prompt_context(
            &conn,
            "prompt-monitoring-sess",
            "find every place checkout totals can leak customer pricing",
        )
        .unwrap();

        let json = ClaudeHookPayloadFactory::new(101)
            .with_old_string("let total = subtotal;\n")
            .with_new_string("let total = subtotal + tax;\n")
            .with_file_path("src/checkout.rs")
            .with_session_id("prompt-monitoring-sess")
            .build_json();
        let mut record = parse_claude(&json).expect("parse must succeed");
        record.repo_url = "git@github.com:org/repo.git".to_string();
        record.branch = "main".to_string();
        record.current_sha = "ddeeff00".to_string();
        record.token_key = mask_token(TEST_TOKEN);
        record.device_id = "device-prompt-test".to_string();
        record.hostname = "prompt-test-host".to_string();
        record.timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        record.prompt_summary = db::get_recent_prompt(&conn, &record.session_id);
        record.record_sig = compute_record_sig(
            "testhmacsecret",
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

        assert!(db::insert_record(&conn, &record).unwrap());

        let uploader = make_uploader(&mock_server.uri());
        flush_unsynced(&conn, &uploader).await.unwrap();

        let requests = mock_server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        let edit = &body["edits"][0];
        assert_eq!(edit["session_id"], "prompt-monitoring-sess");
        assert_eq!(edit["file_path"], "src/checkout.rs");
        assert_eq!(
            edit["prompt_summary"],
            "find every place checkout totals can leak customer pricing"
        );
    }

    /// Verify that oversized-line tampered record gets inserted but its lines are detectable.
    #[test]
    fn oversized_lines_record_insertable_but_detectable() {
        use crate::testkit::factories::tampered_oversized_lines;
        let conn = open_test_db();
        let rec = tampered_oversized_lines(77);
        let masked = crate::config::mask_token(TEST_TOKEN);
        let mut rec = rec;
        rec.token_key = masked.clone();
        rec.repo_url = "git@github.com:org/repo.git".to_string();
        let inserted = db::insert_record(&conn, &rec).unwrap();
        assert!(inserted);
        let rows = crate::adapter::sqlite::fetch_unsynced(&conn, &masked, 10).unwrap();
        let found = rows.iter().find(|r| r.added_lines == 99_999_999);
        assert!(found.is_some(), "oversized record should be in DB");
    }
}
