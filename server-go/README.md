# aitrack-server (Go)

Go implementation of the AiTrack server. Feature-equivalent to `server-java/` and wire-compatible with the Rust client per `CONTRACT.md`.

## Quick start

```bash
# Build
go build ./...

# Run with PostgreSQL / ParadeDB (required)
DATABASE_URL=postgres://aitrack:aitrack_secret@localhost:5432/aitrack go run .

# Run tests
go test -ldflags=-linkmode=external ./... -cover

# On Linux/Docker (no Darwin dyld workaround needed)
go test ./... -coverprofile=cover.out
go tool cover -func=cover.out | tail -1
```

## Configuration

Loaded from `config.yaml` (optional), then overridden by environment variables.

| Env var | Default | Description |
|---|---|---|
| `DATABASE_URL` | *(required)* | PostgreSQL / ParadeDB DSN, e.g. `postgres://user:pass@host:5432/db?sslmode=disable`. **Required.** |
| `AITRACK_PORT` | `8080` | HTTP listen port |
| `AITRACK_SECRET_KEY` | `""` | Base64 32-byte AES-256-GCM key for encrypting `hmac_secret` at rest. **Required in production.** Generate: `openssl rand -base64 32` |
| `AITRACK_ADMIN_KEY` | `""` | `X-Admin-Key` header value for `POST /admin/tokens`. **Required before deployment.** Generate: `openssl rand -hex 32` |
| `AITRACK_TIMESTAMP_WINDOW` | `300` | HMAC replay window (seconds) |
| `AITRACK_RATE_LIMIT_PER_HOUR` | `30` | Max edits per (token, file_path) per hour |
| `AITRACK_MAX_ADDED_LINES` | `5000` | Oversized threshold |
| `AITRACK_REPO_WHITELIST_ENFORCE` | `false` | Hard-reject edits from non-whitelisted repos |
| `AITRACK_REPO_WHITELIST_URLS` | `""` | Comma-separated allowed repo URLs |
| `AITRACK_MAX_BATCH_SIZE` | `500` | Max edits per upload request (400 if exceeded) |
| `AITRACK_MAX_REQUEST_BODY_BYTES` | `8388608` | Request body size limit in bytes, 8 MiB (413 if exceeded) |

Configuration is environment-variable-only. No `config.yaml` is supported.

## Endpoints

| Method | Path | Auth | Description |
|---|---|---|---|
| `POST` | `/admin/tokens` | `X-Admin-Key` | Issue a new token + hmac_secret |
| `POST` | `/api/v1/ai-track/edits` | Bearer + HMAC | Ingest a batch of edit records |
| `GET` | `/api/v1/ai-track/edits` | Bearer | Query edit records (paginated) |
| `POST` | `/api/v1/ai-track/heartbeat` | Bearer + HMAC | Record client heartbeat |
| `GET` | `/api/v1/ai-track/stats` | Bearer | Aggregate stats (`group_by=token\|repo\|device`) |
| `GET` | `/api/v1/ai-track/devices` | Bearer | List known devices |

### POST /admin/tokens

Request headers: `X-Admin-Key: <admin_key>`

```json
{"owner": "alice", "note": "optional"}
```

Response:
```json
{"credential": "aitrack_...-...", "token_key": "abcdef…7890"}
```

`credential` is shown only once. Store it in `~/.aitrack/config.toml` under the `credential` key.

### POST /api/v1/ai-track/edits

Headers: `Authorization: Bearer <token>`, `X-AiTrack-Timestamp`, `X-AiTrack-Signature`, `X-AiTrack-Device`, `X-AiTrack-Client`

See `CONTRACT.md` for the complete request/response schema and HMAC canonical string.

Response:
```json
{"accepted": 3, "rejected": [{"index": 1, "reason": "sig_mismatch"}], "flagged": []}
```

### GET /api/v1/ai-track/edits

Query params: `token_key`, `repo`, `page` (default 0), `size` (default 20, max 100).

### POST /api/v1/ai-track/heartbeat

See `CONTRACT.md` for body schema. Response: `{"ok": true}`.

## 10-step validation chain (parity with Java)

| Step | What | Outcome on failure |
|---|---|---|
| 1 | Bearer token present and active | 401 |
| 2 | `X-AiTrack-Timestamp` within ±300 s | 401 |
| 3 | `X-AiTrack-Signature` matches HMAC over raw body bytes | 401 |
| 4 | `record_sig` HMAC matches per-edit canonical string | REJECTED `sig_mismatch` |
| 5 | `diff_hunk` line counts consistent with `added_lines`/`removed_lines` (±1) | FLAGGED `diff_inconsistent` |
| 6 | `repo_url` in whitelist (if configured) | REJECTED `repo_not_whitelisted` (enforce=true) or FLAGGED `repo_unknown` |
| 7 | `file_path` plausibility (absolute path + remote repo) | FLAGGED `path_mismatch` |
| 8 | `added_lines` ≤ `max_added_lines` | FLAGGED `oversized` |
| 9 | Rate limit: ≤ `rate_limit_per_hour` edits per (token, file_path) per hour | REJECTED `rate_limited` |
| 10 | Persist (ACCEPTED or FLAGGED edits only) | — |

Pre-validation guard (equivalent to Java `EditValidator`): required fields null/blank → REJECTED `malformed` (prevents NPE-equivalent panic before step 4).

## HMAC canonical strings (CONTRACT.md compliance)

**request_sig**: `HMAC_SHA256(hmac_secret, timestamp + "\n" + sha256_hex(raw_body_bytes))`

**record_sig**:
```
HMAC_SHA256(hmac_secret,
  token_key + "\n" + device_id + "\n" + hostname + "\n" + timestamp + "\n" + tool + "\n"
  + file_path + "\n" + repo_url + "\n" + current_sha + "\n"
  + added_lines (decimal) + "\n" + removed_lines (decimal) + "\n"
  + sha256_hex(diff_hunk or ""))
```

Field order and `\n` separator are byte-identical to the Rust client and Java server.

## Security

- `hmac_secret` encrypted at rest with AES-256-GCM (`secret_key`). Falls back to `plain:` prefix in dev mode when `secret_key` is unset.
- All HMAC comparisons use **constant-time equality** (`subtle.ConstantTimeCompare`) to prevent timing attacks.
- `X-Admin-Key` verified with constant-time compare; server returns 503 if not configured.
- Request body limited to 8 MiB via `http.MaxBytesReader`; `edits` array limited to 500 entries per request.
- SQL uses parameterised queries throughout (no string interpolation).

## Database

PostgreSQL / ParadeDB via `pgx/v5` (pure Go, no cgo). Schema auto-migrated on startup: `tokens`, `edit_records`, `device_heartbeats`. Requires `DATABASE_URL` to be set.

For BM25 full-text search and vector ANN, use [ParadeDB](https://www.paradedb.com/) (PostgreSQL-compatible, pre-installs `pg_search` and `pgvector`).

## Parity with Java server

| Feature | Java | Go |
|---|---|---|
| ORM | Spring Data JPA / Hibernate | Raw `database/sql` |
| DB | H2 (default) / PostgreSQL | PostgreSQL / ParadeDB (pgx/v5) |
| Router | Spring MVC | chi v5 |
| Validation chain | `ValidationService` | `service.ValidationService` |
| HMAC | `SignatureService` | `service.SignatureService` |
| Encryption | `HmacSecretEncryptor` (AES-256-GCM) | `service.HmacSecretEncryptor` |
| Edit guard | `EditValidator` | `service.EditValidator` |
| Token key format | `first6…last4` | same |
| enforce=true behaviour | hard reject | same |
| null field guard | explicit before HMAC | same |

## Module

```
module github.com/aitrack/server
go 1.25
```

Key dependencies: `github.com/go-chi/chi/v5` (v5.2.5), `github.com/jackc/pgx/v5` (v5.9.x), `gopkg.in/yaml.v3`.
