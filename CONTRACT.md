# AiTrack Protocol Contract v1.1

This document is the single source of truth shared by the `aitrack` Rust client and the `aitrack-server` Java and Go services. All implementations MUST follow this contract exactly.

**v1.1 change:** added the `hostname` field — the reporting computer's OS hostname. One API token used across multiple developer machines is normal and allowed; `hostname` makes per-machine activity visible so cheating can be reviewed manually. It is NOT an access-control mechanism — no per-token isolation is added.

---

## Components

- **Client** `aitrack`: Rust CLI (this crate)
- **Server** `aitrack-server`: Java 17 + Spring Boot 3

---

## Client Commands

```
aitrack init    [--claude] [--codex] [--cursor] [--api-url URL] [--api-token TOK] [--hmac-secret S]
aitrack remove  [--claude] [--codex] [--cursor]
aitrack capture --tool <claude|codex|cursor>   (default: claude)  [--api-url URL] [--api-token TOK]
aitrack inspect [--limit N]  (default 20, max 200)  [--pending] [--current-token]
aitrack stats
aitrack status
aitrack clean   [--all] [--force]
aitrack heartbeat
```

---

## Local Storage

- Directory: `~/.aitrack/`
- `~/.aitrack/config.toml` — permissions **0600**
  - Keys: `api_url`, `token`, `device_id`, `hmac_secret`
- `~/.aitrack/records.db` — SQLite, created with `chmod 0600`
- `device_id`: UUIDv4 generated on first run, persisted to `config.toml`

### records Table Schema

```sql
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
```

---

## Diff Algorithm (Hardening point 12.4)

Use true Myers/LCS minimum diff via the `similar` crate.

- `added_lines` = actual new lines added
- `removed_lines` = actual lines deleted
- `diff_hunk` = standard unified diff output from `similar` (multi-hunk supported)

This prevents inflated line counts from naive line-count statistics.

---

## Record Signature `record_sig` (Hardening points 12.1/12.2)

Computed at insert time to prevent tampering and forgery:

```
record_sig = HMAC_SHA256(
  key = hmac_secret,
  msg = token_key + "\n"
      + device_id + "\n"
      + hostname + "\n"
      + timestamp + "\n"
      + tool + "\n"
      + file_path + "\n"
      + repo_url + "\n"
      + current_sha + "\n"
      + added_lines (decimal) + "\n"
      + removed_lines (decimal) + "\n"
      + sha256_hex(diff_hunk if NULL use empty string "")
)
```

Output: lowercase hex string.

**Field order and `\n` separator MUST be identical between client and server.**

---

## Upload Request

```
POST {api_url}/api/v1/ai-track/edits
Headers:
  Authorization: Bearer {token}
  Content-Type: application/json
  X-AiTrack-Device: {device_id}
  X-AiTrack-Client: aitrack/{version}
  X-AiTrack-Timestamp: {unix seconds}
  X-AiTrack-Signature: HMAC_SHA256(hmac_secret, "{X-AiTrack-Timestamp}\n{sha256_hex(body bytes)}")
```

### Request Body

```json
{
  "device_id": "<uuid>",
  "client_version": "1.0.0",
  "edits": [
    {
      "tool": "claude",
      "tool_version": "claude-code",
      "provider": "anthropic",
      "model": null,
      "session_id": "...",
      "repo_url": "git@github.com:org/repo.git",
      "branch": "main",
      "current_sha": "a1b2c3...",
      "file_path": "src/main.rs",
      "added_lines": 12,
      "removed_lines": 3,
      "diff_hunk": "@@ -10,7 +10,16 @@\n ...",
      "metadata": null,
      "timestamp": "2026-05-17T10:21:00Z",
      "device_id": "<uuid>",
      "hostname": "MacBook-Pro.local",
      "record_sig": "<hex>"
    }
  ]
}
```

**Note:** Edit objects contain 17 fields. `token_key` is NOT included (local SQL filter only).
`hostname` is the OS hostname of the reporting machine, captured client-side at capture time.

---

## Upload Response

```json
{
  "accepted": 3,
  "rejected": [{"index": 1, "reason": "invalid_sig"}],
  "flagged": [{"index": 2, "reason": "duplicate"}]
}
```

Client behavior:
- `accepted` + `flagged` rows: `UPDATE synced=1, synced_at=now`
- `rejected` rows: `retry_count += 1`
- Upload SQL WHERE includes `retry_count < 5`

---

## Heartbeat (Hardening point 12.3)

Detects silent hook removal:

```
POST {api_url}/api/v1/ai-track/heartbeat
Same X-AiTrack-* signature headers (X-AiTrack-Signature computed over body)

Body:
{
  "device_id": "<uuid>",
  "hostname": "MacBook-Pro.local",
  "token_key_masked": "<masked>",
  "client_version": "1.0.0",
  "ts": <unix seconds>,
  "hooks": {"claude": true, "codex": false, "cursor": false},
  "pending_count": 5
}
```

Throttle: sent at end of each `capture`, only if >1h since last heartbeat (tracked in config or DB).
`aitrack heartbeat` forces immediate send.

---

## Hook Templates

### Claude Code (`~/.claude/settings.json`)

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "apply_patch|Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "<abs aitrack path> capture --tool claude",
            "timeout": 10
          }
        ]
      }
    ]
  }
}
```

### Codex CLI (`~/.codex/config.toml`)

```toml
# aitrack
[[hooks.PostToolUse]]
matcher = "apply_patch|Edit|Write"

[[hooks.PostToolUse.hooks]]
type = "command"
command = "<abs aitrack path> capture --tool codex"
timeout = 10
```

### Cursor (`~/.cursor/hooks.json`)

```json
{
  "hooks": {
    "afterFileEdit": [
      {
        "command": "<abs aitrack path> capture --tool cursor"
      }
    ]
  }
}
```

**Note:** Cursor hook has NO `timeout` field. Claude and Codex have `timeout = 10`.

All install/remove operations MUST be idempotent (dedup on install, clean empty containers on remove).

---

## Capture Flow

1. Read stdin JSON
2. Select adapter by `--tool` (claude/codex/cursor)
3. Parse payload per adapter struct
4. Compute diff using `similar` (Myers LCS)
5. Spawn `git` for repo metadata: `rev-parse --git-dir`, `remote get-url origin`, `branch --show-current`, `rev-parse HEAD`
6. Resolve `hostname` (OS hostname of the reporting machine)
7. Compute `record_sig`
8. Insert with 2-second dedup window
9. Flush unsynced rows to server
10. Throttled heartbeat

On adapter parse failure: write a local log line (hardening point 12.8, do NOT silently swallow).

---

## Hardening Points Summary

| # | Point | What we fix |
|---|-------|-------------|
| 12.1 | record_sig HMAC | Prevents local DB record tampering |
| 12.2 | record_sig binding device_id+token | Prevents cross-device record forgery |
| 12.3 | Heartbeat with hook status | Detects silent hook uninstall |
| 12.4 | Myers/LCS diff (similar crate) | Prevents inflated line count gaming |
| 12.8 | Parse failure logging | No silent swallowing of adapter errors |
