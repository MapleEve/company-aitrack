#!/usr/bin/env bash
# E2E runner script — starts both server implementations and runs all scenarios.
# Usage (local, requires Docker):
#   cd company-aitrack
#   bash e2e/run.sh [java|go|both]
#
# Usage (in Docker via docker-compose.e2e.yml):
#   SERVER_URL and ADMIN_KEY must be set in the environment.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

SERVER_URL="${SERVER_URL:-}"
ADMIN_KEY="${ADMIN_KEY:-e2e-admin-key}"
SERVER_IMPL="${SERVER_IMPL:-}"
TARGET="${1:-both}"

log() { echo "[e2e] $*"; }

# ── Docker-internal path (when run inside e2e container) ─────────────────────
if [ -n "$SERVER_URL" ] && [ -n "$SERVER_IMPL" ]; then
    log "Running e2e runner against $SERVER_URL (impl=$SERVER_IMPL)"
    export E2E_FIXTURES_DIR="/e2e/fixtures"
    exec /e2e-runner
fi

# ── Local path (run directly on host with Docker available) ──────────────────
if ! command -v docker &>/dev/null; then
    echo "ERROR: docker is required to run e2e locally"
    exit 1
fi

log "Building server images..."
(cd "$REPO_ROOT" && docker build -f docker/Dockerfile.server-java -t aitrack-server-java:e2e . 2>&1 | tail -5)
(cd "$REPO_ROOT" && docker build -f docker/Dockerfile.server-go   -t aitrack-server-go:e2e   . 2>&1 | tail -5)
log "Building e2e runner image..."
(cd "$REPO_ROOT" && docker build -f e2e/Dockerfile.e2e -t aitrack-e2e:latest . 2>&1 | tail -5)

E2E_NET="aitrack-e2e-net"
PG_CONTAINER="aitrack-e2e-postgres"

start_postgres() {
    log "Starting PostgreSQL for Go server..."
    docker network create "$E2E_NET" 2>/dev/null || true
    docker rm -f "$PG_CONTAINER" 2>/dev/null || true
    docker run -d --name "$PG_CONTAINER" \
        --network "$E2E_NET" \
        -e POSTGRES_USER=aitrack \
        -e POSTGRES_PASSWORD=aitrack_secret \
        -e POSTGRES_DB=aitrack_e2e \
        postgres:16-alpine
    local timeout=30
    while ! docker exec "$PG_CONTAINER" pg_isready -U aitrack -d aitrack_e2e &>/dev/null; do
        if [ "$timeout" -le 0 ]; then
            log "PostgreSQL did not become ready"
            return 1
        fi
        sleep 1
        timeout=$((timeout - 1))
    done
    log "PostgreSQL ready"
}

stop_postgres() {
    docker rm -f "$PG_CONTAINER" 2>/dev/null || true
    docker network rm "$E2E_NET" 2>/dev/null || true
}

run_e2e() {
    local impl="$1"
    local image="aitrack-server-${impl}:e2e"
    local port="8080"
    local container="aitrack-e2e-server-${impl}"

    log "Starting $impl server..."
    docker rm -f "$container" 2>/dev/null || true
    if [ "$impl" = "java" ]; then
        docker run -d --name "$container" \
            -e AITRACK_ADMIN_KEY="$ADMIN_KEY" \
            -p "${port}:8080" \
            "$image"
        # Wait for Spring to start
        sleep 15
    else
        # Go server is PostgreSQL-only; start a dedicated PG container
        start_postgres
        docker run -d --name "$container" \
            --network "$E2E_NET" \
            -e AITRACK_ADMIN_KEY="$ADMIN_KEY" \
            -e DATABASE_URL="postgres://aitrack:aitrack_secret@${PG_CONTAINER}:5432/aitrack_e2e?sslmode=disable" \
            -p "${port}:8080" \
            "$image"
        sleep 5
    fi

    local server_url="http://localhost:${port}"
    log "Running e2e scenarios against $impl ($server_url)..."
    set +e
    docker run --rm \
        --network host \
        -e SERVER_URL="$server_url" \
        -e ADMIN_KEY="$ADMIN_KEY" \
        -e SERVER_IMPL="$impl" \
        aitrack-e2e:latest
    local exit_code=$?
    set -e

    log "Stopping $impl server..."
    docker rm -f "$container" 2>/dev/null || true
    if [ "$impl" = "go" ]; then
        stop_postgres
    fi

    return $exit_code
}

overall=0

if [ "$TARGET" = "java" ] || [ "$TARGET" = "both" ]; then
    if ! run_e2e java; then
        log "FAIL: Java e2e"
        overall=1
    else
        log "PASS: Java e2e"
    fi
fi

if [ "$TARGET" = "go" ] || [ "$TARGET" = "both" ]; then
    if ! run_e2e go; then
        log "FAIL: Go e2e"
        overall=1
    else
        log "PASS: Go e2e"
    fi
fi

if [ $overall -ne 0 ]; then
    log "E2E SUITE FAILED"
    exit 1
fi
log "E2E SUITE PASSED"
