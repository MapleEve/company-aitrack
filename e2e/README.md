# aitrack E2E Tests

End-to-end tests covering the full contract from admin token issuance through edit ingestion, anti-cheat validation, heartbeat, and stats queries. Tests run against both the Java and Go server implementations to prove wire compatibility.

## Structure

```
e2e/
  fixtures/prompts/       — real code snippets extracted from local prompt/skill files
  factory/factory.go      — shared deterministic payload builders (seed-based)
  scenarios/runner.go     — Go program: all test scenarios
  Dockerfile.e2e          — container image for the runner
  run.sh                  — local runner script (uses Docker)
  go.mod
```

## Running locally (requires Docker)

From `aitrack-selfbuilt/`:

```bash
# Run against both Java and Go (full suite)
bash e2e/run.sh both

# Java only
bash e2e/run.sh java

# Go only
bash e2e/run.sh go
```

The script:
1. Builds all three Docker images (client, server-java, server-go)
2. Starts each server in a container
3. Runs the runner against it
4. Tears down the container
5. Reports PASS/FAIL per implementation

## Real-binary client E2E (run-client-e2e.sh)

`run.sh` uses a Go runner that **simulates** the client by constructing signed
requests by hand. The full capture pipeline inside the real `aitrack` Rust binary
(stdin hook parsing → similar diff → git metadata → record_sig → SQLite insert →
flush_unsynced) was never exercised end-to-end.

`e2e/run-client-e2e.sh` closes that gap.

### What it tests

For each of the two server implementations (Java, Go) it:

1. Compiles the real `aitrack` binary via `cargo build --release`.
2. Starts the server in a Docker container.
3. Issues `POST /admin/tokens` to obtain a fresh `token`, `hmac_secret`, and `token_key`.
4. Creates an isolated `AITRACK_HOME` temp directory with `config.toml` pointing
   at the test server — the real `~/.aitrack/` is never read or written.
5. Creates a real git repo (with a commit) so the binary can resolve `repo_url`,
   `branch`, and `current_sha`.
6. Pipes real PostToolUse hook JSON into `aitrack capture --tool <tool>` for all
   three adapters: **claude**, **codex**, **cursor**.
7. Asserts the full chain for each capture:
   - Client local `$AITRACK_HOME/records.db` contains the record.
   - `record_sig` in the DB is a 64-char hex HMAC (not empty).
   - `synced=1` confirms `flush_unsynced` ran and the server accepted the record.
   - `GET /api/v1/ai-track/edits` returns the record with correct `file_path`,
     `added_lines > 0`, `hostname`, and `diff_hunk` containing `@@`.
8. Asserts `GET /api/v1/ai-track/stats` reflects the edits.
9. Runs `aitrack heartbeat` and asserts `GET /api/v1/ai-track/devices` shows the device.
10. Cleans up the container and temp directories.

### Requirements

- `docker` (images built by `e2e/run.sh` or built on first run)
- `cargo` (Rust toolchain)
- `sqlite3` CLI
- `curl`, `git`, `python3`, `uuidgen`

### Running

```bash
# From repo root — both implementations
bash e2e/run-client-e2e.sh both

# One implementation only
bash e2e/run-client-e2e.sh java
bash e2e/run-client-e2e.sh go
```

### Isolation guarantee

The script always sets `AITRACK_HOME` to a fresh `mktemp -d` directory.
The real `~/.aitrack/` directory is never touched.
A global `trap` and per-run `docker rm -f` ensure containers and temp dirs are
cleaned up on exit, interrupt, or error.

## Running with docker-compose.e2e.yml

```bash
# Java profile
docker compose -f docker/docker-compose.e2e.yml --profile java up --abort-on-container-exit

# Go profile
docker compose -f docker/docker-compose.e2e.yml --profile go up --abort-on-container-exit
```

## Scenarios

| Scenario | Description |
|---|---|
| Admin token auth | 403 wrong key, 400 missing owner, 200 valid issuance |
| Contract validation | 401 no auth / wrong token / expired ts / bad sig; 400 empty edits |
| Full happy path | issue token → POST /edits → accepted=1 → GET /edits → /stats → /devices |
| Anti-cheat | tampered record_sig → rejected; oversized → flagged; missing field → rejected |
| Heartbeat | POST /heartbeat → ok=true; GET /devices reflects device |
| Repo whitelist | unknown repo with enforce=false → accepted or flagged (not hard-rejected) |

## Fixtures

`fixtures/prompts/` contains code snippets extracted from local prompt and skill files:
- `claude_edit_snippet.txt` — Rust HMAC signature code from local Claude skill context
- `codex_edit_snippet.txt` — Go signature implementation from local workflow context
- `cursor_edit_snippet.txt` — Java validation code from local AGENTS.md context

All e2e payload content (diff hunks, old/new strings) is derived from these fixtures. No hand-written fake strings.

## Factory

`factory/factory.go` provides seed-deterministic builders:
- `DefaultEditParams(seed, tok)` — valid edit params with fixture-derived diff content
- `BuildBatchRequest(deviceID, edits...)` — full upload request JSON
- `BuildHeartbeatRequest(...)` — heartbeat body JSON
- `TamperedRecordSig(p)` / `OversizedEdit(p)` / `MissingFieldEdit(p)` — negative case builders
- `ComputeRecordSig(...)` / `ComputeRequestSig(...)` — canonical HMAC per CONTRACT.md
