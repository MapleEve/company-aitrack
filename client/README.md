# aitrack

Hardened AI coding edit telemetry CLI — captures editor hook events, signs them with HMAC, and reports verified attribution to the aitrack server.

## Commands

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

## Installation

```bash
cargo build --release
# Binary: target/release/aitrack
aitrack init --claude --api-url https://your-server --api-token YOUR_TOKEN --hmac-secret YOUR_SECRET
```

## Local Storage

- `~/.aitrack/config.toml` (0600) — api_url, token, device_id, hmac_secret
- `~/.aitrack/records.db` (0600) — SQLite, all captured edits

## Hardening Points

| Point | Description |
|-------|-------------|
| **12.1** | `record_sig`: HMAC-SHA256 over each record prevents local DB tampering |
| **12.2** | Signature binds `device_id` + `token_key` + `repo` — cross-device forgery impossible |
| **12.3** | Heartbeat reports hook install status; throttled to 1/hour; forced with `aitrack heartbeat` |
| **12.4** | Myers/LCS diff via `similar` crate — actual added/removed lines, not naive line counts |
| **12.8** | Adapter parse failures are logged to stderr, not silently swallowed |

## Upload Protocol

```
POST {api_url}/api/v1/ai-track/edits
Authorization: Bearer {token}
X-AiTrack-Device: {device_id}
X-AiTrack-Client: aitrack/{version}
X-AiTrack-Timestamp: {unix_seconds}
X-AiTrack-Signature: HMAC_SHA256(hmac_secret, "{ts}\n{sha256(body)}")
```

See `../CONTRACT.md` for the full shared client-server protocol.

## Testing

### Run tests

```bash
cargo test
```

### Coverage measurement

```bash
# Install (one-time)
cargo install cargo-llvm-cov

# Measure
cargo llvm-cov --summary-only
```

### Test structure

All tests are `#[cfg(test)]` inline modules within each source file. HTTP mocking uses `wiremock`. Temp files use `tempfile`.

| Module | Tests | Line Coverage |
|--------|-------|---------------|
| `adapters/claude.rs` | 9 | 98.6% |
| `adapters/codex.rs` | 9 | 99.3% |
| `adapters/cursor.rs` | 8 | 100% |
| `config.rs` | 17 | 83.9% |
| `crypto.rs` | 13 | 100% |
| `db.rs` | 15 | 91.7% |
| `diff.rs` | 12 | 100% |
| `git.rs` | 4 | 97.2% |
| `heartbeat.rs` | 9 | 97.4% |
| `init.rs` | 23 | 95.1% |
| `uploader.rs` | 12 | 99.0% |
| `testkit/factories.rs` | (factory module) | 95.1% |
| **TOTAL** | **140** | **87.75% lines / 90.24% functions** |

`main.rs` (CLI dispatch) is excluded from the table — it is binary entry-point code not exercised by unit tests.

### Testkit factories

`src/testkit/factories.rs` provides seed-deterministic builders for all domain objects:

- `EditRecordFactory::new(seed).with_*().build()` → `Record`
- `ApiConfigFactory::new(seed).with_*().build()` → `Config`
- `ClaudeHookPayloadFactory::new(seed).with_*().build_json()` → JSON string
- `CodexHookPayloadFactory::new(seed).with_*().build_json()` → JSON string
- `CursorHookPayloadFactory::new(seed).with_*().build_json()` → JSON string
- `tampered_record_sig(seed)` → Record with corrupted sig
- `tampered_expired_timestamp(seed)` → Record with year-2000 timestamp
- `tampered_oversized_lines(seed)` → Record with 99,999,999 lines
- `malformed_json()` → Syntactically broken JSON string
- `codex_wrong_event(seed)` / `codex_non_edit_tool(seed)` → Filter-path JSON

## Module Layout

```
src/
  main.rs         — command dispatch
  cli.rs          — clap argument definitions
  config.rs       — ~/.aitrack/config.toml read/write, token masking
  db.rs           — SQLite records table, CRUD operations
  crypto.rs       — HMAC-SHA256, record_sig, request_sig
  diff.rs         — Myers/LCS diff via similar crate
  git.rs          — spawn git for repo metadata
  init.rs         — install/remove hooks (Claude/Codex/Cursor)
  uploader.rs     — flush unsynced records to server
  heartbeat.rs    — throttled heartbeat POST
  adapters/
    mod.rs
    claude.rs     — parse Claude Code PostToolUse payload
    codex.rs      — parse Codex CLI postToolUse payload
    cursor.rs     — parse Cursor afterFileEdit payload
```
