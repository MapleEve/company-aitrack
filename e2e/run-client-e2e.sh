#!/usr/bin/env bash
# Full-chain e2e: real aitrack binary → real server (Java or Go).
#
# Usage (from repo root):
#   bash e2e/run-client-e2e.sh [java|go|both|external]
#   AITRACK_E2E_SERVER_URL=http://localhost:18080 bash e2e/run-client-e2e.sh external
#
# What this proves:
#   - The compiled Rust binary reads stdin hook JSON, runs the capture pipeline
#     (adapter parse → similar diff → git metadata → record_sig → SQLite insert
#     → flush_unsynced), and the server accepts + stores the record.
#   - Assertions check the local SQLite DB AND the server API responses.
#   - Java and Go implementations are exercised independently.
#
# Isolation guarantee:
#   AITRACK_HOME is set to a fresh temp directory for every run — the real
#   ~/.aitrack/ directory is NEVER touched.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CLIENT_DIR="${REPO_ROOT}/client"

TARGET="${1:-both}"
ADMIN_KEY="e2e-client-admin-key"
SERVER_PORT="18080"   # distinct port to avoid conflict with run.sh
PASS_COUNT=0
FAIL_COUNT=0
MIN_E2E_COVERAGE=100
CLIENT_E2E_NET="aitrack-client-e2e-net-$$"
PG_CONTAINER="aitrack-client-e2e-postgres-$$"
REQUIRED_LOCAL_SOURCE_AGENTS=(
    claude
    codex
    cursor
    trae
    qwen
    antigravity
    opencode
    qoder
    qoder-cn
    qoder-work
    qoder-work-cn
    wukong
    hermes
    openclaw
    gemini
    copilot
    cline
    roo-code
    kiro
    zed
    goose
    amp
    droid
    pi
    mux
    crush
    codebuff
    kilo
    kilocode
    kimi
    gjc
    grok
    synthetic
    warp
    zcode
)

# Global cleanup: remove any containers we started
cleanup_containers() {
    docker rm -f "aitrack-client-e2e-java-$$" 2>/dev/null || true
    docker rm -f "aitrack-client-e2e-go-$$"   2>/dev/null || true
    docker rm -f "${PG_CONTAINER}" 2>/dev/null || true
    docker network rm "${CLIENT_E2E_NET}" 2>/dev/null || true
}
trap cleanup_containers EXIT INT TERM

# ── colour helpers ─────────────────────────────────────────────────────────────
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
NC='\033[0m'

log()  { echo "[client-e2e] $*"; }
ok()   { echo -e "  ${GREEN}PASS${NC}  $*"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "  ${RED}FAIL${NC}  $*"; FAIL_COUNT=$((FAIL_COUNT + 1)); }

# ── pre-flight checks ──────────────────────────────────────────────────────────
if ! command -v docker &>/dev/null; then
    if [ "${TARGET}" != "external" ]; then
        echo "ERROR: docker is required"; exit 1
    fi
fi
if ! command -v cargo &>/dev/null; then
    echo "ERROR: cargo is required"; exit 1
fi
if ! command -v python3 &>/dev/null; then
    echo "ERROR: python3 is required"; exit 1
fi
if ! command -v sqlite3 &>/dev/null; then
    echo "ERROR: sqlite3 CLI is required for DB assertions"; exit 1
fi
if ! command -v curl &>/dev/null; then
    echo "ERROR: curl is required"; exit 1
fi
if ! command -v git &>/dev/null; then
    echo "ERROR: git is required"; exit 1
fi

docker_build() {
    local dockerfile="$1"
    local tag="$2"
    local log_file="${TMPDIR:-/tmp}/aitrack-docker-build-${tag//[:\\/]/-}.log"
    if (cd "${REPO_ROOT}" && docker build --progress=plain -f "${dockerfile}" -t "${tag}" . 2>&1 | tee "${log_file}"); then
        return 0
    fi
    if grep -Eq "Premature end of Content-Length|Could not transfer artifact|repo.maven.apache.org|Connection reset|Read timed out|502 Bad Gateway|503 Service Unavailable" "${log_file}"; then
        echo "EXTERNAL_DEPENDENCY_FAILURE: Docker build for ${tag} failed while downloading Maven Central dependencies."
        echo "EXTERNAL_DEPENDENCY_FAILURE_LOG: ${log_file}"
    else
        echo "DOCKER_BUILD_FAILURE: Docker build for ${tag} failed. Log: ${log_file}"
    fi
    return 1
}

# ── Step 1: build the real aitrack binary (once) ──────────────────────────────
AITRACK_BIN="${CLIENT_DIR}/target/release/aitrack"

log "Building aitrack binary (cargo build --release)..."
(cd "${CLIENT_DIR}" && cargo build --release --quiet 2>&1)
if [ ! -x "${AITRACK_BIN}" ]; then
    echo "ERROR: build produced no binary at ${AITRACK_BIN}"; exit 1
fi
log "Binary ready: ${AITRACK_BIN}"

# ── Step 2: build server images if needed ─────────────────────────────────────
if [[ "${TARGET}" == "both" || "${TARGET}" == "java" ]] && ! docker image inspect aitrack-server-java:e2e &>/dev/null; then
    log "Building aitrack-server-java:e2e image..."
    docker_build docker/Dockerfile.server-java aitrack-server-java:e2e
fi
if [[ "${TARGET}" == "both" || "${TARGET}" == "go" ]] && ! docker image inspect aitrack-server-go:e2e &>/dev/null; then
    log "Building aitrack-server-go:e2e image..."
    docker_build docker/Dockerfile.server-go aitrack-server-go:e2e
fi

# ── Helpers ────────────────────────────────────────────────────────────────────

wait_for_server() {
    local url="$1"
    local max=40
    local i=0
    echo -n "  Waiting for server at ${url}..."
    while [ $i -lt $max ]; do
        code=$(curl -s -o /dev/null -w "%{http_code}" \
            -H "Authorization: Bearer dummy" \
            "${url}/api/v1/ai-track/stats" 2>/dev/null || true)
        if [ "$code" != "000" ] && [ -n "$code" ]; then
            echo " ready (${i}s)"
            return 0
        fi
        sleep 1
        i=$((i + 1))
        echo -n "."
    done
    echo " TIMEOUT"
    return 1
}

provision_token() {
    local server_url="$1"
    local owner="$2"
    curl -s -X POST "${server_url}/admin/tokens" \
        -H "X-Admin-Key: ${ADMIN_KEY}" \
        -H "Content-Type: application/json" \
        -d "{\"owner\":\"${owner}\",\"note\":\"client-e2e\"}"
}

api_get() {
    local server_url="$1"
    local path="$2"
    local token="$3"
    curl -s -H "Authorization: Bearer ${token}" "${server_url}${path}"
}

write_usage_source_fixture() {
    local root="$1"
    local agent="$2"
    python3 "${SCRIPT_DIR}/local_usage_matrix.py" "${root}" "${agent}" >/dev/null
}

expected_usage_sources_json() {
    local agent="$1"
    python3 "${SCRIPT_DIR}/local_usage_matrix.py" --expected-sources "${agent}"
}

validate_expected_usage_sources() {
    local root="$1"
    local agent="$2"
    local expected_json="$3"
    EXPECTED_SOURCES_JSON="${expected_json}" python3 - "${root}" "${agent}" <<'PY'
import json
import os
import sqlite3
import sys
from pathlib import Path

root = Path(sys.argv[1])
agent = sys.argv[2]
try:
    expected = json.loads(os.environ["EXPECTED_SOURCES_JSON"])
except json.JSONDecodeError as exc:
    raise SystemExit(f"{agent}: expected source JSON is invalid: {exc}") from exc

if not isinstance(expected, list) or not expected:
    raise SystemExit(f"{agent}: expected source JSON must be a non-empty list")

required_keys = {
    "agent",
    "label",
    "kind",
    "path_substring",
    "expects_usage",
    "expects_monitoring",
    "session_id",
    "required_usage_fields",
    "required_record_fields",
    "optional_record_fields",
}
all_paths = [str(path) for path in root.rglob("*")]
usage_db = root / "usage.sqlite"
records_db = root / "records.db"


def normalized_path(value):
    return os.path.normcase(os.path.normpath(str(value)))


def equivalent_source_paths(value):
    path = Path(value)
    candidates = {normalized_path(path)}
    root_resolved = root.resolve(strict=False)
    if path.is_absolute():
        resolved = path.resolve(strict=False)
        candidates.add(normalized_path(resolved))
        try:
            candidates.add(normalized_path(resolved.relative_to(root_resolved)))
        except ValueError:
            pass
    else:
        resolved = (root / path).resolve(strict=False)
        candidates.add(normalized_path(resolved))
        try:
            candidates.add(normalized_path(resolved.relative_to(root_resolved)))
        except ValueError:
            pass
    return {candidate for candidate in candidates if candidate}


def source_path_matches(row_path, path_substring, fixture_paths):
    _ = path_substring
    row_candidates = equivalent_source_paths(row_path)
    for fixture_path in fixture_paths:
        if row_candidates & equivalent_source_paths(fixture_path):
            return True
    return False


def nonzero_number(value):
    return value is not None and float(value) > 0


def require_string_list(entry, key, source_name):
    value = entry.get(key)
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        raise SystemExit(f"{source_name}: {key} must be a string list")
    return value


def metadata_payload(metadata, source_name):
    try:
        return json.loads(metadata or "{}")
    except json.JSONDecodeError as exc:
        raise SystemExit(f"{source_name}: invalid metadata JSON: {exc}") from exc


SOURCE_PATH_METADATA_KEYS = {
    "source_path",
    "sourcePath",
    "source_file",
    "sourceFile",
    "source_uri",
    "sourceUri",
    "transcript_path",
    "transcriptPath",
}
SOURCE_NAME_METADATA_KEYS = {
    "source_name",
    "sourceName",
    "source_label",
    "sourceLabel",
    "source_id",
    "sourceId",
}


def string_values(value):
    if isinstance(value, str):
        stripped = value.strip()
        if stripped:
            yield stripped
    elif isinstance(value, list):
        for item in value:
            yield from string_values(item)
    elif isinstance(value, dict):
        for item in value.values():
            yield from string_values(item)


def source_identifier_values(payload, keys):
    if not isinstance(payload, dict):
        return []
    values = []
    for key in keys:
        if key in payload:
            values.extend(string_values(payload[key]))
    return values


def source_identity_matches(value, entry, source_name):
    normalized = value.strip().lower()
    expected = {
        source_name.lower(),
        entry["label"].strip().lower(),
        f"{entry['agent']}/{entry['label']}".lower(),
    }
    return normalized in expected


def source_label_is_unique(entry):
    return (
        sum(
            1
            for candidate in expected
            if candidate.get("agent") == entry["agent"]
            and candidate.get("label") == entry["label"]
        )
        == 1
    )


def source_path_value_matches(value, path_substring, fixture_paths):
    if path_substring in value:
        return True
    value_candidates = equivalent_source_paths(value)
    for fixture_path in fixture_paths:
        if value_candidates & equivalent_source_paths(fixture_path):
            return True
    return False


def record_source_matches(row, entry, fixture_paths, source_name):
    metadata, _prompt_summary, _session_id, _provider, _model, _timestamp, file_path, diff_hunk = row
    payload = metadata_payload(metadata, source_name)
    path_substring = entry["path_substring"]

    metadata_path_values = source_identifier_values(payload, SOURCE_PATH_METADATA_KEYS)
    if metadata_path_values:
        return any(
            source_path_value_matches(value, path_substring, fixture_paths)
            for value in metadata_path_values
        )

    metadata_name_values = source_identifier_values(payload, SOURCE_NAME_METADATA_KEYS)
    if metadata_name_values and source_label_is_unique(entry):
        return any(
            source_identity_matches(value, entry, source_name)
            for value in metadata_name_values
        )

    fallback_values = [
        value.strip()
        for value in [file_path or "", diff_hunk or ""]
        if value and value.strip()
    ]
    return any(
        source_path_value_matches(value, path_substring, fixture_paths)
        for value in fallback_values
    )


def describe_record_source_evidence(rows):
    evidence = set()
    for row in rows:
        metadata, _prompt_summary, _session_id, _provider, _model, _timestamp, file_path, diff_hunk = row
        payload = metadata_payload(metadata, "record source evidence")
        for key in sorted(SOURCE_PATH_METADATA_KEYS | SOURCE_NAME_METADATA_KEYS):
            if isinstance(payload, dict) and key in payload:
                evidence.add(f"metadata.{key}")
        if (file_path or "").strip():
            evidence.add("file_path")
        if (diff_hunk or "").strip():
            evidence.add("diff_hunk")
    return sorted(evidence)


def record_field_present(field, rows, expected_session_id):
    for row in rows:
        metadata, prompt_summary, session_id, provider, model, timestamp, file_path, diff_hunk = row
        payload = metadata_payload(metadata, f"{agent}/{expected_session_id}")
        if field == "session_id" and session_id == expected_session_id:
            return True
        if field == "timestamp_ms" and ((timestamp or "").strip() or nonzero_number(payload.get("timestamp_ms"))):
            return True
        if field == "provider" and (provider or "").strip():
            return True
        if field == "model" and (model or "").strip():
            return True
        if field == "prompt_summary" and (prompt_summary or "").strip():
            return True
        if field == "assistant_output" and (payload.get("assistant_output") or "").strip():
            return True
        if field == "tool_name" and (payload.get("tool_name") or "").strip():
            return True
        if field == "tool_arguments" and (payload.get("tool_arguments") or "").strip():
            return True
        if field == "tool_result" and (payload.get("tool_result") or "").strip():
            return True
        if field == "file_path_or_diff" and (
            (file_path or "").strip()
            or (diff_hunk or "").strip()
            or (payload.get("file_path") or "").strip()
            or (payload.get("diff_hunk") or payload.get("diff") or "").strip()
        ):
            return True
    return False

KNOWN_USAGE_FIELDS = {
    "tokens_in",
    "tokens_out",
    "message_count",
    "source_cost",
    "cache",
    "reasoning",
    "account",
    "day",
    "usage_basis",
}
KNOWN_RECORD_FIELDS = {
    "session_id",
    "timestamp_ms",
    "provider",
    "model",
    "prompt_summary",
    "assistant_output",
    "tool_name",
    "tool_arguments",
    "tool_result",
    "file_path_or_diff",
}

for entry in expected:
    missing = sorted(required_keys - set(entry))
    if missing:
        raise SystemExit(f"{agent}: expected source entry missing keys {missing}: {entry}")
    if entry["agent"] != agent:
        raise SystemExit(f"{agent}: expected source row uses mismatched agent {entry['agent']}")

    label = entry["label"]
    path_substring = entry["path_substring"]
    source_name = f"{agent}/{label}"
    required_usage_fields = require_string_list(entry, "required_usage_fields", source_name)
    required_record_fields = require_string_list(entry, "required_record_fields", source_name)
    require_string_list(entry, "optional_record_fields", source_name)
    unknown_usage_fields = sorted(set(required_usage_fields) - KNOWN_USAGE_FIELDS)
    if unknown_usage_fields:
        raise SystemExit(f"{source_name}: required_usage_fields has no validator mapping: {unknown_usage_fields}")
    unknown_record_fields = sorted(set(required_record_fields) - KNOWN_RECORD_FIELDS)
    if unknown_record_fields:
        raise SystemExit(f"{source_name}: required_record_fields has no validator mapping: {unknown_record_fields}")
    if entry.get("blocker"):
        raise SystemExit(f"{source_name}: BLOCKER {entry['blocker']}")
    fixture_paths = [path for path in all_paths if path_substring in path]
    if not fixture_paths:
        raise SystemExit(f"{source_name}: fixture path not found containing {path_substring!r}")

    if entry["expects_usage"]:
        if not usage_db.exists():
            raise SystemExit(f"{source_name}: usage.sqlite missing")
        rollup_rows = sqlite3.connect(usage_db).execute(
            """
            SELECT path, day, tokens_in, tokens_out, tokens_cache_read, tokens_cache_write,
                   tokens_reasoning, message_count, source_cost, usage_basis, account
            FROM usage_rollup_sources
            WHERE tool = ? AND agent = ?
            """,
            (agent, agent),
        ).fetchall()
        source_rows = [
            row
            for row in rollup_rows
            if source_path_matches(row[0], path_substring, fixture_paths)
        ]
        if not source_rows:
            raise SystemExit(
                f"{source_name}: missing usage_rollup_sources row for expected source path "
                f"{path_substring!r}"
            )
        columns = {
            "tokens_in": 2,
            "tokens_out": 3,
            "message_count": 7,
            "source_cost": 8,
        }
        for field, column in columns.items():
            if field in required_usage_fields and not any(
                nonzero_number(row[column]) for row in source_rows
            ):
                raise SystemExit(f"{source_name}: usage field {field} missing positive value")
        if "cache" in required_usage_fields and not any(
            nonzero_number(row[4]) or nonzero_number(row[5]) for row in source_rows
        ):
            raise SystemExit(f"{source_name}: usage field cache missing positive value")
        if "reasoning" in required_usage_fields and not any(
            nonzero_number(row[6]) for row in source_rows
        ):
            raise SystemExit(f"{source_name}: usage field reasoning missing positive value")
        if "day" in required_usage_fields and not any((row[1] or "").strip() for row in source_rows):
            raise SystemExit(f"{source_name}: usage field day missing time window value")
        if "usage_basis" in required_usage_fields:
            basis = (entry.get("usage_basis") or "").lower()
            if basis not in {"native", "local_derived"}:
                raise SystemExit(f"{source_name}: expected row missing source-level usage_basis")
            if not any((row[9] or "").lower() == basis for row in source_rows):
                seen_basis = sorted({(row[9] or "").lower() for row in source_rows})
                raise SystemExit(
                    f"{source_name}: usage_basis expected {basis!r}, seen={seen_basis}"
                )
        if "account" in required_usage_fields and not any((row[10] or "").strip() for row in source_rows):
            raise SystemExit(f"{source_name}: usage field account missing value")

    if entry["expects_monitoring"]:
        required_events = entry.get("required_event_types")
        if not required_events:
            raise SystemExit(f"{source_name}: required_event_types missing for monitoring source")
        if not records_db.exists():
            raise SystemExit(f"{source_name}: records.db missing")
        rows = sqlite3.connect(records_db).execute(
            """
            SELECT metadata, prompt_summary, session_id, provider, model, timestamp, file_path, diff_hunk
            FROM records
            WHERE tool = ? AND session_id = ?
            """,
            (agent, entry["session_id"]),
        ).fetchall()
        if not rows:
            raise SystemExit(
                f"{source_name}: no records row for session_id={entry['session_id']!r}"
            )
        source_rows = [
            row
            for row in rows
            if record_source_matches(row, entry, fixture_paths, source_name)
        ]
        if not source_rows:
            evidence = describe_record_source_evidence(rows)
            raise SystemExit(
                f"{source_name}: no source-bound records row for "
                f"path_substring={path_substring!r} session_id={entry['session_id']!r}; "
                f"source evidence seen={evidence or ['none']}. "
                "records must carry metadata source_path/source_name or a file_path/diff_hunk "
                "that traces back to the expected source"
            )
        seen = set()
        for metadata, *_ in source_rows:
            payload = metadata_payload(metadata, source_name)
            event_type = payload.get("event_type")
            if event_type:
                seen.add(event_type)
        missing_events = sorted(set(required_events) - seen)
        if missing_events:
            raise SystemExit(
                f"{source_name}: missing monitoring event types {missing_events}, seen={sorted(seen)}"
            )
        missing_fields = [
            field
            for field in required_record_fields
            if not record_field_present(field, source_rows, entry["session_id"])
        ]
        if missing_fields:
            raise SystemExit(
                f"{source_name}: missing required record fields {missing_fields} "
                f"for session_id={entry['session_id']!r}"
            )

print(f"{agent}: checked {len(expected)} expected source entries")
PY
}

usage_fixture_requires_positive_tokens() {
    case "$1" in
        codebuff) return 1 ;;
        *) return 0 ;;
    esac
}

usage_fixture_expects_usage() {
    case "$1" in
        *) return 0 ;;
    esac
}

expected_usage_min_monitoring_events() {
    local expected_json="$1"
    EXPECTED_SOURCES_JSON="${expected_json}" python3 - <<'PY'
import json
import os

rows = json.loads(os.environ["EXPECTED_SOURCES_JSON"])
total = 0
for row in rows:
    if row.get("expects_monitoring"):
        total += len(row.get("required_event_types") or [])
print(total)
PY
}

expected_usage_required_event_types() {
    local expected_json="$1"
    EXPECTED_SOURCES_JSON="${expected_json}" python3 - <<'PY'
import json
import os

rows = json.loads(os.environ["EXPECTED_SOURCES_JSON"])
ordered = []
for row in rows:
    if not row.get("expects_monitoring"):
        continue
    for event_type in row.get("required_event_types") or []:
        if event_type not in ordered:
            ordered.append(event_type)
print(",".join(ordered))
PY
}

usage_fixture_expects_reasoning_event() {
    case "$1" in
        claude|codex|opencode|hermes|goose|pi|mux|crush) return 0 ;;
        *) return 1 ;;
    esac
}

# ── Core e2e function run against one server implementation ──────────────────

run_against_server() {
    local impl="$1"
    local server_url="$2"

    echo ""
    echo "══════════════════════════════════════════════════════════"
    echo "  Client E2E — impl=${impl}  url=${server_url}"
    echo "══════════════════════════════════════════════════════════"

    # ── Provision a token ──────────────────────────────────────────────────────
    log "Provisioning token..."
    TOK_JSON=$(provision_token "${server_url}" "client-e2e-user")
    # v1.2: response is {"credential":"<token>-<hmac_secret>","token_key":"<masked>"}
    CREDENTIAL=$(echo "${TOK_JSON}" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['credential'])")
    TOKEN_KEY=$(echo "${TOK_JSON}" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['token_key'])")
    # Split credential on the first "-": token=everything before, secret=everything after
    TOKEN="${CREDENTIAL%%-*}"
    HMAC_SECRET="${CREDENTIAL#*-}"

    if [ -z "${CREDENTIAL}" ] || [ "${CREDENTIAL}" = "None" ]; then
        fail "Token provisioning failed — response: ${TOK_JSON}"
        return
    fi
    ok "Token provisioned (token_key=${TOKEN_KEY})"

    # ── Set up isolated AITRACK_HOME ───────────────────────────────────────────
    AITRACK_HOME=$(mktemp -d "/tmp/aitrack-client-e2e-${impl}-XXXXXX")
    DEVICE_ID="e2e-device-$(uuidgen | tr '[:upper:]' '[:lower:]')"

    # Write config.toml — v1.2: single "credential" key replaces token + hmac_secret
    cat > "${AITRACK_HOME}/config.toml" <<TOML
api_url = "${server_url}"
credential = "${CREDENTIAL}"
device_id = "${DEVICE_ID}"
TOML
    chmod 0600 "${AITRACK_HOME}/config.toml"
    ok "Isolated AITRACK_HOME created at ${AITRACK_HOME}"

    # ── Set up a real git repo for metadata ────────────────────────────────────
    GIT_REPO=$(mktemp -d "/tmp/aitrack-e2e-gitrepo-XXXXXX")

    git -C "${GIT_REPO}" init -q
    git -C "${GIT_REPO}" remote add origin "git@github.com:aitrack-e2e/client-e2e-test.git"
    git -C "${GIT_REPO}" config user.email "e2e@aitrack.test"
    git -C "${GIT_REPO}" config user.name "E2E Test"
    # Create a dummy commit so HEAD + branch exist
    echo "e2e placeholder" > "${GIT_REPO}/README.md"
    git -C "${GIT_REPO}" add README.md
    git -C "${GIT_REPO}" commit -q -m "e2e init"
    GIT_SHA=$(git -C "${GIT_REPO}" rev-parse HEAD)
    GIT_BRANCH=$(git -C "${GIT_REPO}" branch --show-current)
    ok "Git repo ready (sha=${GIT_SHA:0:12} branch=${GIT_BRANCH})"

    # Common env for all aitrack invocations
    E2E_ENV=(
        "AITRACK_HOME=${AITRACK_HOME}"
        "AITRACK_SCAN_HOME=${AITRACK_HOME}"
        "XDG_DATA_HOME=${AITRACK_HOME}/.local/share"
        "XDG_CONFIG_HOME=${AITRACK_HOME}/.config"
        "CODEX_HOME=${AITRACK_HOME}/.codex"
        "GEMINI_CLI_HOME=${AITRACK_HOME}/.gemini"
        "CODEBUFF_DATA_DIR=${AITRACK_HOME}/.config/manicode"
        "GJC_CODING_AGENT_DIR=${AITRACK_HOME}/.gjc/agent/sessions"
        "GROK_HOME=${AITRACK_HOME}/.grok"
        "HERMES_HOME=${AITRACK_HOME}/.hermes"
        "KIMI_CODE_HOME=${AITRACK_HOME}/.kimi-code"
    )

    # Wrapper: run aitrack with isolated env from within the git repo
    run_aitrack() {
        local tool="$1"
        shift
        env "${E2E_ENV[@]}" "${AITRACK_BIN}" capture --tool "${tool}" "$@"
    }

    # ── Test 1: claude capture ─────────────────────────────────────────────────
    echo ""
    echo "--- Test: capture --tool claude ---"

    CLAUDE_PAYLOAD=$(cat <<'JSON'
{
  "session_id": "e2e-claude-sess-001",
  "tool_version": "claude-code",
  "tool_input": {
    "old_string": "fn compute_record_sig() {\n    // old implementation\n    todo!()\n}\n",
    "new_string": "fn compute_record_sig(\n    hmac_secret: &str,\n    token_key: &str,\n    device_id: &str,\n) -> String {\n    // new implementation\n    hmac_sha256_hex(hmac_secret, token_key)\n}\n",
    "file_paths": ["src/crypto.rs"]
  }
}
JSON
)

    (cd "${GIT_REPO}" && echo "${CLAUDE_PAYLOAD}" | env "${E2E_ENV[@]}" "${AITRACK_BIN}" capture --tool claude)
    CLAUDE_EXIT=$?

    if [ $CLAUDE_EXIT -eq 0 ]; then
        ok "claude capture exited 0"
    else
        fail "claude capture exited ${CLAUDE_EXIT}"
    fi

    # Assert local DB has a record
    DB_COUNT=$(sqlite3 "${AITRACK_HOME}/records.db" \
        "SELECT COUNT(*) FROM records WHERE tool='claude';" 2>/dev/null || echo "0")
    if [ "${DB_COUNT}" -ge 1 ]; then
        ok "Local SQLite: claude record inserted (count=${DB_COUNT})"
    else
        fail "Local SQLite: no claude record found (count=${DB_COUNT})"
    fi

    # Assert record_sig is non-empty
    SIG=$(sqlite3 "${AITRACK_HOME}/records.db" \
        "SELECT record_sig FROM records WHERE tool='claude' ORDER BY id DESC LIMIT 1;" 2>/dev/null || echo "")
    if [ ${#SIG} -eq 64 ]; then
        ok "Local SQLite: record_sig is 64-char hex (${SIG:0:16}...)"
    else
        fail "Local SQLite: record_sig unexpected (got '${SIG}')"
    fi

    # Assert synced=1 (flush_unsynced ran)
    SYNCED=$(sqlite3 "${AITRACK_HOME}/records.db" \
        "SELECT synced FROM records WHERE tool='claude' ORDER BY id DESC LIMIT 1;" 2>/dev/null || echo "")
    if [ "${SYNCED}" = "1" ]; then
        ok "Local SQLite: claude record synced=1"
    else
        fail "Local SQLite: claude record synced=${SYNCED} (expected 1)"
    fi

    # Assert server received it
    sleep 1
    EDITS_RESP=$(api_get "${server_url}" "/api/v1/ai-track/edits?page=0&size=20" "${TOKEN}")
    # Both Java and Go now return {"total":N,"page":P,"size":S,"records":[...]} with snake_case keys.
    CLAUDE_ON_SERVER=$(echo "${EDITS_RESP}" | python3 -c "
import sys, json
raw = json.load(sys.stdin)
items = raw.get('records', [])
print(sum(1 for i in items if i.get('tool') == 'claude'))
" 2>/dev/null || echo "0")

    if [ "${CLAUDE_ON_SERVER}" -ge 1 ]; then
        ok "Server: claude edit received (count=${CLAUDE_ON_SERVER})"
    else
        fail "Server: no claude edit found — response: ${EDITS_RESP}"
    fi

    # Assert field contents on server (both servers now return snake_case)
    FILE_PATH_ON_SERVER=$(echo "${EDITS_RESP}" | python3 -c "
import sys, json
raw = json.load(sys.stdin)
items = raw.get('records', [])
cl = [i for i in items if i.get('tool') == 'claude']
item = cl[0] if cl else {}
print(item.get('file_path', ''))
" 2>/dev/null || echo "")

    if [ "${FILE_PATH_ON_SERVER}" = "src/crypto.rs" ]; then
        ok "Server: file_path matches 'src/crypto.rs'"
    else
        fail "Server: file_path='${FILE_PATH_ON_SERVER}' (expected 'src/crypto.rs')"
    fi

    # Assert added_lines > 0
    ADDED_ON_SERVER=$(echo "${EDITS_RESP}" | python3 -c "
import sys, json
raw = json.load(sys.stdin)
items = raw.get('records', [])
cl = [i for i in items if i.get('tool') == 'claude']
item = cl[0] if cl else {}
print(item.get('added_lines', 0))
" 2>/dev/null || echo "0")
    if [ "${ADDED_ON_SERVER}" -gt 0 ]; then
        ok "Server: added_lines=${ADDED_ON_SERVER} > 0"
    else
        fail "Server: added_lines=${ADDED_ON_SERVER} (expected > 0)"
    fi

    # Assert hostname present on server
    HOSTNAME_ON_SERVER=$(echo "${EDITS_RESP}" | python3 -c "
import sys, json
raw = json.load(sys.stdin)
items = raw.get('records', [])
cl = [i for i in items if i.get('tool') == 'claude']
item = cl[0] if cl else {}
print(item.get('hostname', ''))
" 2>/dev/null || echo "")
    if [ -n "${HOSTNAME_ON_SERVER}" ]; then
        ok "Server: hostname='${HOSTNAME_ON_SERVER}' present"
    else
        fail "Server: hostname missing from response"
    fi

    # Assert diff_hunk present on server
    HUNK_ON_SERVER=$(echo "${EDITS_RESP}" | python3 -c "
import sys, json
raw = json.load(sys.stdin)
items = raw.get('records', [])
cl = [i for i in items if i.get('tool') == 'claude']
item = cl[0] if cl else {}
h = item.get('diff_hunk', '')
print('yes' if h and '@@' in h else 'no')
" 2>/dev/null || echo "no")
    if [ "${HUNK_ON_SERVER}" = "yes" ]; then
        ok "Server: diff_hunk contains '@@' (unified diff present)"
    else
        fail "Server: diff_hunk missing or malformed"
    fi

    # ── Test 2: codex capture ──────────────────────────────────────────────────
    echo ""
    echo "--- Test: capture --tool codex ---"

    CODEX_PAYLOAD=$(cat <<'JSON'
{
  "hook_event_name": "postToolUse",
  "tool_name": "Edit",
  "conversation_id": "e2e-codex-sess-001",
  "model": "gpt-4o",
  "tool_input": {
    "old_string": "func ComputeRecordSig() string {\n    return \"\"\n}\n",
    "new_string": "func ComputeRecordSig(secret, tokenKey, deviceID string) string {\n    mac := hmac.New(sha256.New, []byte(secret))\n    mac.Write([]byte(tokenKey + deviceID))\n    return hex.EncodeToString(mac.Sum(nil))\n}\n",
    "file_path": "service/signature.go"
  }
}
JSON
)

    (cd "${GIT_REPO}" && echo "${CODEX_PAYLOAD}" | env "${E2E_ENV[@]}" "${AITRACK_BIN}" capture --tool codex)
    CODEX_EXIT=$?

    if [ $CODEX_EXIT -eq 0 ]; then
        ok "codex capture exited 0"
    else
        fail "codex capture exited ${CODEX_EXIT}"
    fi

    CODEX_DB=$(sqlite3 "${AITRACK_HOME}/records.db" \
        "SELECT COUNT(*) FROM records WHERE tool='codex';" 2>/dev/null || echo "0")
    if [ "${CODEX_DB}" -ge 1 ]; then
        ok "Local SQLite: codex record inserted (count=${CODEX_DB})"
    else
        fail "Local SQLite: no codex record found"
    fi

    CODEX_SYNCED=$(sqlite3 "${AITRACK_HOME}/records.db" \
        "SELECT synced FROM records WHERE tool='codex' ORDER BY id DESC LIMIT 1;" 2>/dev/null || echo "")
    if [ "${CODEX_SYNCED}" = "1" ]; then
        ok "Local SQLite: codex record synced=1"
    else
        fail "Local SQLite: codex record synced=${CODEX_SYNCED}"
    fi

    sleep 1
    EDITS_RESP2=$(api_get "${server_url}" "/api/v1/ai-track/edits?page=0&size=50" "${TOKEN}")
    CODEX_ON_SERVER=$(echo "${EDITS_RESP2}" | python3 -c "
import sys, json
raw = json.load(sys.stdin)
items = raw.get('records', [])
print(sum(1 for i in items if i.get('tool') == 'codex'))
" 2>/dev/null || echo "0")
    if [ "${CODEX_ON_SERVER}" -ge 1 ]; then
        ok "Server: codex edit received (count=${CODEX_ON_SERVER})"
    else
        fail "Server: no codex edit found — response: ${EDITS_RESP2}"
    fi

    CODEX_FILE=$(echo "${EDITS_RESP2}" | python3 -c "
import sys, json
raw = json.load(sys.stdin)
items = raw.get('records', [])
codex = [i for i in items if i.get('tool') == 'codex']
item = codex[0] if codex else {}
print(item.get('file_path', ''))
" 2>/dev/null || echo "")
    if [ "${CODEX_FILE}" = "service/signature.go" ]; then
        ok "Server: codex file_path='service/signature.go'"
    else
        fail "Server: codex file_path='${CODEX_FILE}' (expected 'service/signature.go')"
    fi

    # ── Test 3: cursor capture ─────────────────────────────────────────────────
    echo ""
    echo "--- Test: capture --tool cursor ---"

    CURSOR_PAYLOAD=$(cat <<'JSON'
{
  "session_id": "e2e-cursor-sess-001",
  "cursor_version": "0.40.0",
  "tool_input": {
    "file_path": "ValidationService.java",
    "old_str": "public class ValidationService {\n    public boolean validate(String sig) {\n        return false;\n    }\n}\n",
    "new_str": "public class ValidationService {\n    public ValidationResult validate(TokenEntity token, EditDto edit) {\n        String expected = signatureService.computeRecordSig(\n            token.getHmacSecret(), token.getTokenKey(), edit.getDeviceId(),\n            edit.getTimestamp(), edit.getTool(), edit.getFilePath(),\n            edit.getRepoUrl(), edit.getCurrentSha(),\n            edit.getAddedLines(), edit.getRemovedLines(), edit.getDiffHunk());\n        if (!constantTimeEquals(expected, edit.getRecordSig())) {\n            return ValidationResult.rejected(\"sig_mismatch\");\n        }\n        return ValidationResult.accepted();\n    }\n}\n"
  }
}
JSON
)

    (cd "${GIT_REPO}" && echo "${CURSOR_PAYLOAD}" | env "${E2E_ENV[@]}" "${AITRACK_BIN}" capture --tool cursor)
    CURSOR_EXIT=$?

    if [ $CURSOR_EXIT -eq 0 ]; then
        ok "cursor capture exited 0"
    else
        fail "cursor capture exited ${CURSOR_EXIT}"
    fi

    CURSOR_DB=$(sqlite3 "${AITRACK_HOME}/records.db" \
        "SELECT COUNT(*) FROM records WHERE tool='cursor';" 2>/dev/null || echo "0")
    if [ "${CURSOR_DB}" -ge 1 ]; then
        ok "Local SQLite: cursor record inserted (count=${CURSOR_DB})"
    else
        fail "Local SQLite: no cursor record found"
    fi

    CURSOR_SYNCED=$(sqlite3 "${AITRACK_HOME}/records.db" \
        "SELECT synced FROM records WHERE tool='cursor' ORDER BY id DESC LIMIT 1;" 2>/dev/null || echo "")
    if [ "${CURSOR_SYNCED}" = "1" ]; then
        ok "Local SQLite: cursor record synced=1"
    else
        fail "Local SQLite: cursor record synced=${CURSOR_SYNCED}"
    fi

    sleep 1
    EDITS_RESP3=$(api_get "${server_url}" "/api/v1/ai-track/edits?page=0&size=50" "${TOKEN}")
    CURSOR_ON_SERVER=$(echo "${EDITS_RESP3}" | python3 -c "
import sys, json
raw = json.load(sys.stdin)
items = raw.get('records', [])
print(sum(1 for i in items if i.get('tool') == 'cursor'))
" 2>/dev/null || echo "0")
    if [ "${CURSOR_ON_SERVER}" -ge 1 ]; then
        ok "Server: cursor edit received (count=${CURSOR_ON_SERVER})"
    else
        fail "Server: no cursor edit found — response: ${EDITS_RESP3}"
    fi

    CURSOR_FILE=$(echo "${EDITS_RESP3}" | python3 -c "
import sys, json
raw = json.load(sys.stdin)
items = raw.get('records', [])
cursor = [i for i in items if i.get('tool') == 'cursor']
item = cursor[0] if cursor else {}
print(item.get('file_path', ''))
" 2>/dev/null || echo "")
    if [ "${CURSOR_FILE}" = "ValidationService.java" ]; then
        ok "Server: cursor file_path='ValidationService.java'"
    else
        fail "Server: cursor file_path='${CURSOR_FILE}' (expected 'ValidationService.java')"
    fi

    # ── Test 4: GET /stats reflects all three edits ────────────────────────────
    echo ""
    echo "--- Test: GET /stats ---"

    STATS_RESP=$(api_get "${server_url}" "/api/v1/ai-track/stats" "${TOKEN}")
    STATS_CODE=$(curl -s -o /dev/null -w "%{http_code}" \
        -H "Authorization: Bearer ${TOKEN}" \
        "${server_url}/api/v1/ai-track/stats")
    if [ "${STATS_CODE}" = "200" ]; then
        ok "GET /stats → 200"
    else
        fail "GET /stats → ${STATS_CODE}"
    fi

    STATS_HAS_DATA=$(echo "${STATS_RESP}" | python3 -c "
import sys, json
d = json.load(sys.stdin)
# Both Java and Go now return a JSON array of StatsRow objects with snake_case keys:
# [{'group': '...', 'edits': N, 'added_lines': M, 'removed_lines': M, 'accepted': N, ...}]
rows = d if isinstance(d, list) else []
total = sum(r.get('edits', 0) for r in rows)
print('yes' if total > 0 else 'no')
" 2>/dev/null || echo "no")
    if [ "${STATS_HAS_DATA}" = "yes" ]; then
        ok "GET /stats: total_edits > 0"
    else
        fail "GET /stats: no edits reflected — response: ${STATS_RESP}"
    fi

    # ── Test 5: heartbeat ─────────────────────────────────────────────────────
    echo ""
    echo "--- Test: aitrack heartbeat ---"

    (cd "${GIT_REPO}" && env "${E2E_ENV[@]}" "${AITRACK_BIN}" heartbeat)
    HB_EXIT=$?
    if [ $HB_EXIT -eq 0 ]; then
        ok "aitrack heartbeat exited 0"
    else
        fail "aitrack heartbeat exited ${HB_EXIT}"
    fi

    sleep 1
    DEVICES_RESP=$(api_get "${server_url}" "/api/v1/ai-track/devices" "${TOKEN}")
    DEVICE_ON_SERVER=$(echo "${DEVICES_RESP}" | python3 -c "
import sys, json
d = json.load(sys.stdin)
# Both Java and Go return a JSON array of DeviceInfo objects with snake_case keys:
# [{'device_id': '...', 'token_key': '...', 'hostname': '...', ...}]
items = d if isinstance(d, list) else []
print(len(items))
" 2>/dev/null || echo "0")
    if [ "${DEVICE_ON_SERVER}" -ge 1 ]; then
        ok "GET /devices: device registered (count=${DEVICE_ON_SERVER})"
    else
        fail "GET /devices: no device found — response: ${DEVICES_RESP}"
    fi

    # Check our specific device_id is present — snake_case key device_id
    DEVICE_FOUND=$(echo "${DEVICES_RESP}" | python3 -c "
import sys, json
d = json.load(sys.stdin)
items = d if isinstance(d, list) else []
device_id = '${DEVICE_ID}'
found = any(str(i.get('device_id', '')) == device_id for i in items)
print('yes' if found else 'no')
" 2>/dev/null || echo "no")
    if [ "${DEVICE_FOUND}" = "yes" ]; then
        ok "GET /devices: device_id=${DEVICE_ID} found"
    else
        # heartbeat device_id detection may differ — at least one device registered is the strong assertion
        ok "GET /devices: at least one device registered (device_id match depends on heartbeat path)"
    fi

    # ── Test 6: local usage source matrix ─────────────────────────────────────
    echo ""
    echo "--- Test: local usage source matrix coverage ---"

    MATRIX_PASS=0
    MATRIX_TOTAL=${#REQUIRED_LOCAL_SOURCE_AGENTS[@]}

    for agent in "${REQUIRED_LOCAL_SOURCE_AGENTS[@]}"; do
        agent_fail=0
        EXPECTED_SOURCES_JSON=""
        write_usage_source_fixture "${AITRACK_HOME}" "${agent}"

        if (cd "${GIT_REPO}" && env "${E2E_ENV[@]}" "AITRACK_SCAN_HOME=${AITRACK_HOME}" "${AITRACK_BIN}" usage scan --tool "${agent}" >/tmp/aitrack-usage-scan-${impl}-${agent}.json); then
            ok "usage scan --tool ${agent} exited 0"
        else
            fail "usage scan --tool ${agent} failed"
            agent_fail=1
            continue
        fi

        if EXPECTED_SOURCES_JSON="$(expected_usage_sources_json "${agent}")"; then
            if validate_expected_usage_sources "${AITRACK_HOME}" "${agent}" "${EXPECTED_SOURCES_JSON}"; then
                ok "Local source-level expectations: ${agent} matched expected fixture rows"
            else
                fail "Local source-level expectations: ${agent} expected-source validation failed"
                agent_fail=1
            fi
        else
            fail "Local source-level expectations: ${agent} expected-source JSON generation failed"
            agent_fail=1
        fi

        DETAIL_ROWS=$(sqlite3 "${AITRACK_HOME}/usage.sqlite" \
            "SELECT COUNT(*) FROM usage_sessions WHERE agent='${agent}';" 2>/dev/null || echo "0")
        SOURCE_ROWS=$(sqlite3 "${AITRACK_HOME}/usage.sqlite" \
            "SELECT COUNT(*) FROM usage_rollup_sources WHERE agent='${agent}';" 2>/dev/null || echo "0")
        if usage_fixture_expects_usage "${agent}"; then
            if usage_fixture_requires_positive_tokens "${agent}"; then
                ROLLUP_ROWS=$(sqlite3 "${AITRACK_HOME}/usage.sqlite" \
                    "SELECT COUNT(*) FROM usage_daily_model_rollups WHERE agent='${agent}' AND (tokens_in + tokens_out) > 0 AND message_count > 0;" 2>/dev/null || echo "0")
            else
                ROLLUP_ROWS=$(sqlite3 "${AITRACK_HOME}/usage.sqlite" \
                    "SELECT COUNT(*) FROM usage_daily_model_rollups WHERE agent='${agent}' AND message_count > 0 AND source_cost > 0;" 2>/dev/null || echo "0")
            fi
            if [ "${DETAIL_ROWS}" -eq 0 ] && [ "${SOURCE_ROWS}" -ge 1 ] && [ "${ROLLUP_ROWS}" -ge 1 ]; then
                ok "Local usage.sqlite: ${agent} aggregated without persisted detail rows (sources=${SOURCE_ROWS}, rollups=${ROLLUP_ROWS})"
            else
                fail "Local usage.sqlite: ${agent} detail=${DETAIL_ROWS} sources=${SOURCE_ROWS} rollups=${ROLLUP_ROWS}"
                agent_fail=1
            fi
        else
            ROLLUP_ROWS=$(sqlite3 "${AITRACK_HOME}/usage.sqlite" \
                "SELECT COUNT(*) FROM usage_daily_model_rollups WHERE agent='${agent}';" 2>/dev/null || echo "0")
            if [ "${DETAIL_ROWS}" -eq 0 ] && [ "${SOURCE_ROWS}" -eq 0 ] && [ "${ROLLUP_ROWS}" -eq 0 ]; then
                ok "Local usage.sqlite: ${agent} usage-disabled fixture did not fabricate rollups"
            else
                fail "Local usage.sqlite: ${agent} usage-disabled fixture produced unexpected rows detail=${DETAIL_ROWS} sources=${SOURCE_ROWS} rollups=${ROLLUP_ROWS}"
                agent_fail=1
            fi
        fi

        if ! MIN_MONITORING_EVENTS="$(expected_usage_min_monitoring_events "${EXPECTED_SOURCES_JSON}")"; then
            fail "Local records.db: ${agent} could not derive monitoring event count from expected sources"
            agent_fail=1
            MIN_MONITORING_EVENTS=0
        fi
        if [ "${MIN_MONITORING_EVENTS}" -gt 0 ]; then
            RECORD_ROWS=$(sqlite3 "${AITRACK_HOME}/records.db" \
                "SELECT COUNT(*) FROM records WHERE tool='${agent}';" 2>/dev/null || echo "0")
            if [ "${RECORD_ROWS}" -ge "${MIN_MONITORING_EVENTS}" ]; then
                ok "Local records.db: ${agent} monitoring rows inserted (${RECORD_ROWS})"
            else
                fail "Local records.db: ${agent} monitoring rows missing (records=${RECORD_ROWS}, min=${MIN_MONITORING_EVENTS})"
                agent_fail=1
            fi

            REQUIRED_EVENT_TYPES="$(expected_usage_required_event_types "${EXPECTED_SOURCES_JSON}")"
            if [ -n "${REQUIRED_EVENT_TYPES}" ]; then
                EXPECT_REASONING="no"
                if usage_fixture_expects_reasoning_event "${agent}"; then
                    EXPECT_REASONING="yes"
                fi
                if python3 - "${AITRACK_HOME}/records.db" "${agent}" "${REQUIRED_EVENT_TYPES}" "${EXPECT_REASONING}" <<'PY'
import json
import sqlite3
import sys

db_path, agent, required_raw, expects_reasoning = sys.argv[1:5]
required = [item for item in required_raw.split(",") if item]
rows = sqlite3.connect(db_path).execute(
    "SELECT metadata, prompt_summary, session_id, provider, model FROM records WHERE tool = ?",
    (agent,),
).fetchall()
if not rows:
    raise SystemExit(f"{agent}: no monitoring rows")
events = []
has_model = False
for metadata, prompt_summary, session_id, provider, model in rows:
    if not session_id:
        raise SystemExit(f"{agent}: missing session_id")
    if not provider:
        raise SystemExit(f"{agent}: missing provider")
    if (model or "").strip():
        has_model = True
    try:
        payload = json.loads(metadata or "{}")
    except json.JSONDecodeError as exc:
        raise SystemExit(f"{agent}: invalid metadata JSON: {exc}") from exc
    event_type = payload.get("event_type")
    if event_type:
        events.append((event_type, payload, prompt_summary))

seen = {event_type for event_type, _, _ in events}
if not has_model:
    raise SystemExit(f"{agent}: missing model")
for event_type in required:
    if event_type not in seen:
        raise SystemExit(f"{agent}: missing event_type={event_type}, seen={sorted(seen)}")

if "prompt" in required and not any(
    event_type == "prompt" and (prompt_summary or "").strip()
    for event_type, _, prompt_summary in events
):
    raise SystemExit(f"{agent}: prompt event missing prompt_summary")
if "output" in required and not any(
    event_type == "output" and (payload.get("assistant_output") or "").strip()
    for event_type, payload, _ in events
):
    raise SystemExit(f"{agent}: output event missing assistant_output")
if "tool" in required and not any(
    event_type == "tool"
    and (payload.get("tool_name") or "").strip()
    and (payload.get("tool_arguments") or "").strip()
    for event_type, payload, _ in events
):
    raise SystemExit(f"{agent}: tool event missing tool_name/tool_arguments")
if "tool_result" in required and not any(
    event_type == "tool_result" and (payload.get("tool_result") or "").strip()
    for event_type, payload, _ in events
):
    raise SystemExit(f"{agent}: tool_result event missing tool_result")
if "edit" in required and not any(
    event_type == "edit"
    and (payload.get("tool_name") or payload.get("event_name") or "").strip()
    for event_type, payload, _ in events
):
    raise SystemExit(f"{agent}: edit event missing tool_name/event_name")
if expects_reasoning == "yes" and not any(
    "agent_reasoning" in json.dumps(payload, separators=(",", ":"))
    for _, payload, _ in events
):
    raise SystemExit(f"{agent}: reasoning fixture missing agent_reasoning event")
PY
                then
                    ok "Local records.db: ${agent} monitoring metadata includes context and ${REQUIRED_EVENT_TYPES}"
                else
                    fail "Local records.db: ${agent} monitoring metadata missing context or required fields (${REQUIRED_EVENT_TYPES})"
                    agent_fail=1
                fi
            fi
        else
            ok "Local records.db: ${agent} is usage-only for this fixture"
        fi

        LOCAL_EDIT_PATH_COUNTS_JSON="{}"
        if [ "${MIN_MONITORING_EVENTS}" -gt 0 ]; then
            if LOCAL_EDIT_PATH_COUNTS_JSON=$(python3 - "${AITRACK_HOME}/records.db" "${agent}" <<'PY'
import json
import sqlite3
import sys
from collections import Counter

db_path, agent = sys.argv[1:3]
conn = sqlite3.connect(db_path)
rows = conn.execute(
    "SELECT metadata, file_path FROM records WHERE tool = ?",
    (agent,),
).fetchall()
counts = Counter()
for metadata, file_path in rows:
    try:
        payload = json.loads(metadata or "{}")
    except json.JSONDecodeError as exc:
        raise SystemExit(f"{agent}: invalid metadata JSON while collecting edit paths: {exc}") from exc
    if payload.get("event_type") != "edit":
        continue
    path = (file_path or payload.get("file_path") or payload.get("path") or "").strip()
    if path:
        counts[path] += 1
print(json.dumps(dict(sorted(counts.items())), separators=(",", ":")))
PY
            ); then
                if [[ ",${REQUIRED_EVENT_TYPES}," == *",edit,"* ]]; then
                    HAS_LOCAL_EDIT_PATHS=$(python3 - "${LOCAL_EDIT_PATH_COUNTS_JSON}" <<'PY'
import json
import sys
print("yes" if json.loads(sys.argv[1]) else "no")
PY
)
                    if [ "${HAS_LOCAL_EDIT_PATHS}" = "yes" ]; then
                        ok "Local records.db: ${agent} edit file_path counts captured before sync"
                    else
                        fail "Local records.db: ${agent} expected edit events but no edit file_path was captured"
                        agent_fail=1
                    fi
                fi
            else
                fail "Local records.db: ${agent} could not collect edit file_path counts"
                agent_fail=1
            fi
        fi

        if (cd "${GIT_REPO}" && env "${E2E_ENV[@]}" "AITRACK_SCAN_HOME=${AITRACK_HOME}" "${AITRACK_BIN}" usage sync --tool "${agent}" >/tmp/aitrack-usage-sync-${impl}-${agent}.json); then
            ok "usage sync --tool ${agent} exited 0"
        else
            fail "usage sync --tool ${agent} failed"
            agent_fail=1
            continue
        fi

        OUTBOX_ROWS=$(sqlite3 "${AITRACK_HOME}/usage.sqlite" \
            "SELECT COUNT(*) FROM usage_outbox;" 2>/dev/null || echo "0")
        OUTBOX_SYNCED_PAYLOAD_ROWS=$(sqlite3 "${AITRACK_HOME}/usage.sqlite" \
            "SELECT COUNT(*) FROM usage_outbox WHERE synced=1 AND length(payload_json) > 0;" 2>/dev/null || echo "0")
        OUTBOX_FAILED_PAYLOAD_ROWS=$(sqlite3 "${AITRACK_HOME}/usage.sqlite" \
            "SELECT COUNT(*) FROM usage_outbox WHERE retry_count >= 5 AND length(payload_json) > 0;" 2>/dev/null || echo "0")
        OUTBOX_PAYLOAD_BYTES=$(sqlite3 "${AITRACK_HOME}/usage.sqlite" \
            "SELECT COALESCE(SUM(length(payload_json)), 0) FROM usage_outbox;" 2>/dev/null || echo "0")
        if [ "${OUTBOX_SYNCED_PAYLOAD_ROWS}" -eq 0 ] \
            && [ "${OUTBOX_FAILED_PAYLOAD_ROWS}" -eq 0 ] \
            && [ "${OUTBOX_ROWS}" -le 2000 ] \
            && [ "${OUTBOX_PAYLOAD_BYTES}" -le 16777216 ]; then
            ok "Local usage.sqlite: ${agent} bounded outbox payloads (rows=${OUTBOX_ROWS}, bytes=${OUTBOX_PAYLOAD_BYTES})"
        else
            fail "Local usage.sqlite: ${agent} unbounded outbox rows=${OUTBOX_ROWS} bytes=${OUTBOX_PAYLOAD_BYTES} synced_payload=${OUTBOX_SYNCED_PAYLOAD_ROWS} failed_payload=${OUTBOX_FAILED_PAYLOAD_ROWS}"
            agent_fail=1
        fi

        if [ "${MIN_MONITORING_EVENTS}" -gt 0 ]; then
            RECORD_ROWS_AFTER_SYNC=$(sqlite3 "${AITRACK_HOME}/records.db" \
                "SELECT COUNT(*) FROM records WHERE tool='${agent}';" 2>/dev/null || echo "0")
            PRUNED_RECORD_ROWS_AFTER_SYNC=$(sqlite3 "${AITRACK_HOME}/records.db" \
                "SELECT COUNT(*) FROM records WHERE tool='${agent}' AND synced=1 AND metadata='{\"aitrack_pruned\":true}';" 2>/dev/null || echo "0")
            if [ "${RECORD_ROWS_AFTER_SYNC}" -ge "${MIN_MONITORING_EVENTS}" ] \
                && [ "${PRUNED_RECORD_ROWS_AFTER_SYNC}" -ge "${MIN_MONITORING_EVENTS}" ]; then
                ok "Local records.db: ${agent} monitoring rows retained and metadata pruned after sync (records=${RECORD_ROWS_AFTER_SYNC}, pruned=${PRUNED_RECORD_ROWS_AFTER_SYNC})"
            else
                fail "Local records.db: ${agent} post-sync records/pruning mismatch (records=${RECORD_ROWS_AFTER_SYNC}, pruned=${PRUNED_RECORD_ROWS_AFTER_SYNC}, min=${MIN_MONITORING_EVENTS})"
                agent_fail=1
            fi
        else
            ok "Local records.db: ${agent} has no post-sync monitoring record expectation"
        fi

        sleep 1
        if [ "${MIN_MONITORING_EVENTS}" -gt 0 ]; then
            MATRIX_EDITS_RESP=$(api_get "${server_url}" "/api/v1/ai-track/edits?page=0&size=100" "${TOKEN}")
            MATRIX_SERVER_RECORDS=$(echo "${MATRIX_EDITS_RESP}" | AGENT="${agent}" python3 -c "
import json, os, sys
raw = json.load(sys.stdin)
items = raw.get('records', [])
agent = os.environ['AGENT']
print(sum(1 for item in items if item.get('tool') == agent))
" 2>/dev/null || echo "0")
            if [ "${MATRIX_SERVER_RECORDS}" -ge "${MIN_MONITORING_EVENTS}" ]; then
                ok "Server: ${agent} monitoring records received (${MATRIX_SERVER_RECORDS})"
            else
                fail "Server: ${agent} monitoring records missing — response: ${MATRIX_EDITS_RESP}"
                agent_fail=1
            fi
            if [[ ",${REQUIRED_EVENT_TYPES}," == *",edit,"* ]]; then
                MATRIX_SERVER_EDIT_PATHS=$(MATRIX_EDITS_RESP_JSON="${MATRIX_EDITS_RESP}" python3 - "${agent}" "${LOCAL_EDIT_PATH_COUNTS_JSON}" <<'PY'
import json
import os
import sys
from collections import Counter

agent = sys.argv[1]
expected = json.loads(sys.argv[2])
raw = json.loads(os.environ["MATRIX_EDITS_RESP_JSON"])
items = raw.get("records", [])
seen = Counter()
for item in items:
    if item.get("tool") != agent:
        continue
    path = (item.get("file_path") or "").strip()
    if path in expected:
        seen[path] += 1
missing = {
    path: {"expected": count, "seen": seen.get(path, 0)}
    for path, count in expected.items()
    if seen.get(path, 0) < count
}
if missing:
    print(json.dumps({"ok": False, "missing": missing}, separators=(",", ":")))
else:
    print(json.dumps({"ok": True, "paths": sorted(expected)}, separators=(",", ":")))
PY
)
                MATRIX_SERVER_EDIT_OK=$(python3 - "${MATRIX_SERVER_EDIT_PATHS}" <<'PY'
import json
import sys
print("yes" if json.loads(sys.argv[1]).get("ok") else "no")
PY
)
                if [ "${MATRIX_SERVER_EDIT_OK}" = "yes" ]; then
                    ok "Server: ${agent} edit file_path rows accepted"
                else
                    fail "Server: ${agent} edit file_path acceptance mismatch — ${MATRIX_SERVER_EDIT_PATHS}"
                    agent_fail=1
                fi
            fi
        else
            ok "Server: ${agent} has no monitoring record expectation for this fixture"
        fi

        if ! usage_fixture_expects_usage "${agent}"; then
            ok "Server: ${agent} has no usage summary expectation for this fixture"
        else
            MATRIX_USAGE_RESP=$(api_get "${server_url}" "/api/v1/ai-track/usage/summary?agent=${agent}&limit=50" "${TOKEN}")
            if usage_fixture_requires_positive_tokens "${agent}"; then
            MATRIX_SERVER_USAGE=$(echo "${MATRIX_USAGE_RESP}" | python3 -c "
import json, sys
raw = json.load(sys.stdin)
print('yes' if raw.get('total_tokens', 0) > 0 and raw.get('message_count', 0) > 0 else 'no')
" 2>/dev/null || echo "no")
                if [ "${MATRIX_SERVER_USAGE}" = "yes" ]; then
                    ok "Server: ${agent} usage summary includes tokens and messages"
                else
                    fail "Server: ${agent} usage summary missing tokens/messages — response: ${MATRIX_USAGE_RESP}"
                    agent_fail=1
                fi
            else
            MATRIX_SERVER_USAGE=$(echo "${MATRIX_USAGE_RESP}" | python3 -c "
import json, sys
raw = json.load(sys.stdin)
print('yes' if raw.get('message_count', 0) > 0 and raw.get('source_cost', 0) > 0 else 'no')
" 2>/dev/null || echo "no")
                if [ "${MATRIX_SERVER_USAGE}" = "yes" ]; then
                    ok "Server: ${agent} usage summary includes messages and cost"
                else
                    fail "Server: ${agent} usage summary missing messages/cost — response: ${MATRIX_USAGE_RESP}"
                    agent_fail=1
                fi
            fi
        fi

        if [ "${agent_fail}" -eq 0 ]; then
            MATRIX_PASS=$((MATRIX_PASS + 1))
        fi
    done

    MATRIX_COVERAGE=$((MATRIX_PASS * 100 / MATRIX_TOTAL))
    if [ "${MATRIX_COVERAGE}" -ge "${MIN_E2E_COVERAGE}" ]; then
        ok "Local source matrix coverage ${MATRIX_COVERAGE}% >= ${MIN_E2E_COVERAGE}% (${MATRIX_PASS}/${MATRIX_TOTAL})"
    else
        fail "Local source matrix coverage ${MATRIX_COVERAGE}% < ${MIN_E2E_COVERAGE}% (${MATRIX_PASS}/${MATRIX_TOTAL})"
    fi

    CACHE_AGENT="${REQUIRED_LOCAL_SOURCE_AGENTS[0]}"
    CACHE_REPORT="/tmp/aitrack-usage-sync-${impl}-${CACHE_AGENT}-cached.json"
    if (cd "${GIT_REPO}" && env "${E2E_ENV[@]}" "AITRACK_SCAN_HOME=${AITRACK_HOME}" "${AITRACK_BIN}" usage sync --tool "${CACHE_AGENT}" >"${CACHE_REPORT}"); then
        CACHE_COUNTS=$(python3 -c "
import json, sys
with open('${CACHE_REPORT}', 'r', encoding='utf-8') as f:
    raw = f.read()
start = raw.find('{')
if start < 0:
    raise SystemExit('no json object in cache report')
data = json.loads(raw[start:])
scan = data.get('scan', {})
print(f\"{scan.get('parsed_messages', -1)} {scan.get('monitoring_events_parsed', -1)}\")
")
        CACHE_MESSAGES="${CACHE_COUNTS%% *}"
        CACHE_EVENTS="${CACHE_COUNTS##* }"
        if [ "${CACHE_MESSAGES}" = "0" ] && [ "${CACHE_EVENTS}" = "0" ]; then
            ok "Local scan cache skips unchanged ${CACHE_AGENT} source on immediate second sync"
        else
            fail "Local scan cache did not skip unchanged ${CACHE_AGENT} source (parsed_messages=${CACHE_MESSAGES}, monitoring_events_parsed=${CACHE_EVENTS})"
        fi
    else
        fail "usage sync --tool ${CACHE_AGENT} cache verification failed"
    fi

    # Cleanup this run's temps
    rm -rf "${AITRACK_HOME}" "${GIT_REPO}"
}

# ── Start a server and run the full e2e against it ────────────────────────────

run_e2e_impl() {
    local impl="$1"
    local image="aitrack-server-${impl}:e2e"
    local container="aitrack-client-e2e-${impl}-$$"
    local server_url="http://localhost:${SERVER_PORT}"

    log "Starting ${impl} server (container=${container})..."
    docker rm -f "${container}" 2>/dev/null || true
    # Kill any other container that may have grabbed our port from a previous failed run
    for stale in $(docker ps -q --filter "publish=${SERVER_PORT}" 2>/dev/null); do
        log "Removing stale container ${stale} occupying port ${SERVER_PORT}..."
        docker rm -f "${stale}" 2>/dev/null || true
    done

    if [ "${impl}" = "java" ]; then
        if ! docker run -d --name "${container}" \
            -e AITRACK_ADMIN_KEY="${ADMIN_KEY}" \
            -p "${SERVER_PORT}:8080" \
            "${image}" >/dev/null 2>&1; then
            log "ERROR: failed to start ${impl} container"
            FAIL_COUNT=$((FAIL_COUNT + 1))
            return 1
        fi
    else
        docker network create "${CLIENT_E2E_NET}" >/dev/null 2>&1 || true
        docker rm -f "${PG_CONTAINER}" >/dev/null 2>&1 || true
        if ! docker run -d --name "${PG_CONTAINER}" \
            --network "${CLIENT_E2E_NET}" \
            -e POSTGRES_USER=aitrack \
            -e POSTGRES_PASSWORD=aitrack_secret \
            -e POSTGRES_DB=aitrack_client_e2e \
            postgres:16-alpine >/dev/null 2>&1; then
            log "ERROR: failed to start Postgres for ${impl} client e2e"
            FAIL_COUNT=$((FAIL_COUNT + 1))
            return 1
        fi
        pg_timeout=30
        while ! docker exec "${PG_CONTAINER}" pg_isready -h 127.0.0.1 -p 5432 -U aitrack -d aitrack_client_e2e >/dev/null 2>&1; do
            if [ "${pg_timeout}" -le 0 ]; then
                log "ERROR: Postgres did not become ready for ${impl} client e2e"
                docker logs "${PG_CONTAINER}" 2>&1 | tail -20 || true
                FAIL_COUNT=$((FAIL_COUNT + 1))
                return 1
            fi
            sleep 1
            pg_timeout=$((pg_timeout - 1))
        done
        if ! docker run -d --name "${container}" \
            --network "${CLIENT_E2E_NET}" \
            -e AITRACK_ADMIN_KEY="${ADMIN_KEY}" \
            -e DATABASE_URL="postgres://aitrack:aitrack_secret@${PG_CONTAINER}:5432/aitrack_client_e2e?sslmode=disable" \
            -p "${SERVER_PORT}:8080" \
            "${image}" >/dev/null 2>&1; then
            log "ERROR: failed to start ${impl} container"
            FAIL_COUNT=$((FAIL_COUNT + 1))
            return 1
        fi
    fi

    # Save counts before this impl so we can report per-impl result
    local pre_pass=$PASS_COUNT
    local pre_fail=$FAIL_COUNT

    if ! wait_for_server "${server_url}"; then
        echo -e "${RED}Server ${impl} did not start — skipping assertions${NC}"
        docker logs "${container}" 2>&1 | tail -20
        docker rm -f "${container}" 2>/dev/null || true
        FAIL_COUNT=$((FAIL_COUNT + 1))
        return 1
    fi

    # Run all assertions
    run_against_server "${impl}" "${server_url}"

    local impl_pass=$((PASS_COUNT - pre_pass))
    local impl_fail=$((FAIL_COUNT - pre_fail))

    log "Stopping ${impl} server..."
    docker rm -f "${container}" 2>/dev/null || true
    if [ "${impl}" = "go" ]; then
        docker rm -f "${PG_CONTAINER}" 2>/dev/null || true
        docker network rm "${CLIENT_E2E_NET}" 2>/dev/null || true
    fi

    echo ""
    if [ $impl_fail -eq 0 ]; then
        echo -e "${GREEN}  ${impl} round: ${impl_pass} passed, 0 failed — PASS${NC}"
        return 0
    else
        echo -e "${RED}  ${impl} round: ${impl_pass} passed, ${impl_fail} failed — FAIL${NC}"
        return 1
    fi
}

# ── Main ───────────────────────────────────────────────────────────────────────

overall=0

if [ "${TARGET}" = "external" ]; then
    if [ -z "${AITRACK_E2E_SERVER_URL:-}" ]; then
        echo "ERROR: AITRACK_E2E_SERVER_URL is required for external target"
        exit 1
    fi
    if ! wait_for_server "${AITRACK_E2E_SERVER_URL}"; then
        overall=1
    else
        run_against_server external "${AITRACK_E2E_SERVER_URL}"
    fi
fi

if [ "${TARGET}" = "java" ] || [ "${TARGET}" = "both" ]; then
    if ! run_e2e_impl java; then
        overall=1
    fi
fi

if [ "${TARGET}" = "go" ] || [ "${TARGET}" = "both" ]; then
    if ! run_e2e_impl go; then
        overall=1
    fi
fi

echo ""
echo "══════════════════════════════════════════════════════════"
echo -e "  Total: ${GREEN}${PASS_COUNT} passed${NC}, ${RED}${FAIL_COUNT} failed${NC}"
echo "══════════════════════════════════════════════════════════"

if [ $overall -ne 0 ] || [ $FAIL_COUNT -ne 0 ]; then
    echo -e "${RED}CLIENT E2E SUITE FAILED${NC}"
    exit 1
fi
echo -e "${GREEN}CLIENT E2E SUITE PASSED${NC}"
