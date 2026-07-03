use anyhow::{Context, Result};
use rusqlite::{params, Connection};

use crate::domain::model::{
    sanitize_non_sig_record_fields, truncate_chars, InspectRow, Record, MAX_STORED_PROMPT_CHARS,
};

pub(crate) const MAX_SYNCED_RECORD_ROWS: i64 = 10_000;
pub(crate) const MAX_PENDING_RECORD_ROWS: i64 = 1_000;
pub(crate) const MAX_LOCAL_RECORD_ROWS: i64 = MAX_SYNCED_RECORD_ROWS + MAX_PENDING_RECORD_ROWS;
pub(crate) const MAX_UNUPLOADABLE_PENDING_RECORD_ROWS: i64 = 250;
pub(crate) const MAX_RETRY_EXHAUSTED_PENDING_RECORD_ROWS: i64 = 100;
pub(crate) const MAX_PROMPT_CONTEXT_ROWS: i64 = 256;
const DB_RECLAIM_PRUNE_THRESHOLD_ROWS: usize = 128;
const INCREMENTAL_VACUUM_PAGES: i64 = 512;
const FULL_VACUUM_FREELIST_THRESHOLD_PAGES: i64 = 4096;

const PRUNED_DIFF_HUNK_MARKER: &str = "[aitrack-pruned:diff_hunk]";
const PRUNED_METADATA_MARKER: &str = "{\"aitrack_pruned\":true}";
const PRUNED_PROMPT_MARKER: &str = "[aitrack-pruned:prompt_summary]";

pub fn insert_record(conn: &Connection, r: &Record) -> Result<bool> {
    let mut r = r.clone();
    sanitize_non_sig_record_fields(&mut r);

    // 2-second dedup window for the same logical event. Different agents or
    // metadata variants can legitimately touch the same file with the same diff.
    let is_dup: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM records
         WHERE tool = ?1
           AND file_path = ?2
           AND repo_url = ?3
           AND ((diff_hunk IS NULL AND ?4 IS NULL) OR (diff_hunk = ?4))
           AND ((metadata IS NULL AND ?5 IS NULL) OR (metadata = ?5))
           AND datetime(timestamp) > datetime('now', '-2 seconds')",
        params![r.tool, r.file_path, r.repo_url, r.diff_hunk, r.metadata],
        |row| row.get(0),
    )?;

    if is_dup {
        return Ok(false);
    }

    conn.execute(
        "INSERT INTO records (tool, tool_version, provider, model, session_id,
            repo_url, branch, current_sha, file_path, added_lines, removed_lines,
            diff_hunk, metadata, synced, timestamp, token_key, device_id, hostname, record_sig,
            prompt_summary)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,0,?14,?15,?16,?17,?18,?19)",
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
            r.prompt_summary.clone(),
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
                retry_count, timestamp, token_key, device_id, hostname, record_sig,
                prompt_summary
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
    let diff_marker = ids.len() + 1;
    let metadata_marker = ids.len() + 2;
    let prompt_marker = ids.len() + 3;
    let mut params = ids
        .iter()
        .copied()
        .map(rusqlite::types::Value::Integer)
        .collect::<Vec<_>>();
    params.push(rusqlite::types::Value::Text(
        PRUNED_DIFF_HUNK_MARKER.to_string(),
    ));
    params.push(rusqlite::types::Value::Text(
        PRUNED_METADATA_MARKER.to_string(),
    ));
    params.push(rusqlite::types::Value::Text(
        PRUNED_PROMPT_MARKER.to_string(),
    ));

    conn.execute(
        &format!(
            "UPDATE records
             SET synced = 1,
                 synced_at = datetime('now'),
                 diff_hunk = CASE
                   WHEN diff_hunk IS NULL THEN NULL
                   ELSE ?{diff_marker}
                 END,
                 metadata = CASE
                   WHEN metadata IS NULL THEN NULL
                   ELSE ?{metadata_marker}
                 END,
                 prompt_summary = CASE
                   WHEN prompt_summary IS NULL THEN NULL
                   ELSE ?{prompt_marker}
                 END
             WHERE id IN ({placeholders})"
        ),
        rusqlite::params_from_iter(params),
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

/// Backfill repo metadata into unsynced records whose `repo_url` is empty.
///
/// Called after every capture when the current git context is valid.
/// Unsynced records with an empty `repo_url` were captured before git info
/// was available (e.g. outside a repo, or git shelled out and returned "").
/// Backfilling allows them to be picked up by `fetch_unsynced` which filters
/// `repo_url != ''`. Synced records are never touched.
pub fn backfill_repo_info(
    conn: &Connection,
    repo_url: &str,
    branch: &str,
    current_sha: &str,
    token_key: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE records SET repo_url = ?1, branch = ?2, current_sha = ?3
         WHERE synced = 0 AND (repo_url = '' OR repo_url IS NULL) AND token_key = ?4",
        params![repo_url, branch, current_sha, token_key],
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
    conn.query_row("SELECT COUNT(*) FROM records WHERE synced = 0", [], |row| {
        row.get(0)
    })
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
        (true, true) => {
            format!("{base} WHERE synced = 0 AND token_key = ?1 ORDER BY id DESC LIMIT ?2")
        }
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
    reclaim_after_prune(conn, n)?;
    Ok(n)
}

pub fn prune_old_synced_records(conn: &Connection, keep_latest_synced: i64) -> Result<usize> {
    if keep_latest_synced <= 0 {
        return Ok(0);
    }

    let current: i64 =
        conn.query_row("SELECT COUNT(*) FROM records WHERE synced = 1", [], |row| {
            row.get(0)
        })?;
    let overage = current.saturating_sub(keep_latest_synced);
    if overage <= 0 {
        return Ok(0);
    }

    let n = conn.execute(
        "DELETE FROM records
         WHERE id IN (
           SELECT id FROM records
           WHERE synced = 1
           ORDER BY id ASC
           LIMIT ?1
         )",
        params![overage],
    )?;
    Ok(n)
}

pub fn prune_local_record_storage(conn: &Connection) -> Result<usize> {
    let mut changed = 0usize;

    changed += prune_old_synced_records(conn, MAX_SYNCED_RECORD_ROWS)?;
    changed += prune_records_matching(
        conn,
        "synced = 0 AND retry_count >= 5",
        MAX_RETRY_EXHAUSTED_PENDING_RECORD_ROWS,
    )?;
    changed += prune_records_matching(
        conn,
        "synced = 0 AND (token_key = '' OR token_key = 'legacy' OR record_sig = '')",
        MAX_UNUPLOADABLE_PENDING_RECORD_ROWS,
    )?;
    changed += strip_retry_exhausted_pending_original_text(conn)?;
    changed += prune_records_matching(conn, "synced = 0", MAX_PENDING_RECORD_ROWS)?;
    changed += prune_records_to_total_limit(conn, MAX_LOCAL_RECORD_ROWS)?;
    changed += prune_prompt_context(conn)?;

    reclaim_after_prune(conn, changed)?;
    Ok(changed)
}

fn prune_records_matching(
    conn: &Connection,
    where_clause: &'static str,
    keep_latest: i64,
) -> Result<usize> {
    if keep_latest < 0 {
        return Ok(0);
    }

    let current: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM records WHERE {where_clause}"),
        [],
        |row| row.get(0),
    )?;
    let overage = current.saturating_sub(keep_latest);
    if overage <= 0 {
        return Ok(0);
    }

    let deleted = conn.execute(
        &format!(
            "DELETE FROM records
             WHERE id IN (
               SELECT id FROM records
               WHERE {where_clause}
               ORDER BY id ASC
               LIMIT ?1
             )"
        ),
        params![overage],
    )?;
    Ok(deleted)
}

fn strip_retry_exhausted_pending_original_text(conn: &Connection) -> Result<usize> {
    let stripped = conn.execute(
        "UPDATE records
         SET diff_hunk = CASE
               WHEN diff_hunk IS NULL THEN NULL
               ELSE ?1
             END,
             metadata = CASE
               WHEN metadata IS NULL THEN NULL
               ELSE ?2
             END,
             prompt_summary = CASE
               WHEN prompt_summary IS NULL THEN NULL
               ELSE ?3
             END
         WHERE synced = 0
           AND retry_count >= 5
           AND (
             (diff_hunk IS NOT NULL AND diff_hunk != ?1)
             OR (metadata IS NOT NULL AND metadata != ?2)
             OR (prompt_summary IS NOT NULL AND prompt_summary != ?3)
           )",
        params![
            PRUNED_DIFF_HUNK_MARKER,
            PRUNED_METADATA_MARKER,
            PRUNED_PROMPT_MARKER
        ],
    )?;
    Ok(stripped)
}

fn prune_records_to_total_limit(conn: &Connection, keep_latest: i64) -> Result<usize> {
    if keep_latest <= 0 {
        return Ok(0);
    }

    let current: i64 = conn.query_row("SELECT COUNT(*) FROM records", [], |row| row.get(0))?;
    let overage = current.saturating_sub(keep_latest);
    if overage <= 0 {
        return Ok(0);
    }

    let deleted = conn.execute(
        "DELETE FROM records
         WHERE id IN (
           SELECT id FROM records
           ORDER BY synced DESC, retry_count DESC, id ASC
           LIMIT ?1
         )",
        params![overage],
    )?;
    Ok(deleted)
}

pub fn clean_all(conn: &Connection) -> Result<usize> {
    let n = conn.execute("DELETE FROM records", [])?;
    reclaim_after_prune(conn, n)?;
    Ok(n)
}

pub(crate) fn reclaim_after_prune(conn: &Connection, deleted_rows: usize) -> Result<()> {
    if deleted_rows < DB_RECLAIM_PRUNE_THRESHOLD_ROWS || !conn.is_autocommit() {
        return Ok(());
    }

    let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
    let vacuum_sql = format!("PRAGMA incremental_vacuum({INCREMENTAL_VACUUM_PAGES});");
    let _ = conn.execute_batch(&vacuum_sql);

    let freelist_pages: i64 = conn
        .query_row("PRAGMA freelist_count", [], |row| row.get(0))
        .unwrap_or(0);
    if freelist_pages >= FULL_VACUUM_FREELIST_THRESHOLD_PAGES {
        let _ = conn.execute_batch("VACUUM;");
    }
    Ok(())
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
    conn.execute_batch(super::schema::CREATE_KV_TABLE_SQL)?;
    Ok(())
}

/// Get an integer timestamp from the KV store by key.
pub fn get_kv(conn: &Connection, key: &str) -> Option<i64> {
    conn.query_row("SELECT value FROM kv WHERE key = ?1", params![key], |row| {
        row.get(0)
    })
    .ok()
}

/// Set an integer timestamp in the KV store (upsert).
pub fn set_kv(conn: &Connection, key: &str, value: i64) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO kv (key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}

pub fn ensure_prompt_context_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(super::schema::CREATE_PROMPT_CONTEXT_TABLE_SQL)?;
    Ok(())
}

pub fn insert_prompt_context(conn: &Connection, session_id: &str, prompt_text: &str) -> Result<()> {
    let truncated = truncate_chars(prompt_text, MAX_STORED_PROMPT_CHARS);
    conn.execute(
        "INSERT INTO prompt_context (session_id, prompt_text) VALUES (?1, ?2)",
        params![session_id, truncated],
    )?;
    prune_prompt_context(conn)?;
    Ok(())
}

fn prune_prompt_context(conn: &Connection) -> Result<usize> {
    let current: i64 =
        conn.query_row("SELECT COUNT(*) FROM prompt_context", [], |row| row.get(0))?;
    let overage = current.saturating_sub(MAX_PROMPT_CONTEXT_ROWS);
    if overage <= 0 {
        return Ok(0);
    }
    let deleted = conn.execute(
        "DELETE FROM prompt_context
         WHERE rowid IN (
           SELECT rowid FROM prompt_context
           ORDER BY rowid ASC
           LIMIT ?1
         )",
        params![overage],
    )?;
    Ok(deleted)
}

pub fn get_recent_prompt(conn: &Connection, session_id: &str) -> Option<String> {
    conn.query_row(
        "SELECT prompt_text FROM prompt_context WHERE session_id = ?1 ORDER BY created_at DESC LIMIT 1",
        params![session_id],
        |row| row.get(0),
    ).ok()
}

// ---------------------------------------------------------------------------
// Private row mappers
// ---------------------------------------------------------------------------

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
        prompt_summary: row.get(22)?,
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
    use rusqlite::Connection;

    use super::*;

    fn open_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(super::super::schema::CREATE_TABLE_SQL)
            .unwrap();
        ensure_kv_table(&conn).unwrap();
        ensure_prompt_context_table(&conn).unwrap();
        conn
    }

    #[test]
    fn schema_batch_applies_size_maintenance_pragmas() {
        let conn = open_test_db();
        let auto_vacuum: i64 = conn
            .query_row("PRAGMA auto_vacuum", [], |row| row.get(0))
            .unwrap();
        let journal_size_limit: i64 = conn
            .query_row("PRAGMA journal_size_limit", [], |row| row.get(0))
            .unwrap();
        let busy_timeout: i64 = conn
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .unwrap();

        assert_eq!(auto_vacuum, 2);
        assert_eq!(journal_size_limit, 67_108_864);
        assert_eq!(busy_timeout, 5_000);
    }

    #[test]
    fn reclaim_after_prune_skips_tiny_deletes_and_accepts_large_deletes() {
        let conn = open_test_db();

        reclaim_after_prune(&conn, 1).unwrap();
        reclaim_after_prune(&conn, DB_RECLAIM_PRUNE_THRESHOLD_ROWS).unwrap();
    }
}
