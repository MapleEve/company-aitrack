/// DDL for the core `records` table and its index.
pub const CREATE_TABLE_SQL: &str = "
PRAGMA auto_vacuum = INCREMENTAL;
PRAGMA journal_mode = WAL;
PRAGMA journal_size_limit = 67108864;
PRAGMA wal_autocheckpoint = 1000;
PRAGMA busy_timeout = 5000;

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
  embedding BLOB,
  prompt_summary TEXT
);
CREATE INDEX IF NOT EXISTS idx_synced ON records(synced);
CREATE INDEX IF NOT EXISTS idx_record_sig ON records(record_sig);
CREATE UNIQUE INDEX IF NOT EXISTS idx_record_sig_nonempty ON records(record_sig) WHERE record_sig != '';
CREATE INDEX IF NOT EXISTS idx_records_pending_retry ON records(synced, retry_count, id);
CREATE INDEX IF NOT EXISTS idx_records_pending_identity ON records(synced, token_key, record_sig, id);
";

/// Idempotent migrations applied after table creation.
pub const MIGRATIONS: &[&str] = &[
    "ALTER TABLE records ADD COLUMN device_id TEXT NOT NULL DEFAULT ''",
    "ALTER TABLE records ADD COLUMN hostname TEXT NOT NULL DEFAULT ''",
    "ALTER TABLE records ADD COLUMN record_sig TEXT NOT NULL DEFAULT ''",
    "ALTER TABLE records ADD COLUMN embedding BLOB",
    "ALTER TABLE records ADD COLUMN prompt_summary TEXT",
    "CREATE INDEX IF NOT EXISTS idx_record_sig ON records(record_sig)",
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_record_sig_nonempty ON records(record_sig) WHERE record_sig != ''",
    "CREATE INDEX IF NOT EXISTS idx_records_pending_retry ON records(synced, retry_count, id)",
    "CREATE INDEX IF NOT EXISTS idx_records_pending_identity ON records(synced, token_key, record_sig, id)",
];

/// DDL for the key-value store table.
pub const CREATE_KV_TABLE_SQL: &str =
    "CREATE TABLE IF NOT EXISTS kv (key TEXT PRIMARY KEY, value INTEGER NOT NULL);";

/// DDL for the prompt context table.
pub const CREATE_PROMPT_CONTEXT_TABLE_SQL: &str = "
CREATE TABLE IF NOT EXISTS prompt_context (
  session_id TEXT NOT NULL,
  prompt_text TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_pc_sess ON prompt_context(session_id, created_at);
";
