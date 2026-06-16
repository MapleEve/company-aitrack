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
    ("codex", "HookJsonl"),
    ("cursor", "HookJsonl"),
    ("opencode", "HookJsonl"),
    ("qoder", "HookJsonl"),
    ("qoder-cn", "HookJsonl"),
    ("qoder-work", "HookJsonl"),
    ("qoder-work-cn", "HookJsonl"),
    ("qoder", "Sqlite"),
    ("qoder-cn", "Sqlite"),
    ("qoder-work", "Sqlite"),
    ("qoder-work-cn", "Sqlite"),
    ("qoder", "IdeSnapshot"),
    ("qoder-cn", "IdeSnapshot"),
    ("qoder", "SessionJsonl"),
    ("qoder-work", "SessionJsonl"),
    ("wukong", "SessionJsonl"),
    ("trae", "GenericCache"),
]

SCAN_ALIAS_NAMES = {"roocode", "kilo-code", "gajae-code"}

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
        "FULL_LOCAL_SOURCE_CAPABILITIES",
    ]:
        if symbol not in text:
            fail(f"missing agent source spec symbol {symbol}")

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


def assert_usage_parser_surface() -> None:
    text = read("client/src/usage/mod.rs")
    for symbol in [
        "output_text_from_object",
        "tool_arguments_from_object",
        "tool_result_from_object",
        "is_skill_event",
        "is_tool_approval_event",
        "is_tool_result_event",
        "usage_monitoring_seen",
        'join("logs")',
        'Some("other")',
    ]:
        if symbol not in text:
            fail(f"usage parser missing {symbol}")

    for test_name in [
        "json_scan_extracts_output_tool_call_and_tool_result_monitoring_records",
        "json_scan_extracts_skill_approval_and_explicit_other_agent_events",
    ]:
        if test_name not in text:
            fail(f"missing usage parser regression test {test_name}")


def assert_e2e_matrix_gate() -> None:
    text = read("e2e/run-client-e2e.sh")
    agent_text = read("client/src/agent.rs")
    if "MIN_E2E_COVERAGE=90" not in text:
        fail("client e2e coverage threshold is not 90")
    if "MATRIX_COVERAGE" not in text:
        fail("client e2e matrix coverage calculation missing")

    registered = [
        name
        for name in re.findall(r'name:\s*"([^"]+)"', agent_text)
        if name not in SCAN_ALIAS_NAMES
    ]
    for agent in registered:
        if re.search(rf"^\s*{re.escape(agent)}\s*$", text, re.MULTILINE) is None:
            fail(f"client e2e matrix missing agent {agent}")
    for event_type in ["output", "tool_result", "skill", "tool_approval", "other"]:
        if f'\\"event_type\\":\\"{event_type}\\"' not in text:
            fail(f"client e2e does not assert {event_type}")


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


def main() -> None:
    assert_private_paths_are_untracked()
    assert_agent_source_specs()
    assert_usage_parser_surface()
    assert_e2e_matrix_gate()
    assert_ci_gate()
    assert_client_dependency_freeze()
    print("Architecture gate passed")


if __name__ == "__main__":
    main()
