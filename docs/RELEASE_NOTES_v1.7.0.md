# v1.7.0 Release Notes Draft

## Summary

v1.7.0 turns aitrack into a broader local agent telemetry platform while preserving the original signed edit-evidence model. Claude Code, Codex CLI, and Cursor remain the native edit-hook path; additional agent frameworks can now contribute status, heartbeat, local transcript monitoring, usage rollups, and quota snapshots through local-only collection.

## Highlights

- Dynamic agent registry/status/heartbeat with canonical agent keys and aliases.
- Local usage source scanning for logs, JSON/JSONL/NDJSON, CSV, SQLite, caches, and local client state.
- Java and Go usage APIs for rollups, subscription snapshots, and summary queries.
- Transcript recovery for bounded prompt, tool, window, and reconstructable edit monitoring events.
- Bounded scanner design: 30-day default lookback, explicit `--since/--until` backfills, per-agent caps, JSONL/CSV row caps, and persistent file cursor cache.
- CI guardrails for architecture, source matrix coverage, full client E2E, and cache behavior.

## Supported Local Source Agents

Default local scans cover:

`claude`, `codex`, `cursor`, `trae`, `qwen`, `baidu-comate`, `wenxin`, `antigravity`, `opencode`, `qoder`, `qoder-cn`, `qoder-work`, `qoder-work-cn`, `wukong`, `hermes`, `openclaw`, `gemini`, `copilot`, `cline`, `roo-code`, `kiro`, `zed`, `goose`, `amp`, `droid`, `pi`, `mux`, `crush`, `codebuff`, `kilo`, `kilocode`, `kimi`, `gjc`, `grok`, `synthetic`, `warp`, and `zcode`.

Explicit `--tool` also accepts `roocode`, `kilo-code`, and `gajae-code` as aliases.

## Upgrade Notes

- No server-side breaking API removal is expected.
- New usage endpoints require the same Bearer token and request-signature model as other write APIs.
- `EditRecord` remains the signed monitoring-event domain. Usage rollups and quota snapshots are scalar usage data and do not replace diff-backed edit evidence.
- Default local scans are incremental and recent-window bounded. Use `--since/--until` for controlled historical backfills.

## Validation Evidence

- Rust client unit tests: 300 passed.
- Local source E2E matrix: 37 / 37 required agents covered.
- Client E2E cache assertion: immediate second sync of unchanged source parses 0 messages and 0 monitoring events.
- PR gate includes Rust, Java, Go, architecture, coverage, Java+Go E2E, Rust local-source E2E, Codecov, FOSSA, and automated review checks.

## Before Publishing

- Update release versions from `1.6.3` to `1.7.0` in `client/Cargo.toml` and `server-java/pom.xml`.
- Change the changelog and roadmap status from draft to released.
- Run all PR checks on the release commit.
- Tag `v1.7.0` or run the release workflow with `allow_release=true` and `tag_name=v1.7.0`.
