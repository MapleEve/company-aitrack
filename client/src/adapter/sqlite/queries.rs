use anyhow::{Context, Result};
use rusqlite::{params, Connection};

use crate::domain::model::{InspectRow, Record};

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
) -> Result<()> {
    conn.execute(
        "UPDATE records SET repo_url = ?1, branch = ?2, current_sha = ?3
         WHERE synced = 0 AND (repo_url = '' OR repo_url IS NULL)",
        params![repo_url, branch, current_sha],
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
    conn.execute_batch(super::schema::CREATE_KV_TABLE_SQL)?;
    Ok(())
}

/// Get an integer timestamp from the KV store by key.
pub fn get_kv(conn: &Connection, key: &str) -> Option<i64> {
    conn.query_row(
        "SELECT value FROM kv WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
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
    let truncated: String = prompt_text.chars().take(512).collect();
    conn.execute(
        "INSERT INTO prompt_context (session_id, prompt_text) VALUES (?1, ?2)",
        params![session_id, truncated],
    )?;
    Ok(())
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
