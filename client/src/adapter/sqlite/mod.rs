pub mod keyword_store;
pub mod queries;
pub mod schema;
pub mod vec;

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::fs;
use std::fs::OpenOptions;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use crate::config::db_path;
use crate::domain::model::{InspectRow, Record};
use crate::port::storage::StoragePort;

// ---------------------------------------------------------------------------
// Public re-exports — keep the same surface that db/ exposed so lib.rs and
// all other callers need no changes.
// ---------------------------------------------------------------------------
pub use queries::{
    backfill_repo_info, clean_all, clean_synced, ensure_kv_table, ensure_prompt_context_table,
    fetch_unsynced, get_kv, get_last_heartbeat, get_recent_prompt, increment_retry,
    insert_prompt_context, insert_record, inspect_records, mark_synced, pending_count,
    pending_count_all, prune_local_record_storage, prune_old_synced_records, set_kv,
    set_last_heartbeat, token_breakdown,
};

/// SQLite-backed storage adapter.
pub struct SqliteStorage {
    pub conn: Connection,
}

impl SqliteStorage {
    pub fn open() -> Result<Self> {
        Ok(Self { conn: open_db()? })
    }
}

impl StoragePort for SqliteStorage {
    fn save_record(&self, record: &Record) -> rusqlite::Result<bool> {
        queries::insert_record(&self.conn, record)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))
    }

    fn pending_count(&self, token_key: &str) -> i64 {
        queries::pending_count(&self.conn, token_key)
    }

    fn fetch_unsynced(&self, token_key: &str, limit: i64) -> rusqlite::Result<Vec<Record>> {
        queries::fetch_unsynced(&self.conn, token_key, limit)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))
    }

    fn mark_synced(&self, ids: &[i64]) -> anyhow::Result<()> {
        queries::mark_synced(&self.conn, ids)
    }

    fn increment_retry(&self, ids: &[i64]) -> anyhow::Result<()> {
        queries::increment_retry(&self.conn, ids)
    }

    fn inspect_records(
        &self,
        limit: i64,
        pending_only: bool,
        token_key: &str,
    ) -> anyhow::Result<Vec<InspectRow>> {
        queries::inspect_records(&self.conn, limit, pending_only, token_key)
    }
}

/// Open (or create) the records database and run all pending migrations.
///
/// sqlite-vec is registered as an auto_extension on the very first call so
/// that every subsequent `Connection::open*` also gets the extension.  If the
/// extension fails to load the `vec::VEC_DISABLED` flag is set and the rest of
/// the pipeline continues without vector support.
pub fn open_db() -> Result<Connection> {
    // Register sqlite-vec once for the lifetime of the process.
    static VEC_REGISTERED: std::sync::Once = std::sync::Once::new();
    VEC_REGISTERED.call_once(|| {
        vec::register_auto_extension();
    });

    let path = db_path();
    let dir = path.parent().unwrap();
    fs::create_dir_all(dir).context("create ~/.aitrack")?;

    // Create the file atomically with 0o600 before SQLite opens it.
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        opts.mode(0o600);
    }
    let _ = opts.open(&path);

    let conn = Connection::open(&path).context("open records.db")?;

    conn.execute_batch(schema::CREATE_TABLE_SQL)
        .context("create records table")?;

    for migration in schema::MIGRATIONS {
        let _ = conn.execute(migration, []);
    }

    queries::ensure_kv_table(&conn)?;
    queries::ensure_prompt_context_table(&conn)?;

    vec::init_sqlite_vec(&conn);
    if let Err(e) = vec::ensure_vec_table(&conn) {
        eprintln!("[aitrack] could not create vec_records table: {e}");
    }

    Ok(conn)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use schema::CREATE_TABLE_SQL;

    fn make_record(tool: &str, file_path: &str, token_key: &str) -> Record {
        Record {
            id: 0,
            tool: tool.to_string(),
            tool_version: Some("v1".to_string()),
            provider: "anthropic".to_string(),
            model: None,
            session_id: "sess-1".to_string(),
            repo_url: "git@github.com:org/repo.git".to_string(),
            branch: "main".to_string(),
            current_sha: "abc123".to_string(),
            file_path: file_path.to_string(),
            added_lines: 5,
            removed_lines: 2,
            diff_hunk: Some("@@ -1,2 +1,5 @@\n-old\n+new".to_string()),
            metadata: None,
            synced: 0,
            synced_at: None,
            retry_count: 0,
            timestamp: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            token_key: token_key.to_string(),
            device_id: "device-1".to_string(),
            hostname: "test-host".to_string(),
            record_sig: format!("sig-{tool}-{token_key}-{}", file_path.replace('/', "_")),
            prompt_summary: None,
        }
    }

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
        let _ = conn.execute("ALTER TABLE records ADD COLUMN embedding BLOB", []);
        let _ = conn.execute("ALTER TABLE records ADD COLUMN prompt_summary TEXT", []);
        ensure_kv_table(&conn).unwrap();
        ensure_prompt_context_table(&conn).unwrap();
        conn
    }

    fn record_ids_by_synced(conn: &Connection, synced: i64) -> Vec<i64> {
        let mut stmt = conn
            .prepare("SELECT id FROM records WHERE synced = ?1 ORDER BY id")
            .unwrap();
        stmt.query_map([synced], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    }

    fn record_count_where(conn: &Connection, where_clause: &str) -> i64 {
        conn.query_row(
            &format!("SELECT COUNT(*) FROM records WHERE {where_clause}"),
            [],
            |row| row.get(0),
        )
        .unwrap()
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

    fn record_exists_with_file_path(conn: &Connection, file_path: &str) -> bool {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM records WHERE file_path = ?1",
                [file_path],
                |row| row.get(0),
            )
            .unwrap();
        count > 0
    }

    #[test]
    fn insert_and_fetch_unsynced() {
        let conn = open_test_db();
        let r = make_record("claude", "src/main.rs", "tok123");
        let inserted = insert_record(&conn, &r).unwrap();
        assert!(inserted);

        let rows = fetch_unsynced(&conn, "tok123", 100).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].tool, "claude");
        assert_eq!(rows[0].file_path, "src/main.rs");
    }

    #[test]
    fn insert_record_truncates_non_signature_text_fields() {
        let conn = open_test_db();
        let mut r = make_record("claude", "src/large.rs", "tok-large");
        r.metadata = Some("m".repeat(crate::domain::model::MAX_STORED_METADATA_CHARS + 16));
        r.prompt_summary = Some("p".repeat(crate::domain::model::MAX_STORED_PROMPT_CHARS + 16));
        assert!(insert_record(&conn, &r).unwrap());

        let rows = fetch_unsynced(&conn, "tok-large", 100).unwrap();
        assert_eq!(
            rows[0].metadata.as_ref().unwrap().chars().count(),
            crate::domain::model::MAX_STORED_METADATA_CHARS
        );
        assert_eq!(
            rows[0].prompt_summary.as_ref().unwrap().chars().count(),
            crate::domain::model::MAX_STORED_PROMPT_CHARS
        );
    }

    #[test]
    fn dedup_window_prevents_second_insert() {
        let conn = open_test_db();
        let r = make_record("claude", "src/dup.rs", "tok-dup");
        let ins1 = insert_record(&conn, &r).unwrap();
        assert!(ins1);
        let ins2 = insert_record(&conn, &r).unwrap();
        assert!(!ins2, "duplicate within 2s should be rejected");
    }

    #[test]
    fn prompt_context_is_truncated_and_pruned() {
        let conn = open_test_db();
        let long_prompt = "x".repeat(crate::domain::model::MAX_STORED_PROMPT_CHARS + 64);
        insert_prompt_context(&conn, "sess-large", &long_prompt).unwrap();
        assert_eq!(
            get_recent_prompt(&conn, "sess-large")
                .unwrap()
                .chars()
                .count(),
            crate::domain::model::MAX_STORED_PROMPT_CHARS
        );

        let worst_case_prompt = "w".repeat(crate::domain::model::MAX_STORED_PROMPT_CHARS);
        for idx in 0..(queries::MAX_PROMPT_CONTEXT_ROWS + 10) {
            insert_prompt_context(&conn, &format!("sess-{idx}"), &worst_case_prompt).unwrap();
        }
        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM prompt_context", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, queries::MAX_PROMPT_CONTEXT_ROWS);
        let stored_chars: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(length(prompt_text)), 0) FROM prompt_context",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(stored_chars <= queries::MAX_PROMPT_CONTEXT_ROWS * 4096);
    }

    #[test]
    fn different_file_path_not_deduped() {
        let conn = open_test_db();
        let r1 = make_record("claude", "src/a.rs", "tok-a");
        let r2 = make_record("claude", "src/b.rs", "tok-a");
        assert!(insert_record(&conn, &r1).unwrap());
        assert!(insert_record(&conn, &r2).unwrap());
    }

    #[test]
    fn mark_synced_and_retry_count() {
        let conn = open_test_db();
        let r = make_record("claude", "src/sync.rs", "tok-sync");
        insert_record(&conn, &r).unwrap();

        let rows = fetch_unsynced(&conn, "tok-sync", 100).unwrap();
        let id = rows[0].id;

        mark_synced(&conn, &[id]).unwrap();
        let after = fetch_unsynced(&conn, "tok-sync", 100).unwrap();
        assert!(after.is_empty(), "should be empty after mark_synced");
    }

    #[test]
    fn mark_synced_strips_original_text_fields() {
        let conn = open_test_db();
        let mut r = make_record("claude", "src/sync-strip.rs", "tok-sync-strip");
        r.diff_hunk = Some("@@ -1 +1 @@\n-raw\n+changed".to_string());
        r.metadata = Some("{\"raw\":\"metadata\"}".to_string());
        r.prompt_summary = Some("raw prompt".to_string());
        insert_record(&conn, &r).unwrap();

        let row = fetch_unsynced(&conn, "tok-sync-strip", 100).unwrap();
        let id = row[0].id;
        mark_synced(&conn, &[id]).unwrap();

        let (diff_hunk, metadata, prompt_summary) = original_text_fields(&conn, id);
        assert_eq!(diff_hunk.as_deref(), Some("[aitrack-pruned:diff_hunk]"));
        assert_eq!(metadata.as_deref(), Some("{\"aitrack_pruned\":true}"));
        assert_eq!(
            prompt_summary.as_deref(),
            Some("[aitrack-pruned:prompt_summary]")
        );
        let record_sig: String = conn
            .query_row(
                "SELECT record_sig FROM records WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(record_sig, r.record_sig);
    }

    #[test]
    fn increment_retry_keeps_pending_original_text_fields() {
        let conn = open_test_db();
        let mut r = make_record("claude", "src/retry-keep.rs", "tok-retry-keep");
        r.diff_hunk = Some("@@ -1 +1 @@\n-raw\n+retry".to_string());
        r.metadata = Some("{\"raw\":\"metadata\"}".to_string());
        r.prompt_summary = Some("raw retry prompt".to_string());
        insert_record(&conn, &r).unwrap();

        let row = fetch_unsynced(&conn, "tok-retry-keep", 100).unwrap();
        let id = row[0].id;
        increment_retry(&conn, &[id]).unwrap();

        let (diff_hunk, metadata, prompt_summary) = original_text_fields(&conn, id);
        assert_eq!(diff_hunk, r.diff_hunk);
        assert_eq!(metadata, r.metadata);
        assert_eq!(prompt_summary, r.prompt_summary);
    }

    #[test]
    fn increment_retry_removes_after_5() {
        let conn = open_test_db();
        let r = make_record("claude", "src/retry.rs", "tok-retry");
        insert_record(&conn, &r).unwrap();

        let rows = fetch_unsynced(&conn, "tok-retry", 100).unwrap();
        let id = rows[0].id;

        for _ in 0..5 {
            increment_retry(&conn, &[id]).unwrap();
        }

        let after = fetch_unsynced(&conn, "tok-retry", 100).unwrap();
        assert!(
            after.is_empty(),
            "retry_count=5 should be excluded from fetch"
        );
    }

    #[test]
    fn pending_count_counts_unsynced() {
        let conn = open_test_db();
        let r1 = make_record("claude", "src/p1.rs", "tok-pending");
        let r2 = make_record("claude", "src/p2.rs", "tok-pending");
        insert_record(&conn, &r1).unwrap();
        insert_record(&conn, &r2).unwrap();

        let count = pending_count(&conn, "tok-pending");
        assert_eq!(count, 2);
    }

    #[test]
    fn pending_count_all_includes_all_tokens() {
        let conn = open_test_db();
        insert_record(&conn, &make_record("claude", "src/a.rs", "tok-a")).unwrap();
        insert_record(&conn, &make_record("codex", "src/b.rs", "tok-b")).unwrap();

        assert_eq!(pending_count_all(&conn), 2);
    }

    #[test]
    fn clean_synced_only_removes_synced_records() {
        let conn = open_test_db();
        let r1 = make_record("claude", "src/cs1.rs", "tok-cs");
        let r2 = make_record("claude", "src/cs2.rs", "tok-cs");
        insert_record(&conn, &r1).unwrap();
        insert_record(&conn, &r2).unwrap();

        let rows = fetch_unsynced(&conn, "tok-cs", 100).unwrap();
        mark_synced(&conn, &[rows[0].id]).unwrap();

        let deleted = clean_synced(&conn).unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(pending_count(&conn, "tok-cs"), 1);
    }

    #[test]
    fn prune_old_synced_records_preserves_unsynced_rows() {
        let conn = open_test_db();
        for idx in 0..4 {
            insert_record(
                &conn,
                &make_record(
                    "claude",
                    &format!("src/prune-unsynced-{idx}.rs"),
                    "tok-prune",
                ),
            )
            .unwrap();
        }

        let unsynced = fetch_unsynced(&conn, "tok-prune", 100).unwrap();
        mark_synced(&conn, &[unsynced[0].id, unsynced[1].id]).unwrap();

        let deleted = prune_old_synced_records(&conn, 1).unwrap();

        assert_eq!(deleted, 1);
        assert_eq!(
            record_ids_by_synced(&conn, 0),
            vec![unsynced[2].id, unsynced[3].id]
        );
    }

    #[test]
    fn prune_old_synced_records_keeps_latest_synced_rows_by_id() {
        let conn = open_test_db();
        for idx in 0..5 {
            insert_record(
                &conn,
                &make_record("claude", &format!("src/prune-latest-{idx}.rs"), "tok-prune"),
            )
            .unwrap();
        }

        let rows = fetch_unsynced(&conn, "tok-prune", 100).unwrap();
        let ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
        mark_synced(&conn, &ids).unwrap();

        let deleted = prune_old_synced_records(&conn, 2).unwrap();

        assert_eq!(deleted, 3);
        assert_eq!(record_ids_by_synced(&conn, 1), vec![ids[3], ids[4]]);
    }

    #[test]
    fn prune_old_synced_records_is_noop_when_under_limit() {
        let conn = open_test_db();
        for idx in 0..2 {
            insert_record(
                &conn,
                &make_record("claude", &format!("src/prune-under-{idx}.rs"), "tok-prune"),
            )
            .unwrap();
        }

        let rows = fetch_unsynced(&conn, "tok-prune", 100).unwrap();
        let ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
        mark_synced(&conn, &ids).unwrap();

        let deleted = prune_old_synced_records(&conn, 5).unwrap();

        assert_eq!(deleted, 0);
        assert_eq!(record_ids_by_synced(&conn, 1), ids);
    }

    #[test]
    fn prune_old_synced_records_ignores_non_positive_limits() {
        let conn = open_test_db();
        for idx in 0..2 {
            insert_record(
                &conn,
                &make_record(
                    "claude",
                    &format!("src/prune-non-positive-{idx}.rs"),
                    "tok-prune",
                ),
            )
            .unwrap();
        }

        let rows = fetch_unsynced(&conn, "tok-prune", 100).unwrap();
        let ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
        mark_synced(&conn, &ids).unwrap();

        assert_eq!(prune_old_synced_records(&conn, 0).unwrap(), 0);
        assert_eq!(prune_old_synced_records(&conn, -1).unwrap(), 0);
        assert_eq!(record_ids_by_synced(&conn, 1), ids);
    }

    #[test]
    fn prune_local_record_storage_caps_unuploadable_pending_records() {
        let conn = open_test_db();
        let mut inserted = Vec::new();
        for idx in 0..(queries::MAX_UNUPLOADABLE_PENDING_RECORD_ROWS + 8) {
            let mut r = make_record("claude", &format!("src/no-credential-{idx}.rs"), "");
            r.token_key = String::new();
            r.record_sig = String::new();
            let diff_hunk = format!("@@ -1 +1 @@\n-old-{idx}\n+new-{idx}");
            let metadata = format!("raw metadata {idx}");
            let prompt_summary = format!("raw prompt {idx}");
            r.diff_hunk = Some(diff_hunk.clone());
            r.metadata = Some(metadata.clone());
            r.prompt_summary = Some(prompt_summary.clone());
            insert_record(&conn, &r).unwrap();
            inserted.push((
                idx,
                conn.last_insert_rowid(),
                diff_hunk,
                metadata,
                prompt_summary,
            ));
        }

        prune_local_record_storage(&conn).unwrap();

        assert_eq!(
            record_count_where(
                &conn,
                "synced = 0 AND (token_key = '' OR token_key = 'legacy' OR record_sig = '')"
            ),
            queries::MAX_UNUPLOADABLE_PENDING_RECORD_ROWS
        );
        assert_eq!(
            record_count_where(
                &conn,
                "COALESCE(diff_hunk, '') LIKE '%old-%'
                 AND COALESCE(metadata, '') LIKE '%raw metadata%'
                 AND COALESCE(prompt_summary, '') LIKE '%raw prompt%'"
            ),
            queries::MAX_UNUPLOADABLE_PENDING_RECORD_ROWS
        );

        for idx in 0..8 {
            assert!(
                !record_exists_with_file_path(&conn, &format!("src/no-credential-{idx}.rs")),
                "old over-cap unuploadable row {idx} should be deleted"
            );
        }

        for (idx, id, diff_hunk, metadata, prompt_summary) in inserted.into_iter().skip(8) {
            let (stored_diff_hunk, stored_metadata, stored_prompt_summary) =
                original_text_fields(&conn, id);
            assert_eq!(
                stored_diff_hunk.as_deref(),
                Some(diff_hunk.as_str()),
                "retained unuploadable row {idx} should keep diff_hunk"
            );
            assert_eq!(
                stored_metadata.as_deref(),
                Some(metadata.as_str()),
                "retained unuploadable row {idx} should keep metadata"
            );
            assert_eq!(
                stored_prompt_summary.as_deref(),
                Some(prompt_summary.as_str()),
                "retained unuploadable row {idx} should keep prompt_summary"
            );
        }
    }

    #[test]
    fn prune_local_record_storage_strips_retry_exhausted_pending_text() {
        let conn = open_test_db();
        for idx in 0..(queries::MAX_RETRY_EXHAUSTED_PENDING_RECORD_ROWS + 8) {
            let mut r = make_record(
                "claude",
                &format!("src/retry-exhausted-{idx}.rs"),
                "tok-retry",
            );
            r.diff_hunk = Some(format!("@@ -1 +1 @@\n-old-retry-{idx}\n+new-retry-{idx}"));
            r.metadata = Some(format!("retry raw metadata {idx}"));
            r.prompt_summary = Some(format!("retry raw prompt {idx}"));
            insert_record(&conn, &r).unwrap();
        }
        conn.execute("UPDATE records SET retry_count = 5", [])
            .unwrap();

        prune_local_record_storage(&conn).unwrap();

        assert_eq!(
            record_count_where(&conn, "synced = 0 AND retry_count >= 5"),
            queries::MAX_RETRY_EXHAUSTED_PENDING_RECORD_ROWS
        );
        assert_eq!(
            record_count_where(
                &conn,
                "COALESCE(diff_hunk, '') LIKE '%old-retry%'
                 OR COALESCE(metadata, '') LIKE '%retry raw metadata%'
                 OR COALESCE(prompt_summary, '') LIKE '%retry raw prompt%'"
            ),
            0
        );
    }

    #[test]
    fn clean_all_removes_everything() {
        let conn = open_test_db();
        insert_record(&conn, &make_record("claude", "src/ca1.rs", "tok-ca")).unwrap();
        insert_record(&conn, &make_record("claude", "src/ca2.rs", "tok-ca")).unwrap();

        let deleted = clean_all(&conn).unwrap();
        assert_eq!(deleted, 2);
        assert_eq!(pending_count_all(&conn), 0);
    }

    #[test]
    fn inspect_records_returns_rows() {
        let conn = open_test_db();
        insert_record(
            &conn,
            &make_record("claude", "src/inspect.rs", "tok-inspect"),
        )
        .unwrap();

        let rows = inspect_records(&conn, 10, false, "").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].tool, "claude");
    }

    #[test]
    fn inspect_records_pending_filter() {
        let conn = open_test_db();
        let r = make_record("claude", "src/ip.rs", "tok-ip");
        insert_record(&conn, &r).unwrap();

        let pending = inspect_records(&conn, 10, true, "").unwrap();
        assert_eq!(pending.len(), 1);

        let all = fetch_unsynced(&conn, "tok-ip", 10).unwrap();
        mark_synced(&conn, &[all[0].id]).unwrap();
        let pending_after = inspect_records(&conn, 10, true, "").unwrap();
        assert!(pending_after.is_empty());
    }

    #[test]
    fn inspect_records_token_filter() {
        let conn = open_test_db();
        insert_record(&conn, &make_record("claude", "src/t1.rs", "tok-x")).unwrap();
        insert_record(&conn, &make_record("claude", "src/t2.rs", "tok-y")).unwrap();

        let for_x = inspect_records(&conn, 10, false, "tok-x").unwrap();
        assert_eq!(for_x.len(), 1);
        assert_eq!(for_x[0].token_key, "tok-x");
    }

    #[test]
    fn token_breakdown_groups_by_token() {
        let conn = open_test_db();
        insert_record(&conn, &make_record("claude", "src/b1.rs", "tok-g1")).unwrap();
        insert_record(&conn, &make_record("claude", "src/b2.rs", "tok-g1")).unwrap();
        insert_record(&conn, &make_record("claude", "src/b3.rs", "tok-g2")).unwrap();

        let breakdown = token_breakdown(&conn).unwrap();
        assert_eq!(breakdown.len(), 2);
        let g1 = breakdown.iter().find(|(k, _, _)| k == "tok-g1").unwrap();
        assert_eq!(g1.1, 2);
    }

    #[test]
    fn kv_get_set_last_heartbeat() {
        let conn = open_test_db();
        assert!(get_last_heartbeat(&conn).is_none());

        set_last_heartbeat(&conn, 1234567890).unwrap();
        assert_eq!(get_last_heartbeat(&conn), Some(1234567890));

        set_last_heartbeat(&conn, 9999999999).unwrap();
        assert_eq!(get_last_heartbeat(&conn), Some(9999999999));
    }

    #[test]
    fn empty_ids_mark_synced_is_noop() {
        let conn = open_test_db();
        mark_synced(&conn, &[]).unwrap();
        increment_retry(&conn, &[]).unwrap();
    }

    // ---------------------------------------------------------------------------
    // backfill_repo_info tests
    // ---------------------------------------------------------------------------

    #[test]
    fn backfill_updates_empty_repo_url_records() {
        let conn = open_test_db();
        // Insert a record with empty repo_url (simulates capture outside git repo)
        let mut r = make_record("claude", "src/lib.rs", "tok-bf");
        r.repo_url = "".to_string();
        r.branch = "".to_string();
        r.current_sha = "".to_string();
        insert_record(&conn, &r).unwrap();

        backfill_repo_info(
            &conn,
            "git@github.com:org/repo.git",
            "main",
            "abc123",
            "tok-bf",
        )
        .unwrap();

        let rows = fetch_unsynced(&conn, "tok-bf", 100).unwrap();
        assert_eq!(rows[0].repo_url, "git@github.com:org/repo.git");
        assert_eq!(rows[0].branch, "main");
        assert_eq!(rows[0].current_sha, "abc123");
    }

    #[test]
    fn backfill_does_not_touch_synced_records() {
        let conn = open_test_db();
        let mut r = make_record("claude", "src/main.rs", "tok-syn");
        r.repo_url = "".to_string();
        insert_record(&conn, &r).unwrap();
        // Directly mark the inserted record as synced via SQL so we bypass
        // the fetch_unsynced filter that excludes empty-repo_url records.
        conn.execute("UPDATE records SET synced = 1", []).unwrap();

        backfill_repo_info(
            &conn,
            "git@github.com:org/other.git",
            "dev",
            "def456",
            "tok-syn",
        )
        .unwrap();

        // Query directly — synced records should NOT be updated
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM records WHERE repo_url = 'git@github.com:org/other.git'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "synced records must not be backfilled");
    }

    #[test]
    fn backfill_skips_records_with_nonempty_repo_url() {
        let conn = open_test_db();
        let r = make_record("claude", "src/config.rs", "tok-noemp");
        // r.repo_url is already "git@github.com:org/repo.git" from make_record
        insert_record(&conn, &r).unwrap();

        backfill_repo_info(
            &conn,
            "git@github.com:org/NEW.git",
            "feature",
            "999",
            "tok-noemp",
        )
        .unwrap();

        let rows = fetch_unsynced(&conn, "tok-noemp", 100).unwrap();
        // Should remain unchanged
        assert_eq!(rows[0].repo_url, "git@github.com:org/repo.git");
    }

    #[test]
    fn backfill_empty_database_is_noop() {
        let conn = open_test_db();
        // No records — must not error
        backfill_repo_info(&conn, "git@github.com:org/repo.git", "main", "abc", "").unwrap();
    }
}
