use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use rusqlite::types::ValueRef;
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::fs::OpenOptions;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use crate::adapter::http::upload::HttpUploader;
use crate::adapter::sqlite::{open_db as open_records_db, pending_count_all};
use crate::agent;
use crate::config::{load_config, mask_token, resolve_api_config, split_credential, usage_db_path};
use crate::domain::crypto::{compute_record_sig, compute_request_sig};
use crate::domain::diff::compute_diff;
use crate::domain::model::Record;
use crate::{git, uploader};

const MAX_UPLOAD_ITEMS: i64 = 500;
const MAX_SCAN_FILES_PER_AGENT: usize = 2500;
const MAX_JSON_BYTES: u64 = 16 * 1024 * 1024;
const MAX_SQLITE_ROWS_PER_TABLE: usize = 2000;
const MAX_EVENTS_PER_FILE: usize = 1000;
const MAX_CAPTURE_TEXT: usize = 4096;

#[derive(Debug, Clone, Default, Serialize)]
pub struct UsageScanReport {
    pub parsed_messages: usize,
    pub monitoring_events_parsed: usize,
    pub sessions_inserted: usize,
    pub monitoring_records_inserted: usize,
    pub rollups_upserted: usize,
    pub subscription_snapshots_upserted: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct UsageSyncReport {
    pub scan: UsageScanReport,
    pub enqueued_rollups: usize,
    pub enqueued_subscriptions: usize,
    pub uploaded: usize,
    pub failed: usize,
    pub pending: i64,
    pub pending_monitoring_events: i64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct UsageStatus {
    pub sessions: i64,
    pub rollups: i64,
    pub subscription_snapshots: i64,
    pub pending_outbox: i64,
    pub pending_monitoring_events: i64,
}

#[derive(Debug, Clone)]
pub struct UsageScanOptions {
    pub tools: Vec<String>,
    pub since: Option<String>,
    pub until: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UsageSyncOptions {
    pub scan: UsageScanOptions,
    pub api_url: Option<String>,
    pub credential: Option<String>,
}

#[derive(Debug, Clone)]
struct UsageSessionRow {
    source: String,
    session_hash: String,
    dedup_hash: String,
    timestamp_ms: i64,
    day: String,
    agent: String,
    model: String,
    provider: String,
    account: String,
    tokens_in: i64,
    tokens_out: i64,
    tokens_cache_read: i64,
    tokens_cache_write: i64,
    tokens_reasoning: i64,
    message_count: i64,
    source_cost: f64,
}

#[derive(Debug, Clone)]
struct LocalUsageMessage {
    agent: String,
    provider: String,
    model: String,
    session_id: String,
    account: String,
    timestamp_ms: i64,
    day: String,
    tokens: TokenBuckets,
    message_count: i64,
    source_cost: f64,
    dedup_key: String,
}

#[derive(Debug, Clone, Default)]
struct TokenBuckets {
    input: i64,
    output: i64,
    cache_read: i64,
    cache_write: i64,
    reasoning: i64,
}

impl TokenBuckets {
    fn total(&self) -> i64 {
        self.input + self.output + self.cache_read + self.cache_write + self.reasoning
    }
}

#[derive(Debug, Clone)]
struct MonitoringEvent {
    source_key: String,
    agent: String,
    provider: String,
    model: Option<String>,
    session_id: String,
    timestamp_ms: i64,
    event_type: String,
    prompt_text: Option<String>,
    file_path: String,
    repo_url: String,
    branch: String,
    current_sha: String,
    added_lines: i64,
    removed_lines: i64,
    diff_hunk: Option<String>,
    metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RollupItem {
    device_id: String,
    day: String,
    agent: String,
    model: String,
    account: String,
    tokens_in: i64,
    tokens_out: i64,
    tokens_cache_read: i64,
    tokens_cache_write: i64,
    tokens_reasoning: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RollupPayload {
    items: Vec<RollupItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SubscriptionPayload {
    device_id: String,
    agent: String,
    account: String,
    plan: Option<String>,
    quota_session_remaining: Option<i64>,
    quota_weekly_remaining: Option<i64>,
    quota_reset_at: Option<String>,
    reader_status: String,
    snapshotted_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutboxType {
    Rollup,
    Subscription,
}

impl OutboxType {
    fn as_str(self) -> &'static str {
        match self {
            OutboxType::Rollup => "rollup",
            OutboxType::Subscription => "subscription",
        }
    }

    fn endpoint(self) -> &'static str {
        match self {
            OutboxType::Rollup => "/api/v1/ai-track/usage/rollup",
            OutboxType::Subscription => "/api/v1/ai-track/usage/subscription",
        }
    }

    fn from_str(raw: &str) -> Option<Self> {
        match raw {
            "rollup" => Some(OutboxType::Rollup),
            "subscription" => Some(OutboxType::Subscription),
            _ => None,
        }
    }
}

struct OutboxItem {
    id: i64,
    payload_type: OutboxType,
    payload_json: String,
}

#[derive(Default)]
struct ScanResult {
    messages: Vec<LocalUsageMessage>,
    events: Vec<MonitoringEvent>,
}

pub async fn scan_now(options: UsageScanOptions) -> Result<UsageScanReport> {
    let mut conn = open_usage_db()?;
    scan_into(&mut conn, options, None).await
}

pub async fn sync_now(options: UsageSyncOptions) -> Result<UsageSyncReport> {
    let (api_url, credential) =
        resolve_api_config(options.api_url.clone(), options.credential.clone());

    let mut conn = open_usage_db()?;
    let scan = scan_into(
        &mut conn,
        options.scan,
        if credential.is_empty() {
            None
        } else {
            Some(credential.as_str())
        },
    )
    .await?;

    let cfg = load_config();
    let enqueued_rollups = enqueue_dirty_rollups(&conn, &cfg.device_id)?;
    let enqueued_subscriptions = enqueue_dirty_subscriptions(&conn)?;

    if api_url.is_empty() || credential.is_empty() {
        return Ok(UsageSyncReport {
            scan,
            enqueued_rollups,
            enqueued_subscriptions,
            pending: pending_outbox(&conn)?,
            pending_monitoring_events: pending_count_all(&open_records_db()?),
            ..UsageSyncReport::default()
        });
    }

    let drain = drain_outbox(&conn, &api_url, &credential, &cfg.device_id).await?;
    let records = open_records_db()?;
    let edit_uploader = HttpUploader::new(api_url, credential);
    uploader::flush_unsynced(&records, &edit_uploader).await?;

    Ok(UsageSyncReport {
        scan,
        enqueued_rollups,
        enqueued_subscriptions,
        uploaded: drain.uploaded,
        failed: drain.failed,
        pending: pending_outbox(&conn)?,
        pending_monitoring_events: pending_count_all(&records),
    })
}

pub fn status() -> Result<UsageStatus> {
    let conn = open_usage_db()?;
    let records = open_records_db()?;
    Ok(UsageStatus {
        sessions: count_table(&conn, "usage_sessions")?,
        rollups: count_table(&conn, "usage_daily_model_rollups")?,
        subscription_snapshots: count_table(&conn, "usage_subscription_snapshots")?,
        pending_outbox: pending_outbox(&conn)?,
        pending_monitoring_events: pending_count_all(&records),
    })
}

async fn scan_into(
    conn: &mut Connection,
    options: UsageScanOptions,
    credential_override: Option<&str>,
) -> Result<UsageScanReport> {
    let scan = scan_local_sources(&options)?;
    let inserted = insert_usage_sessions(conn, &scan.messages)?;
    let monitoring_inserted = insert_monitoring_events(conn, &scan.events, credential_override)?;
    let rollups = rebuild_rollups(conn)?;
    let subscriptions = upsert_local_subscription_snapshots(conn)?;

    Ok(UsageScanReport {
        parsed_messages: scan.messages.len(),
        monitoring_events_parsed: scan.events.len(),
        sessions_inserted: inserted,
        monitoring_records_inserted: monitoring_inserted,
        rollups_upserted: rollups,
        subscription_snapshots_upserted: subscriptions,
    })
}

fn open_usage_db() -> Result<Connection> {
    let path = usage_db_path();
    let dir = path.parent().context("usage db has no parent")?;
    fs::create_dir_all(dir).context("create ~/.aitrack")?;

    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        opts.mode(0o600);
    }
    let _ = opts.open(&path);

    let conn = Connection::open(&path).context("open usage.sqlite")?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "busy_timeout", 5000)?;
    conn.execute_batch(SCHEMA).context("create usage schema")?;
    Ok(conn)
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS usage_sessions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  source TEXT NOT NULL,
  session_hash TEXT NOT NULL,
  dedup_hash TEXT NOT NULL UNIQUE,
  timestamp_ms INTEGER NOT NULL,
  day TEXT NOT NULL,
  agent TEXT NOT NULL,
  model TEXT NOT NULL,
  provider TEXT NOT NULL,
  account TEXT NOT NULL DEFAULT '',
  tokens_in INTEGER NOT NULL DEFAULT 0,
  tokens_out INTEGER NOT NULL DEFAULT 0,
  tokens_cache_read INTEGER NOT NULL DEFAULT 0,
  tokens_cache_write INTEGER NOT NULL DEFAULT 0,
  tokens_reasoning INTEGER NOT NULL DEFAULT 0,
  message_count INTEGER NOT NULL DEFAULT 0,
  source_cost REAL NOT NULL DEFAULT 0,
  inserted_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_usage_sessions_day ON usage_sessions(day);
CREATE INDEX IF NOT EXISTS idx_usage_sessions_agent ON usage_sessions(agent);

CREATE TABLE IF NOT EXISTS usage_daily_model_rollups (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  device_id TEXT NOT NULL,
  day TEXT NOT NULL,
  agent TEXT NOT NULL,
  model TEXT NOT NULL,
  account TEXT NOT NULL DEFAULT '',
  tokens_in INTEGER NOT NULL DEFAULT 0,
  tokens_out INTEGER NOT NULL DEFAULT 0,
  tokens_cache_read INTEGER NOT NULL DEFAULT 0,
  tokens_cache_write INTEGER NOT NULL DEFAULT 0,
  tokens_reasoning INTEGER NOT NULL DEFAULT 0,
  message_count INTEGER NOT NULL DEFAULT 0,
  dirty INTEGER NOT NULL DEFAULT 1,
  uploaded_at TEXT,
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE(device_id, day, agent, model, account)
);
CREATE INDEX IF NOT EXISTS idx_usage_rollups_dirty ON usage_daily_model_rollups(dirty);

CREATE TABLE IF NOT EXISTS usage_subscription_snapshots (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  device_id TEXT NOT NULL,
  agent TEXT NOT NULL,
  account TEXT NOT NULL DEFAULT '',
  plan TEXT,
  quota_session_remaining INTEGER,
  quota_weekly_remaining INTEGER,
  quota_reset_at TEXT,
  reader_status TEXT NOT NULL,
  snapshotted_at TEXT NOT NULL,
  dirty INTEGER NOT NULL DEFAULT 1,
  uploaded_at TEXT,
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE(device_id, agent, account)
);
CREATE INDEX IF NOT EXISTS idx_usage_subscriptions_dirty ON usage_subscription_snapshots(dirty);

CREATE TABLE IF NOT EXISTS usage_outbox (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  payload_type TEXT NOT NULL,
  natural_key TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  payload_sha256 TEXT NOT NULL,
  synced INTEGER NOT NULL DEFAULT 0,
  synced_at TEXT,
  retry_count INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE(payload_type, natural_key, payload_sha256)
);
CREATE INDEX IF NOT EXISTS idx_usage_outbox_pending ON usage_outbox(synced, retry_count, id);

CREATE TABLE IF NOT EXISTS usage_monitoring_seen (
  source_key TEXT PRIMARY KEY,
  event_type TEXT NOT NULL,
  inserted_at TEXT NOT NULL DEFAULT (datetime('now'))
);
"#;

fn scan_local_sources(options: &UsageScanOptions) -> Result<ScanResult> {
    let home = scan_home().context("cannot find home directory")?;
    let tools = selected_scan_tools(&options.tools);
    let since = options.since.as_deref();
    let until = options.until.as_deref();

    let mut result = ScanResult::default();
    for tool in tools {
        let roots = scan_roots(&home, &tool);
        let mut files = Vec::new();
        let mut seen_paths = HashSet::new();
        for root in roots {
            collect_supported_files(&root, &mut files, &mut seen_paths);
            if files.len() >= MAX_SCAN_FILES_PER_AGENT {
                break;
            }
        }
        files.sort();
        files.truncate(MAX_SCAN_FILES_PER_AGENT);

        for file in files {
            let mut chunk = scan_source_file(&tool, &file)?;
            chunk
                .messages
                .retain(|m| day_in_range(&m.day, since, until));
            chunk.events.retain(|e| {
                let day = day_from_timestamp_ms(e.timestamp_ms);
                day_in_range(&day, since, until)
            });
            result.messages.extend(chunk.messages);
            result.events.extend(chunk.events);
        }
    }

    dedup_messages(&mut result.messages);
    dedup_events(&mut result.events);
    Ok(result)
}

fn scan_home() -> Option<PathBuf> {
    std::env::var("AITRACK_SCAN_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(dirs::home_dir)
}

fn selected_scan_tools(raw: &[String]) -> Vec<String> {
    if raw.is_empty() {
        return agent::registered_agents()
            .iter()
            .map(|a| a.name.to_string())
            .collect();
    }
    let mut out = Vec::new();
    for item in raw {
        let name = item.trim().to_ascii_lowercase();
        if !name.is_empty() && !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

fn scan_roots(home: &Path, tool: &str) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(registered) = agent::agent_by_name(tool) {
        roots.push(registered.marker_path(home));
    } else {
        roots.push(home.join(format!(".{tool}")));
    }

    match tool {
        "claude" => {
            roots.push(home.join(".claude").join("projects"));
            roots.push(home.join(".claude").join("transcripts"));
        }
        "codex" => {
            roots.push(home.join(".codex").join("sessions"));
        }
        "cursor" => {
            roots.push(home.join("Library/Application Support/Cursor/User/globalStorage"));
            roots.push(home.join(".config/Cursor/User/globalStorage"));
        }
        "trae" => {
            roots.push(home.join("Library/Application Support/Trae"));
            roots.push(home.join(".config/Trae"));
        }
        "opencode" => {
            roots.push(home.join(".local/share/opencode"));
            roots.push(home.join(".config/opencode"));
        }
        _ => {}
    }

    roots.push(crate::config::config_dir().join("sources").join(tool));
    roots.push(crate::config::config_dir().join("cache").join(tool));
    roots
}

fn collect_supported_files(root: &Path, out: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>) {
    if out.len() >= MAX_SCAN_FILES_PER_AGENT || !root.exists() {
        return;
    }
    let Ok(meta) = fs::metadata(root) else {
        return;
    };
    if meta.is_file() {
        if is_supported_file(root) && seen.insert(root.to_path_buf()) {
            out.push(root.to_path_buf());
        }
        return;
    }
    if skip_dir(root) {
        return;
    }

    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        if out.len() >= MAX_SCAN_FILES_PER_AGENT {
            break;
        }
        collect_supported_files(&entry.path(), out, seen);
    }
}

fn skip_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    matches!(
        name,
        ".git" | "node_modules" | "target" | "Cache" | "Caches" | "CachedData"
    )
}

fn is_supported_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|s| s.to_str()),
        Some("json")
            | Some("jsonl")
            | Some("ndjson")
            | Some("log")
            | Some("csv")
            | Some("db")
            | Some("sqlite")
            | Some("sqlite3")
    )
}

fn scan_source_file(tool: &str, path: &Path) -> Result<ScanResult> {
    let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
        return Ok(ScanResult::default());
    };
    match ext {
        "csv" => scan_csv_file(tool, path),
        "db" | "sqlite" | "sqlite3" => scan_sqlite_file(tool, path),
        _ => scan_text_json_file(tool, path),
    }
}

fn scan_text_json_file(tool: &str, path: &Path) -> Result<ScanResult> {
    let meta = fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
    if meta.len() > MAX_JSON_BYTES {
        return Ok(ScanResult::default());
    }
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let modified_ms = meta
        .modified()
        .ok()
        .and_then(system_time_ms)
        .unwrap_or_else(now_ms);

    let mut result = ScanResult::default();
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    if matches!(ext, "jsonl" | "ndjson" | "log") {
        for (idx, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || !trimmed.starts_with(['{', '[']) {
                continue;
            }
            if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
                collect_from_json_value(
                    tool,
                    path,
                    &format!("line:{idx}"),
                    &value,
                    modified_ms,
                    &mut result,
                );
            }
        }
    } else if let Ok(value) = serde_json::from_str::<Value>(&text) {
        collect_from_json_value(tool, path, "json", &value, modified_ms, &mut result);
    }
    Ok(result)
}

fn scan_csv_file(tool: &str, path: &Path) -> Result<ScanResult> {
    let meta = fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
    if meta.len() > MAX_JSON_BYTES {
        return Ok(ScanResult::default());
    }
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut lines = text.lines();
    let Some(header_line) = lines.next() else {
        return Ok(ScanResult::default());
    };
    let headers: Vec<String> = split_csv_line(header_line)
        .into_iter()
        .map(|h| h.trim().to_ascii_lowercase())
        .collect();

    let mut result = ScanResult::default();
    for (idx, line) in lines.enumerate() {
        let values = split_csv_line(line);
        if values.is_empty() {
            continue;
        }
        let row = headers
            .iter()
            .cloned()
            .zip(values)
            .collect::<HashMap<_, _>>();
        if let Some(msg) = usage_from_csv(tool, path, idx, &row) {
            result.messages.push(msg);
        }
        if let Some(event) = event_from_csv(tool, path, idx, &row) {
            result.events.push(event);
        }
    }
    Ok(result)
}

fn scan_sqlite_file(tool: &str, path: &Path) -> Result<ScanResult> {
    let meta = fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
    if meta.len() > 128 * 1024 * 1024 {
        return Ok(ScanResult::default());
    }
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let Ok(conn) = Connection::open_with_flags(path, flags) else {
        return Ok(ScanResult::default());
    };
    let mut result = ScanResult::default();
    let tables = sqlite_tables(&conn)?;
    for table in tables.into_iter().take(50) {
        scan_sqlite_table(tool, path, &conn, &table, &mut result)?;
    }
    Ok(result)
}

fn sqlite_tables(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
    )?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn scan_sqlite_table(
    tool: &str,
    path: &Path,
    conn: &Connection,
    table: &str,
    result: &mut ScanResult,
) -> Result<()> {
    let columns = sqlite_columns(conn, table)?;
    let interesting: Vec<String> = columns
        .into_iter()
        .filter(|c| is_interesting_column(c))
        .collect();
    if interesting.is_empty() {
        return Ok(());
    }
    let select_cols = interesting
        .iter()
        .map(|c| quote_identifier(c))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT rowid, {select_cols} FROM {} ORDER BY rowid DESC LIMIT {}",
        quote_identifier(table),
        MAX_SQLITE_ROWS_PER_TABLE
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        let rowid: i64 = row.get(0)?;
        let mut object = Map::new();
        for (offset, column) in interesting.iter().enumerate() {
            let value = row.get_ref(offset + 1)?;
            if let Some(json_value) = sqlite_value_to_json(value) {
                object.insert(column.clone(), json_value);
            }
        }
        Ok((rowid, Value::Object(object)))
    })?;

    for row in rows.flatten() {
        let row_key = format!("sqlite:{table}:{}", row.0);
        collect_from_json_value(tool, path, &row_key, &row.1, now_ms(), result);
        if result.events.len() >= MAX_EVENTS_PER_FILE {
            break;
        }
    }
    Ok(())
}

fn sqlite_columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let sql = format!("PRAGMA table_info({})", quote_identifier(table));
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn sqlite_value_to_json(value: ValueRef<'_>) -> Option<Value> {
    match value {
        ValueRef::Null => None,
        ValueRef::Integer(v) => Some(Value::from(v)),
        ValueRef::Real(v) => Some(Value::from(v)),
        ValueRef::Text(v) => {
            let text = String::from_utf8_lossy(v).to_string();
            let trimmed = text.trim();
            if trimmed.starts_with(['{', '[']) {
                serde_json::from_str(trimmed)
                    .ok()
                    .or(Some(Value::String(text)))
            } else {
                Some(Value::String(text))
            }
        }
        ValueRef::Blob(_) => None,
    }
}

fn is_interesting_column(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [
        "prompt",
        "message",
        "content",
        "request",
        "response",
        "payload",
        "json",
        "data",
        "body",
        "tool",
        "model",
        "session",
        "conversation",
        "token",
        "usage",
        "time",
        "created",
        "path",
        "workspace",
        "window",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn quote_identifier(raw: &str) -> String {
    format!("\"{}\"", raw.replace('"', "\"\""))
}

fn collect_from_json_value(
    tool: &str,
    path: &Path,
    row_key: &str,
    value: &Value,
    fallback_timestamp_ms: i64,
    result: &mut ScanResult,
) {
    let mut usage_index = 0usize;
    collect_usage_recursive(
        tool,
        path,
        row_key,
        value,
        value,
        fallback_timestamp_ms,
        &mut usage_index,
        &mut result.messages,
    );

    let mut event_index = 0usize;
    collect_events_recursive(
        tool,
        path,
        row_key,
        value,
        value,
        fallback_timestamp_ms,
        &mut event_index,
        &mut result.events,
    );
}

#[allow(clippy::too_many_arguments)]
fn collect_usage_recursive(
    tool: &str,
    path: &Path,
    row_key: &str,
    root: &Value,
    value: &Value,
    fallback_timestamp_ms: i64,
    index: &mut usize,
    out: &mut Vec<LocalUsageMessage>,
) {
    match value {
        Value::Object(map) => {
            if let Some(tokens) = token_buckets_from_object(map) {
                let timestamp_ms =
                    timestamp_ms_from_value(value).or_else(|| timestamp_ms_from_value(root));
                let timestamp_ms = timestamp_ms.unwrap_or(fallback_timestamp_ms);
                let model = first_string(value, MODEL_KEYS)
                    .or_else(|| first_string(root, MODEL_KEYS))
                    .unwrap_or_else(|| "unknown".to_string());
                let provider = first_string(value, PROVIDER_KEYS)
                    .or_else(|| first_string(root, PROVIDER_KEYS))
                    .unwrap_or_else(|| tool.to_string());
                let session_id = first_string(value, SESSION_KEYS)
                    .or_else(|| first_string(root, SESSION_KEYS))
                    .unwrap_or_else(|| stable_hash(path.to_string_lossy().as_ref()));
                let account = account_from_context(tool, &session_id, root);
                let cost = first_f64(value, COST_KEYS)
                    .or_else(|| first_f64(root, COST_KEYS))
                    .unwrap_or(0.0);
                let message_count = first_i64(value, MESSAGE_COUNT_KEYS)
                    .or_else(|| first_i64(root, MESSAGE_COUNT_KEYS))
                    .unwrap_or(1)
                    .max(1);
                let source_seed = format!(
                    "{}|{}|{}|{}|{}|{}|{}",
                    tool,
                    path.display(),
                    row_key,
                    *index,
                    session_id,
                    timestamp_ms,
                    tokens.total()
                );
                *index += 1;
                out.push(LocalUsageMessage {
                    agent: tool.to_string(),
                    provider,
                    model: normalize_model(&model),
                    session_id,
                    account,
                    timestamp_ms,
                    day: day_from_timestamp_ms(timestamp_ms),
                    tokens,
                    message_count,
                    source_cost: cost.max(0.0),
                    dedup_key: stable_hash(&source_seed),
                });
            }
            for child in map.values() {
                collect_usage_recursive(
                    tool,
                    path,
                    row_key,
                    root,
                    child,
                    fallback_timestamp_ms,
                    index,
                    out,
                );
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_usage_recursive(
                    tool,
                    path,
                    row_key,
                    root,
                    child,
                    fallback_timestamp_ms,
                    index,
                    out,
                );
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_events_recursive(
    tool: &str,
    path: &Path,
    row_key: &str,
    root: &Value,
    value: &Value,
    fallback_timestamp_ms: i64,
    index: &mut usize,
    out: &mut Vec<MonitoringEvent>,
) {
    if out.len() >= MAX_EVENTS_PER_FILE {
        return;
    }
    match value {
        Value::Object(map) => {
            let prompt = prompt_text_from_object(map);
            let tool_name = tool_name_from_object(map);
            let window_title = string_from_object(map, WINDOW_KEYS);
            let path_value = file_path_from_object(map);
            let old_text = string_from_object(map, OLD_TEXT_KEYS);
            let new_text = string_from_object(map, NEW_TEXT_KEYS);

            let event_type = if prompt.is_some() {
                Some("prompt")
            } else if old_text.is_some() || new_text.is_some() || path_value.is_some() {
                Some("edit")
            } else if tool_name.is_some() {
                Some("tool")
            } else if window_title.is_some() {
                Some("window")
            } else {
                None
            };

            if let Some(kind) = event_type {
                let timestamp_ms =
                    timestamp_ms_from_value(value).or_else(|| timestamp_ms_from_value(root));
                let timestamp_ms = timestamp_ms.unwrap_or(fallback_timestamp_ms);
                let session_id = first_string(value, SESSION_KEYS)
                    .or_else(|| first_string(root, SESSION_KEYS))
                    .unwrap_or_else(|| stable_hash(path.to_string_lossy().as_ref()));
                let provider = first_string(value, PROVIDER_KEYS)
                    .or_else(|| first_string(root, PROVIDER_KEYS))
                    .unwrap_or_else(|| tool.to_string());
                let model =
                    first_string(value, MODEL_KEYS).or_else(|| first_string(root, MODEL_KEYS));
                let repo = repo_context(value).or_else(|| repo_context(root));
                let file_path = path_value.clone().unwrap_or_else(|| {
                    synthetic_file_path(kind, tool, &session_id, tool_name.as_deref())
                });
                let diff = old_text
                    .as_deref()
                    .zip(new_text.as_deref())
                    .map(|(old, new)| compute_diff(old, new));
                let added_lines = diff.as_ref().map(|d| d.added).unwrap_or(0);
                let removed_lines = diff.as_ref().map(|d| d.removed).unwrap_or(0);
                let diff_hunk = diff.and_then(|d| {
                    if d.hunk.is_empty() {
                        None
                    } else {
                        Some(d.hunk)
                    }
                });
                let prompt_text = prompt.map(|s| truncate_chars(&s, MAX_CAPTURE_TEXT));
                let metadata = event_metadata(kind, tool_name.as_deref(), window_title.as_deref());
                let seed = format!(
                    "{}|{}|{}|{}|{}|{}|{}|{}",
                    tool,
                    path.display(),
                    row_key,
                    *index,
                    kind,
                    session_id,
                    timestamp_ms,
                    stable_hash(prompt_text.as_deref().unwrap_or(""))
                );
                *index += 1;
                out.push(MonitoringEvent {
                    source_key: stable_hash(&seed),
                    agent: tool.to_string(),
                    provider,
                    model: model.map(|m| normalize_model(&m)),
                    session_id,
                    timestamp_ms,
                    event_type: kind.to_string(),
                    prompt_text,
                    file_path,
                    repo_url: repo
                        .as_ref()
                        .map(|r| r.repo_url.clone())
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or_else(|| "local".to_string()),
                    branch: repo
                        .as_ref()
                        .map(|r| r.branch.clone())
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or_else(|| "unknown".to_string()),
                    current_sha: repo
                        .as_ref()
                        .map(|r| r.current_sha.clone())
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or_else(|| stable_hash(&seed)[..12].to_string()),
                    added_lines,
                    removed_lines,
                    diff_hunk,
                    metadata,
                });
            }

            for child in map.values() {
                collect_events_recursive(
                    tool,
                    path,
                    row_key,
                    root,
                    child,
                    fallback_timestamp_ms,
                    index,
                    out,
                );
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_events_recursive(
                    tool,
                    path,
                    row_key,
                    root,
                    child,
                    fallback_timestamp_ms,
                    index,
                    out,
                );
            }
        }
        _ => {}
    }
}

fn usage_from_csv(
    tool: &str,
    path: &Path,
    idx: usize,
    row: &HashMap<String, String>,
) -> Option<LocalUsageMessage> {
    let tokens = TokenBuckets {
        input: csv_i64(
            row,
            &["input_tokens", "prompt_tokens", "tokens_in", "input"],
        ),
        output: csv_i64(
            row,
            &["output_tokens", "completion_tokens", "tokens_out", "output"],
        ),
        cache_read: csv_i64(
            row,
            &[
                "cache_read_input_tokens",
                "cached_input_tokens",
                "tokens_cache_read",
                "cache_read",
            ],
        ),
        cache_write: csv_i64(
            row,
            &[
                "cache_creation_input_tokens",
                "tokens_cache_write",
                "cache_write",
            ],
        ),
        reasoning: csv_i64(row, &["reasoning_tokens", "tokens_reasoning", "reasoning"]),
    };
    if tokens.total() <= 0 {
        return None;
    }
    let timestamp_ms = row
        .get("timestamp")
        .or_else(|| row.get("created_at"))
        .or_else(|| row.get("date"))
        .and_then(|s| parse_timestamp_ms_str(s))
        .unwrap_or_else(now_ms);
    let session_id = csv_string(row, &["session_id", "session", "conversation_id"])
        .unwrap_or_else(|| format!("csv-{}", stable_hash(path.to_string_lossy().as_ref())));
    let model = csv_string(row, &["model", "model_id"]).unwrap_or_else(|| "unknown".to_string());
    let provider = csv_string(row, &["provider"]).unwrap_or_else(|| tool.to_string());
    let account = csv_string(row, &["account", "email"]).unwrap_or_else(|| "local".to_string());
    let seed = format!(
        "{}|{}|{}|{}|{}",
        tool,
        path.display(),
        idx,
        session_id,
        tokens.total()
    );
    Some(LocalUsageMessage {
        agent: tool.to_string(),
        provider,
        model: normalize_model(&model),
        session_id,
        account,
        timestamp_ms,
        day: day_from_timestamp_ms(timestamp_ms),
        tokens,
        message_count: csv_i64(row, &["message_count", "messages"]).max(1),
        source_cost: csv_f64(row, &["cost"]).unwrap_or(0.0),
        dedup_key: stable_hash(&seed),
    })
}

fn event_from_csv(
    tool: &str,
    path: &Path,
    idx: usize,
    row: &HashMap<String, String>,
) -> Option<MonitoringEvent> {
    let prompt = csv_string(row, &["prompt", "user_prompt", "message"]);
    let tool_name = csv_string(row, &["tool_name", "tool"]);
    let window_title = csv_string(row, &["window_title", "windowtitle", "title", "app_name"]);
    let explicit_file_path = csv_string(row, &["file_path", "filepath", "path"]);
    let old_text = csv_string(row, &["old_string", "old_str", "old"]);
    let new_text = csv_string(row, &["new_string", "new_str", "new", "content"]);
    if prompt.is_none()
        && tool_name.is_none()
        && window_title.is_none()
        && explicit_file_path.is_none()
        && old_text.is_none()
        && new_text.is_none()
    {
        return None;
    }
    let timestamp_ms = row
        .get("timestamp")
        .or_else(|| row.get("created_at"))
        .or_else(|| row.get("date"))
        .and_then(|s| parse_timestamp_ms_str(s))
        .unwrap_or_else(now_ms);
    let session_id = csv_string(row, &["session_id", "session", "conversation_id"])
        .unwrap_or_else(|| format!("csv-{}", stable_hash(path.to_string_lossy().as_ref())));
    let kind = if prompt.is_some() {
        "prompt"
    } else if old_text.is_some() || new_text.is_some() || explicit_file_path.is_some() {
        "edit"
    } else if tool_name.is_some() {
        "tool"
    } else {
        "window"
    };
    let diff = old_text
        .as_deref()
        .zip(new_text.as_deref())
        .map(|(old, new)| compute_diff(old, new));
    let added_lines = diff.as_ref().map(|d| d.added).unwrap_or(0);
    let removed_lines = diff.as_ref().map(|d| d.removed).unwrap_or(0);
    let diff_hunk = diff.and_then(|d| {
        if d.hunk.is_empty() {
            None
        } else {
            Some(d.hunk)
        }
    });
    let file_path = explicit_file_path
        .unwrap_or_else(|| synthetic_file_path(kind, tool, &session_id, tool_name.as_deref()));
    let seed = format!(
        "csv|{}|{}|{}|{}|{}",
        tool,
        path.display(),
        idx,
        kind,
        session_id
    );
    Some(MonitoringEvent {
        source_key: stable_hash(&seed),
        agent: tool.to_string(),
        provider: csv_string(row, &["provider"]).unwrap_or_else(|| tool.to_string()),
        model: csv_string(row, &["model", "model_id"]).map(|m| normalize_model(&m)),
        session_id,
        timestamp_ms,
        event_type: kind.to_string(),
        prompt_text: prompt.map(|s| truncate_chars(&s, MAX_CAPTURE_TEXT)),
        file_path,
        repo_url: csv_string(row, &["repo_url"]).unwrap_or_else(|| "local".to_string()),
        branch: csv_string(row, &["branch"]).unwrap_or_else(|| "unknown".to_string()),
        current_sha: csv_string(row, &["current_sha", "sha"])
            .unwrap_or_else(|| stable_hash(&seed)[..12].to_string()),
        added_lines,
        removed_lines,
        diff_hunk,
        metadata: event_metadata(kind, tool_name.as_deref(), window_title.as_deref()),
    })
}

fn insert_usage_sessions(conn: &mut Connection, messages: &[LocalUsageMessage]) -> Result<usize> {
    let tx = conn.transaction()?;
    let mut inserted = 0usize;
    {
        let mut stmt = tx.prepare(
            "INSERT OR IGNORE INTO usage_sessions
             (source, session_hash, dedup_hash, timestamp_ms, day, agent, model, provider, account,
              tokens_in, tokens_out, tokens_cache_read, tokens_cache_write, tokens_reasoning,
              message_count, source_cost)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        )?;
        for msg in messages {
            let row = UsageSessionRow::from_message(msg);
            inserted += stmt.execute(params![
                row.source,
                row.session_hash,
                row.dedup_hash,
                row.timestamp_ms,
                row.day,
                row.agent,
                row.model,
                row.provider,
                row.account,
                row.tokens_in,
                row.tokens_out,
                row.tokens_cache_read,
                row.tokens_cache_write,
                row.tokens_reasoning,
                row.message_count,
                row.source_cost,
            ])?;
        }
    }
    tx.commit()?;
    Ok(inserted)
}

impl UsageSessionRow {
    fn from_message(msg: &LocalUsageMessage) -> Self {
        Self {
            source: msg.agent.clone(),
            session_hash: stable_hash(&format!("{}|{}", msg.agent, msg.session_id)),
            dedup_hash: stable_hash(&msg.dedup_key),
            timestamp_ms: msg.timestamp_ms,
            day: msg.day.clone(),
            agent: msg.agent.clone(),
            model: msg.model.clone(),
            provider: msg.provider.clone(),
            account: msg.account.clone(),
            tokens_in: msg.tokens.input.max(0),
            tokens_out: msg.tokens.output.max(0),
            tokens_cache_read: msg.tokens.cache_read.max(0),
            tokens_cache_write: msg.tokens.cache_write.max(0),
            tokens_reasoning: msg.tokens.reasoning.max(0),
            message_count: msg.message_count.max(1),
            source_cost: msg.source_cost.max(0.0),
        }
    }
}

fn insert_monitoring_events(
    usage_conn: &Connection,
    events: &[MonitoringEvent],
    credential_override: Option<&str>,
) -> Result<usize> {
    if events.is_empty() {
        return Ok(0);
    }
    let cfg = load_config();
    let credential = credential_override.unwrap_or(cfg.credential.as_str());
    let (token_key, hmac_secret) = if credential.is_empty() {
        (String::new(), String::new())
    } else {
        match split_credential(credential) {
            Ok((token, secret)) => (mask_token(&token), secret),
            Err(_) => (String::new(), String::new()),
        }
    };
    let device_id = if cfg.device_id.is_empty() {
        "local".to_string()
    } else {
        cfg.device_id
    };
    let hostname = gethostname::gethostname()
        .into_string()
        .unwrap_or_else(|_| "unknown".to_string());
    let records = open_records_db()?;
    let mut inserted = 0usize;

    for event in events {
        let seen = usage_conn.execute(
            "INSERT OR IGNORE INTO usage_monitoring_seen (source_key, event_type) VALUES (?1, ?2)",
            params![event.source_key, event.event_type],
        )?;
        if seen == 0 {
            continue;
        }
        let record = event.to_record(&token_key, &hmac_secret, &device_id, &hostname);
        if insert_record_by_signature(&records, &record)? {
            inserted += 1;
        }
    }

    Ok(inserted)
}

impl MonitoringEvent {
    fn to_record(
        &self,
        token_key: &str,
        hmac_secret: &str,
        device_id: &str,
        hostname: &str,
    ) -> Record {
        let timestamp = rfc3339_from_ms(self.timestamp_ms);
        let record_sig = if token_key.is_empty() {
            String::new()
        } else {
            compute_record_sig(
                hmac_secret,
                token_key,
                device_id,
                hostname,
                &timestamp,
                &self.agent,
                &self.file_path,
                &self.repo_url,
                &self.current_sha,
                self.added_lines,
                self.removed_lines,
                self.diff_hunk.as_deref(),
            )
        };
        Record {
            id: 0,
            tool: self.agent.clone(),
            tool_version: None,
            provider: self.provider.clone(),
            model: self.model.clone(),
            session_id: self.session_id.clone(),
            repo_url: self.repo_url.clone(),
            branch: self.branch.clone(),
            current_sha: self.current_sha.clone(),
            file_path: self.file_path.clone(),
            added_lines: self.added_lines,
            removed_lines: self.removed_lines,
            diff_hunk: self.diff_hunk.clone(),
            metadata: self.metadata.clone(),
            synced: 0,
            synced_at: None,
            retry_count: 0,
            timestamp,
            token_key: token_key.to_string(),
            device_id: device_id.to_string(),
            hostname: hostname.to_string(),
            record_sig,
            prompt_summary: self.prompt_text.clone(),
        }
    }
}

fn insert_record_by_signature(conn: &Connection, record: &Record) -> Result<bool> {
    if !record.record_sig.is_empty() {
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM records WHERE record_sig = ?1",
                params![record.record_sig],
                |row| row.get(0),
            )
            .unwrap_or(false);
        if exists {
            return Ok(false);
        }
    }
    crate::adapter::sqlite::insert_record(conn, record)
}

fn rebuild_rollups(conn: &mut Connection) -> Result<usize> {
    let cfg = load_config();
    let device_id = cfg.device_id;
    let tx = conn.transaction()?;
    let rows = {
        let mut stmt = tx.prepare(
            "SELECT day, agent, model, account,
                    SUM(tokens_in), SUM(tokens_out), SUM(tokens_cache_read),
                    SUM(tokens_cache_write), SUM(tokens_reasoning), SUM(message_count)
             FROM usage_sessions
             GROUP BY day, agent, model, account
             ORDER BY day, agent, model, account",
        )?;
        let mapped = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, i64>(9)?,
            ))
        })?;
        mapped.collect::<rusqlite::Result<Vec<_>>>()?
    };

    let mut upserted = 0usize;
    for (
        day,
        agent,
        model,
        account,
        tokens_in,
        tokens_out,
        tokens_cache_read,
        tokens_cache_write,
        tokens_reasoning,
        message_count,
    ) in rows
    {
        upserted += tx.execute(
            "INSERT INTO usage_daily_model_rollups
             (device_id, day, agent, model, account, tokens_in, tokens_out, tokens_cache_read,
              tokens_cache_write, tokens_reasoning, message_count, dirty, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 1, datetime('now'))
             ON CONFLICT(device_id, day, agent, model, account) DO UPDATE SET
               tokens_in = excluded.tokens_in,
               tokens_out = excluded.tokens_out,
               tokens_cache_read = excluded.tokens_cache_read,
               tokens_cache_write = excluded.tokens_cache_write,
               tokens_reasoning = excluded.tokens_reasoning,
               message_count = excluded.message_count,
               dirty = 1,
               updated_at = datetime('now')",
            params![
                device_id,
                day,
                agent,
                model,
                account,
                tokens_in,
                tokens_out,
                tokens_cache_read,
                tokens_cache_write,
                tokens_reasoning,
                message_count,
            ],
        )?;
    }
    tx.commit()?;
    Ok(upserted)
}

fn upsert_local_subscription_snapshots(conn: &Connection) -> Result<usize> {
    let cfg = load_config();
    let mut count = 0usize;
    for snapshot in local_subscription_snapshots(&cfg.device_id) {
        count += conn.execute(
            "INSERT INTO usage_subscription_snapshots
             (device_id, agent, account, plan, quota_session_remaining, quota_weekly_remaining,
              quota_reset_at, reader_status, snapshotted_at, dirty, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, datetime('now'))
             ON CONFLICT(device_id, agent, account) DO UPDATE SET
               plan = excluded.plan,
               quota_session_remaining = excluded.quota_session_remaining,
               quota_weekly_remaining = excluded.quota_weekly_remaining,
               quota_reset_at = excluded.quota_reset_at,
               reader_status = excluded.reader_status,
               snapshotted_at = excluded.snapshotted_at,
               dirty = 1,
               updated_at = datetime('now')",
            params![
                snapshot.device_id,
                snapshot.agent,
                snapshot.account,
                snapshot.plan,
                snapshot.quota_session_remaining,
                snapshot.quota_weekly_remaining,
                snapshot.quota_reset_at,
                snapshot.reader_status,
                snapshot.snapshotted_at,
            ],
        )?;
    }
    Ok(count)
}

fn local_subscription_snapshots(device_id: &str) -> Vec<SubscriptionPayload> {
    let mut snapshots = Vec::new();
    if let Some(snapshot) = read_codex_rate_limit(device_id) {
        snapshots.push(snapshot);
    }
    if let Some(snapshot) = read_claude_rate_limit(device_id) {
        snapshots.push(snapshot);
    }
    snapshots
}

fn read_codex_rate_limit(device_id: &str) -> Option<SubscriptionPayload> {
    let home = scan_home()?;
    let sessions_dir = home.join(".codex").join("sessions");
    let mut files = Vec::new();
    collect_files_with_extension(&sessions_dir, "jsonl", &mut files);
    files.sort_by(|a, b| b.cmp(a));

    for file in files {
        let text = fs::read_to_string(file).ok()?;
        for line in text.lines().rev() {
            let obj: Value = serde_json::from_str(line).ok()?;
            let payload = obj.get("payload")?;
            if payload.get("type").and_then(|v| v.as_str()) != Some("token_count") {
                continue;
            }
            let rate_limits = payload.get("rate_limits")?;
            let plan = rate_limits
                .get("plan_type")
                .and_then(|v| v.as_str())
                .map(titlecase);
            let five_hour = parse_codex_window(rate_limits.get("primary"))
                .or_else(|| parse_codex_window(rate_limits.get("secondary")))
                .filter(|w| w.window_minutes == 300);
            let seven_day = parse_codex_window(rate_limits.get("primary"))
                .filter(|w| w.window_minutes == 10080)
                .or_else(|| {
                    parse_codex_window(rate_limits.get("secondary"))
                        .filter(|w| w.window_minutes == 10080)
                });
            if five_hour.is_none() && seven_day.is_none() {
                continue;
            }
            return Some(SubscriptionPayload {
                device_id: device_id.to_string(),
                agent: "codex".to_string(),
                account: "local".to_string(),
                plan,
                quota_session_remaining: five_hour.as_ref().map(|w| w.remaining_percent),
                quota_weekly_remaining: seven_day.as_ref().map(|w| w.remaining_percent),
                quota_reset_at: five_hour
                    .as_ref()
                    .and_then(|w| w.resets_at.clone())
                    .or_else(|| seven_day.as_ref().and_then(|w| w.resets_at.clone())),
                reader_status: "ok".to_string(),
                snapshotted_at: Utc::now().to_rfc3339(),
            });
        }
    }
    None
}

fn read_claude_rate_limit(device_id: &str) -> Option<SubscriptionPayload> {
    let path = crate::config::config_dir().join("claude-rate-limits.json");
    let text = fs::read_to_string(path).ok()?;
    let obj: Value = serde_json::from_str(&text).ok()?;
    let five = parse_claude_window(obj.get("five_hour"));
    let seven = parse_claude_window(obj.get("seven_day"));
    if five.is_none() && seven.is_none() {
        return None;
    }
    Some(SubscriptionPayload {
        device_id: device_id.to_string(),
        agent: "claude".to_string(),
        account: "local".to_string(),
        plan: None,
        quota_session_remaining: five.as_ref().map(|w| w.remaining_percent),
        quota_weekly_remaining: seven.as_ref().map(|w| w.remaining_percent),
        quota_reset_at: five
            .as_ref()
            .and_then(|w| w.resets_at.clone())
            .or_else(|| seven.as_ref().and_then(|w| w.resets_at.clone())),
        reader_status: "ok".to_string(),
        snapshotted_at: Utc::now().to_rfc3339(),
    })
}

struct RateWindow {
    remaining_percent: i64,
    resets_at: Option<String>,
    window_minutes: i64,
}

fn parse_codex_window(raw: Option<&Value>) -> Option<RateWindow> {
    let raw = raw?;
    let used = raw.get("used_percent")?.as_f64()?;
    let window_minutes = raw.get("window_minutes")?.as_i64()?;
    let resets_at = parse_reset_time(raw);
    if reset_is_past(&resets_at) {
        return None;
    }
    Some(RateWindow {
        remaining_percent: remaining_percent(used),
        resets_at,
        window_minutes,
    })
}

fn parse_claude_window(raw: Option<&Value>) -> Option<RateWindow> {
    let raw = raw?;
    let used = raw
        .get("used_percentage")
        .or_else(|| raw.get("utilization"))?
        .as_f64()?;
    let resets_at = parse_reset_time(raw);
    Some(RateWindow {
        remaining_percent: remaining_percent(used),
        resets_at,
        window_minutes: 0,
    })
}

fn parse_reset_time(raw: &Value) -> Option<String> {
    let value = raw.get("resets_at")?;
    if let Some(epoch) = value.as_f64() {
        let secs = epoch.trunc() as i64;
        return Utc
            .timestamp_opt(secs, 0)
            .single()
            .map(|dt| dt.to_rfc3339());
    }
    let s = value.as_str()?;
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc).to_rfc3339())
}

fn reset_is_past(value: &Option<String>) -> bool {
    let Some(value) = value else {
        return false;
    };
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc) <= Utc::now())
        .unwrap_or(false)
}

fn remaining_percent(used_percent: f64) -> i64 {
    (100.0 - used_percent).round().clamp(0.0, 100.0) as i64
}

fn collect_files_with_extension(root: &Path, extension: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files_with_extension(&path, extension, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some(extension) {
            out.push(path);
        }
    }
}

fn enqueue_dirty_rollups(conn: &Connection, device_id: &str) -> Result<usize> {
    let mut stmt = conn.prepare(
        "SELECT device_id, day, agent, model, account, tokens_in, tokens_out,
                tokens_cache_read, tokens_cache_write, tokens_reasoning
         FROM usage_daily_model_rollups
         WHERE dirty = 1
         ORDER BY day, agent, model, account
         LIMIT ?1",
    )?;
    let items = stmt
        .query_map(params![MAX_UPLOAD_ITEMS], |row| {
            Ok(RollupItem {
                device_id: row.get(0)?,
                day: row.get(1)?,
                agent: row.get(2)?,
                model: row.get(3)?,
                account: row.get(4)?,
                tokens_in: row.get(5)?,
                tokens_out: row.get(6)?,
                tokens_cache_read: row.get(7)?,
                tokens_cache_write: row.get(8)?,
                tokens_reasoning: row.get(9)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if items.is_empty() {
        return Ok(0);
    }

    let natural_key = format!(
        "{}:{}",
        device_id,
        stable_hash(
            &items
                .iter()
                .map(|i| format!("{}|{}|{}|{}", i.day, i.agent, i.model, i.account))
                .collect::<Vec<_>>()
                .join("\n")
        )
    );
    enqueue_payload(
        conn,
        OutboxType::Rollup,
        &natural_key,
        &RollupPayload {
            items: items.clone(),
        },
    )?;
    Ok(items.len())
}

fn enqueue_dirty_subscriptions(conn: &Connection) -> Result<usize> {
    let mut stmt = conn.prepare(
        "SELECT device_id, agent, account, plan, quota_session_remaining, quota_weekly_remaining,
                quota_reset_at, reader_status, snapshotted_at
         FROM usage_subscription_snapshots
         WHERE dirty = 1
         ORDER BY agent, account
         LIMIT ?1",
    )?;
    let snapshots = stmt
        .query_map(params![MAX_UPLOAD_ITEMS], |row| {
            Ok(SubscriptionPayload {
                device_id: row.get(0)?,
                agent: row.get(1)?,
                account: row.get(2)?,
                plan: row.get(3)?,
                quota_session_remaining: row.get(4)?,
                quota_weekly_remaining: row.get(5)?,
                quota_reset_at: row.get(6)?,
                reader_status: row.get(7)?,
                snapshotted_at: row.get(8)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut enqueued = 0usize;
    for snapshot in snapshots {
        let natural_key = format!(
            "{}:{}:{}",
            snapshot.device_id, snapshot.agent, snapshot.account
        );
        enqueue_payload(conn, OutboxType::Subscription, &natural_key, &snapshot)?;
        enqueued += 1;
    }
    Ok(enqueued)
}

fn enqueue_payload<T: Serialize>(
    conn: &Connection,
    payload_type: OutboxType,
    natural_key: &str,
    payload: &T,
) -> Result<()> {
    let payload_json = serde_json::to_string(payload)?;
    let payload_sha256 = stable_hash(&payload_json);
    conn.execute(
        "INSERT OR IGNORE INTO usage_outbox
         (payload_type, natural_key, payload_json, payload_sha256)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            payload_type.as_str(),
            natural_key,
            payload_json,
            payload_sha256
        ],
    )?;
    Ok(())
}

struct DrainReport {
    uploaded: usize,
    failed: usize,
}

async fn drain_outbox(
    conn: &Connection,
    api_url: &str,
    credential: &str,
    device_id: &str,
) -> Result<DrainReport> {
    let items = fetch_pending_outbox(conn)?;
    let mut uploaded = 0usize;
    let mut failed = 0usize;
    for item in items {
        match upload_outbox_item(api_url, credential, device_id, &item).await {
            Ok(()) => {
                mark_outbox_synced(conn, item.id)?;
                mark_payload_uploaded(conn, &item)?;
                uploaded += 1;
            }
            Err(e) => {
                eprintln!("[aitrack] usage upload failed: {e}");
                increment_outbox_retry(conn, item.id)?;
                failed += 1;
            }
        }
    }
    Ok(DrainReport { uploaded, failed })
}

fn fetch_pending_outbox(conn: &Connection) -> Result<Vec<OutboxItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, payload_type, payload_json
         FROM usage_outbox
         WHERE synced = 0 AND retry_count < 5
         ORDER BY id
         LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![MAX_UPLOAD_ITEMS], |row| {
        let payload_type_raw: String = row.get(1)?;
        let payload_type = OutboxType::from_str(&payload_type_raw).ok_or_else(|| {
            rusqlite::Error::InvalidParameterName(format!(
                "unknown usage payload type {payload_type_raw}"
            ))
        })?;
        Ok(OutboxItem {
            id: row.get(0)?,
            payload_type,
            payload_json: row.get(2)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

async fn upload_outbox_item(
    api_url: &str,
    credential: &str,
    device_id: &str,
    item: &OutboxItem,
) -> Result<()> {
    let (token, hmac_secret) = split_credential(credential)?;
    let body = item.payload_json.as_bytes().to_vec();
    let body_sha256 = stable_hash_bytes(&body);
    let body_timestamp = Utc::now().to_rfc3339();
    let unix_ts = Utc::now().timestamp() as u64;
    let sig = compute_request_sig(&hmac_secret, unix_ts, &body);
    let url = format!("{}{}", api_url, item.payload_type.endpoint());

    let resp = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .header("X-AiTrack-Body-Sha256", body_sha256)
        .header("X-AiTrack-Body-Timestamp", body_timestamp)
        .header("X-AiTrack-Device", device_id)
        .header(
            "X-AiTrack-Client",
            format!("aitrack/{}", env!("CARGO_PKG_VERSION")),
        )
        .header("X-AiTrack-Timestamp", unix_ts.to_string())
        .header("X-AiTrack-Signature", sig)
        .body(body)
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("HTTP {}", resp.status());
    }
    Ok(())
}

fn mark_outbox_synced(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "UPDATE usage_outbox
         SET synced = 1, synced_at = datetime('now'), updated_at = datetime('now')
         WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

fn increment_outbox_retry(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "UPDATE usage_outbox
         SET retry_count = retry_count + 1, updated_at = datetime('now')
         WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

fn mark_payload_uploaded(conn: &Connection, item: &OutboxItem) -> Result<()> {
    match item.payload_type {
        OutboxType::Rollup => {
            let payload: RollupPayload = serde_json::from_str(&item.payload_json)?;
            for row in payload.items {
                conn.execute(
                    "UPDATE usage_daily_model_rollups
                     SET dirty = 0, uploaded_at = datetime('now'), updated_at = datetime('now')
                     WHERE device_id = ?1 AND day = ?2 AND agent = ?3 AND model = ?4 AND account = ?5",
                    params![row.device_id, row.day, row.agent, row.model, row.account],
                )?;
            }
        }
        OutboxType::Subscription => {
            let payload: SubscriptionPayload = serde_json::from_str(&item.payload_json)?;
            conn.execute(
                "UPDATE usage_subscription_snapshots
                 SET dirty = 0, uploaded_at = datetime('now'), updated_at = datetime('now')
                 WHERE device_id = ?1 AND agent = ?2 AND account = ?3",
                params![payload.device_id, payload.agent, payload.account],
            )?;
        }
    }
    Ok(())
}

fn pending_outbox(conn: &Connection) -> Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM usage_outbox WHERE synced = 0 AND retry_count < 5",
        [],
        |row| row.get(0),
    )
    .context("count usage outbox")
}

fn count_table(conn: &Connection, table: &str) -> Result<i64> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    conn.query_row(&sql, [], |row| row.get(0))
        .with_context(|| format!("count {table}"))
}

fn token_buckets_from_object(map: &Map<String, Value>) -> Option<TokenBuckets> {
    let mut tokens = TokenBuckets {
        input: number_from_object(map, INPUT_TOKEN_KEYS),
        output: number_from_object(map, OUTPUT_TOKEN_KEYS),
        cache_read: number_from_object(map, CACHE_READ_TOKEN_KEYS),
        cache_write: number_from_object(map, CACHE_WRITE_TOKEN_KEYS),
        reasoning: number_from_object(map, REASONING_TOKEN_KEYS),
    };
    if tokens.total() <= 0 {
        let total = number_from_object(map, TOTAL_TOKEN_KEYS);
        if total > 0 {
            tokens.input = total;
        }
    }
    (tokens.total() > 0).then_some(tokens)
}

fn prompt_text_from_object(map: &Map<String, Value>) -> Option<String> {
    if matches!(
        string_from_object(map, &["role", "author_role"]).as_deref(),
        Some("user" | "human")
    ) {
        if let Some(text) = content_text(map.get("content")) {
            return Some(text);
        }
        if let Some(text) = string_from_object(map, &["text", "message"]) {
            return Some(text);
        }
    }
    string_from_object(map, PROMPT_KEYS).or_else(|| {
        if has_user_event_name(map) {
            content_text(map.get("content"))
        } else {
            None
        }
    })
}

fn has_user_event_name(map: &Map<String, Value>) -> bool {
    string_from_object(map, &["type", "event", "event_type", "hook_event_name"])
        .map(|s| {
            let lower = s.to_ascii_lowercase();
            lower.contains("prompt") || lower.contains("user")
        })
        .unwrap_or(false)
}

fn content_text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(s) => Some(s.clone()),
        Value::Array(items) => {
            let mut parts = Vec::new();
            for item in items {
                if let Some(text) = first_string(item, &["text", "content"]) {
                    parts.push(text);
                }
            }
            (!parts.is_empty()).then(|| parts.join("\n"))
        }
        Value::Object(map) => string_from_object(map, &["text", "content"]),
        _ => None,
    }
}

fn tool_name_from_object(map: &Map<String, Value>) -> Option<String> {
    string_from_object(map, TOOL_NAME_KEYS).filter(|s| !s.trim().is_empty())
}

fn file_path_from_object(map: &Map<String, Value>) -> Option<String> {
    if let Some(file) = string_from_object(map, FILE_PATH_KEYS) {
        return Some(file);
    }
    for key in ["file_paths", "paths", "files"] {
        if let Some(Value::Array(items)) = get_case_insensitive(map, key) {
            if let Some(first) = items.iter().filter_map(Value::as_str).next() {
                return Some(first.to_string());
            }
        }
    }
    None
}

fn repo_context(value: &Value) -> Option<git::RepoInfo> {
    let cwd = first_string(value, WORKSPACE_KEYS)?;
    let path = PathBuf::from(cwd);
    if !path.exists() {
        return None;
    }
    let repo = git::infer_repo_info(&path);
    (!repo.repo_url.is_empty()).then_some(repo)
}

fn event_metadata(
    kind: &str,
    tool_name: Option<&str>,
    window_title: Option<&str>,
) -> Option<String> {
    let mut map = Map::new();
    map.insert("event_type".to_string(), Value::String(kind.to_string()));
    if let Some(tool) = tool_name {
        map.insert(
            "tool_name".to_string(),
            Value::String(truncate_chars(tool, 256)),
        );
    }
    if let Some(title) = window_title {
        map.insert(
            "window_title".to_string(),
            Value::String(truncate_chars(title, MAX_CAPTURE_TEXT)),
        );
    }
    serde_json::to_string(&Value::Object(map)).ok()
}

fn synthetic_file_path(
    kind: &str,
    tool: &str,
    session_id: &str,
    tool_name: Option<&str>,
) -> String {
    let label = match (kind, tool_name) {
        ("tool", Some(name)) | ("edit", Some(name)) => name,
        _ => kind,
    };
    format!(
        "__aitrack__/{tool}/{}/{}",
        sanitize_path_segment(label),
        &stable_hash(session_id)[..12]
    )
}

fn sanitize_path_segment(raw: &str) -> String {
    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .chars()
        .take(64)
        .collect::<String>()
}

fn account_from_context(tool: &str, session_id: &str, root: &Value) -> String {
    if let Some(account) = first_string(root, ACCOUNT_KEYS) {
        return account;
    }
    if tool == "cursor" {
        return session_id
            .split(':')
            .next()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("local")
            .to_string();
    }
    "local".to_string()
}

fn first_string(value: &Value, keys: &[&str]) -> Option<String> {
    match value {
        Value::Object(map) => {
            if let Some(s) = string_from_object(map, keys) {
                return Some(s);
            }
            for child in map.values() {
                if let Some(s) = first_string(child, keys) {
                    return Some(s);
                }
            }
            None
        }
        Value::Array(items) => items.iter().find_map(|v| first_string(v, keys)),
        _ => None,
    }
}

fn first_i64(value: &Value, keys: &[&str]) -> Option<i64> {
    match value {
        Value::Object(map) => {
            let n = keys
                .iter()
                .find_map(|key| get_case_insensitive(map, key).and_then(value_as_i64));
            if n.is_some() {
                return n;
            }
            for child in map.values() {
                if let Some(n) = first_i64(child, keys) {
                    return Some(n);
                }
            }
            None
        }
        Value::Array(items) => items.iter().find_map(|v| first_i64(v, keys)),
        _ => None,
    }
}

fn first_f64(value: &Value, keys: &[&str]) -> Option<f64> {
    match value {
        Value::Object(map) => {
            let n = keys
                .iter()
                .find_map(|key| get_case_insensitive(map, key).and_then(value_as_f64));
            if n.is_some() {
                return n;
            }
            for child in map.values() {
                if let Some(n) = first_f64(child, keys) {
                    return Some(n);
                }
            }
            None
        }
        Value::Array(items) => items.iter().find_map(|v| first_f64(v, keys)),
        _ => None,
    }
}

fn string_from_object(map: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| get_case_insensitive(map, key).and_then(value_as_string))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn number_from_object(map: &Map<String, Value>, keys: &[&str]) -> i64 {
    keys.iter()
        .find_map(|key| get_case_insensitive(map, key).and_then(value_as_i64))
        .unwrap_or(0)
        .max(0)
}

fn get_case_insensitive<'a>(map: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    map.get(key).or_else(|| {
        let lower = key.to_ascii_lowercase();
        map.iter()
            .find(|(candidate, _)| candidate.to_ascii_lowercase() == lower)
            .map(|(_, value)| value)
    })
}

fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn value_as_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_u64().and_then(|v| i64::try_from(v).ok()))
            .or_else(|| n.as_f64().map(|v| v.round() as i64)),
        Value::String(s) => s.trim().parse::<f64>().ok().map(|v| v.round() as i64),
        _ => None,
    }
}

fn value_as_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn timestamp_ms_from_value(value: &Value) -> Option<i64> {
    if let Some(n) = first_i64(value, TIMESTAMP_KEYS) {
        return Some(normalize_epoch_ms(n));
    }
    first_string(value, TIMESTAMP_KEYS).and_then(|s| parse_timestamp_ms_str(&s))
}

fn parse_timestamp_ms_str(raw: &str) -> Option<i64> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(n) = s.parse::<f64>() {
        return Some(normalize_epoch_ms(n.round() as i64));
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp_millis());
    }
    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return date
            .and_hms_opt(0, 0, 0)
            .map(|dt| dt.and_utc().timestamp_millis());
    }
    None
}

fn normalize_epoch_ms(raw: i64) -> i64 {
    if raw > 10_000_000_000 {
        raw
    } else {
        raw.saturating_mul(1000)
    }
}

fn day_from_timestamp_ms(ms: i64) -> String {
    Utc.timestamp_millis_opt(ms)
        .single()
        .unwrap_or_else(Utc::now)
        .format("%Y-%m-%d")
        .to_string()
}

fn rfc3339_from_ms(ms: i64) -> String {
    Utc.timestamp_millis_opt(ms)
        .single()
        .unwrap_or_else(Utc::now)
        .to_rfc3339()
}

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

fn system_time_ms(value: std::time::SystemTime) -> Option<i64> {
    value
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_millis()).ok())
}

fn day_in_range(day: &str, since: Option<&str>, until: Option<&str>) -> bool {
    if since.is_some_and(|s| day < s) {
        return false;
    }
    if until.is_some_and(|u| day > u) {
        return false;
    }
    true
}

fn dedup_messages(messages: &mut Vec<LocalUsageMessage>) {
    let mut seen = HashSet::new();
    messages.retain(|m| seen.insert(m.dedup_key.clone()));
}

fn dedup_events(events: &mut Vec<MonitoringEvent>) {
    let mut seen = HashSet::new();
    events.retain(|e| seen.insert(e.source_key.clone()));
}

fn normalize_model(raw: &str) -> String {
    let model = raw.trim();
    if model.is_empty() {
        return "unknown".to_string();
    }
    model.to_string()
}

fn split_csv_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = line.chars().peekable();
    let mut in_quotes = false;
    while let Some(ch) = chars.next() {
        match ch {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                cur.push('"');
                let _ = chars.next();
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                out.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(ch),
        }
    }
    out.push(cur.trim().to_string());
    out
}

fn csv_string(row: &HashMap<String, String>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|k| row.get(*k))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn csv_i64(row: &HashMap<String, String>, keys: &[&str]) -> i64 {
    csv_string(row, keys)
        .and_then(|s| s.parse::<f64>().ok())
        .map(|v| v.round() as i64)
        .unwrap_or(0)
        .max(0)
}

fn csv_f64(row: &HashMap<String, String>, keys: &[&str]) -> Option<f64> {
    csv_string(row, keys).and_then(|s| s.parse::<f64>().ok())
}

fn truncate_chars(raw: &str, max: usize) -> String {
    raw.chars().take(max).collect()
}

fn stable_hash(value: &str) -> String {
    stable_hash_bytes(value.as_bytes())
}

fn stable_hash_bytes(value: &[u8]) -> String {
    hex::encode(Sha256::digest(value))
}

fn titlecase(raw: &str) -> String {
    let mut chars = raw.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

const INPUT_TOKEN_KEYS: &[&str] = &[
    "input_tokens",
    "prompt_tokens",
    "tokens_in",
    "inputTokens",
    "promptTokens",
    "total_input_tokens",
];
const OUTPUT_TOKEN_KEYS: &[&str] = &[
    "output_tokens",
    "completion_tokens",
    "tokens_out",
    "outputTokens",
    "completionTokens",
    "total_output_tokens",
];
const CACHE_READ_TOKEN_KEYS: &[&str] = &[
    "cache_read_input_tokens",
    "cached_input_tokens",
    "cache_read_tokens",
    "tokens_cache_read",
    "cacheReadInputTokens",
];
const CACHE_WRITE_TOKEN_KEYS: &[&str] = &[
    "cache_creation_input_tokens",
    "cache_write_input_tokens",
    "cache_write_tokens",
    "tokens_cache_write",
    "cacheCreationInputTokens",
];
const REASONING_TOKEN_KEYS: &[&str] = &["reasoning_tokens", "tokens_reasoning", "reasoningTokens"];
const TOTAL_TOKEN_KEYS: &[&str] = &["total_tokens", "tokens_total", "totalTokens"];
const MESSAGE_COUNT_KEYS: &[&str] = &["message_count", "messages", "messageCount"];
const COST_KEYS: &[&str] = &["cost", "source_cost", "total_cost"];
const MODEL_KEYS: &[&str] = &["model", "model_id", "modelId", "model_name", "modelName"];
const PROVIDER_KEYS: &[&str] = &["provider", "provider_id", "providerId"];
const SESSION_KEYS: &[&str] = &[
    "session_id",
    "sessionId",
    "conversation_id",
    "conversationId",
    "chat_id",
    "thread_id",
    "threadId",
];
const ACCOUNT_KEYS: &[&str] = &["account", "email", "user_email", "userEmail"];
const TIMESTAMP_KEYS: &[&str] = &[
    "timestamp",
    "ts",
    "time",
    "created_at",
    "createdAt",
    "created",
    "date",
    "started_at",
    "updated_at",
];
const PROMPT_KEYS: &[&str] = &[
    "prompt",
    "user_prompt",
    "userPrompt",
    "input_text",
    "inputText",
    "message",
];
const TOOL_NAME_KEYS: &[&str] = &["tool_name", "toolName", "tool", "name"];
const FILE_PATH_KEYS: &[&str] = &[
    "file_path",
    "filePath",
    "path",
    "filename",
    "file",
    "target_file",
    "targetFile",
];
const OLD_TEXT_KEYS: &[&str] = &["old_string", "old_str", "oldString", "old"];
const NEW_TEXT_KEYS: &[&str] = &["new_string", "new_str", "newString", "new", "content"];
const WORKSPACE_KEYS: &[&str] = &[
    "cwd",
    "workspace",
    "workspace_path",
    "workspacePath",
    "project_path",
    "projectPath",
];
const WINDOW_KEYS: &[&str] = &[
    "window_title",
    "windowTitle",
    "title",
    "app_name",
    "appName",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::ENV_LOCK;
    use tempfile::TempDir;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn with_home<F: FnOnce(&Path)>(f: F) {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let dir = TempDir::new().unwrap();
        std::env::set_var("AITRACK_HOME", dir.path());
        std::env::set_var("AITRACK_SCAN_HOME", dir.path());
        f(dir.path());
        std::env::remove_var("AITRACK_HOME");
        std::env::remove_var("AITRACK_SCAN_HOME");
    }

    fn make_message(seed: i64) -> LocalUsageMessage {
        LocalUsageMessage {
            agent: "codex".to_string(),
            model: "gpt-5-20260101".to_string(),
            provider: "openai".to_string(),
            session_id: format!("session-{seed}"),
            account: "local".to_string(),
            timestamp_ms: 1_789_000_000_000 + seed,
            day: "2026-06-16".to_string(),
            tokens: TokenBuckets {
                input: 10,
                output: 20,
                cache_read: 3,
                cache_write: 4,
                reasoning: 5,
            },
            source_cost: 0.01,
            message_count: 1,
            dedup_key: format!("dedup-{seed}"),
        }
    }

    fn monitoring_rows() -> Vec<(String, Option<String>, String, i64, i64, Option<String>)> {
        let records = open_records_db().unwrap();
        let mut stmt = records
            .prepare(
                "SELECT metadata, prompt_summary, file_path, added_lines, removed_lines, diff_hunk
                 FROM records ORDER BY id",
            )
            .unwrap();
        stmt.query_map([], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
    }

    fn event_type(metadata: &str) -> String {
        serde_json::from_str::<Value>(metadata)
            .unwrap()
            .get("event_type")
            .and_then(Value::as_str)
            .unwrap()
            .to_string()
    }

    #[test]
    fn inserting_same_message_is_idempotent_and_excludes_workspace() {
        with_home(|_| {
            let mut conn = open_usage_db().unwrap();
            let msg = make_message(1);
            assert_eq!(insert_usage_sessions(&mut conn, &[msg.clone()]).unwrap(), 1);
            assert_eq!(insert_usage_sessions(&mut conn, &[msg]).unwrap(), 0);
            let has_workspace_column: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('usage_sessions') WHERE name LIKE 'workspace%'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(has_workspace_column, 0);
        });
    }

    #[test]
    fn rollup_groups_by_day_agent_model_account() {
        with_home(|_| {
            let mut conn = open_usage_db().unwrap();
            insert_usage_sessions(&mut conn, &[make_message(1), make_message(2)]).unwrap();
            assert_eq!(rebuild_rollups(&mut conn).unwrap(), 1);
            let row: (i64, i64, i64, i64, i64) = conn
                .query_row(
                    "SELECT tokens_in, tokens_out, tokens_cache_read, tokens_cache_write, tokens_reasoning
                     FROM usage_daily_model_rollups",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )
                .unwrap();
            assert_eq!(row, (20, 40, 6, 8, 10));
        });
    }

    #[test]
    fn json_scan_extracts_usage_and_prompt_monitoring_event() {
        with_home(|home| {
            let dir = home.join(".codex").join("sessions");
            fs::create_dir_all(&dir).unwrap();
            fs::write(
                dir.join("session.jsonl"),
                serde_json::json!({
                    "conversation_id": "sess-json",
                    "model": "gpt-5",
                    "timestamp": "2026-06-16T10:00:00Z",
                    "role": "user",
                    "content": "review the checkout flow",
                    "usage": {
                        "input_tokens": 12,
                        "output_tokens": 8,
                        "cached_input_tokens": 2
                    }
                })
                .to_string(),
            )
            .unwrap();

            let mut conn = open_usage_db().unwrap();
            let report = tokio_test::block_on(scan_into(
                &mut conn,
                UsageScanOptions {
                    tools: vec!["codex".to_string()],
                    since: None,
                    until: None,
                },
                None,
            ))
            .unwrap();
            assert_eq!(report.parsed_messages, 1);
            assert_eq!(report.monitoring_events_parsed, 1);
            assert_eq!(report.sessions_inserted, 1);
            assert_eq!(report.monitoring_records_inserted, 1);
        });
    }

    #[test]
    fn json_scan_extracts_prompt_tool_window_and_edit_monitoring_records() {
        with_home(|home| {
            let dir = home.join(".codex").join("sessions");
            fs::create_dir_all(&dir).unwrap();
            let lines = [
                serde_json::json!({
                    "session_id": "sess-monitoring",
                    "timestamp": "2026-06-16T10:00:00Z",
                    "model": "gpt-5",
                    "prompt": "audit checkout prompt monitoring"
                }),
                serde_json::json!({
                    "session_id": "sess-monitoring",
                    "timestamp": "2026-06-16T10:00:01Z",
                    "model": "gpt-5",
                    "tool_name": "Read"
                }),
                serde_json::json!({
                    "session_id": "sess-monitoring",
                    "timestamp": "2026-06-16T10:00:02Z",
                    "window_title": "checkout.rs - editor"
                }),
                serde_json::json!({
                    "session_id": "sess-monitoring",
                    "timestamp": "2026-06-16T10:00:03Z",
                    "file_path": "src/checkout.rs",
                    "old_string": "let total = subtotal;\n",
                    "new_string": "let total = subtotal + tax;\n"
                }),
            ]
            .into_iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("\n");
            fs::write(dir.join("monitoring.jsonl"), lines).unwrap();

            let mut conn = open_usage_db().unwrap();
            let report = tokio_test::block_on(scan_into(
                &mut conn,
                UsageScanOptions {
                    tools: vec!["codex".to_string()],
                    since: None,
                    until: None,
                },
                None,
            ))
            .unwrap();

            assert_eq!(report.monitoring_events_parsed, 4);
            assert_eq!(report.monitoring_records_inserted, 4);
            let rows = monitoring_rows();
            let types = rows
                .iter()
                .map(|row| event_type(&row.0))
                .collect::<Vec<_>>();
            assert_eq!(types, vec!["prompt", "tool", "window", "edit"]);
            assert_eq!(
                rows[0].1.as_deref(),
                Some("audit checkout prompt monitoring")
            );
            assert!(rows[1].2.starts_with("__aitrack__/codex/Read/"));
            assert!(rows[2].0.contains("checkout.rs - editor"));
            assert_eq!(rows[3].2, "src/checkout.rs");
            assert_eq!((rows[3].3, rows[3].4), (1, 1));
            assert!(rows[3]
                .5
                .as_deref()
                .unwrap_or_default()
                .contains("-let total"));
        });
    }

    #[test]
    fn csv_scan_extracts_prompt_tool_window_and_edit_monitoring_records() {
        with_home(|home| {
            let dir = home.join("sources").join("cursor");
            fs::create_dir_all(&dir).unwrap();
            fs::write(
                dir.join("monitoring.csv"),
                concat!(
                    "session_id,timestamp,prompt,tool_name,window_title,file_path,old_string,new_string,model\n",
                    "csv-monitoring,2026-06-16T10:00:00Z,review billing prompt,,,,,gpt-5\n",
                    "csv-monitoring,2026-06-16T10:00:01Z,,ApplyPatch,,,,,gpt-5\n",
                    "csv-monitoring,2026-06-16T10:00:02Z,,,Billing Dashboard,,,,gpt-5\n",
                    "csv-monitoring,2026-06-16T10:00:03Z,,,,src/billing.rs,old line,new line,gpt-5\n"
                ),
            )
            .unwrap();

            let mut conn = open_usage_db().unwrap();
            let report = tokio_test::block_on(scan_into(
                &mut conn,
                UsageScanOptions {
                    tools: vec!["cursor".to_string()],
                    since: None,
                    until: None,
                },
                None,
            ))
            .unwrap();

            assert_eq!(report.monitoring_events_parsed, 4);
            assert_eq!(report.monitoring_records_inserted, 4);
            let rows = monitoring_rows();
            let types = rows
                .iter()
                .map(|row| event_type(&row.0))
                .collect::<Vec<_>>();
            assert_eq!(types, vec!["prompt", "tool", "window", "edit"]);
            assert_eq!(rows[0].1.as_deref(), Some("review billing prompt"));
            assert!(rows[1].2.starts_with("__aitrack__/cursor/ApplyPatch/"));
            assert!(rows[2].0.contains("Billing Dashboard"));
            assert_eq!(rows[3].2, "src/billing.rs");
            assert_eq!((rows[3].3, rows[3].4), (1, 1));
        });
    }

    #[test]
    fn scan_now_status_and_default_tool_selection_cover_local_sources() {
        with_home(|home| {
            let dir = home.join("sources").join("opencode");
            fs::create_dir_all(&dir).unwrap();
            fs::write(
                dir.join("usage.json"),
                serde_json::json!({
                    "session_id": "default-scan",
                    "timestamp": "2026-06-16T11:00:00Z",
                    "model": "gpt-5",
                    "provider": "openai",
                    "account": "employee@example.com",
                    "input_tokens": 20,
                    "output_tokens": 30,
                    "message_count": 2,
                    "prompt": "monitor local prompt text"
                })
                .to_string(),
            )
            .unwrap();

            let report = tokio_test::block_on(scan_now(UsageScanOptions {
                tools: vec![],
                since: Some("2026-06-16".to_string()),
                until: Some("2026-06-16".to_string()),
            }))
            .unwrap();
            assert_eq!(report.parsed_messages, 1);
            assert_eq!(report.monitoring_events_parsed, 1);

            let current = status().unwrap();
            assert_eq!(current.sessions, 1);
            assert_eq!(current.rollups, 1);
            assert_eq!(current.pending_monitoring_events, 1);

            let skipped = tokio_test::block_on(scan_now(UsageScanOptions {
                tools: vec!["opencode".to_string()],
                since: Some("2026-06-17".to_string()),
                until: Some("2026-06-17".to_string()),
            }))
            .unwrap();
            assert_eq!(skipped.parsed_messages, 0);
            assert_eq!(skipped.monitoring_events_parsed, 0);
        });
    }

    #[test]
    fn discovery_helpers_cover_roots_supported_files_and_skipped_dirs() {
        with_home(|home| {
            assert!(selected_scan_tools(&[]).contains(&"codex".to_string()));
            assert_eq!(
                selected_scan_tools(&[
                    " Cursor ".to_string(),
                    "cursor".to_string(),
                    "".to_string(),
                    "TRAE".to_string()
                ]),
                vec!["cursor".to_string(), "trae".to_string()]
            );

            assert!(scan_roots(home, "claude")
                .iter()
                .any(|p| p.ends_with(".claude/projects")));
            assert!(scan_roots(home, "cursor")
                .iter()
                .any(|p| p.to_string_lossy().contains("Cursor/User/globalStorage")));
            assert!(scan_roots(home, "trae")
                .iter()
                .any(|p| p.to_string_lossy().contains("Trae")));
            assert!(scan_roots(home, "opencode")
                .iter()
                .any(|p| p.ends_with(".config/opencode")));
            assert!(scan_roots(home, "custom")
                .iter()
                .any(|p| p.ends_with(".custom")));

            let root = home.join("tree");
            fs::create_dir_all(root.join("node_modules")).unwrap();
            fs::create_dir_all(root.join("nested")).unwrap();
            fs::write(root.join("a.jsonl"), "{}").unwrap();
            fs::write(root.join("nested").join("b.sqlite3"), "").unwrap();
            fs::write(root.join("node_modules").join("ignored.json"), "{}").unwrap();
            fs::write(root.join("notes.txt"), "{}").unwrap();

            let mut files = Vec::new();
            let mut seen = HashSet::new();
            collect_supported_files(&root, &mut files, &mut seen);
            files.sort();
            assert_eq!(files.len(), 2);
            assert!(files.iter().any(|p| p.ends_with("a.jsonl")));
            assert!(files.iter().any(|p| p.ends_with("b.sqlite3")));
            assert!(skip_dir(&root.join("Cache")));
            assert!(is_supported_file(&root.join("records.db")));
            assert!(!is_supported_file(&root.join("notes.txt")));

            let mut single = Vec::new();
            let mut seen_single = HashSet::new();
            collect_supported_files(&root.join("a.jsonl"), &mut single, &mut seen_single);
            collect_supported_files(&root.join("a.jsonl"), &mut single, &mut seen_single);
            assert_eq!(single.len(), 1);
        });
    }

    #[test]
    fn json_variants_cover_arrays_nested_usage_and_large_files() {
        with_home(|home| {
            let dir = home.join("cache").join("trae");
            fs::create_dir_all(&dir).unwrap();
            fs::write(
                dir.join("nested.ndjson"),
                format!(
                    "not-json\n{}\n{}\n",
                    serde_json::json!([
                        {
                            "conversationId": "nested-usage",
                            "createdAt": "2026-06-16",
                            "modelName": " gpt-5 ",
                            "providerId": "openai",
                            "email": "dev@example.com",
                            "usage": {
                                "totalTokens": 77,
                                "messageCount": 3,
                                "total_cost": "0.42"
                            }
                        }
                    ]),
                    serde_json::json!({
                        "type": "user_message",
                        "content": [
                            {"text": "first prompt part"},
                            {"content": "second prompt part"}
                        ],
                        "sessionId": "nested-usage",
                        "timestamp": 1_789_000_000
                    })
                ),
            )
            .unwrap();
            let result = scan_text_json_file("trae", &dir.join("nested.ndjson")).unwrap();
            let usage = result
                .messages
                .iter()
                .find(|message| message.tokens.input == 77)
                .expect("nested usage total_tokens should be parsed");
            assert_eq!(usage.message_count, 3);
            assert_eq!(usage.source_cost, 0.42);
            assert!(result.events.iter().any(|event| {
                event.prompt_text.as_deref() == Some("first prompt part\nsecond prompt part")
            }));

            let oversized = dir.join("oversized.json");
            let file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&oversized)
                .unwrap();
            file.set_len(MAX_JSON_BYTES + 1).unwrap();
            let empty = scan_text_json_file("trae", &oversized).unwrap();
            assert_eq!(empty.messages.len(), 0);
            assert_eq!(empty.events.len(), 0);
        });
    }

    #[test]
    fn sqlite_scan_extracts_usage_and_monitoring_from_local_tables() {
        with_home(|home| {
            let dir = home.join("sources").join("trae");
            fs::create_dir_all(&dir).unwrap();
            let db_path = dir.join("state.sqlite");
            {
                let conn = Connection::open(&db_path).unwrap();
                conn.execute_batch(
                    r#"
                    CREATE TABLE events (
                      payload TEXT,
                      body BLOB,
                      input_tokens INTEGER,
                      output_tokens TEXT,
                      cost REAL,
                      message_count INTEGER,
                      model TEXT,
                      provider TEXT,
                      session_id TEXT,
                      account TEXT,
                      timestamp TEXT,
                      prompt TEXT,
                      tool_name TEXT,
                      window_title TEXT,
                      file_paths TEXT,
                      old_string TEXT,
                      new_string TEXT
                    );
                    CREATE TABLE boring (id INTEGER PRIMARY KEY, note TEXT);
                    "#,
                )
                .unwrap();
                conn.execute(
                    "INSERT INTO events
                     (payload, body, input_tokens, output_tokens, cost, message_count, model, provider,
                      session_id, account, timestamp, prompt, tool_name, window_title, file_paths,
                      old_string, new_string)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
                    params![
                        serde_json::json!({
                            "messages": [
                                {
                                    "role": "user",
                                    "content": {"text": "sqlite prompt body"},
                                    "created_at": "2026-06-16T12:00:00Z"
                                }
                            ]
                        })
                        .to_string(),
                        vec![1_u8, 2, 3],
                        10_i64,
                        "15",
                        1.5_f64,
                        2_i64,
                        "gpt-5",
                        "openai",
                        "sqlite-session",
                        "operator@example.com",
                        "2026-06-16T12:00:00Z",
                        "",
                        "ApplyPatch",
                        "Editor Window",
                        serde_json::json!(["src/sqlite.rs"]).to_string(),
                        "old\n",
                        "new\n"
                    ],
                )
                .unwrap();
            }

            let result = scan_source_file("trae", &db_path).unwrap();
            assert_eq!(result.messages.len(), 1);
            assert_eq!(result.messages[0].tokens.input, 10);
            assert_eq!(result.messages[0].tokens.output, 15);
            assert_eq!(result.messages[0].message_count, 2);
            assert!(result.events.len() >= 1);
            assert!(result
                .events
                .iter()
                .any(|event| event.prompt_text.as_deref() == Some("sqlite prompt body")));
            assert!(result
                .events
                .iter()
                .any(|event| event.file_path == "src/sqlite.rs"));
            assert_eq!(quote_identifier("a\"b"), "\"a\"\"b\"");
            assert!(is_interesting_column("window_title"));
            assert!(!is_interesting_column("plain_note"));
        });
    }

    #[test]
    fn csv_usage_rows_cover_token_cost_and_quote_parsing() {
        with_home(|home| {
            let path = home.join("usage.csv");
            fs::write(
                &path,
                concat!(
                    "session_id,timestamp,prompt_tokens,completion_tokens,cached_input_tokens,",
                    "cache_creation_input_tokens,reasoning_tokens,message_count,cost,model,provider,email,prompt\n",
                    "\"acct:session\",2026-06-16,10.4,20.6,3,4,5,2,0.25,gpt-5,openai,dev@example.com,\"prompt, with comma\"\n",
                    "empty,2026-06-16,0,0,0,0,0,1,0,gpt-5,openai,dev@example.com,\n"
                ),
            )
            .unwrap();
            let result = scan_csv_file("cursor", &path).unwrap();
            assert_eq!(result.messages.len(), 1);
            assert_eq!(result.messages[0].tokens.input, 10);
            assert_eq!(result.messages[0].tokens.output, 21);
            assert_eq!(result.messages[0].tokens.cache_read, 3);
            assert_eq!(result.messages[0].tokens.cache_write, 4);
            assert_eq!(result.messages[0].tokens.reasoning, 5);
            assert_eq!(result.messages[0].message_count, 2);
            assert_eq!(result.messages[0].source_cost, 0.25);
            assert_eq!(result.messages[0].account, "dev@example.com");
            assert_eq!(
                result.events[0].prompt_text.as_deref(),
                Some("prompt, with comma")
            );
            assert_eq!(
                split_csv_line("\"a,b\",\"c\"\"d\",e"),
                vec!["a,b", "c\"d", "e"]
            );
        });
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sync_now_without_api_keeps_usage_and_monitoring_pending() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let dir = TempDir::new().unwrap();
        std::env::set_var("AITRACK_HOME", dir.path());
        std::env::set_var("AITRACK_SCAN_HOME", dir.path());
        crate::config::save_config(&crate::config::Config {
            api_url: String::new(),
            credential: String::new(),
            device_id: "device-sync-local".to_string(),
        })
        .unwrap();
        let source = dir.path().join("sources").join("codex");
        fs::create_dir_all(&source).unwrap();
        fs::write(
            source.join("usage.jsonl"),
            serde_json::json!({
                "session_id": "sync-local",
                "timestamp": "2026-06-16T13:00:00Z",
                "model": "gpt-5",
                "input_tokens": 5,
                "output_tokens": 7,
                "prompt": "local sync prompt"
            })
            .to_string(),
        )
        .unwrap();

        let report = sync_now(UsageSyncOptions {
            scan: UsageScanOptions {
                tools: vec!["codex".to_string()],
                since: None,
                until: None,
            },
            api_url: None,
            credential: None,
        })
        .await
        .unwrap();
        assert_eq!(report.scan.parsed_messages, 1);
        assert_eq!(report.enqueued_rollups, 1);
        assert_eq!(report.uploaded, 0);
        assert_eq!(report.pending, 1);
        assert_eq!(report.pending_monitoring_events, 1);
        std::env::remove_var("AITRACK_HOME");
        std::env::remove_var("AITRACK_SCAN_HOME");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn subscription_snapshots_enqueue_and_upload_to_usage_endpoint() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/ai-track/usage/subscription"))
            .and(header("X-AiTrack-Device", "device-subscriptions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
            .mount(&mock_server)
            .await;

        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let dir = TempDir::new().unwrap();
        std::env::set_var("AITRACK_HOME", dir.path());
        std::env::set_var("AITRACK_SCAN_HOME", dir.path());
        crate::config::save_config(&crate::config::Config {
            api_url: mock_server.uri(),
            credential: "aitrack_testtoken12345-testhmacsecret".to_string(),
            device_id: "device-subscriptions".to_string(),
        })
        .unwrap();

        let codex_sessions = dir.path().join(".codex").join("sessions").join("2026");
        fs::create_dir_all(&codex_sessions).unwrap();
        fs::write(
            codex_sessions.join("rate.jsonl"),
            serde_json::json!({
                "payload": {
                    "type": "token_count",
                    "rate_limits": {
                        "plan_type": "pro",
                        "primary": {
                            "used_percent": 25.0,
                            "window_minutes": 300,
                            "resets_at": "2999-01-01T00:00:00Z"
                        },
                        "secondary": {
                            "used_percent": 80.0,
                            "window_minutes": 10080,
                            "resets_at": 32503680000.0
                        }
                    }
                }
            })
            .to_string(),
        )
        .unwrap();
        fs::write(
            dir.path().join("claude-rate-limits.json"),
            serde_json::json!({
                "five_hour": {
                    "used_percentage": 40,
                    "resets_at": "2999-01-01T00:00:00Z"
                },
                "seven_day": {
                    "utilization": 10,
                    "resets_at": 32503680000.0
                }
            })
            .to_string(),
        )
        .unwrap();

        let conn = open_usage_db().unwrap();
        assert_eq!(upsert_local_subscription_snapshots(&conn).unwrap(), 2);
        assert_eq!(enqueue_dirty_subscriptions(&conn).unwrap(), 2);
        assert_eq!(pending_outbox(&conn).unwrap(), 2);

        let report = drain_outbox(
            &conn,
            &mock_server.uri(),
            "aitrack_testtoken12345-testhmacsecret",
            "device-subscriptions",
        )
        .await
        .unwrap();
        assert_eq!(report.uploaded, 2);
        assert_eq!(report.failed, 0);
        assert_eq!(pending_outbox(&conn).unwrap(), 0);
        let dirty: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM usage_subscription_snapshots WHERE dirty = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(dirty, 0);
        std::env::remove_var("AITRACK_HOME");
        std::env::remove_var("AITRACK_SCAN_HOME");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn outbox_http_failure_increments_retry_and_keeps_pending() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/ai-track/usage/rollup"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let dir = TempDir::new().unwrap();
        std::env::set_var("AITRACK_HOME", dir.path());
        std::env::set_var("AITRACK_SCAN_HOME", dir.path());
        crate::config::save_config(&crate::config::Config {
            api_url: mock_server.uri(),
            credential: "aitrack_testtoken12345-testhmacsecret".to_string(),
            device_id: "device-failure".to_string(),
        })
        .unwrap();
        let mut conn = open_usage_db().unwrap();
        insert_usage_sessions(&mut conn, &[make_message(9)]).unwrap();
        rebuild_rollups(&mut conn).unwrap();
        assert_eq!(enqueue_dirty_rollups(&conn, "device-failure").unwrap(), 1);

        let report = drain_outbox(
            &conn,
            &mock_server.uri(),
            "aitrack_testtoken12345-testhmacsecret",
            "device-failure",
        )
        .await
        .unwrap();
        assert_eq!(report.uploaded, 0);
        assert_eq!(report.failed, 1);
        assert_eq!(pending_outbox(&conn).unwrap(), 1);
        let retry_count: i64 = conn
            .query_row("SELECT retry_count FROM usage_outbox", [], |row| row.get(0))
            .unwrap();
        assert_eq!(retry_count, 1);
        std::env::remove_var("AITRACK_HOME");
        std::env::remove_var("AITRACK_SCAN_HOME");
    }

    #[test]
    fn fetch_pending_outbox_rejects_unknown_payload_type() {
        with_home(|_| {
            let conn = open_usage_db().unwrap();
            conn.execute(
                "INSERT INTO usage_outbox (payload_type, natural_key, payload_json, payload_sha256)
                 VALUES ('bad', 'bad-key', '{}', 'bad-sha')",
                [],
            )
            .unwrap();
            let err = match fetch_pending_outbox(&conn) {
                Ok(_) => panic!("bad payload type should fail"),
                Err(err) => err.to_string(),
            };
            assert!(err.contains("unknown usage payload type bad"));
        });
    }

    #[test]
    fn parser_helpers_cover_edge_cases() {
        let mut map = Map::new();
        map.insert("totalTokens".to_string(), Value::from(99));
        let tokens = token_buckets_from_object(&map).unwrap();
        assert_eq!(tokens.input, 99);
        map.insert("input_tokens".to_string(), Value::from(-5));
        assert_eq!(token_buckets_from_object(&map).unwrap().input, 99);

        assert_eq!(OutboxType::from_str("rollup"), Some(OutboxType::Rollup));
        assert_eq!(
            OutboxType::Subscription.endpoint(),
            "/api/v1/ai-track/usage/subscription"
        );
        assert_eq!(OutboxType::Subscription.as_str(), "subscription");
        assert_eq!(OutboxType::from_str("other"), None);

        let prompt_obj = serde_json::json!({
            "role": "human",
            "content": {"text": "object prompt"}
        });
        assert_eq!(
            prompt_text_from_object(prompt_obj.as_object().unwrap()).as_deref(),
            Some("object prompt")
        );
        let user_event = serde_json::json!({
            "event_type": "user_message",
            "content": "event prompt"
        });
        assert_eq!(
            prompt_text_from_object(user_event.as_object().unwrap()).as_deref(),
            Some("event prompt")
        );

        let files = serde_json::json!({"files": ["src/main.rs", "src/lib.rs"]});
        assert_eq!(
            file_path_from_object(files.as_object().unwrap()).as_deref(),
            Some("src/main.rs")
        );
        assert!(
            synthetic_file_path("tool", "codex", "session-1", Some("Read File"))
                .starts_with("__aitrack__/codex/Read_File/")
        );
        assert_eq!(sanitize_path_segment(" !! "), "");

        let cursor_root = serde_json::json!({"usage": [{"account": ""}]});
        assert_eq!(
            account_from_context("cursor", "user@example.com:session", &cursor_root),
            "user@example.com"
        );
        assert_eq!(
            account_from_context("codex", "session", &cursor_root),
            "local"
        );

        let nested = serde_json::json!({"outer": [{"cost": "1.75", "count": "2.4"}]});
        assert_eq!(first_f64(&nested, &["cost"]), Some(1.75));
        assert_eq!(first_i64(&nested, &["count"]), Some(2));
        assert_eq!(value_as_string(&Value::from(42)).as_deref(), Some("42"));
        assert_eq!(value_as_i64(&Value::from(42_u64)), Some(42));
        assert_eq!(value_as_i64(&Value::from("42.6")), Some(43));
        assert_eq!(value_as_f64(&Value::from("3.5")), Some(3.5));

        assert_eq!(parse_timestamp_ms_str(""), None);
        assert_eq!(
            day_from_timestamp_ms(parse_timestamp_ms_str("2026-06-16").unwrap()),
            "2026-06-16"
        );
        assert_eq!(
            day_from_timestamp_ms(parse_timestamp_ms_str("1789000000").unwrap()),
            "2026-09-10"
        );
        assert!(parse_timestamp_ms_str("not-a-date").is_none());
        assert!(rfc3339_from_ms(1_789_000_000_000).starts_with("2026-"));
        assert!(system_time_ms(std::time::UNIX_EPOCH).is_some());
        assert!(!day_in_range("2026-06-16", Some("2026-06-17"), None));
        assert!(!day_in_range("2026-06-16", None, Some("2026-06-15")));
        assert_eq!(normalize_model("   "), "unknown");
        assert_eq!(truncate_chars("abcdef", 3), "abc");
        assert_eq!(titlecase("team"), "Team");
        assert_eq!(titlecase(""), "");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sync_uploads_json_rollup_to_usage_endpoint() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/ai-track/usage/rollup"))
            .and(header("X-AiTrack-Device", "device-test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
            .mount(&mock_server)
            .await;

        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let dir = TempDir::new().unwrap();
        std::env::set_var("AITRACK_HOME", dir.path());
        std::env::set_var("AITRACK_SCAN_HOME", dir.path());
        crate::config::save_config(&crate::config::Config {
            api_url: mock_server.uri(),
            credential: "aitrack_testtoken12345-testhmacsecret".to_string(),
            device_id: "device-test".to_string(),
        })
        .unwrap();
        let mut conn = open_usage_db().unwrap();
        insert_usage_sessions(&mut conn, &[make_message(3)]).unwrap();
        rebuild_rollups(&mut conn).unwrap();
        assert_eq!(enqueue_dirty_rollups(&conn, "device-test").unwrap(), 1);
        let report = drain_outbox(
            &conn,
            &mock_server.uri(),
            "aitrack_testtoken12345-testhmacsecret",
            "device-test",
        )
        .await
        .unwrap();
        assert_eq!(report.uploaded, 1);
        assert_eq!(pending_outbox(&conn).unwrap(), 0);
        std::env::remove_var("AITRACK_HOME");
        std::env::remove_var("AITRACK_SCAN_HOME");
    }
}
