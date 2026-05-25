# AiTrack Protocol Contract v1.2

This document is the single source of truth shared by the `aitrack` Rust client and the `aitrack-server` Java and Go services. All implementations MUST follow this contract exactly.

**v1.2 change:** the API token and HMAC secret are issued and stored as a single combined **credential** string (`<token>-<hmac_secret>`), so it is not obvious that two separate keys exist. See the *Credential* section below.

**v1.1 change:** added the `hostname` field — the reporting computer's OS hostname. One credential used across multiple developer machines is normal and allowed; `hostname` makes per-machine activity visible so cheating can be reviewed manually. It is NOT an access-control mechanism — no per-token isolation is added.

---

## Credential

`POST /admin/tokens` issues a single opaque **credential** instead of two separate values:

```
credential = "<token>" + "-" + "<hmac_secret>"
```

- `token` is `aitrack_<hex>` — it contains no `-` character.
- `hmac_secret` is the HMAC signing key.
- The credential is split on the **first** `-`: everything before it is the `token`, everything after it is the `hmac_secret`.
- `POST /admin/tokens` response: `{ "credential": "<token>-<hmac_secret>", "token_key": "<masked>" }` — `token` and `hmac_secret` are NOT returned as separate fields.

The client stores only `credential` (never the two parts separately). Internally it splits the credential on use: `token` goes in the `Authorization: Bearer` header; `hmac_secret` is the HMAC key for `record_sig` and `X-AiTrack-Signature`. `hmac_secret` is never sent over the wire.

---

## Components

- **Client** `aitrack`: Rust CLI (this crate)
- **Server** `aitrack-server`: Java 17 + Spring Boot 3

---

## Client Commands

```
aitrack init    [--claude] [--codex] [--cursor] [--api-url URL] [--credential CRED]
aitrack remove  [--claude] [--codex] [--cursor]
aitrack capture --tool <claude|codex|cursor>   (default: claude)  [--api-url URL] [--credential CRED]
aitrack prompt-capture --tool <claude>   (reads UserPromptSubmit stdin, stores prompt)
aitrack inspect [--limit N]  (default 20, max 200)  [--pending] [--current-token]
aitrack stats
aitrack status
aitrack clean   [--all] [--force]
aitrack heartbeat
```

`--credential` takes the single combined credential string issued by `POST /admin/tokens`.

---

## Local Storage

- Directory: `~/.aitrack/`
- `~/.aitrack/config.toml` — permissions **0600**
  - Keys: `api_url`, `credential`, `device_id`
  - `credential` is the combined `<token>-<hmac_secret>` string; the two parts are never stored separately.
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
  record_sig TEXT NOT NULL DEFAULT '',
  prompt_summary TEXT
);
CREATE INDEX IF NOT EXISTS idx_synced ON records(synced);
```

---

## Diff Algorithm (Hardening point H4)

Use true Myers/LCS minimum diff via the `similar` crate.

- `added_lines` = actual new lines added
- `removed_lines` = actual lines deleted
- `diff_hunk` = standard unified diff output from `similar` (multi-hunk supported)

This prevents inflated line counts from naive line-count statistics.

---

## Record Signature `record_sig` (Hardening points H1/H2)

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
      "record_sig": "<hex>",
      "prompt_summary": "fix the auth middleware to handle token expiry"
    }
  ]
}
```

**Note:** Edit objects contain 17 required fields + 1 optional field (`prompt_summary`). `token_key` is NOT included (local SQL filter only).
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

## Heartbeat (Hardening point H3)

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

```json
"UserPromptSubmit": [
  {
    "hooks": [
      {
        "type": "command",
        "command": "<abs aitrack path> prompt-capture --tool claude",
        "timeout": 10
      }
    ]
  }
]
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
    "postToolUse": [
      {
        "command": "<abs aitrack path> capture --tool cursor",
        "matcher": "Write",
        "timeout": 10
      }
    ],
    "afterFileEdit": [
      {
        "command": "<abs aitrack path> capture --tool cursor",
        "matcher": "Write",
        "timeout": 10
      }
    ]
  }
}
```

**Note:** Cursor registers in both `postToolUse` and `afterFileEdit` to cover all trigger paths. Both entries carry `matcher: "Write"` and `timeout: 10`.

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
8b. backfill_repo_info — UPDATE all unsynced records with empty `repo_url` to current git context (non-fatal; allows records captured outside git to eventually be uploaded)
9. Flush unsynced rows to server
10. Throttled heartbeat

On adapter parse failure: write a local log line (hardening point H6, do NOT silently swallow).

---

## Hardening Points Summary

| # | Point | What we fix |
|---|-------|-------------|
| H1 | record_sig HMAC | Prevents local DB record tampering |
| H2 | record_sig binding device_id+token | Prevents cross-device record forgery |
| H3 | Heartbeat with hook status | Detects silent hook uninstall |
| H4 | Myers/LCS diff (similar crate) | Prevents inflated line count gaming |
| H5 | Server rate limit per (token, file_path)/hour | Prevents flooding / edit count inflation |
| H6 | Parse failure logging | No silent swallowing of adapter errors |
| H7 | repo_url whitelist (enforce=true) | Prevents repo spoofing |
| H8 | file_path plausibility check (no `..`) | Prevents path injection |

---

## 4. Semantic Search Endpoints (Phase DB-3)

These endpoints are active only when the server runs in PostgreSQL/ParadeDB mode (`SPRING_PROFILES_ACTIVE=postgres` for Java; `DATABASE_URL` set for Go). Both return **HTTP 501 Not Implemented** when using embedded H2/SQLite.

Auth: **`X-Admin-Key`** header (same as `/admin/**` endpoints).

---

### 4.1 `GET /api/v1/ai-track/edits/search` — BM25 Full-Text Search

Search edit records by relevance using ParadeDB BM25 full-text index on `diff_hunk` and `prompt_summary`.

**Query Parameters**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `q` | string | ✓ | Search query text |
| `limit` | int | ✗ | Max results (default: 20, max: 100) |
| `token_key` | string | ✗ | Filter to a specific developer token |
| `repo` | string | ✗ | Filter to a specific repository |

**Response `200 OK`**

```json
{
  "query": "refactor authentication",
  "total": 3,
  "results": [
    {
      "record_id": "abc123",
      "token_key": "tk_xxxx",
      "repo": "my-org/backend",
      "file_path": "src/auth/handler.go",
      "diff_hunk": "@@ -10,5 +10,8 @@ ...",
      "ai_lines_added": 12,
      "ai_lines_removed": 3,
      "ts": 1748000000,
      "score": 0.8734
    }
  ]
}
```

**Errors**

| Status | Condition |
|--------|-----------|
| 400 | `q` is missing or empty |
| 403 | Missing or invalid `X-Admin-Key` |
| 501 | Server running in embedded DB mode (H2/SQLite) |

---

### 4.2 `POST /api/v1/ai-track/edits/similar` — Vector ANN Similarity Search

Find edit records semantically similar to a given embedding vector using pgvector HNSW cosine distance.

**Request Body**

```json
{
  "embedding": [0.023, -0.147, 0.891, "..."],
  "limit": 10,
  "token_key": "tk_xxxx",
  "repo": "my-org/backend"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `embedding` | float[] | ✓ | 384-dimensional query vector (all-MiniLM-L6-v2 space) |
| `limit` | int | ✗ | Max results (default: 10, max: 50) |
| `token_key` | string | ✗ | Filter to a specific developer token |
| `repo` | string | ✗ | Filter to a specific repository |

**Response `200 OK`**

```json
{
  "results": [
    {
      "record_id": "def456",
      "token_key": "tk_xxxx",
      "repo": "my-org/backend",
      "file_path": "src/auth/middleware.go",
      "diff_hunk": "@@ -5,3 +5,9 @@ ...",
      "ai_lines_added": 8,
      "ai_lines_removed": 1,
      "ts": 1748000100,
      "distance": 0.142
    }
  ]
}
```

`distance` is cosine distance in [0, 2] — lower means more similar.

**Errors**

| Status | Condition |
|--------|-----------|
| 400 | `embedding` missing, wrong dimension (must be 384), or `limit` > 50 |
| 403 | Missing or invalid `X-Admin-Key` |
| 501 | Server running in embedded DB mode, or no embeddings stored yet |

---

## 5. Phase 3: Developer Profile API

### Endpoint

`GET /api/v1/ai-track/profiles/{token_key}`

**Auth**: `X-Admin-Key` header

**Path param**: `token_key` (string) — token identifier, format `aitrack_XXXXX`

**Response (200)**:

```json
{
  "token_key": "aitrack_abc12…ef90",
  "owner": "alice",
  "total_edits": 247,
  "total_added_lines": 8420,
  "total_removed_lines": 3190,
  "first_seen": "2026-03-01T09:15:00Z",
  "last_seen": "2026-05-19T14:22:00Z",
  "generated_at": "2026-05-19T14:30:00Z",
  "frequency": {
    "daily_avg_30d": 8.3,
    "weekly_avg_12w": 41.2,
    "daily_trend": [{"date": "2026-05-19", "count": 12}]
  },
  "depth": {
    "avg_lines": 47.2,
    "p50_lines": 18,
    "p90_lines": 142,
    "small_count": 89,
    "medium_count": 112,
    "large_count": 46,
    "comment_density": 0.12
  },
  "languages": {"Go": 148, "TypeScript": 43, "Python": 28, "Java": 20, "Other": 8},
  "tools": {"claude": 210, "codex": 28, "cursor": 9},
  "prompt_patterns": {"generate": 148, "fix_debug": 43, "refactor": 12, "explain": 8, "test": 20, "other": 16}
}
```

**Errors**: 403 (invalid admin key), 404 (no records for token_key)

**Computation**: Includes ACCEPTED + FLAGGED records, excludes REJECTED. Computed on-demand (Phase 4 will add caching).

**`prompt_patterns`**: Categorized from `prompt_summary` text using keyword matching. Only counts records that have non-null `prompt_summary`. Categories: `generate` (generate/create/write/implement/add), `fix_debug` (fix/debug/error/bug/broken/failing), `refactor` (refactor/clean/improve/reorganize/rename), `explain` (explain/what/how/why/understand/describe), `test` (test/spec/mock/assert/verify), `other` (remainder).
