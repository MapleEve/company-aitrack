#!/usr/bin/env python3
"""Repository architecture gate for local agent usage collection."""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]

REQUIRED_MATRIX = [
    ("claude", "HookJsonl"),
    ("claude", "SessionJsonl"),
    ("codex", "HookJsonl"),
    ("codex", "SessionJsonl"),
    ("cursor", "HookJsonl"),
    ("opencode", "SessionJsonl"),
    ("opencode", "Sqlite"),
    ("qoder", "HookJsonl"),
    ("qoder-cn", "HookJsonl"),
    ("wukong", "SessionJsonl"),
    ("trae", "SessionJsonl"),
    ("openclaw", "SessionJsonl"),
    ("gemini", "TelemetryLog"),
    ("copilot", "IdeSnapshot"),
    ("cline", "SessionJsonl"),
    ("kiro", "Sqlite"),
    ("zed", "Sqlite"),
    ("goose", "Sqlite"),
    ("pi", "SessionJsonl"),
    ("crush", "Sqlite"),
]

DEFAULT_SCAN_EXCLUDED_NAMES = {
    "antigravity",
    "roocode",
    "roo-code",
    "kilo-code",
    "gajae-code",
    "qoder-work",
    "qoder-work-cn",
    "synthetic",
}

ALLOWED_CLIENT_DEPS = {
    "anyhow",
    "base64",
    "chrono",
    "clap",
    "dirs",
    "ed25519-dalek",
    "gethostname",
    "hex",
    "hmac",
    "rand",
    "reqwest",
    "rusqlite",
    "serde",
    "serde_json",
    "sha2",
    "similar",
    "sqlite-vec",
    "tempfile",
    "thiserror",
    "tokio",
    "tokio-test",
    "toml",
    "uuid",
    "wiremock",
    "zerocopy",
}


def fail(message: str) -> None:
    print(f"ARCHITECTURE GATE FAIL: {message}", file=sys.stderr)
    sys.exit(1)


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def git_ls_files() -> list[str]:
    result = subprocess.run(
        ["git", "ls-files"],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=True,
    )
    return [line for line in result.stdout.splitlines() if line.strip()]


def assert_private_paths_are_untracked() -> None:
    tracked = git_ls_files()
    forbidden = [
        path
        for path in tracked
        if path == "CLAUDE.LOCAL.md"
        or path.startswith("internal/")
        or path.startswith("docs/adr/")
    ]
    if forbidden:
        fail(f"private/local paths are tracked: {', '.join(forbidden)}")


def assert_agent_source_specs() -> None:
    text = read("client/src/agent.rs")
    for symbol in [
        "LocalSourceKind",
        "LocalSourceCapabilities",
        "LocalSourceSpec",
        "local_source_specs",
        "OFFICIAL_TRANSCRIPT_CAPABILITIES",
        "OFFICIAL_HOOK_ACTION_CAPABILITIES",
        "OFFICIAL_PROMPT_TOOL_HOOK_CAPABILITIES",
        "LOCAL_USAGE_STATS_CAPABILITIES",
    ]:
        if symbol not in text:
            fail(f"missing agent source spec symbol {symbol}")
    if "UNVERIFIED_LOCAL_CAPABILITIES" in text:
        fail("unverified local capabilities must not be listed as default support evidence")

    for field in [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "session_context",
    ]:
        if f"{field}: true" not in text:
            fail(f"source capabilities do not require {field}")

    for agent, kind in REQUIRED_MATRIX:
        pattern = (
            rf'agent:\s*"{re.escape(agent)}"[\s\S]{{0,220}}'
            rf"kind:\s*LocalSourceKind::{kind}"
        )
        if re.search(pattern, text) is None:
            fail(f"missing source matrix cell {agent}/{kind}")

    registered = re.findall(r'name:\s*"([^"]+)"', text)
    default_agents = [
        name for name in registered if name not in DEFAULT_SCAN_EXCLUDED_NAMES
    ]
    for agent in default_agents:
        if re.search(rf'agent:\s*"{re.escape(agent)}"', text) is None:
            fail(f"missing local source spec for default agent {agent}")
    for agent in DEFAULT_SCAN_EXCLUDED_NAMES:
        if agent in {"roocode", "kilo-code", "gajae-code"}:
            continue
        if re.search(rf'agent:\s*"{re.escape(agent)}"', text) is not None:
            fail(f"{agent} must not have a default source spec without verified local evidence")
    for agent in ["baidu-comate", "wenxin"]:
        if re.search(rf'agent:\s*"{re.escape(agent)}"', text):
            fail(f"{agent} has no verified default local root and must not have a default source spec")


def assert_usage_parser_surface() -> None:
    text = read("client/src/usage/mod.rs")
    for symbol in [
        "DEFAULT_SCAN_LOOKBACK_DAYS",
        "MAX_SCAN_WINDOW_DAYS",
        "MAX_SCAN_FILES_PER_RUN",
        "MAX_SCAN_CANDIDATES_PER_AGENT",
        "MAX_SCAN_DIR_ENTRIES_PER_AGENT",
        "MAX_USAGE_SCAN_FILE_CACHE_ROWS",
        "MAX_USAGE_MONITORING_SEEN_ROWS",
        "MAX_USAGE_ROLLUP_SOURCE_ROWS",
        "MAX_JSONL_LINES_PER_FILE",
        "MAX_CSV_ROWS_PER_FILE",
        "MAX_SQLITE_TABLES_PER_FILE",
        "MAX_SQLITE_ROWS_PER_FILE",
        "MAX_EVENTS_PER_FILE",
        "ScanWindow",
        "FileScanPlan",
        "ScanCandidate",
        "usage_scan_file_cache",
        "ensure_usage_scan_file_cache_schema",
        "usage_rollup_sources",
        "ensure_usage_rollup_sources_schema",
        "replace_rollup_source",
        "compact_legacy_usage_sessions",
        "should_scan_source_file",
        "mark_source_file_scanned",
        "prune_usage_auxiliary_tables",
        "prune_table_by_rowid",
        "prune_rollup_source_rows",
        "output_text_from_object",
        "tool_arguments_from_object",
        "tool_result_from_object",
        "is_skill_event",
        "is_tool_approval_event",
        "is_tool_result_event",
        "usage_monitoring_seen",
        "ScanRootKind",
        "is_native_file",
        "is_import_file",
        "is_disallowed_native_file",
        "local-sources",
        'Some("other")',
    ]:
        if symbol not in text:
            fail(f"usage parser missing {symbol}")
    if "collect_files_with_extension" in text:
        fail("codex quota reader must not use unbounded extension recursion")
    if "roots.push(codex_home.clone())" in read("client/src/agent.rs"):
        fail("codex default scan roots must not include the whole CODEX_HOME")
    if (
        re.search(r'join\(\s*"logs"\s*\)[\s\S]{0,120}join\(\s*tool\s*\)', text)
        or re.search(r'join\(\s*"cache"\s*\)[\s\S]{0,120}join\(\s*tool\s*\)', text)
        or re.search(r'join\(\s*tool\s*\)[\s\S]{0,120}join\(\s*"logs"\s*\)', text)
        or re.search(r'join\(\s*tool\s*\)[\s\S]{0,120}join\(\s*"cache"\s*\)', text)
    ):
        fail("default import roots must not include generic logs/cache directories")

    for test_name in [
        "default_scan_uses_recent_window_and_explicit_window_backfills_old_usage",
        "scan_file_cache_skips_unchanged_default_scan_and_reopens_changed_file",
        "explicit_scan_window_is_clamped_to_thirty_days",
        "usage_auxiliary_pruning_keeps_newest_rows",
        "rollup_source_pruning_subtracts_old_contributions_before_rescan",
        "json_scan_extracts_output_tool_call_and_tool_result_monitoring_records",
        "json_scan_extracts_skill_approval_and_explicit_other_agent_events",
        "discovery_helpers_cover_roots_supported_files_and_skipped_dirs",
        "changed_source_replaces_previous_rollup_contribution",
        "source_rollup_keeps_database_bounded_for_many_messages",
        "scan_budget_resumes_after_cached_frontier",
        "default_scan_uses_global_recent_queue_so_late_agents_are_not_starved",
    ]:
        if test_name not in text:
            fail(f"missing usage parser regression test {test_name}")

    scan_into_match = re.search(r"async fn scan_into[\s\S]*?\n}\n\nfn open_usage_db", text)
    if not scan_into_match:
        fail("usage scan_into function not found")
    scan_into_body = scan_into_match.group(0)
    if "insert_usage_sessions" in scan_into_body or "rebuild_rollups" in scan_into_body:
        fail("normal usage scan path must not persist or rebuild from usage_sessions detail rows")

    expected_limits = {
        "DEFAULT_SCAN_LOOKBACK_DAYS: i64 = 30": "default usage scan window must stay bounded",
        "MAX_SCAN_WINDOW_DAYS: i64 = 30": "explicit usage scan window must stay bounded",
        "MAX_SCAN_FILES_PER_RUN: usize = 5": "per-run scan file budget must stay bounded",
        "MAX_SCAN_FILES_PER_AGENT: usize = 200": "per-agent scan file cap must stay bounded",
        "MAX_SCAN_CANDIDATES_PER_AGENT: usize = 800": "per-agent candidate cap must stay bounded",
        "MAX_SCAN_DIR_ENTRIES_PER_AGENT: usize = 5000": "directory traversal cap must stay bounded",
        "MAX_USAGE_SCAN_FILE_CACHE_ROWS: usize = 20_000": "scan file cache row cap must stay bounded",
        "MAX_USAGE_MONITORING_SEEN_ROWS: usize = 50_000": "monitoring seen row cap must stay bounded",
        "MAX_USAGE_ROLLUP_SOURCE_ROWS: usize = 20_000": "rollup source row cap must stay bounded",
        "MAX_JSONL_LINES_PER_FILE: usize = 2000": "jsonl line cap must stay bounded",
        "MAX_CSV_ROWS_PER_FILE: usize = 2000": "csv row cap must stay bounded",
        "MAX_SQLITE_TABLES_PER_FILE: usize = 10": "sqlite table cap must stay bounded",
        "MAX_SQLITE_ROWS_PER_FILE: usize = 5000": "sqlite row cap per file must stay bounded",
        "MAX_EVENTS_PER_FILE: usize = 200": "monitoring events per file must stay bounded",
    }
    for needle, message in expected_limits.items():
        if needle not in text:
            fail(message)

    schema_text = read("client/src/adapter/sqlite/schema.rs")
    if "idx_record_sig" not in schema_text:
        fail("records sqlite schema must index record_sig for bounded dedup lookups")


def assert_e2e_matrix_gate() -> None:
    text = read("e2e/run-client-e2e.sh")
    fixture_text = read("e2e/local_usage_matrix.py")
    agent_text = read("client/src/agent.rs")
    if "MIN_E2E_COVERAGE=100" not in text:
        fail("client e2e coverage threshold must require every default local source agent")
    if "MATRIX_COVERAGE" not in text:
        fail("client e2e matrix coverage calculation missing")
    if "Local scan cache skips unchanged" not in text:
        fail("client e2e does not verify unchanged local scan cache behavior")
    for needle in ["DETAIL_ROWS", "SOURCE_ROWS", "ROLLUP_ROWS", "usage_rollup_sources", "usage_daily_model_rollups"]:
        if needle not in text:
            fail(f"client e2e matrix must verify bounded local rollup storage: missing {needle}")

    registered = [
        name
        for name in re.findall(r'name:\s*"([^"]+)"', agent_text)
        if name not in DEFAULT_SCAN_EXCLUDED_NAMES
    ]
    for agent in registered:
        if re.search(rf"^\s*{re.escape(agent)}\s*$", text, re.MULTILINE) is None:
            fail(f"client e2e matrix missing agent {agent}")
    for agent in DEFAULT_SCAN_EXCLUDED_NAMES:
        if re.search(rf"^\s*{re.escape(agent)}\s*$", text, re.MULTILINE) is not None:
            fail(f"client e2e matrix must not claim default native support for {agent}")
    combined = text + "\n" + fixture_text
    for forbidden in [
        "matrix-${agent}",
        "prompt for local collection",
        "write_usage_jsonl_fixture",
        "write_usage_sqlite_fixture",
        "write_usage_json_fixture",
        "CREATE TABLE messages (data TEXT)",
    ]:
        if forbidden in combined:
            fail(f"client e2e matrix still accepts generic fixture proof: {forbidden}")
    for needle in [
        "local_usage_matrix.py",
        "usage_fixture_requires_positive_tokens",
        "usage_fixture_min_monitoring_events",
        "usage_fixture_required_event_types",
        "prompt_summary",
        "assistant_output",
        "tool_name",
        "tool_arguments",
    ]:
        if needle not in text:
            fail(f"client e2e matrix missing harness marker {needle}")
    for needle in [
        "state.vscdb",
        "ui_messages.json",
        "api_conversation_history.json",
        "session-usage.json",
        "chat_message",
        "token_info",
        "opencode.db",
        "threads.db",
        "sessions.db",
        "crush.db",
        "kilo.db",
        "db.sqlite",
        "usage-2026-06-16.json",
        "session-events.jsonl",
        "messages.json",
        "wire.jsonl",
        "updates.jsonl",
        "usageMetadata",
        "candidatesTokenCount",
        "tokensIn",
        "cacheReads",
        "spendCents",
    ]:
        if needle not in fixture_text:
            fail(f"client e2e native fixture coverage missing {needle}")
    for agent in registered:
        if agent in DEFAULT_SCAN_EXCLUDED_NAMES:
            continue
        if f'"{agent}"' not in fixture_text:
            fail(f"client e2e fixture generator missing real fixture branch for {agent}")
    if 'source_dir="${root}/sources/${agent}"' in text:
        fail("client e2e matrix still uses one generic sources/<agent> fixture")
    min_events_body = re.search(
        r"usage_fixture_min_monitoring_events\(\)[\s\S]*?\n}",
        text,
    ).group(0)
    required_events_body = re.search(
        r"usage_fixture_required_event_types\(\)[\s\S]*?\n}",
        text,
    ).group(0)
    for agent in ["gemini", "qwen", "trae", "wukong"]:
        if agent not in min_events_body:
            fail(f"client e2e monitoring event expectation missing {agent}")
        if agent not in required_events_body:
            fail(f"client e2e monitoring field expectation missing {agent}")


def assert_e2e_diagnostics_gate() -> None:
    for path in ["e2e/run.sh", "e2e/run-client-e2e.sh"]:
        text = read(path)
        if "tail " + "-5" in text or re.search(r"docker build[\s\S]{0,160}\|\s*tail", text):
            fail(f"{path} must not truncate docker build logs")
        if "--progress=plain" not in text:
            fail(f"{path} must use plain docker progress for CI diagnostics")

    dockerfile = read("docker/Dockerfile.server-java")
    if (ROOT / ("docker/" + "maven-" + "settings.xml")).exists():
        fail("Java Docker build must not carry a custom Maven mirror settings file")
    if "COPY docker/" + "maven-" + "settings.xml" in dockerfile or "/root/.m2/" + "settings.xml" in dockerfile:
        fail("Java Docker build must use the same default Maven repository path as Java CI")
    if re.search(r"\bmvn\s+-q\b", dockerfile):
        fail("Java Docker build must not hide Maven verify errors with -q")


def assert_public_docs_support_counts() -> None:
    docs = "\n".join(
        read(path)
        for path in [
            "README.md",
            "README.zh-CN.md",
            "README.en.md",
            "README.ja.md",
            "README.ko.md",
            "CONTRACT.md",
            "client/README.md",
            "docs/AGENT_SUPPORT.md",
            "docs/API.md",
            "docs/ARCHITECTURE.md",
            "docs/DEPLOYMENT.md",
            "docs/DEVELOPMENT.md",
            "docs/PRIVACY.md",
            "docs/ROADMAP.md",
            "docs/RELEASE_NOTES_v1.7.0.md",
            "docs/SECURITY_MODEL.md",
            "docs/TESTING.md",
            "CHANGELOG.md",
        ]
    )
    for forbidden in [
        "35 个默认",
        "默认 35",
        "扫描默认 35",
        "35 / 35",
        "3" + "7 个默认",
        "默认 " + "3" + "7",
        "扫描默认 " + "3" + "7",
        "3" + "7 / " + "3" + "7",
        "公开" + "文档",
        "本机" + "日志、会话记录、JSON/JSONL/NDJSON、CSV、SQLite、缓存",
        "本机工具" + "日志",
        "local transcript / " + "cache scan",
        "agent " + "logs",
        "cache " + "files " + "expose",
        "~/.aitrack/" + "sources/<agent>",
        "本地" + "工具目录",
        "ローカル" + "ログ",
        "로컬 " + "로그",
        "로컬 agent " + "로그",
        "baidu-comate",
        "wenxin",
    ]:
        if forbidden in docs:
            fail(f"public docs contain stale support wording: {forbidden}")

    expected_agents = [
        "claude",
        "codex",
        "cursor",
        "trae",
        "qwen",
        "opencode",
        "qoder",
        "qoder-cn",
        "wukong",
        "hermes",
        "openclaw",
        "gemini",
        "copilot",
        "cline",
        "kiro",
        "zed",
        "goose",
        "amp",
        "droid",
        "pi",
        "mux",
        "crush",
        "codebuff",
        "kilo",
        "kilocode",
        "kimi",
        "gjc",
        "grok",
        "warp",
        "zcode",
    ]
    support_doc = read("docs/AGENT_SUPPORT.md")
    default_list_match = re.search(
        r"以下 30 个规范 key：\n\n(.+?)。",
        support_doc,
        flags=re.S,
    )
    if not default_list_match:
        fail("AGENT_SUPPORT default scan list must explicitly name 30 verified keys")
    default_list = default_list_match.group(1)
    for agent in expected_agents:
        if f"`{agent}`" not in default_list:
            fail(f"AGENT_SUPPORT default scan list missing {agent}")
    for agent in ["antigravity", "qoder-work", "qoder-work-cn", "roo-code", "synthetic"]:
        if f"`{agent}`" in default_list:
            fail(f"AGENT_SUPPORT default scan list includes unverified key {agent}")


def assert_ci_gate() -> None:
    text = read(".github/workflows/ci.yml")
    if "Architecture gate" not in text:
        fail("CI is missing architecture gate job")
    if "python3 scripts/architecture_gate.py" not in text:
        fail("CI does not run architecture gate script")
    if "bash e2e/run-client-e2e.sh both" not in text:
        fail("CI does not run real client e2e for both servers")


def assert_client_dependency_freeze() -> None:
    text = read("client/Cargo.toml")
    deps = set()
    in_dep_section = False
    for line in text.splitlines():
        stripped = line.strip()
        if stripped == "[dependencies]" or stripped == "[dev-dependencies]":
            in_dep_section = True
            continue
        if stripped.startswith("[") and stripped.endswith("]"):
            in_dep_section = False
        if in_dep_section and stripped and not stripped.startswith("#"):
            name = stripped.split("=", 1)[0].strip()
            if name:
                deps.add(name)
    extra = sorted(deps - ALLOWED_CLIENT_DEPS)
    if extra:
        fail(f"client dependency list changed without gate update: {', '.join(extra)}")


def assert_generated_coverage_hygiene() -> None:
    gitignore = read(".gitignore")
    for pattern in ["*.profraw", "*.profdata"]:
        if pattern not in gitignore:
            fail(f".gitignore must ignore generated coverage artifact {pattern}")
    tracked = [path for path in git_ls_files() if path.endswith((".profraw", ".profdata"))]
    if tracked:
        fail(f"generated coverage artifacts are tracked: {', '.join(tracked)}")


def main() -> None:
    assert_private_paths_are_untracked()
    assert_agent_source_specs()
    assert_usage_parser_surface()
    assert_e2e_matrix_gate()
    assert_e2e_diagnostics_gate()
    assert_public_docs_support_counts()
    assert_ci_gate()
    assert_client_dependency_freeze()
    assert_generated_coverage_hygiene()
    print("Architecture gate passed")


if __name__ == "__main__":
    main()
