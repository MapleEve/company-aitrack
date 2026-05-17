# aitrack Docker

Three multi-stage Dockerfiles: each stage builds the component, runs tests, enforces a coverage gate, then produces a minimal runtime image.

## Prerequisites

- Docker 20+
- Images are built from the repo root (`aitrack-selfbuilt/`), not from the `docker/` directory — the build context must include all three components.

---

## Individual image builds

All commands run from `aitrack-selfbuilt/`:

### Rust client

```bash
docker build -f docker/Dockerfile.client -t aitrack-client:latest .
```

- Builds with `cargo build --release`
- Runs `cargo test`
- Checks `cargo llvm-cov` line coverage ≥ 90% (build fails if not met)
- Runtime: `debian:bookworm-slim` + `/usr/local/bin/aitrack`

### Java server

```bash
docker build -f docker/Dockerfile.server-java -t aitrack-server-java:latest .
```

- Runs `mvn verify` (includes JaCoCo LINE coverage ≥ 90% gate)
- Runtime: `eclipse-temurin:17-jre` on port 8080
- H2 database persisted to `/app/data`

### Go server

```bash
docker build -f docker/Dockerfile.server-go -t aitrack-server-go:latest .
```

- Runs `go test ./... -coverprofile=cover.out`
- Checks total coverage ≥ 90% (build fails if not met)
- Runtime: `gcr.io/distroless/base-debian12` (no shell) on port 8080

---

## Deployment

### Start Java server

```bash
docker compose -f docker/docker-compose.yml --profile java up -d
```

Server is available at `http://localhost:8080`.

### Start Go server

```bash
docker compose -f docker/docker-compose.yml --profile go up -d
```

Server is available at `http://localhost:8081`.

### Environment variables

| Variable | Default | Description |
|---|---|---|
| `AITRACK_ADMIN_KEY` | `dev-admin-key-change-in-prod` | Required to call `POST /admin/tokens` |
| `AITRACK_SECRET_KEY` | `""` | AES-256-GCM key for encrypting hmac_secret at rest; leave blank for dev |

Set via `.env` file or shell export before running compose.

---

## E2E tests

```bash
# Run both Java and Go e2e
bash e2e/run.sh both

# Run Java only
bash e2e/run.sh java

# Run Go only
bash e2e/run.sh go
```

See `e2e/README.md` for details.

---

## Data volumes

| Service | Volume | Path in container |
|---|---|---|
| server-java | `aitrack-java-data` | `/app/data` |
| server-go | `aitrack-go-data` | `/data` |

Remove volumes with: `docker compose -f docker/docker-compose.yml down -v`
