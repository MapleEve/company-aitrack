# aitrack-server

Hardened AI coding edit telemetry server. Spring Boot 3.x / Java 17 / H2 (or PostgreSQL).

## Requirements

- JDK 17+
- Maven 3.8+

## Run

```bash
mvn spring-boot:run
# Server starts on http://localhost:8080
```

H2 console available at `http://localhost:8080/h2-console`
(JDBC URL: `jdbc:h2:file:./data/aitrack`, no password)

## Switch to PostgreSQL

In `application.yml`, replace the `spring.datasource` and `spring.jpa.database-platform` blocks:

```yaml
spring:
  datasource:
    url: jdbc:postgresql://localhost:5432/aitrack
    driver-class-name: org.postgresql.Driver
    username: aitrack
    password: secret
  jpa:
    database-platform: org.hibernate.dialect.PostgreSQLDialect
```

Add the PostgreSQL driver to `pom.xml`:
```xml
<dependency>
    <groupId>org.postgresql</groupId>
    <artifactId>postgresql</artifactId>
</dependency>
```

## Endpoints

### Admin

| Method | Path | Description |
|--------|------|-------------|
| POST | `/admin/tokens` | Issue a new token |

**Issue token** — returns plaintext token (shown once only):
```bash
curl -X POST http://localhost:8080/admin/tokens \
  -H 'Content-Type: application/json' \
  -d '{"owner":"alice","note":"CI pipeline"}'
# → {"token":"aitrack_...","hmac_secret":"...","token_key":"abc123…ef01"}
```

> In production, secure `/admin/**` with network ACL or an admin secret header.

### Client API

All signed endpoints require:
- `Authorization: Bearer {token}`
- `X-AiTrack-Device: {device_id}`
- `X-AiTrack-Client: aitrack/{version}`
- `X-AiTrack-Timestamp: {unix_seconds}`
- `X-AiTrack-Signature: HMAC_SHA256(hmac_secret, "{timestamp}\n{sha256_hex(body)}")`

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/v1/ai-track/edits` | Submit edit batch |
| POST | `/api/v1/ai-track/heartbeat` | Device heartbeat |
| GET | `/api/v1/ai-track/stats?group_by=token\|repo\|device` | Aggregated stats |
| GET | `/api/v1/ai-track/devices` | Device list with heartbeat status |
| GET | `/api/v1/ai-track/edits?token_key=&repo=&page=&size=` | Paginated edit query |

### POST /api/v1/ai-track/edits

Body:
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
      "current_sha": "abc123",
      "file_path": "src/main.rs",
      "added_lines": 10,
      "removed_lines": 3,
      "diff_hunk": "@@ -1,3 +1,10 @@\n-old\n+new\n",
      "metadata": null,
      "timestamp": "2026-05-17T10:00:00Z",
      "device_id": "<uuid>",
      "hostname": "MacBook-Pro.local",
      "record_sig": "<HMAC_SHA256 of canonical string>"
    }
  ]
}
```

**record_sig canonical string** (fields joined with `\n`):
```
token_key + "\n" + device_id + "\n" + hostname + "\n" + timestamp + "\n" + tool + "\n" +
file_path + "\n" + repo_url + "\n" + current_sha + "\n" +
added_lines(decimal) + "\n" + removed_lines(decimal) + "\n" +
sha256_hex(diff_hunk if null use "")
```

Response:
```json
{"accepted": 1, "rejected": [], "flagged": []}
```

## Hardening Points (anti-cheat validation chain)

| Step | Check | Outcome on failure | Hardening ref |
|------|-------|--------------------|---------------|
| 1 | Bearer token active | `401` | — |
| 2 | `X-AiTrack-Timestamp` within 300s of server time | `401` | 12.2 replay prevention |
| 3 | `X-AiTrack-Signature` HMAC matches | `401` | — |
| 4 | `record_sig` HMAC matches per edit | `rejected: sig_mismatch` | 12.1 local DB tampering |
| 5 | diff_hunk parsed line counts match ±1 | `flagged: diff_inconsistent` | 12.4 fabricated diffs |
| 6 | `repo_url` in whitelist (if enforce=true) | `flagged: repo_unknown` | 12.9 repo spoofing |
| 7 | `file_path` plausibility vs `repo_url` | `flagged: path_mismatch` | 12.10 path injection |
| 8 | `added_lines > 5000` | `flagged: oversized` | 12.1/12.4 line inflation |
| 9 | Rate limit per (token_key, file_path) per hour | `rejected: rate_limited` | 12.5 flooding |
| 10 | ACCEPTED/FLAGGED edits persisted to DB | — | All surviving edits stored |

Flagged edits are still ingested (client sees them as accepted); the server side-channels them for review.

## token_key derivation

`token_key` = strip `"aitrack_"` prefix, then `first_6 + "…" + last_4`.
Example: `"aitrack_abcdef1234567890abcdef1234567890"` → `"abcdef…7890"`.

The server stores only `sha256(token)` — the plaintext token is shown once at issuance.
`hmac_secret` is stored encrypted at rest with AES-256-GCM (`HmacSecretEncryptor`); it is decrypted in memory only when needed to recompute `record_sig`.

## Testing

### Running Tests

```bash
# Run all tests (unit + integration against H2 in-memory DB)
mvn test

# Run tests + coverage check (fails if LINE coverage < 90%)
mvn verify
```

### Test Structure

```
src/test/java/com/aitrack/server/
├── testkit/                        # Test factory classes
│   ├── EditDtoFactory.java         # Valid EditDto with builder-style overrides
│   ├── EditBatchRequestFactory.java
│   ├── TokenEntityFactory.java     # Pre-decrypted TokenEntity (plaintext hmacSecret)
│   ├── CreateTokenRequestFactory.java
│   ├── HeartbeatRequestFactory.java
│   ├── HookPayloadFactory.java     # Raw PostToolUse JSON (claude/codex/cursor)
│   ├── UploadResponseFactory.java
│   └── TamperedFactory.java        # Negative examples: bad sig, oversized, null fields
├── SignatureServiceTest.java        # SHA-256, HMAC-SHA256 known-value tests (pre-existing)
├── SignatureServiceCanonicalTest.java  # record_sig canonical string vs CONTRACT.md
├── DiffConsistencyServiceTest.java  # Diff parsing, allowed delta (pre-existing)
├── ValidationChainTest.java         # Step-level chain tests (pre-existing)
├── ValidationServiceTest.java       # All 10 validation steps, every branch
├── EditValidatorTest.java           # All required-field malformed cases
├── HmacSecretEncryptorTest.java     # AES-256-GCM encrypt/decrypt, plain: fallback
├── TokenServiceTest.java            # computeTokenKey variants (pre-existing)
├── IngestServiceTest.java           # Full ingest path: accept/flag/reject, entity fields
├── HeartbeatServiceTest.java        # New device creation, existing device update, hooks JSON
├── StatsServiceTest.java            # getStats (token/repo/device), getDevices silent flag
├── EditsControllerTest.java         # MockMvc: auth steps 1-3, valid POST, malformed, GET
├── HeartbeatControllerTest.java     # MockMvc: auth, missing device_id, repeat heartbeat
├── AdminTokenControllerTest.java    # MockMvc: X-Admin-Key auth, 403, 400, token uniqueness
└── StatsControllerTest.java         # MockMvc: /stats group_by variants, /devices, 401
```

### Factory Pattern

All factories follow the same contract:

```java
// Default valid instance
EditDto dto = EditDtoFactory.build();

// Builder-style override (record_sig must be recomputed for sig-bound fields)
EditDto dto = EditDtoFactory.with(e -> e.setToolVersion("claude-code-v2"));

// Tool-specific variant
EditDto dto = EditDtoFactory.buildForTool("codex");

// Negative example (for rejection/flagging tests)
EditDto bad = TamperedFactory.badRecordSig();     // sig_mismatch
EditDto bad = TamperedFactory.oversizedAddedLines(); // oversized flag
EditDto bad = TamperedFactory.nullTool();          // malformed rejection
```

### JaCoCo Coverage

JaCoCo is configured in `pom.xml` with three executions:

| Execution | Phase | Goal | Effect |
|-----------|-------|------|--------|
| `prepare-agent` | `initialize` | `prepare-agent` | Instruments bytecode for coverage tracking |
| `report` | `test` | `report` | Generates HTML/XML/CSV report in `target/site/jacoco/` |
| `check` | `verify` | `check` | Fails build if LINE coverage < 90% |

**Coverage gate:** `LINE COVEREDRATIO >= 0.90` at BUNDLE level.

Excluded from the gate (cannot be meaningfully unit-tested):
- `AiTrackServerApplication` — Spring Boot entry point
- `WebConfig` — MVC config with no logic

View the HTML report after `mvn verify`:
```
target/site/jacoco/index.html
```

### Classes Covered by Tests

| Class | Test file(s) |
|-------|-------------|
| `SignatureService` | `SignatureServiceTest`, `SignatureServiceCanonicalTest` |
| `DiffConsistencyService` | `DiffConsistencyServiceTest` |
| `EditValidator` | `EditValidatorTest` |
| `ValidationService` | `ValidationServiceTest`, `ValidationChainTest` |
| `HmacSecretEncryptor` | `HmacSecretEncryptorTest` |
| `TokenService` | `TokenServiceTest`, `AdminTokenControllerTest` |
| `IngestService` | `IngestServiceTest`, `EditsControllerTest` |
| `HeartbeatService` | `HeartbeatServiceTest`, `HeartbeatControllerTest` |
| `StatsService` | `StatsServiceTest`, `StatsControllerTest` |
| `EditsController` | `EditsControllerTest` |
| `HeartbeatController` | `HeartbeatControllerTest` |
| `AdminTokenController` | `AdminTokenControllerTest` |
| `StatsController` | `StatsControllerTest` |
| `RequestAuthHelper` | `EditsControllerTest`, `HeartbeatControllerTest`, `StatsControllerTest` |
| `AiTrackProperties` | All integration tests via Spring context |

### Docker / JDK-17 Verification Required

Because no local JDK 17 is available, the following must be confirmed in Docker:

1. `mvn test` passes with zero failures
2. `mvn verify` passes the JaCoCo LINE coverage gate (≥ 0.90)
3. Controller integration tests start the full Spring context with H2 — confirm no classpath issues
4. `HmacSecretEncryptorTest` verifies AES-256-GCM — JVM `javax.crypto` must support 256-bit keys (standard in JDK 17)

## Configuration

Key `application.yml` settings:

| Key | Default | Description |
|-----|---------|-------------|
| `aitrack.timestamp-window-seconds` | `300` | Max clock skew for replay prevention |
| `aitrack.rate-limit-per-hour` | `30` | Max edits per (token_key, file_path) per hour |
| `aitrack.max-added-lines` | `5000` | Oversized edit threshold |
| `aitrack.repo-whitelist.enforce` | `false` | Whether to hard-reject unknown repos |
| `aitrack.repo-whitelist.urls` | `[]` | Allowed repo URLs |
