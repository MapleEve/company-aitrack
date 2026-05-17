# company-aitrack

[![CI](https://img.shields.io/github/actions/workflow/status/MapleEve/company-aitrack/ci.yml?branch=main&label=CI&logo=githubactions&logoColor=white)](https://github.com/MapleEve/company-aitrack/actions)
[![Codecov](https://img.shields.io/codecov/c/github/MapleEve/company-aitrack?logo=codecov&logoColor=white)](https://codecov.io/gh/MapleEve/company-aitrack)
[![Release](https://img.shields.io/github/v/release/MapleEve/company-aitrack?logo=github)](https://github.com/MapleEve/company-aitrack/releases)
[![License](https://img.shields.io/github/license/MapleEve/company-aitrack)](LICENSE)
[![Self-hosting first](https://img.shields.io/badge/self--hosting-first-blue?logo=docker&logoColor=white)](docs/DEPLOYMENT.md)

[简体中文](README.md) | [English](README.en.md) | [日本語](README.ja.md) | [한국어](README.ko.md)

---

<img src="./docs/assets/readme/hero.en.png" alt="aitrack hero" width="100%" />

---

## Problem

<img src="./docs/assets/readme/problem.en.png" alt="Problem" width="100%" />

AI coding tools (Claude Code, Codex CLI, Cursor) have entered engineering teams at scale, creating three governance challenges that are hard to ignore:

| Pain Point | Reality |
|------------|---------|
| **AI output is hard to attribute reliably** | No native mechanism distinguishes "AI-written" from "human-written" code — reporting tools are meaningless |
| **Line-count metrics are easy to game** | Trivial pastes, redundant completions, and meaningless repetition all inflate line counts far beyond actual contribution |
| **Attribution data can be forged** | Local statistics can be modified before submission — administrators have no way to assess data trustworthiness |

---

## Who It's For

<img src="./docs/assets/readme/audience.en.png" alt="Who It's For" width="100%" />

| Role | Core Need |
|------|-----------|
| **Engineering Effectiveness Teams** | Objectively quantify actual AI tool output, identify low-efficiency usage patterns, support monthly effectiveness reports |
| **Engineering Managers** | Real-time visibility into hook installation status and suspicious data flags — no longer dependent on developer self-reporting |
| **Privacy-conscious · Self-hosting Teams** | All data stays on self-hosted infrastructure, never passes through any third-party cloud service, meeting compliance requirements |

---

## Architecture

aitrack consists of three independent components communicating via Protocol v1.1:

| Component | Stack | Responsibility |
|-----------|-------|----------------|
| **Rust client** `aitrack` | Rust · single binary · no runtime dependencies | Install hooks, capture edit events, HMAC signing, upload data |
| **Java server** `aitrack-server` | Java 17 · Spring Boot 3.2.x · H2 / PostgreSQL | 10-step validation chain, trusted attribution, effectiveness queries (primary implementation) |
| **Go server** `aitrack-server-go` | Go 1.22 · chi v5 · SQLite | Feature-equivalent lightweight alternative implementation |

**Protocol v1.1 key design:**

- All upload requests include `record_sig` (HMAC-SHA256 covering 11 core fields) and a request-level HMAC signature
- `hostname` field (new in v1.1) makes activity from a single token across multiple machines reviewable per device
- Local client database `~/.aitrack/records.db` permissions 0600, `hmac_secret` encrypted with AES-256-GCM at rest

---

## What You Get

<img src="./docs/assets/readme/outcomes.en.png" alt="What You Get" width="100%" />

### HMAC Trusted Attribution

Every edit record generates a `record_sig` at local database insert time, covering 11 fields: `token_key`, `device_id`, `hostname`, `timestamp`, `tool`, `file_path`, `repo_url`, `current_sha`, `added_lines`, `removed_lines`, `diff_hunk(SHA-256)`. The server recomputes and compares at step 4 — any tampered field is detected.

### 10-Step Server Validation Chain

| Step | Check | Failure Outcome |
|------|-------|----------------|
| 1 | Bearer token valid and active | `401` |
| 2 | `X-AiTrack-Timestamp` within ±300s (replay prevention) | `401` |
| 3 | `X-AiTrack-Signature` request HMAC matches | `401` |
| 4 | `record_sig` matches per edit | `rejected: sig_mismatch` |
| 5 | `diff_hunk` line counts consistent with `added_lines`/`removed_lines` (±1) | `flagged: diff_inconsistent` |
| 6 | `repo_url` in whitelist (configurable) | `flagged/rejected: repo_unknown` |
| 7 | `file_path` plausibility check | `flagged: path_mismatch` |
| 8 | `added_lines ≤ 5000` | `flagged: oversized` |
| 9 | Rate limit: ≤ 30 edits per (token, file_path) per hour | `rejected: rate_limited` |
| 10 | Persist (accepted + flagged edits) | — |

### Engineering Effectiveness Metrics

Query aggregated stats by developer, repository, or device via `GET /api/v1/ai-track/stats?group_by=token|repo|device` to support effectiveness reports.

### Per-hostname Manual Review

`GET /api/v1/ai-track/devices` shows each device's heartbeat status and hook installation state. When a hook is silently removed, the next execution of any `aitrack` command automatically reports the anomalous state — administrators can follow up proactively.

---

## Quick Start

### 1. Start the Server

```bash
# Generate keys
export AITRACK_SECRET_KEY=$(openssl rand -base64 32)
export AITRACK_ADMIN_KEY=$(openssl rand -hex 32)

# Build and start (H2 embedded database, suitable for quick evaluation)
docker-compose up -d --build

# Verify service
curl http://localhost:8080/actuator/health
```

### 2. Issue a Token

```bash
curl -X POST http://localhost:8080/admin/tokens \
  -H "X-Admin-Key: $AITRACK_ADMIN_KEY" \
  -H 'Content-Type: application/json' \
  -d '{"owner":"alice","note":"macbook"}'
# Returns token and hmac_secret — shown only once, store securely
```

### 3. Developer-side Hook Installation

```bash
# Build the client
cd client && cargo build --release
# Or extract binary from distribution package to /usr/local/bin/

# Install Claude Code hook
aitrack init --claude \
  --api-url https://aitrack.example.com \
  --api-token <token> \
  --hmac-secret <hmac_secret>

# Check status
aitrack status

# View local records (latest 20)
aitrack inspect --limit 20
```

### 4. View Team Data

Once developers have data flowing, administrators can query team usage and device status:

```bash
TOKEN="aitrack_abcdef1234567890abcdef1234567890"  # replace with the token issued in step 2

# Aggregated effectiveness data by developer (token) — primary entry for monthly reports
curl -s "http://localhost:8080/api/v1/ai-track/stats?group_by=token" \
  -H "Authorization: Bearer $TOKEN"

# All device heartbeats and hook installation status — investigate hook anomalies
curl -s "http://localhost:8080/api/v1/ai-track/devices" \
  -H "Authorization: Bearer $TOKEN"
```

`group_by` also accepts `repo` (by repository), `device` (by device UUID), and `hostname` (by machine name). See [docs/API.md](docs/API.md) for full details.

### 5. Coverage Verification (Docker)

```bash
# Client (Rust, coverage threshold 90%)
docker build -f docker/Dockerfile.client -t aitrack-client:latest .

# Java server (JaCoCo LINE >= 90%)
docker build -f docker/Dockerfile.server-java -t aitrack-server-java:latest .

# Go server (go tool cover >= 90%)
docker build -f docker/Dockerfile.server-go -t aitrack-server-go:latest .

# E2E (one round each for Java + Go)
bash e2e/run.sh both
```

---

## Security & Privacy

<img src="./docs/assets/readme/security.en.png" alt="Security & Privacy" width="100%" />

| Mechanism | Description |
|-----------|-------------|
| **record_sig tamper prevention** | HMAC-SHA256 covers 11 core fields, signed at local database insert, verified per-record by the server |
| **Local database 0600** | `~/.aitrack/config.toml` and `records.db` permissions are 0600, preventing reads by other users on the same machine |
| **Token AES encryption** | `hmac_secret` stored server-side with AES-256-GCM encryption, requires `AITRACK_SECRET_KEY` |
| **Token hash storage** | Server stores only `sha256(token)` — plaintext returned only once at issuance |
| **Local-first** | All data stored on self-hosted infrastructure, never passes through any third-party cloud service |
| **Constant-time comparison** | HMAC verification uses constant-time comparison to prevent timing attacks |
| **Minimal collection** | Collects only file paths, diffs, line counts, and repo metadata — no code content, conversations, or keyboard input |

---

## Documentation

| Document | Description |
|----------|-------------|
| [CONTRACT.md](CONTRACT.md) | Client/server protocol contract (endpoints, field definitions, signing spec, hook templates) |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | System architecture design (component diagram, data flow, deployment topology) |
| [docs/API.md](docs/API.md) | API reference (all endpoints, request/response structures) |
| [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) | Deployment guide (Docker, PostgreSQL migration, production configuration) |
| [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) | Developer guide (local build, module structure, contribution workflow) |
| [docs/SECURITY_MODEL.md](docs/SECURITY_MODEL.md) | Security model (threat modeling, HMAC spec, defense layers) |
| [TESTING.md](TESTING.md) | Testing system (three-tier architecture, factory pattern, coverage thresholds, Docker verification) |
| [CHANGELOG.md](CHANGELOG.md) | Version changelog |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Contribution guide (commit conventions, PR process, testing requirements) |
| [SECURITY.md](SECURITY.md) | Security vulnerability reporting process |

---

## License

<img src="./docs/assets/readme/license.en.png" alt="License" width="100%" />

[MIT License](LICENSE) © 2026 MapleEve
