use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::config::db_path;

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

#[derive(Debug, Clone)]
pub struct Record {
    pub id: i64,
    pub tool: String,
    pub tool_version: Option<String>,
    pub provider: String,
    pub model: Option<String>,
    pub session_id: String,
    pub repo_url: String,
    pub branch: String,
    pub current_sha: String,
    pub file_path: String,
    pub added_lines: i64,
    pub removed_lines: i64,
    pub diff_hunk: Option<String>,
    pub metadata: Option<String>,
    #[allow(dead_code)]
    pub synced: i64,
    #[allow(dead_code)]
    pub synced_at: Option<String>,
    #[allow(dead_code)]
    pub retry_count: i64,
    pub timestamp: String,
    pub token_key: String,
    pub device_id: String,
    pub hostname: String,
    pub record_sig: String,
}

pub fn open_db() -> Result<Connection> {
    let path = db_path();
    let dir = path.parent().unwrap();
    fs::create_dir_all(dir).context("create ~/.aitrack")?;

    let is_new = !path.exists();
    let conn = Connection::open(&path).context("open records.db")?;

    if is_new {
        let mut perms = fs::metadata(&path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&path, perms).context("chmod records.db")?;
    }

    conn.execute_batch(CREATE_TABLE_SQL)
        .context("create records table")?;

    // idempotent migrations
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

    Ok(conn)
}

pub fn insert_record(conn: &Connection, r: &Record) -> Result<bool> {
    // 2-second dedup window
    let is_dup: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM records
         WHERE file_path = ?1 AND repo_url = ?2
           AND ((diff_hunk IS NULL AND ?3 IS NULL) OR (diff_hunk = ?3))
           AND datetime(timestamp) > datetime('now', '-2 seconds')",
        params![r.file_path, r.repo_url, r.diff_hunk],
        |row| row.get(0),
    )?;

    if is_dup {
        return Ok(false);
    }

    conn.execute(
        "INSERT INTO records (tool, tool_version, provider, model, session_id,
            repo_url, branch, current_sha, file_path, added_lines, removed_lines,
            diff_hunk, metadata, synced, timestamp, token_key, device_id, hostname, record_sig)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,0,?14,?15,?16,?17,?18)",
        params![
            r.tool,
            r.tool_version,
            r.provider,
            r.model,
            r.session_id,
            r.repo_url,
            r.branch,
            r.current_sha,
            r.file_path,
            r.added_lines,
            r.removed_lines,
            r.diff_hunk,
            r.metadata,
            r.timestamp,
            r.token_key,
            r.device_id,
            r.hostname,
            r.record_sig,
        ],
    )
    .context("insert record")?;
    Ok(true)
}

pub fn fetch_unsynced(conn: &Connection, token_key: &str, limit: i64) -> Result<Vec<Record>> {
    let mut stmt = conn.prepare(
        "SELECT id, tool, tool_version, provider, model, session_id,
                repo_url, branch, current_sha, file_path, added_lines,
                removed_lines, diff_hunk, metadata, synced, synced_at,
                retry_count, timestamp, token_key, device_id, hostname, record_sig
         FROM records
         WHERE synced = 0 AND repo_url != '' AND token_key = ?1
           AND retry_count < 5
         ORDER BY id LIMIT ?2",
    )?;

    let rows = stmt.query_map(params![token_key, limit], map_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn mark_synced(conn: &Connection, ids: &[i64]) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let placeholders = (1..=ids.len())
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(",");
    conn.execute(
        &format!(
            "UPDATE records SET synced = 1, synced_at = datetime('now') WHERE id IN ({placeholders})"
        ),
        rusqlite::params_from_iter(ids),
    )?;
    Ok(())
}

pub fn increment_retry(conn: &Connection, ids: &[i64]) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let placeholders = (1..=ids.len())
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(",");
    conn.execute(
        &format!("UPDATE records SET retry_count = retry_count + 1 WHERE id IN ({placeholders})"),
        rusqlite::params_from_iter(ids),
    )?;
    Ok(())
}

pub fn pending_count(conn: &Connection, token_key: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM records WHERE synced = 0 AND token_key = ?1",
        params![token_key],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

pub fn pending_count_all(conn: &Connection) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM records WHERE synced = 0",
        [],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

pub fn inspect_records(
    conn: &Connection,
    limit: i64,
    pending_only: bool,
    token_key: &str,
) -> Result<Vec<InspectRow>> {
    let base = "SELECT id, tool, model, file_path, added_lines, removed_lines, \
                synced, retry_count, token_key, timestamp FROM records";

    let sql = match (pending_only, !token_key.is_empty()) {
        (true, true) => format!(
            "{base} WHERE synced = 0 AND token_key = ?1 ORDER BY id DESC LIMIT ?2"
        ),
        (true, false) => format!("{base} WHERE synced = 0 ORDER BY id DESC LIMIT ?1"),
        (false, true) => format!("{base} WHERE token_key = ?1 ORDER BY id DESC LIMIT ?2"),
        (false, false) => format!("{base} ORDER BY id DESC LIMIT ?1"),
    };

    let mut stmt = conn.prepare(&sql)?;

    let rows: Vec<InspectRow> = match (pending_only, !token_key.is_empty()) {
        (_, true) => stmt
            .query_map(params![token_key, limit], map_inspect_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?,
        _ => stmt
            .query_map(params![limit], map_inspect_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?,
    };

    Ok(rows)
}

pub fn token_breakdown(conn: &Connection) -> Result<Vec<(String, i64, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT token_key, COUNT(*), SUM(CASE WHEN synced = 0 THEN 1 ELSE 0 END)
         FROM records GROUP BY token_key ORDER BY token_key",
    )?;
    let rows = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn clean_synced(conn: &Connection) -> Result<usize> {
    let n = conn.execute("DELETE FROM records WHERE synced = 1", [])?;
    Ok(n)
}

pub fn clean_all(conn: &Connection) -> Result<usize> {
    let n = conn.execute("DELETE FROM records", [])?;
    Ok(n)
}

pub fn get_last_heartbeat(conn: &Connection) -> Option<i64> {
    conn.query_row(
        "SELECT value FROM kv WHERE key = 'last_heartbeat_ts'",
        [],
        |row| row.get(0),
    )
    .ok()
}

pub fn set_last_heartbeat(conn: &Connection, ts: i64) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO kv (key, value) VALUES ('last_heartbeat_ts', ?1)",
        params![ts],
    )?;
    Ok(())
}

pub fn ensure_kv_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS kv (key TEXT PRIMARY KEY, value INTEGER NOT NULL);",
    )?;
    Ok(())
}

#[derive(Debug)]
pub struct InspectRow {
    pub id: i64,
    pub tool: String,
    pub model: Option<String>,
    pub file_path: String,
    pub added_lines: i64,
    pub removed_lines: i64,
    pub synced: i64,
    pub retry_count: i64,
    #[allow(dead_code)]
    pub token_key: String,
    pub timestamp: String,
}

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<Record> {
    Ok(Record {
        id: row.get(0)?,
        tool: row.get(1)?,
        tool_version: row.get(2)?,
        provider: row.get(3)?,
        model: row.get(4)?,
        session_id: row.get(5)?,
        repo_url: row.get(6)?,
        branch: row.get(7)?,
        current_sha: row.get(8)?,
        file_path: row.get(9)?,
        added_lines: row.get(10)?,
        removed_lines: row.get(11)?,
        diff_hunk: row.get(12)?,
        metadata: row.get(13)?,
        synced: row.get(14)?,
        synced_at: row.get(15)?,
        retry_count: row.get(16)?,
        timestamp: row.get(17)?,
        token_key: row.get(18)?,
        device_id: row.get(19)?,
        hostname: row.get(20)?,
        record_sig: row.get(21)?,
    })
}

fn map_inspect_row(row: &rusqlite::Row) -> rusqlite::Result<InspectRow> {
    Ok(InspectRow {
        id: row.get(0)?,
        tool: row.get(1)?,
        model: row.get(2)?,
        file_path: row.get(3)?,
        added_lines: row.get(4)?,
        removed_lines: row.get(5)?,
        synced: row.get(6)?,
        retry_count: row.get(7)?,
        token_key: row.get(8)?,
        timestamp: row.get(9)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
        ensure_kv_table(&conn).unwrap();
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
        // Second identical insert within 2-second window
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

        // Increment retry 5 times
        for _ in 0..5 {
            increment_retry(&conn, &[id]).unwrap();
        }

        // fetch_unsynced filters retry_count < 5
        let after = fetch_unsynced(&conn, "tok-retry", 100).unwrap();
        assert!(after.is_empty(), "retry_count=5 should be excluded from fetch");
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

        // Mark one as synced
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
        insert_record(&conn, &make_record("claude", "src/inspect.rs", "tok-inspect")).unwrap();

        let rows = inspect_records(&conn, 10, false, "").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].tool, "claude");
    }

    #[test]
    fn inspect_records_pending_filter() {
        let conn = open_test_db();
        let r = make_record("claude", "src/ip.rs", "tok-ip");
        insert_record(&conn, &r).unwrap();

        // Pending-only: 1 record
        let pending = inspect_records(&conn, 10, true, "").unwrap();
        assert_eq!(pending.len(), 1);

        // Mark synced, now pending-only returns 0
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

        // Overwrite
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
