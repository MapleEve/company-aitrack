pub mod keyword_store;
pub mod queries;
pub mod schema;
pub mod vec;

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::fs;
use std::fs::OpenOptions;
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
    pending_count_all, set_kv, set_last_heartbeat, token_breakdown,
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
    let _ = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path);

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
            record_sig: "sigxyz".to_string(),
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
    fn dedup_window_prevents_second_insert() {
        let conn = open_test_db();
        let r = make_record("claude", "src/dup.rs", "tok-dup");
        let ins1 = insert_record(&conn, &r).unwrap();
        assert!(ins1);
        let ins2 = insert_record(&conn, &r).unwrap();
        assert!(!ins2, "duplicate within 2s should be rejected");
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
}
