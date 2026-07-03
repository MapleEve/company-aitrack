#!/usr/bin/env python3
"""Generate isolated local usage fixtures for the client e2e matrix."""

from __future__ import annotations

import json
import sqlite3
import sys
import tempfile
from pathlib import Path
from typing import Any


ISO_TS = "2026-06-16T14:00:00Z"
ISO_TS_1 = "2026-06-16T14:00:01Z"
EPOCH_S = 1_781_589_600
EPOCH_MS = 1_781_589_600_000
EPOCH_MS_1 = EPOCH_MS + 1000
ZED_ZSTD_FIXTURE = bytes.fromhex(
    "28b52ffd600b019d0800c6d03221406be2064030c376d4de726d0a969004671b475186732aa5bffe23"
    "a4a8280a3001290029002700fc90a6c2366aceb790ee66b900a253751991ea214b376fa4425436fcdc"
    "a55a8d75ed8f73966a953f02c9efe7d91cfd87c139079cb7be4d2817c89ccacd42a0301d4a46c2"
    "743390e8b73749c139bfa62b17180c2039fe39a3b1716b2a6fe130d94a5050cd19c384d86eb7"
    "62f36246b3bc9bb2ae1db275d3cd5dcb132481243fd75decb27173b01c7e9a3d4ea78a7386"
    "d49babe25cb1bd61a4c6861cac355cce0e4158011b00364b026902c6e4d021516aec18184a"
    "594616a55f01c28145701b26775e965e821bc278191c7850cc009e9a47a9a2cc1848293bd4"
    "2c775385b2c9114e922bd8be03a19c"
)

LOCAL_DERIVED_USAGE_SOURCES = set()
SHARED_EDIT_FIXTURE_PATHS = (
    "src/lib.rs",
    "src/main.rs",
)


def fixture_slug(value: str) -> str:
    return "".join(ch if ch.isalnum() else "-" for ch in value.lower()).strip("-")


def edit_fixture_path(agent: str) -> str:
    return f"src/aitrack-matrix/{fixture_slug(agent)}/edit.rs"


def quote_sqlite_identifier(name: str) -> str:
    return '"' + name.replace('"', '""') + '"'


def replace_shared_edit_paths(text: str, agent: str) -> str:
    replacement = edit_fixture_path(agent)
    for path in SHARED_EDIT_FIXTURE_PATHS:
        text = text.replace(path, replacement)
    return text


def normalize_sqlite_fixture_edit_paths(path: Path, agent: str) -> None:
    try:
        conn = sqlite3.connect(path)
    except sqlite3.DatabaseError:
        return
    try:
        try:
            tables = [
                row[0]
                for row in conn.execute(
                    "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'"
                ).fetchall()
            ]
            for table in tables:
                columns = [
                    (row[1], (row[2] or "").upper())
                    for row in conn.execute(f"PRAGMA table_info({quote_sqlite_identifier(table)})")
                ]
                text_columns = [
                    name
                    for name, column_type in columns
                    if "TEXT" in column_type or column_type in {"", "VARCHAR", "CHAR", "CLOB"}
                ]
                if not text_columns:
                    continue
                quoted_table = quote_sqlite_identifier(table)
                quoted_columns = ", ".join(quote_sqlite_identifier(column) for column in text_columns)
                rows = conn.execute(
                    f"SELECT rowid, {quoted_columns} FROM {quoted_table}"
                ).fetchall()
                for row in rows:
                    rowid = row[0]
                    updates: list[tuple[str, str]] = []
                    for column, value in zip(text_columns, row[1:]):
                        if isinstance(value, str):
                            replaced = replace_shared_edit_paths(value, agent)
                            if replaced != value:
                                updates.append((column, replaced))
                    if not updates:
                        continue
                    set_clause = ", ".join(
                        f"{quote_sqlite_identifier(column)} = ?" for column, _ in updates
                    )
                    conn.execute(
                        f"UPDATE {quoted_table} SET {set_clause} WHERE rowid = ?",
                        [value for _, value in updates] + [rowid],
                    )
            conn.commit()
        except sqlite3.DatabaseError:
            return
    finally:
        conn.close()


def normalize_text_fixture_edit_paths(path: Path, agent: str) -> None:
    try:
        text = path.read_text(encoding="utf-8")
    except (UnicodeDecodeError, OSError):
        return
    replaced = replace_shared_edit_paths(text, agent)
    if replaced != text:
        path.write_text(replaced, encoding="utf-8")


def normalize_fixture_edit_paths(root: Path, agent: str) -> None:
    for path in fixture_file_paths(root):
        suffix = path.suffix.lower()
        if suffix in {".db", ".sqlite", ".sqlite3", ".vscdb"}:
            normalize_sqlite_fixture_edit_paths(path, agent)
        else:
            normalize_text_fixture_edit_paths(path, agent)


def sqlite_text_payloads(path: Path) -> list[tuple[str, str]]:
    try:
        conn = sqlite3.connect(path)
    except sqlite3.DatabaseError:
        return []
    payloads: list[tuple[str, str]] = []
    try:
        try:
            tables = [
                row[0]
                for row in conn.execute(
                    "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'"
                ).fetchall()
            ]
            for table in tables:
                columns = [
                    (row[1], (row[2] or "").upper())
                    for row in conn.execute(f"PRAGMA table_info({quote_sqlite_identifier(table)})")
                ]
                text_columns = [
                    name
                    for name, column_type in columns
                    if "TEXT" in column_type or column_type in {"", "VARCHAR", "CHAR", "CLOB"}
                ]
                if not text_columns:
                    continue
                quoted_columns = ", ".join(quote_sqlite_identifier(column) for column in text_columns)
                rows = conn.execute(
                    f"SELECT {quoted_columns} FROM {quote_sqlite_identifier(table)}"
                ).fetchall()
                for row in rows:
                    for column, value in zip(text_columns, row):
                        if isinstance(value, str):
                            payloads.append((f"{table}.{column}", value))
        except sqlite3.DatabaseError:
            return []
    finally:
        conn.close()
    return payloads


def shared_edit_fixture_path_hits(root: Path) -> list[str]:
    hits: list[str] = []
    for path in fixture_file_paths(root):
        suffix = path.suffix.lower()
        if suffix in {".db", ".sqlite", ".sqlite3", ".vscdb"}:
            payloads = sqlite_text_payloads(path)
        else:
            try:
                payloads = [("text", path.read_text(encoding="utf-8"))]
            except (UnicodeDecodeError, OSError):
                continue
        for location, text in payloads:
            for shared_path in SHARED_EDIT_FIXTURE_PATHS:
                if shared_path in text:
                    hits.append(f"{path.relative_to(root)}:{location}:{shared_path}")
    return hits


def usage_source_expected_usage_basis(agent: str, label: str) -> str:
    return "local_derived" if (agent, label) in LOCAL_DERIVED_USAGE_SOURCES else "native"


def usage_fixture_expected_usage_basis(agent: str, label: str | None = None) -> str:
    if label is not None:
        return usage_source_expected_usage_basis(agent, label)
    return "local_derived" if any(
        source_agent == agent for source_agent, _ in LOCAL_DERIVED_USAGE_SOURCES
    ) else "native"


USAGE_FIELDS_BY_CAPABILITY = {
    "token_usage": ["tokens_in", "tokens_out", "message_count", "usage_basis"],
    "aggregate_usage": ["tokens_in", "message_count", "usage_basis"],
    "cache_usage": ["cache"],
    "account_context": ["account"],
    "cost_usage": ["source_cost"],
    "reasoning_usage": ["reasoning"],
}

RECORD_FIELDS_BY_CAPABILITY = {
    "prompt_input": ["prompt_summary"],
    "assistant_output": ["assistant_output"],
    "tool_call": ["tool_name", "tool_arguments"],
    "tool_result": ["tool_result"],
    "edit_diff": ["file_path_or_diff"],
    "model_provider_context": ["provider", "model"],
    "session_context": ["session_id"],
    "time_context": ["timestamp_ms"],
}

MONITORING_CAPABILITIES = {
    "prompt_input",
    "assistant_output",
    "tool_call",
    "tool_result",
    "edit_diff",
}

EVENT_TYPES_BY_CAPABILITY = {
    "prompt_input": "prompt",
    "assistant_output": "output",
    "tool_call": "tool",
    "tool_result": "tool_result",
    "edit_diff": "edit",
}

CAPABILITY_PROFILES = {
    "native_edit_hook": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "cache_usage",
        "cost_usage",
        "reasoning_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "account_context",
        "edit_diff",
    ],
    "claude_hook": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "session_context",
        "edit_diff",
    ],
    "gemini_telemetry": [
        "tool_call",
        "tool_result",
        "token_usage",
        "session_context",
        "model_provider_context",
        "reasoning_usage",
        "edit_diff",
    ],
    "qwen_telemetry": [
        "tool_call",
        "tool_result",
        "token_usage",
        "session_context",
        "model_provider_context",
        "edit_diff",
    ],
    "official_transcript": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "session_context",
        "model_provider_context",
        "edit_diff",
    ],
    "official_session_export": [
        "prompt_input",
        "assistant_output",
        "token_usage",
        "session_context",
        "model_provider_context",
        "cost_usage",
        "reasoning_usage",
    ],
    "official_prompt_tool_hook": [
        "prompt_input",
        "tool_call",
        "tool_result",
        "session_context",
        "edit_diff",
    ],
    "qoder_work_hook_jsonl": [
        "prompt_input",
        "tool_call",
        "tool_result",
        "session_context",
        "edit_diff",
    ],
    "qoder_work_trace_jsonl": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "session_context",
        "time_context",
        "model_provider_context",
        "edit_diff",
    ],
    "qoder_work_local_db": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "cache_usage",
        "cost_usage",
        "reasoning_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "account_context",
        "edit_diff",
    ],
    "kiro_hook_jsonl": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "cache_usage",
        "cost_usage",
        "reasoning_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "account_context",
        "edit_diff",
    ],
    "kiro_cli_session": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "cost_usage",
        "reasoning_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "edit_diff",
    ],
    "local_derived_transcript": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "cache_usage",
        "cost_usage",
        "reasoning_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "account_context",
        "edit_diff",
    ],
    "qoder_transcript_jsonl": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "cache_usage",
        "cost_usage",
        "reasoning_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "edit_diff",
    ],
    "field_level_extended_session": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "session_context",
        "time_context",
        "model_provider_context",
    ],
    "field_level_session": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "session_context",
        "time_context",
        "model_provider_context",
    ],
    "mux_chat_jsonl": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "session_context",
        "time_context",
        "model_provider_context",
        "edit_diff",
    ],
    "mux_session_usage_json": [
        "token_usage",
        "cache_usage",
        "cost_usage",
        "reasoning_usage",
        "session_context",
        "time_context",
        "model_provider_context",
    ],
    "codebuff_project_jsonl": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "cost_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "edit_diff",
    ],
    "warp_sqlite": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "aggregate_usage",
        "cost_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "edit_diff",
    ],
    "antigravity_conversation_sqlite": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "cache_usage",
        "reasoning_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "edit_diff",
    ],
    "qoder_local_db": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "edit_diff",
    ],
    "copilot_official_runtime_jsonl": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "cache_usage",
        "cost_usage",
        "reasoning_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "edit_diff",
    ],
    "copilot_session_store_db": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "cache_usage",
        "reasoning_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "edit_diff",
    ],
    "supplemental_usage_summary": [
        "token_usage",
        "session_context",
        "model_provider_context",
        "cost_usage",
        "reasoning_usage",
    ],
    "supplemental_token_summary": [
        "token_usage",
        "session_context",
        "model_provider_context",
        "cost_usage",
        "reasoning_usage",
    ],
    "conditional_otel": [
        "tool_call",
        "tool_result",
        "token_usage",
        "session_context",
        "model_provider_context",
        "reasoning_usage",
    ],
    "supplemental_state": ["session_context"],
    "cursor_state_vscdb": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "cache_usage",
        "cost_usage",
        "reasoning_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "account_context",
        "edit_diff",
    ],
    "local_transcript_event": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "session_context",
    ],
    "grok_session_jsonl": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "cache_usage",
        "cost_usage",
        "reasoning_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "account_context",
        "edit_diff",
    ],
    "zcode_project_jsonl": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "cache_usage",
        "cost_usage",
        "reasoning_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "account_context",
        "edit_diff",
    ],
    "wukong_sqlite": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "cache_usage",
        "cost_usage",
        "reasoning_usage",
        "session_context",
        "model_provider_context",
        "edit_diff",
    ],
    "opencode_sqlite": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "cost_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "account_context",
        "edit_diff",
    ],
    "hermes_sqlite": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "cache_usage",
        "cost_usage",
        "reasoning_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "edit_diff",
    ],
    "openclaw_session_jsonl": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "cache_usage",
        "cost_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "edit_diff",
    ],
    "cline_family_task": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "session_context",
        "time_context",
        "model_provider_context",
        "edit_diff",
    ],
    "cline_family_ui_usage": [
        "token_usage",
        "cache_usage",
        "cost_usage",
        "session_context",
        "time_context",
        "model_provider_context",
    ],
    "amp_stream_jsonl": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "cache_usage",
        "cost_usage",
        "reasoning_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "account_context",
        "edit_diff",
    ],
    "droid_session_jsonl": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "cache_usage",
        "reasoning_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "edit_diff",
    ],
    "droid_settings_json": [
        "token_usage",
        "cache_usage",
        "reasoning_usage",
        "session_context",
        "time_context",
        "model_provider_context",
    ],
    "crush_sqlite": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "cost_usage",
        "reasoning_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "edit_diff",
    ],
    "kilo_full_local": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "cache_usage",
        "cost_usage",
        "reasoning_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "account_context",
        "edit_diff",
    ],
    "kimi_wire_jsonl": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "cache_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "account_context",
        "edit_diff",
    ],
    "gjc_session_jsonl": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "cache_usage",
        "cost_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "edit_diff",
    ],
    "synthetic_sqlite": [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "cache_usage",
        "cost_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "account_context",
        "edit_diff",
    ],
}

SOURCE_CAPABILITY_PROFILES = {
    ("claude", "hook-jsonl"): "claude_hook",
    ("claude", "projects-jsonl"): "official_transcript",
    ("codex", "hook-jsonl"): "native_edit_hook",
    ("codex", "rollout-jsonl"): "official_transcript",
    ("cursor", "hook-jsonl"): "local_derived_transcript",
    ("cursor", "agent-transcripts-jsonl"): "local_derived_transcript",
    ("cursor", "state-vscdb"): "cursor_state_vscdb",
    ("trae", "trajectory-json"): "official_transcript",
    ("qwen", "telemetry-log"): "qwen_telemetry",
    ("qwen", "project-chats-jsonl"): "field_level_session",
    ("qwen", "usage-record-jsonl"): "supplemental_token_summary",
    ("qwen", "token-usage-jsonl"): "supplemental_token_summary",
    ("opencode", "export-json"): "official_session_export",
    ("opencode", "sqlite"): "opencode_sqlite",
    ("qoder", "hook-jsonl"): "official_prompt_tool_hook",
    ("qoder", "transcript-jsonl"): "qoder_transcript_jsonl",
    ("qoder", "local-db"): "qoder_local_db",
    ("qoder-cn", "hook-jsonl"): "official_prompt_tool_hook",
    ("qoder-cn", "transcript-jsonl"): "qoder_transcript_jsonl",
    ("qoder-cn", "local-db"): "qoder_local_db",
    ("qoder-work", "hook-jsonl"): "qoder_work_hook_jsonl",
    ("qoder-work", "trace-jsonl"): "qoder_work_trace_jsonl",
    ("qoder-work", "local-db"): "qoder_work_local_db",
    ("qoder-work-cn", "hook-jsonl"): "qoder_work_hook_jsonl",
    ("qoder-work-cn", "trace-jsonl"): "qoder_work_trace_jsonl",
    ("qoder-work-cn", "local-db"): "qoder_work_local_db",
    ("wukong", "sqlite"): "wukong_sqlite",
    ("hermes", "sqlite"): "hermes_sqlite",
    ("openclaw", "session-jsonl"): "openclaw_session_jsonl",
    ("gemini", "telemetry-log"): "gemini_telemetry",
    ("gemini", "tmp-chats-jsonl"): "field_level_extended_session",
    ("copilot", "otel-jsonl"): "conditional_otel",
    ("copilot", "official-copilot-runtime-jsonl"): "copilot_official_runtime_jsonl",
    ("copilot", "session-state-jsonl"): "local_transcript_event",
    ("copilot", "session-store-db"): "copilot_session_store_db",
    ("copilot", "vscode-chat-state"): [
        "prompt_input",
        "assistant_output",
        "session_context",
    ],
    ("cline", "vscode-tasks"): "cline_family_task",
    ("cline", "vscode-ui-messages"): "cline_family_ui_usage",
    ("cline", "sessions-db"): "supplemental_state",
    ("roo-code", "vscode-tasks"): "cline_family_task",
    ("roo-code", "vscode-ui-messages"): "cline_family_ui_usage",
    ("kiro", "hook-jsonl"): "kiro_hook_jsonl",
    ("kiro", "data-sqlite"): "field_level_extended_session",
    ("kiro", "cli-session-json"): "kiro_cli_session",
    ("zed", "threads-db"): "official_transcript",
    ("goose", "sessions-db"): "official_transcript",
    ("amp", "threads-jsonl"): "amp_stream_jsonl",
    ("droid", "session-jsonl"): "droid_session_jsonl",
    ("droid", "settings-json"): "droid_settings_json",
    ("pi", "session-jsonl"): "official_transcript",
    ("mux", "chat-jsonl"): "mux_chat_jsonl",
    ("mux", "session-usage-json"): "mux_session_usage_json",
    ("crush", "sqlite"): "crush_sqlite",
    ("codebuff", "project-jsonl"): "codebuff_project_jsonl",
    ("kilo", "sqlite"): "kilo_full_local",
    ("kilo", "storage-json"): "kilo_full_local",
    ("kilocode", "sqlite"): "kilo_full_local",
    ("kilocode", "storage-json"): "kilo_full_local",
    ("kilocode", "vscode-tasks"): "local_transcript_event",
    ("kilocode", "vscode-ui-messages"): "cline_family_ui_usage",
    ("kimi", "wire-jsonl"): "kimi_wire_jsonl",
    ("gjc", "session-jsonl"): "gjc_session_jsonl",
    ("grok", "sessions-jsonl"): "grok_session_jsonl",
    ("synthetic", "sqlite"): "synthetic_sqlite",
    ("warp", "warp-sqlite"): "warp_sqlite",
    ("antigravity", "conversation-sqlite"): "antigravity_conversation_sqlite",
    ("zcode", "projects-jsonl"): "zcode_project_jsonl",
}


def fields_for_capabilities(
    capabilities: list[str],
    field_map: dict[str, list[str]],
) -> list[str]:
    fields: list[str] = []
    for capability in capabilities:
        for field in field_map.get(capability, []):
            if field not in fields:
                fields.append(field)
    return fields


def source_capabilities(agent: str, label: str, profile: str | list[str] | None = None) -> list[str]:
    profile = profile if profile is not None else SOURCE_CAPABILITY_PROFILES.get((agent, label))
    if profile is None:
        raise SystemExit(f"ERROR: no expected field profile for {agent}/{label}")
    return CAPABILITY_PROFILES[profile] if isinstance(profile, str) else profile


def source_capability_fields(
    agent: str,
    label: str,
    profile: str | list[str] | None = None,
) -> tuple[list[str], list[str], list[str]]:
    capabilities = source_capabilities(agent, label, profile)
    return (
        fields_for_capabilities(capabilities, USAGE_FIELDS_BY_CAPABILITY),
        fields_for_capabilities(capabilities, RECORD_FIELDS_BY_CAPABILITY),
        [],
    )


def source_required_event_types(
    agent: str,
    label: str,
    profile: str | list[str] | None = None,
) -> list[str]:
    event_types: list[str] = []
    for capability in source_capabilities(agent, label, profile):
        event_type = EVENT_TYPES_BY_CAPABILITY.get(capability)
        if event_type and event_type not in event_types:
            event_types.append(event_type)
    return event_types


def source_expects_usage(agent: str, label: str, profile: str | list[str] | None = None) -> bool:
    capabilities = source_capabilities(agent, label, profile)
    return any(
        capability in capabilities
        for capability in [
            "token_usage",
            "aggregate_usage",
            "cache_usage",
            "cost_usage",
            "reasoning_usage",
        ]
    ) or (agent, label) in LOCAL_DERIVED_USAGE_SOURCES


def source_expects_monitoring(agent: str, label: str, profile: str | list[str] | None = None) -> bool:
    capabilities = source_capabilities(agent, label, profile)
    return any(capability in MONITORING_CAPABILITIES for capability in capabilities)


def expected_source(
    agent: str,
    label: str,
    kind: str,
    path_substring: str,
    session_id: str,
    profile: str | list[str] | None = None,
) -> dict[str, Any]:
    expects_usage = source_expects_usage(agent, label, profile)
    expects_monitoring = source_expects_monitoring(agent, label, profile)
    required_usage_fields, required_record_fields, optional_record_fields = source_capability_fields(
        agent, label, profile
    )
    if expects_usage:
        for field in ["message_count", "usage_basis", "day"]:
            if field not in required_usage_fields:
                required_usage_fields.append(field)
    else:
        required_usage_fields = []
    if not expects_monitoring:
        required_record_fields = []
        optional_record_fields = []
    else:
        if "timestamp_ms" not in required_record_fields:
            required_record_fields.append("timestamp_ms")
        required_event_types = source_required_event_types(agent, label, profile)

    entry: dict[str, Any] = {
        "agent": agent,
        "label": label,
        "kind": kind,
        "path_substring": path_substring,
        "expects_usage": expects_usage,
        "expects_monitoring": expects_monitoring,
        "session_id": session_id,
    }
    if expects_usage:
        entry["usage_basis"] = usage_source_expected_usage_basis(agent, label)
    if expects_monitoring:
        entry["required_event_types"] = required_event_types
    entry["required_usage_fields"] = required_usage_fields
    entry["required_record_fields"] = required_record_fields
    entry["optional_record_fields"] = optional_record_fields
    return entry


EXPECTED_SOURCES: dict[str, list[dict[str, Any]]] = {
    "claude": [
        expected_source(
            "claude",
            "hook-jsonl",
            "HookJsonl",
            "local-sources/claude/hook-events.jsonl",
            "claude-hook-session",
        ),
        expected_source("claude", "projects-jsonl", "SessionJsonl", ".claude/projects/e2e/session-claude.jsonl", "claude-session"),
    ],
    "codex": [
        expected_source(
            "codex",
            "hook-jsonl",
            "HookJsonl",
            "local-sources/codex/hook-events.jsonl",
            "codex-hook-session",
        ),
        expected_source("codex", "rollout-jsonl", "SessionJsonl", ".codex/sessions/2026/06/16/rollout-e2e-codex.jsonl", "codex-session"),
    ],
    "cursor": [
        expected_source("cursor", "hook-jsonl", "HookJsonl", ".cursor/hooks/cursor-hooks.jsonl", "cursor-hook-session"),
        expected_source("cursor", "agent-transcripts-jsonl", "SessionJsonl", ".cursor/projects/project-one/agent-transcripts/cursor-session.jsonl", "cursor-agent-session"),
        expected_source("cursor", "state-vscdb", "Sqlite", "Cursor/User/globalStorage/state.vscdb", "cursor-session"),
    ],
    "trae": [
        expected_source("trae", "trajectory-json", "SessionJsonl", "trajectories/trajectory_20260616_140000.json", "trae-session"),
    ],
    "qwen": [
        expected_source("qwen", "telemetry-log", "TelemetryLog", ".qwen/telemetry.log", "qwen-prompt"),
        expected_source("qwen", "project-chats-jsonl", "SessionJsonl", ".qwen/projects/project-one/chats/session-qwen.jsonl", "qwen-session"),
        expected_source("qwen", "usage-record-jsonl", "SessionJsonl", ".qwen/usage_record.jsonl", "qwen-session"),
        expected_source("qwen", "token-usage-jsonl", "SessionJsonl", ".qwen/usage/token-usage-2026-06.jsonl", "qwen-token-session"),
    ],
    "opencode": [
        expected_source("opencode", "export-json", "SessionJsonl", ".config/opencode/exports/session-opencode-message.json", "opencode-export-session"),
        expected_source("opencode", "sqlite", "Sqlite", ".local/share/opencode/opencode.db", "opencode-session"),
    ],
    "qoder": [
        expected_source("qoder", "hook-jsonl", "HookJsonl", ".qoder/hooks/qoder-hooks.jsonl", "qoder-hook-session"),
        expected_source("qoder", "transcript-jsonl", "SessionJsonl", ".qoder/projects/project-one/transcript/qoder-session.jsonl", "qoder-transcript-session"),
        expected_source("qoder", "local-db", "Sqlite", "Library/Application Support/Qoder/SharedClientCache/cache/db/local.db", "qoder-session"),
    ],
    "qoder-cn": [
        expected_source("qoder-cn", "hook-jsonl", "HookJsonl", ".lingma/hooks/qoder-cn-hooks.jsonl", "qoder-cn-hook-session"),
        expected_source("qoder-cn", "transcript-jsonl", "SessionJsonl", ".lingma/projects/project-one/transcript/qoder-cn-session.jsonl", "qoder-cn-transcript-session"),
        expected_source("qoder-cn", "local-db", "Sqlite", "Library/Application Support/QoderCN/SharedClientCache/cache/db/local.db", "qoder-cn-session"),
    ],
    "qoder-work": [
        expected_source("qoder-work", "hook-jsonl", "HookJsonl", ".qoderwork/hooks/qoder-work-hooks.jsonl", "qoder-work-hook-session"),
        expected_source("qoder-work", "trace-jsonl", "SessionJsonl", ".qoderwork/data/session-events.jsonl", "qoder-work-session"),
        expected_source("qoder-work", "local-db", "Sqlite", ".qoderwork/messages.db", "qoder-work-db-session"),
    ],
    "qoder-work-cn": [
        expected_source("qoder-work-cn", "hook-jsonl", "HookJsonl", ".qoderwork/hooks/qoder-work-cn-hooks.jsonl", "qoder-work-cn-hook-session"),
        expected_source("qoder-work-cn", "trace-jsonl", "SessionJsonl", ".qoderwork/data/session-events.jsonl", "qoder-work-cn-session"),
        expected_source("qoder-work-cn", "local-db", "Sqlite", ".qoderwork/messages.db", "qoder-work-cn-db-session"),
    ],
    "wukong": [
        expected_source("wukong", "sqlite", "Sqlite", ".wukong/data/wukong.db", "wk-session"),
    ],
    "hermes": [
        expected_source("hermes", "sqlite", "Sqlite", ".hermes/state.db", "hermes-session"),
    ],
    "openclaw": [
        expected_source("openclaw", "session-jsonl", "SessionJsonl", ".openclaw/agents/main/sessions/session-openclaw.jsonl", "openclaw-session"),
    ],
    "gemini": [
        expected_source("gemini", "telemetry-log", "TelemetryLog", ".gemini/telemetry.log", "gemini-prompt"),
        expected_source("gemini", "tmp-chats-jsonl", "SessionJsonl", ".gemini/tmp/project-one-hash/chats/session-2026-06-16-gemini.jsonl", "gemini-session"),
    ],
    "copilot": [
        expected_source("copilot", "otel-jsonl", "IdeSnapshot", ".copilot/otel/events.jsonl", "copilot-session"),
        expected_source("copilot", "official-copilot-runtime-jsonl", "SessionJsonl", ".copilot/session-state/copilot-official-session/events.jsonl", "copilot-official-session"),
        expected_source("copilot", "session-state-jsonl", "SessionJsonl", ".copilot/session-state/copilot-session/events.jsonl", "copilot-session"),
        expected_source("copilot", "session-store-db", "Sqlite", ".copilot/session-store.db", "copilot-session"),
        expected_source("copilot", "vscode-chat-state", "Sqlite", "Library/Application Support/Code/User/workspaceStorage/copilot-workspace/state.vscdb", "copilot-session"),
    ],
    "cline": [
        expected_source(
            "cline",
            "vscode-tasks",
            "SessionJsonl",
            "saoudrizwan.claude-dev/tasks/task-1/api_conversation_history.json",
            "task-1",
        ),
        expected_source(
            "cline",
            "vscode-ui-messages",
            "SessionJsonl",
            "saoudrizwan.claude-dev/tasks/task-1/ui_messages.json",
            "task-1",
        ),
        expected_source("cline", "sessions-db", "Sqlite", ".cline/data/db/sessions.db", "cline-current"),
    ],
    "roo-code": [
        expected_source("roo-code", "vscode-tasks", "SessionJsonl", "rooveterinaryinc.roo-cline/tasks/task-1/api_conversation_history.json", "task-1"),
        expected_source("roo-code", "vscode-ui-messages", "SessionJsonl", "rooveterinaryinc.roo-cline/tasks/task-1/ui_messages.json", "task-1"),
    ],
    "kiro": [
        expected_source("kiro", "hook-jsonl", "HookJsonl", ".kiro/hooks/kiro-hooks.jsonl", "kiro-hook-session"),
        expected_source("kiro", "data-sqlite", "Sqlite", "kiro-cli/data.sqlite3", "kiro-conversation"),
        expected_source(
            "kiro",
            "cli-session-json",
            "SessionJsonl",
            ".kiro/sessions/cli/kiro-session.json",
            "kiro-cli-session",
        ),
    ],
    "zed": [
        expected_source("zed", "threads-db", "Sqlite", ".local/share/zed/threads/threads.db", "zed-thread"),
    ],
    "goose": [
        expected_source("goose", "sessions-db", "Sqlite", ".local/share/goose/sessions/sessions.db", "goose-session"),
    ],
    "amp": [
        expected_source("amp", "threads-jsonl", "SessionJsonl", ".amp/sessions/session-amp.jsonl", "amp-session"),
    ],
    "droid": [
        expected_source("droid", "session-jsonl", "SessionJsonl", ".factory/sessions/session-droid.jsonl", "droid-session"),
        expected_source("droid", "settings-json", "SessionJsonl", ".factory/sessions/session-droid.settings.json", "droid-session"),
    ],
    "pi": [
        expected_source("pi", "session-jsonl", "SessionJsonl", ".pi/agent/sessions/session-pi.jsonl", "pi-session"),
    ],
    "mux": [
        expected_source(
            "mux",
            "chat-jsonl",
            "SessionJsonl",
            ".mux/sessions/workspace-one/chat.jsonl",
            "mux-session",
        ),
        expected_source(
            "mux",
            "session-usage-json",
            "SessionJsonl",
            ".mux/sessions/workspace-one/session-usage.json",
            "workspace-one",
        ),
    ],
    "crush": [
        expected_source("crush", "sqlite", "Sqlite", ".crush/crush.db", "crush-session"),
    ],
    "codebuff": [
        expected_source("codebuff", "project-jsonl", "SessionJsonl", ".config/manicode/projects/project-one/chats/2026-06-16T14-00-00.000Z/chat-messages.json", "codebuff-session"),
    ],
    "kilo": [
        expected_source("kilo", "sqlite", "Sqlite", ".local/share/kilo/kilo.db", "kilo-session"),
        expected_source("kilo", "storage-json", "SessionJsonl", ".local/share/kilo/storage/session/kilo-project/kilo-storage-session.json", "kilo-storage-session"),
    ],
    "kilocode": [
        expected_source("kilocode", "sqlite", "Sqlite", ".local/share/kilo/kilo.db", "kilocode-session"),
        expected_source("kilocode", "storage-json", "SessionJsonl", ".local/share/kilo/storage/session/kilocode-project/kilocode-storage-session.json", "kilocode-storage-session"),
        expected_source("kilocode", "vscode-tasks", "SessionJsonl", "kilocode.kilo-code/tasks/task-1/api_conversation_history.json", "task-1"),
        expected_source("kilocode", "vscode-ui-messages", "SessionJsonl", "kilocode.kilo-code/tasks/task-1/ui_messages.json", "task-1"),
    ],
    "kimi": [
        expected_source("kimi", "wire-jsonl", "SessionJsonl", ".kimi-code/sessions/project/kimi-code-session/agents/main/wire.jsonl", "kimi-code-session"),
    ],
    "gjc": [
        expected_source("gjc", "session-jsonl", "SessionJsonl", ".gjc/agent/sessions/tmp-project/20260616T140000_gjc-session.jsonl", "gjc-session"),
    ],
    "grok": [
        expected_source("grok", "sessions-jsonl", "SessionJsonl", ".grok/sessions/workspace/grok-session/events.jsonl", "grok-session"),
    ],
    "synthetic": [
        expected_source("synthetic", "sqlite", "Sqlite", ".local/share/octofriend/sqlite.db", "synthetic-tree"),
    ],
    "warp": [
        expected_source("warp", "warp-sqlite", "Sqlite", ".warp/warp.sqlite", "warp-conversation"),
    ],
    "antigravity": [
        expected_source("antigravity", "conversation-sqlite", "Sqlite", ".gemini/antigravity-cli/conversations/session-antigravity.db", "session-antigravity"),
    ],
    "zcode": [
        expected_source("zcode", "projects-jsonl", "SessionJsonl", ".zcode/projects/project-zcode/session-zcode.jsonl", "zcode-session"),
    ],
}


def expected_sources_for_agent(agent: str) -> list[dict[str, Any]]:
    if agent not in EXPECTED_SOURCES:
        raise SystemExit(f"ERROR: no expected local source entries for {agent}")
    return EXPECTED_SOURCES[agent]


def write_jsonl(path: Path, rows: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "\n".join(json.dumps(row, separators=(",", ":")) for row in rows) + "\n",
        encoding="utf-8",
    )


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, separators=(",", ":")), encoding="utf-8")


def open_fixture_db(path: Path) -> sqlite3.Connection:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        path.unlink()
    return sqlite3.connect(path)


def write_local_hook_journal(
    root: Path,
    agent: str,
    session_id: str,
    model: str,
    provider: str,
) -> Path:
    journal_dir = root / "local-sources" / agent
    write_json(journal_dir / "aitrack-sources.json", {"files": ["hook-events.jsonl"]})
    path = journal_dir / "hook-events.jsonl"
    base = {
        "session_id": session_id,
        "model": model,
        "provider": provider,
        "account": "local",
        "workspace_roots": ["/tmp/project"],
    }
    write_jsonl(
        path,
        [
            {
                **base,
                "hook_event_name": "UserPromptSubmit",
                "timestamp": ISO_TS,
                "prompt": f"{agent} hook prompt",
                "usage": {
                    "input_tokens": 42,
                    "output_tokens": 17,
                    "cache_read_tokens": 5,
                    "cache_write_tokens": 2,
                    "reasoning_tokens": 3,
                    "cost": 0.011,
                },
            },
            {
                **base,
                "hook_event_name": "PreToolUse",
                "timestamp": ISO_TS_1,
                "tool_name": "read_file",
                "tool_input": {"path": "src/lib.rs"},
            },
            {
                **base,
                "hook_event_name": "PreToolUse",
                "timestamp": ISO_TS_1,
                "tool_name": "apply_patch",
                "tool_input": {
                    "path": "src/lib.rs",
                    "old": "old\n",
                    "new": "new\n",
                    "diff": "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@\n-old\n+new\n",
                },
            },
            {
                **base,
                "hook_event_name": "PostToolUse",
                "timestamp": ISO_TS_1,
                "tool_name": "read_file",
                "tool_response": "file content",
            },
            {
                **base,
                "hook_event_name": "afterAgentResponse",
                "timestamp": ISO_TS_1,
                "assistant_output": f"{agent} hook output",
            },
            {
                **base,
                "hook_event_name": "afterFileEdit",
                "timestamp": ISO_TS_1,
                "file_path": "src/lib.rs",
                "old_string": "old\n",
                "new_string": "new\n",
                "diff": "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@\n-old\n+new\n",
            },
        ],
    )
    return path


def write_kilo_sqlite_fixture(
    root: Path,
    agent: str,
    project_path: str,
    model: str,
    provider: str,
) -> Path:
    path = root / ".local/share/kilo/kilo.db"
    conn = open_fixture_db(path)
    try:
        conn.executescript(
            "CREATE TABLE project (id TEXT PRIMARY KEY, worktree TEXT NOT NULL, vcs TEXT, "
            "name TEXT, time_created INTEGER, time_updated INTEGER, time_initialized INTEGER, "
            "sandboxes TEXT NOT NULL);"
            "CREATE TABLE session (id TEXT PRIMARY KEY, project_id TEXT NOT NULL, workspace_id TEXT, "
            "parent_id TEXT, slug TEXT NOT NULL, directory TEXT NOT NULL, path TEXT, title TEXT NOT NULL, "
            "version TEXT NOT NULL, share_url TEXT, summary_additions INTEGER, summary_deletions INTEGER, "
            "summary_files INTEGER, summary_diffs TEXT, metadata TEXT, cost REAL NOT NULL DEFAULT 0, "
            "tokens_input INTEGER NOT NULL DEFAULT 0, tokens_output INTEGER NOT NULL DEFAULT 0, "
            "tokens_reasoning INTEGER NOT NULL DEFAULT 0, tokens_cache_read INTEGER NOT NULL DEFAULT 0, "
            "tokens_cache_write INTEGER NOT NULL DEFAULT 0, revert TEXT, permission TEXT, agent TEXT, "
            "model TEXT, time_created INTEGER, time_updated INTEGER, time_compacting INTEGER, "
            "time_archived INTEGER);"
            "CREATE TABLE message (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, "
            "time_created INTEGER, time_updated INTEGER, data TEXT NOT NULL);"
            "CREATE TABLE part (id TEXT PRIMARY KEY, message_id TEXT NOT NULL, session_id TEXT NOT NULL, "
            "time_created INTEGER, time_updated INTEGER, data TEXT NOT NULL);"
            "CREATE TABLE todo (session_id TEXT NOT NULL, content TEXT NOT NULL, status TEXT NOT NULL, "
            "priority TEXT NOT NULL, position INTEGER NOT NULL, time_created INTEGER, time_updated INTEGER);"
            "CREATE TABLE session_message (id TEXT PRIMARY KEY, session_id TEXT NOT NULL, type TEXT NOT NULL, "
            "time_created INTEGER, time_updated INTEGER, data TEXT NOT NULL);"
        )
        conn.execute(
            "INSERT INTO project VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            (
                f"{agent}-project",
                project_path,
                "git",
                f"{agent} Project",
                EPOCH_MS,
                EPOCH_MS_1,
                EPOCH_MS,
                "[]",
            ),
        )
        conn.execute(
            "INSERT INTO session VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (
                f"{agent}-session",
                f"{agent}-project",
                None,
                None,
                f"{agent}-session",
                project_path,
                project_path,
                f"{agent} Session",
                "1.0.0",
                None,
                1,
                1,
                1,
                None,
                json.dumps({"account": "local"}, separators=(",", ":")),
                0.01,
                100,
                40,
                4,
                8,
                2,
                None,
                None,
                agent,
                json.dumps({"id": model, "providerID": provider}, separators=(",", ":")),
                EPOCH_MS,
                EPOCH_MS_1,
                None,
                None,
            ),
        )
        conn.execute(
            "INSERT INTO message VALUES (?, ?, ?, ?, ?)",
            (
                f"{agent}-user",
                f"{agent}-session",
                EPOCH_MS,
                EPOCH_MS,
                json.dumps(
                    {
                        "id": f"{agent}-user",
                        "sessionID": f"{agent}-session",
                        "role": "user",
                        "time": {"created": EPOCH_MS},
                    },
                    separators=(",", ":"),
                ),
            ),
        )
        conn.execute(
            "INSERT INTO message VALUES (?, ?, ?, ?, ?)",
            (
                f"{agent}-msg",
                f"{agent}-session",
                EPOCH_MS_1,
                EPOCH_MS_1,
                json.dumps(
                    {
                        "id": f"{agent}-msg",
                        "sessionID": f"{agent}-session",
                        "role": "assistant",
                        "time": {"created": EPOCH_MS_1},
                    },
                    separators=(",", ":"),
                ),
            ),
        )
        conn.execute(
            "INSERT INTO part VALUES (?, ?, ?, ?, ?, ?)",
            (
                f"{agent}-user-text",
                f"{agent}-user",
                f"{agent}-session",
                EPOCH_MS,
                EPOCH_MS,
                json.dumps(
                    {
                        "id": f"{agent}-user-text",
                        "messageID": f"{agent}-user",
                        "sessionID": f"{agent}-session",
                        "type": "text",
                        "text": f"{agent} prompt",
                    },
                    separators=(",", ":"),
                ),
            ),
        )
        conn.execute(
            "INSERT INTO part VALUES (?, ?, ?, ?, ?, ?)",
            (
                f"{agent}-assistant-text",
                f"{agent}-msg",
                f"{agent}-session",
                EPOCH_MS_1,
                EPOCH_MS_1,
                json.dumps(
                    {
                        "id": f"{agent}-assistant-text",
                        "messageID": f"{agent}-msg",
                        "sessionID": f"{agent}-session",
                        "type": "text",
                        "text": f"{agent} output",
                    },
                    separators=(",", ":"),
                ),
            ),
        )
        conn.execute(
            "INSERT INTO part VALUES (?, ?, ?, ?, ?, ?)",
            (
                f"{agent}-tool",
                f"{agent}-msg",
                f"{agent}-session",
                EPOCH_MS_1,
                EPOCH_MS_1,
                json.dumps(
                    {
                        "id": f"{agent}-tool",
                        "messageID": f"{agent}-msg",
                        "sessionID": f"{agent}-session",
                        "type": "tool",
                        "tool": "read_file",
                        "callID": "tool-1",
                        "state": {"input": {"path": "src/lib.rs"}, "output": "file content"},
                    },
                    separators=(",", ":"),
                ),
            ),
        )
        conn.execute(
            "INSERT INTO part VALUES (?, ?, ?, ?, ?, ?)",
            (
                f"{agent}-patch",
                f"{agent}-msg",
                f"{agent}-session",
                EPOCH_MS_1,
                EPOCH_MS_1,
                json.dumps(
                    {
                        "id": f"{agent}-patch",
                        "messageID": f"{agent}-msg",
                        "sessionID": f"{agent}-session",
                        "type": "patch",
                        "hash": "patch-hash",
                        "files": ["src/lib.rs"],
                        "path": "src/lib.rs",
                        "old": "old\n",
                        "new": "new\n",
                    },
                    separators=(",", ":"),
                ),
            ),
        )
        conn.execute(
            "INSERT INTO part VALUES (?, ?, ?, ?, ?, ?)",
            (
                f"{agent}-step-finish",
                f"{agent}-msg",
                f"{agent}-session",
                EPOCH_MS_1,
                EPOCH_MS_1,
                json.dumps(
                    {
                        "id": f"{agent}-step-finish",
                        "messageID": f"{agent}-msg",
                        "sessionID": f"{agent}-session",
                        "type": "step-finish",
                        "reason": "done",
                        "cost": 0.01,
                        "tokens": {
                            "input": 100,
                            "output": 40,
                            "reasoning": 4,
                            "cache": {"read": 8, "write": 2},
                        },
                    },
                    separators=(",", ":"),
                ),
            ),
        )
        conn.execute(
            "INSERT INTO session_message VALUES (?, ?, ?, ?, ?, ?)",
            (
                f"{agent}-next-prompted",
                f"{agent}-session",
                "session.next.prompted",
                EPOCH_MS,
                EPOCH_MS,
                json.dumps(
                    {
                        "sessionID": f"{agent}-session",
                        "prompt": f"{agent} v2 prompt",
                        "timestamp": ISO_TS,
                    },
                    separators=(",", ":"),
                ),
            ),
        )
        conn.execute(
            "INSERT INTO session_message VALUES (?, ?, ?, ?, ?, ?)",
            (
                f"{agent}-next-tool-success",
                f"{agent}-session",
                "session.next.tool.success",
                EPOCH_MS_1,
                EPOCH_MS_1,
                json.dumps(
                    {
                        "sessionID": f"{agent}-session",
                        "tool_name": "read_file",
                        "tool_call_id": "tool-1",
                        "tool_result": "file content",
                        "timestamp": ISO_TS_1,
                    },
                    separators=(",", ":"),
                ),
            ),
        )
        conn.execute(
            "INSERT INTO todo VALUES (?, ?, ?, ?, ?, ?, ?)",
            (
                f"{agent}-session",
                "inspect local capture",
                "completed",
                "medium",
                1,
                EPOCH_MS,
                EPOCH_MS_1,
            ),
        )
        conn.commit()
    finally:
        conn.close()
    return path


def write_kilo_storage_json_fixture(root: Path, agent: str, model: str, provider: str) -> Path:
    session_id = f"{agent}-storage-session"
    storage = root / ".local/share/kilo/storage"
    session_path = storage / "session" / f"{agent}-project" / f"{session_id}.json"
    write_json(
        session_path,
        {
            "id": session_id,
            "projectID": f"{agent}-project",
            "directory": "/tmp/project",
            "path": "/tmp/project",
            "account": "local",
            "model": {"id": model, "providerID": provider},
            "cost": 0.02,
            "tokens_input": 110,
            "tokens_output": 44,
            "tokens_reasoning": 5,
            "tokens_cache_read": 9,
            "tokens_cache_write": 3,
            "time_created": EPOCH_MS,
            "time_updated": EPOCH_MS_1,
        },
    )
    message_dir = storage / "message" / session_id
    write_json(
        message_dir / f"{agent}-storage-user.json",
        {
            "id": f"{agent}-storage-user",
            "sessionID": session_id,
            "role": "user",
            "time": {"created": EPOCH_MS},
        },
    )
    write_json(
        message_dir / f"{agent}-storage-assistant.json",
        {
            "id": f"{agent}-storage-assistant",
            "sessionID": session_id,
            "role": "assistant",
            "time": {"created": EPOCH_MS_1},
        },
    )
    part_root = storage / "part"
    for part_id, message_id, payload in [
        (
            f"{agent}-storage-user-text",
            f"{agent}-storage-user",
            {
                "type": "text",
                "text": f"{agent} storage prompt",
            },
        ),
        (
            f"{agent}-storage-output-text",
            f"{agent}-storage-assistant",
            {
                "type": "text",
                "text": f"{agent} storage output",
            },
        ),
        (
            f"{agent}-storage-tool",
            f"{agent}-storage-assistant",
            {
                "type": "tool",
                "tool": "read_file",
                "callID": "tool-1",
                "state": {"input": {"path": "src/lib.rs"}, "output": "file content"},
            },
        ),
        (
            f"{agent}-storage-step-finish",
            f"{agent}-storage-assistant",
            {
                "type": "step-finish",
                "cost": 0.01,
                "tokens": {
                    "input": 999,
                    "output": 999,
                    "reasoning": 99,
                    "cache": {"read": 99, "write": 99},
                },
            },
        ),
    ]:
        payload.update({"id": part_id, "messageID": message_id, "sessionID": session_id})
        write_json(part_root / message_id / f"{part_id}.json", payload)
    write_json(
        storage / "session_diff" / f"{session_id}.json",
        [
            {
                "path": "src/lib.rs",
                "diff": "@@ -1 +1 @@\n-old\n+new\n",
                "old": "old\n",
                "new": "new\n",
            }
        ],
    )
    return session_path


def pb_varint(value: int) -> bytes:
    out = bytearray()
    while value >= 0x80:
        out.append((value & 0x7F) | 0x80)
        value >>= 7
    out.append(value)
    return bytes(out)


def pb_key(field_number: int, wire_type: int) -> bytes:
    return pb_varint((field_number << 3) | wire_type)


def pb_varint_field(field_number: int, value: int) -> bytes:
    return pb_key(field_number, 0) + pb_varint(value)


def pb_bytes_field(field_number: int, value: bytes) -> bytes:
    return pb_key(field_number, 2) + pb_varint(len(value)) + value


def pb_string_field(field_number: int, value: str) -> bytes:
    return pb_bytes_field(field_number, value.encode("utf-8"))


def pb_timestamp_ms_field(field_number: int, timestamp_ms: int) -> bytes:
    seconds, millis = divmod(timestamp_ms, 1000)
    timestamp = pb_varint_field(1, seconds) + pb_varint_field(2, millis * 1_000_000)
    return pb_bytes_field(field_number, timestamp)


def warp_message(message_id: str, task_id: str, timestamp_ms: int, oneof_field: int, payload: bytes) -> bytes:
    return (
        pb_string_field(1, message_id)
        + pb_string_field(11, task_id)
        + pb_timestamp_ms_field(14, timestamp_ms)
        + pb_bytes_field(oneof_field, payload)
    )


def warp_task_proto() -> bytes:
    task_id = "warp-task"
    task = pb_string_field(1, task_id) + pb_string_field(2, "warp task")
    directory = pb_string_field(1, "/tmp/project")
    context = pb_bytes_field(1, directory)
    user_query = pb_string_field(1, "warp prompt") + pb_bytes_field(2, context)
    task += pb_bytes_field(5, warp_message("warp-user", task_id, EPOCH_MS, 2, user_query))
    task += pb_bytes_field(
        5,
        warp_message("warp-model", task_id, EPOCH_MS, 25, pb_string_field(1, "claude-sonnet-4")),
    )
    task += pb_bytes_field(
        5,
        warp_message("warp-output", task_id, EPOCH_MS + 1000, 3, pb_string_field(1, "warp output")),
    )
    read_file = pb_string_field(1, "src/lib.rs")
    read_files = pb_bytes_field(1, read_file)
    read_call = pb_string_field(1, "tool-1") + pb_bytes_field(5, read_files)
    task += pb_bytes_field(5, warp_message("warp-read-call", task_id, EPOCH_MS + 1000, 4, read_call))
    diff = (
        pb_string_field(1, "src/lib.rs")
        + pb_string_field(2, "old\n")
        + pb_string_field(3, "new\n")
    )
    apply_call = pb_string_field(1, "tool-edit") + pb_bytes_field(6, pb_bytes_field(2, diff))
    task += pb_bytes_field(5, warp_message("warp-apply-call", task_id, EPOCH_MS + 1000, 4, apply_call))
    file_content = pb_string_field(1, "src/lib.rs") + pb_string_field(2, "file content")
    read_result_payload = pb_bytes_field(1, pb_bytes_field(1, file_content))
    read_result = pb_string_field(1, "tool-1") + pb_bytes_field(5, read_result_payload)
    task += pb_bytes_field(5, warp_message("warp-read-result", task_id, EPOCH_MS + 1000, 5, read_result))
    apply_success = pb_bytes_field(2, pb_bytes_field(1, file_content))
    apply_result_payload = pb_bytes_field(1, apply_success)
    apply_result = pb_string_field(1, "tool-edit") + pb_bytes_field(6, apply_result_payload)
    task += pb_bytes_field(5, warp_message("warp-apply-result", task_id, EPOCH_MS + 1000, 5, apply_result))
    return task


def antigravity_gen_metadata_proto(prompt_text: str, output_text: str) -> bytes:
    generation = pb_timestamp_ms_field(4, EPOCH_MS)
    prompt = pb_string_field(3, f"<USER_REQUEST>{prompt_text}")
    output = pb_string_field(3, output_text)
    usage = (
        pb_varint_field(1, 80)
        + pb_varint_field(2, 20)
        + pb_varint_field(5, 10)
        + pb_varint_field(9, 40)
        + pb_varint_field(10, 6)
        + pb_string_field(11, "antigravity-response")
    )
    chat_model = (
        pb_string_field(19, "gemini-3-pro")
        + pb_bytes_field(4, usage)
        + pb_bytes_field(9, generation)
        + pb_bytes_field(2, prompt)
        + pb_bytes_field(2, output)
    )
    return pb_bytes_field(1, chat_model)


def antigravity_trajectory_proto() -> bytes:
    folder = pb_string_field(1, "file:///tmp/project")
    return pb_bytes_field(1, folder) + pb_timestamp_ms_field(2, EPOCH_MS)


def antigravity_text_step_proto(text: str) -> bytes:
    return pb_bytes_field(20, pb_string_field(1, text))


def antigravity_tool_step_proto(name: str, payload: dict[str, Any]) -> bytes:
    call = pb_string_field(2, name) + pb_string_field(
        3, json.dumps(payload, separators=(",", ":"))
    )
    return pb_bytes_field(5, pb_bytes_field(4, call))


def write_vscode_task_fixture(root: Path, extension: str, agent: str, model: str, provider: str) -> Path:
    task_dir = root / ".config/Code/User/globalStorage" / extension / "tasks/task-1"
    task_dir.mkdir(parents=True, exist_ok=True)
    write_json(
        task_dir / "task_metadata.json",
        {"model": model, "provider": provider, "agent": agent},
    )
    write_json(
        task_dir / "api_conversation_history.json",
        [
            {"role": "user", "content": f"{agent} prompt"},
            {
                "role": "assistant",
                "content": [
                    {"type": "text", "text": f"{agent} output"},
                    {
                        "type": "tool_use",
                        "id": "tool-1",
                        "name": "read_file",
                        "input": {"path": "src/lib.rs"},
                    },
                    {
                        "type": "tool_use",
                        "id": "tool-edit",
                        "name": "apply_patch",
                        "input": {"path": "src/lib.rs", "old": "old\n", "new": "new\n"},
                    },
                ],
            },
            {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "tool-1",
                        "content": "file content",
                    }
                ],
            },
        ],
    )
    path = task_dir / "ui_messages.json"
    write_json(
        path,
        [
            {"type": "ask", "ask": "message", "ts": ISO_TS, "prompt": f"{agent} prompt"},
            {
                "type": "say",
                "say": "text",
                "ts": ISO_TS_1,
                "assistant_output": f"{agent} output",
            },
            {
                "type": "say",
                "say": "tool",
                "ts": ISO_TS_1,
                "tool_name": "read_file",
                "tool_arguments": {"path": "src/lib.rs"},
            },
            {
                "type": "say",
                "say": "tool_result",
                "ts": ISO_TS_1,
                "tool_result": "file content",
            },
            {
                "type": "say",
                "say": "api_req_started",
                "ts": ISO_TS,
                "text": json.dumps(
                    {
                        "cost": 0.05,
                        "tokensIn": 40,
                        "tokensOut": 15,
                        "cacheReads": 7,
                        "cacheWrites": 3,
                        "apiProtocol": provider,
                    },
                    separators=(",", ":"),
                ),
            },
        ],
    )
    return path


def write_cline_current_fixture(root: Path) -> Path:
    db_path = root / ".cline/data/db/sessions.db"
    conn = open_fixture_db(db_path)
    try:
        conn.execute(
            """
            CREATE TABLE sessions (
                session_id TEXT PRIMARY KEY,
                source TEXT,
                pid INTEGER,
                started_at TEXT,
                ended_at TEXT,
                exit_code INTEGER,
                status TEXT,
                interactive INTEGER,
                provider TEXT,
                model TEXT,
                cwd TEXT,
                workspace_root TEXT,
                flags TEXT,
                parent_session_id TEXT,
                agent_id TEXT,
                conversation_id TEXT,
                prompt TEXT,
                metadata_json TEXT,
                transcript_path TEXT,
                hook_path TEXT,
                messages_path TEXT,
                updated_at TEXT
            )
            """
        )
        conn.execute(
            "INSERT INTO sessions VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (
                "cline-current",
                "cline",
                4242,
                ISO_TS,
                None,
                None,
                "running",
                1,
                "anthropic",
                "claude-sonnet-4",
                "/tmp/project",
                "/tmp/project",
                json.dumps({"mode": "agent"}, separators=(",", ":")),
                None,
                "cline",
                "conversation-cline",
                "cline prompt",
                json.dumps({"source": "sessions-db"}, separators=(",", ":")),
                str(root / ".cline/data/sessions/cline-current/transcript.jsonl"),
                str(root / ".cline/data/sessions/cline-current/hook.jsonl"),
                str(root / ".cline/data/sessions/cline-current/cline-current.messages.json"),
                ISO_TS_1,
            ),
        )
        conn.commit()
    finally:
        conn.close()

    path = root / ".cline/data/sessions/cline-current/cline-current.messages.json"
    write_json(
        path,
        [
            {"role": "user", "timestamp": ISO_TS, "content": "cline prompt"},
            {
                "role": "assistant",
                "timestamp": ISO_TS_1,
                "content": "cline output",
                "metadata": {
                    "usage": {
                        "inputTokens": 70,
                        "outputTokens": 25,
                        "cacheReads": 3,
                        "reasoningTokens": 4,
                        "cost": 0.07,
                    }
                },
                "tool_calls": [
                    {"id": "tool-1", "name": "read_file", "arguments": {"path": "src/lib.rs"}},
                    {"id": "tool-edit", "name": "apply_patch", "arguments": {"path": "src/lib.rs", "old": "old\n", "new": "new\n"}},
                ],
            },
            {
                "type": "tool_result",
                "timestamp": ISO_TS_1,
                "tool_call_id": "tool-1",
                "content": "file content",
            },
        ],
    )
    return path


def write_agent_fixture(root: Path, agent: str) -> Path:
    if agent == "claude":
        write_local_hook_journal(
            root,
            "claude",
            "claude-hook-session",
            "claude-sonnet-4",
            "anthropic",
        )
        path = root / ".claude/projects/e2e/session-claude.jsonl"
        write_jsonl(
            path,
            [
                {
                    "type": "user",
                    "timestamp": ISO_TS,
                    "sessionId": "claude-session",
                    "message": {"role": "user", "content": "claude prompt"},
                },
                {
                    "type": "assistant",
                    "timestamp": ISO_TS_1,
                    "sessionId": "claude-session",
                    "message": {
                        "role": "assistant",
                        "model": "claude-sonnet-4",
                        "content": [
                            {"type": "text", "text": "claude output"},
                            {"type": "thinking", "thinking": "claude reasoning"},
                            {"type": "redacted_thinking", "data": "redacted-reasoning"},
                            {
                                "type": "tool_use",
                                "id": "tool-1",
                                "name": "read_file",
                                "input": {"path": "src/lib.rs"},
                            },
                            {
                                "type": "tool_use",
                                "id": "tool-edit",
                                "name": "apply_patch",
                                "input": {"path": "src/lib.rs", "old": "old\n", "new": "new\n"},
                            },
                        ],
                        "usage": {
                            "input_tokens": 100,
                            "output_tokens": 45,
                            "cache_read_input_tokens": 10,
                            "cache_creation_input_tokens": 5,
                        },
                    },
                },
                {
                    "type": "user",
                    "timestamp": ISO_TS_1,
                    "sessionId": "claude-session",
                    "message": {
                        "role": "user",
                        "content": [
                            {
                                "type": "tool_result",
                                "tool_use_id": "tool-1",
                                "content": "file content",
                            }
                        ],
                    },
                },
            ],
        )
        return path

    if agent == "codex":
        write_local_hook_journal(
            root,
            "codex",
            "codex-hook-session",
            "gpt-5",
            "openai",
        )
        path = root / ".codex/sessions/2026/06/16/rollout-e2e-codex.jsonl"
        write_jsonl(
            path,
            [
                {
                    "type": "session_meta",
                    "timestamp": ISO_TS,
                    "payload": {
                        "id": "codex-session",
                        "model": "gpt-5",
                        "model_provider": "openai",
                        "cwd": str(root),
                    },
                },
                {
                    "type": "event_msg",
                    "timestamp": ISO_TS_1,
                    "payload": {
                        "type": "task_started",
                        "model_context_window": 258400,
                    },
                },
                {
                    "type": "event_msg",
                    "timestamp": ISO_TS_1,
                    "payload": {
                        "type": "token_count",
                        "model": "gpt-5",
                        "info": {
                            "model_context_window": 258400,
                            "last_token_usage": {
                                "input_tokens": 120,
                                "output_tokens": 55,
                                "cached_input_tokens": 12,
                                "reasoning_output_tokens": 7,
                            },
                            "total_token_usage": {
                                "input_tokens": 320,
                                "output_tokens": 155,
                                "cached_input_tokens": 42,
                                "reasoning_output_tokens": 17,
                            },
                            "rate_limits": {
                                "primary": {"used_percent": 30, "window_minutes": 300},
                                "secondary": {"used_percent": 9, "window_minutes": 10080},
                            },
                        },
                    },
                },
                {
                    "type": "turn_context",
                    "timestamp": ISO_TS,
                    "payload": {"cwd": str(root)},
                },
                {
                    "type": "response_item",
                    "timestamp": ISO_TS,
                    "payload": {
                        "type": "message",
                        "role": "user",
                        "content": [{"type": "input_text", "text": "codex prompt"}],
                    },
                },
                {
                    "type": "response_item",
                    "timestamp": ISO_TS_1,
                    "payload": {
                        "type": "message",
                        "role": "assistant",
                        "content": [{"type": "output_text", "text": "codex output"}],
                    },
                },
                {
                    "type": "response_item",
                    "timestamp": ISO_TS_1,
                    "payload": {
                        "type": "function_call",
                        "call_id": "tool-1",
                        "name": "read_file",
                        "arguments": "{\"path\":\"src/lib.rs\"}",
                    },
                },
                {
                    "type": "response_item",
                    "timestamp": ISO_TS_1,
                    "payload": {
                        "type": "function_call",
                        "call_id": "tool-edit",
                        "name": "apply_patch",
                        "arguments": "{\"path\":\"src/lib.rs\",\"old\":\"old\\n\",\"new\":\"new\\n\"}",
                    },
                },
                {
                    "type": "response_item",
                    "timestamp": ISO_TS_1,
                    "payload": {
                        "type": "custom_tool_call",
                        "call_id": "tool-custom",
                        "name": "read_file",
                        "input": {"path": "src/custom.rs"},
                    },
                },
                {
                    "type": "response_item",
                    "timestamp": ISO_TS_1,
                    "payload": {
                        "type": "custom_tool_call",
                        "status": "completed",
                        "call_id": "tool-custom-edit",
                        "name": "apply_patch",
                        "input": "*** Begin Patch\n*** Update File: src/lib.rs\n@@\n-old\n+new\n*** End Patch\n",
                    },
                },
                {
                    "type": "response_item",
                    "timestamp": ISO_TS_1,
                    "payload": {
                        "type": "custom_tool_call_output",
                        "call_id": "tool-custom",
                        "output": "custom file content",
                    },
                },
                {
                    "type": "response_item",
                    "timestamp": ISO_TS_1,
                    "payload": {
                        "type": "web_search_call",
                        "id": "web-1",
                        "status": "completed",
                        "query": "release notes",
                    },
                },
                {
                    "type": "response_item",
                    "timestamp": ISO_TS_1,
                    "payload": {
                        "type": "tool_search_call",
                        "call_id": "search-1",
                        "query": "tool_search metadata",
                    },
                },
                {
                    "type": "response_item",
                    "timestamp": ISO_TS_1,
                    "payload": {
                        "type": "tool_search_output",
                        "call_id": "search-1",
                        "output": [{"name": "browser", "description": "tool search result"}],
                    },
                },
                {
                    "type": "response_item",
                    "timestamp": ISO_TS_1,
                    "payload": {
                        "type": "reasoning",
                        "id": "reasoning-1",
                        "summary": [{"type": "summary_text", "text": "reasoning summary"}],
                        "content": "reasoning body",
                    },
                },
                {
                    "type": "event_msg",
                    "timestamp": ISO_TS_1,
                    "payload": {
                        "type": "user_message",
                        "message": "event prompt",
                    },
                },
                {
                    "type": "event_msg",
                    "timestamp": ISO_TS_1,
                    "payload": {
                        "type": "agent_message",
                        "message": "event output",
                    },
                },
                {
                    "type": "event_msg",
                    "timestamp": ISO_TS_1,
                    "payload": {
                        "type": "agent_reasoning",
                        "message": "event reasoning",
                    },
                },
                {
                    "type": "event_msg",
                    "timestamp": ISO_TS_1,
                    "payload": {
                        "type": "patch_apply_end",
                        "call_id": "tool-custom-edit",
                        "success": True,
                        "status": "completed",
                        "changes": {
                            "src/lib.rs": {
                                "type": "update",
                                "unified_diff": "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@\n-old\n+new\n",
                                "move_path": None,
                            },
                        },
                    },
                },
                {
                    "type": "response_item",
                    "timestamp": ISO_TS_1,
                    "payload": {
                        "type": "function_call_output",
                        "call_id": "tool-1",
                        "output": "file content",
                    },
                },
            ],
        )
        return path

    if agent == "cursor":
        write_jsonl(
            root / ".cursor/hooks/cursor-hooks.jsonl",
            [
                {
                    "hook_event_name": "beforeSubmitPrompt",
                    "conversation_id": "cursor-hook-session",
                    "generation_id": "cursor-generation",
                    "model": "gpt-5",
                    "model_id": "gpt-5",
                    "workspace_roots": [str(root)],
                    "user_email": "dev@example.com",
                    "transcript_path": str(root / ".cursor/transcripts/cursor-hook-session.jsonl"),
                    "timestamp": ISO_TS,
                    "prompt": "cursor prompt",
                    "usage": {
                        "input_tokens": 80,
                        "output_tokens": 32,
                        "cache_read_tokens": 6,
                        "cache_write_tokens": 2,
                        "reasoning_tokens": 4,
                        "cost": 0.012,
                    },
                },
                {
                    "hook_event_name": "preToolUse",
                    "conversation_id": "cursor-hook-session",
                    "generation_id": "cursor-generation",
                    "model": "gpt-5",
                    "model_id": "gpt-5",
                    "workspace_roots": [str(root)],
                    "transcript_path": str(root / ".cursor/transcripts/cursor-hook-session.jsonl"),
                    "timestamp": ISO_TS_1,
                    "tool_name": "read_file",
                    "input": {"path": "src/lib.rs"},
                },
                {
                    "hook_event_name": "preToolUse",
                    "conversation_id": "cursor-hook-session",
                    "generation_id": "cursor-generation",
                    "model": "gpt-5",
                    "model_id": "gpt-5",
                    "workspace_roots": [str(root)],
                    "transcript_path": str(root / ".cursor/transcripts/cursor-hook-session.jsonl"),
                    "timestamp": ISO_TS_1,
                    "tool_name": "apply_patch",
                    "input": {"path": "src/lib.rs", "old": "old\n", "new": "new\n"},
                },
                {
                    "hook_event_name": "postToolUse",
                    "conversation_id": "cursor-hook-session",
                    "generation_id": "cursor-generation",
                    "model": "gpt-5",
                    "model_id": "gpt-5",
                    "workspace_roots": [str(root)],
                    "transcript_path": str(root / ".cursor/transcripts/cursor-hook-session.jsonl"),
                    "timestamp": ISO_TS_1,
                    "tool_name": "read_file",
                    "response": "file content",
                },
                {
                    "hook_event_name": "afterAgentResponse",
                    "conversation_id": "cursor-hook-session",
                    "generation_id": "cursor-generation",
                    "model": "gpt-5",
                    "model_id": "gpt-5",
                    "workspace_roots": [str(root)],
                    "transcript_path": str(root / ".cursor/transcripts/cursor-hook-session.jsonl"),
                    "timestamp": ISO_TS_1,
                    "response": "cursor output",
                },
                {
                    "hook_event_name": "afterFileEdit",
                    "conversation_id": "cursor-hook-session",
                    "generation_id": "cursor-generation",
                    "model": "gpt-5",
                    "model_id": "gpt-5",
                    "workspace_roots": [str(root)],
                    "transcript_path": str(root / ".cursor/transcripts/cursor-hook-session.jsonl"),
                    "timestamp": ISO_TS_1,
                    "file_path": "src/cursor-edit.rs",
                    "old_string": "old\n",
                    "new_string": "new\n",
                },
            ],
        )
        write_jsonl(
            root / ".cursor/projects/project-one/agent-transcripts/cursor-session.jsonl",
            [
                {
                    "type": "session",
                    "session_id": "cursor-agent-session",
                    "timestamp": ISO_TS,
                    "model": "gpt-5",
                    "provider": "cursor",
                    "workspace": str(root),
                },
                {
                    "type": "user",
                    "session_id": "cursor-agent-session",
                    "timestamp": ISO_TS,
                    "content": "cursor transcript prompt",
                },
                {
                    "type": "assistant",
                    "session_id": "cursor-agent-session",
                    "timestamp": ISO_TS_1,
                    "content": "cursor transcript output",
                    "usage": {
                        "input_tokens": 90,
                        "output_tokens": 36,
                        "cache_read_tokens": 7,
                        "cache_write_tokens": 2,
                        "reasoning_tokens": 5,
                        "cost": 0.013,
                    },
                    "account": "local",
                },
                {
                    "type": "tool_call",
                    "session_id": "cursor-agent-session",
                    "timestamp": ISO_TS_1,
                    "tool_name": "read_file",
                    "tool_call_id": "cursor-read",
                    "tool_arguments": {"path": "src/lib.rs"},
                },
                {
                    "type": "tool_call",
                    "session_id": "cursor-agent-session",
                    "timestamp": ISO_TS_1,
                    "tool_name": "apply_patch",
                    "tool_call_id": "cursor-edit",
                    "tool_arguments": {
                        "path": "src/cursor-agent.rs",
                        "old": "old\n",
                        "new": "new\n",
                    },
                },
                {
                    "type": "tool_result",
                    "session_id": "cursor-agent-session",
                    "timestamp": ISO_TS_1,
                    "tool_call_id": "cursor-read",
                    "tool_result": "cursor transcript file content",
                },
            ],
        )
        path = root / "Library/Application Support/Cursor/User/globalStorage/state.vscdb"
        conn = open_fixture_db(path)
        try:
            conn.execute("CREATE TABLE ItemTable (key TEXT, value TEXT)")
            conn.execute(
                "INSERT INTO ItemTable(key, value) VALUES (?, ?)",
                (
                    "cursorDiskKV.composerData",
                    json.dumps(
                        {
                            "session_id": "cursor-session",
                            "timestamp": ISO_TS,
                            "model": "gpt-5",
                            "provider": "cursor",
                            "usage_basis": usage_fixture_expected_usage_basis(agent, "state-vscdb"),
                            "usage": {
                                "input_tokens": 100,
                                "output_tokens": 40,
                                "cache_read_tokens": 8,
                                "cache_write_tokens": 3,
                                "reasoning_tokens": 6,
                                "cost": 0.016,
                            },
                            "account": "local",
                            "messages": [
                                {"type": "user", "content": "cursor prompt"},
                                {"type": "assistant", "content": "cursor output"},
                                {
                                    "type": "tool_call",
                                    "tool_name": "read_file",
                                    "tool_arguments": {"path": "src/lib.rs"},
                                },
                                {
                                    "type": "tool_call",
                                    "tool_name": "apply_patch",
                                    "tool_arguments": {"path": "src/lib.rs", "old": "old\n", "new": "new\n"},
                                },
                                {"type": "tool_result", "tool_result": "file content"},
                            ],
                        },
                        separators=(",", ":"),
                    ),
                ),
            )
            conn.commit()
        finally:
            conn.close()
        return path

    if agent == "trae":
        path = root / "trajectories/trajectory_20260616_140000.json"
        write_json(
            path,
            {
                "task": "update checkout flow",
                "task_id": "trae-session",
                "start_time": ISO_TS,
                "end_time": ISO_TS_1,
                "provider": "openai",
                "model": "gpt-5",
                "llm_interactions": [
                    {
                        "timestamp": ISO_TS,
                        "input_messages": [{"role": "user", "content": "trae prompt"}],
                        "response": {
                            "role": "assistant",
                            "content": "trae output",
                            "usage": {
                                "input_tokens": 100,
                                "output_tokens": 44,
                                "cache_read_input_tokens": 8,
                                "cache_creation_input_tokens": 4,
                                "reasoning_tokens": 3,
                            },
                            "tool_calls": [
                                {
                                    "id": "tool-1",
                                    "name": "read_file",
                                    "arguments": {"path": "src/main.rs"},
                                },
                                {
                                    "id": "tool-edit",
                                    "name": "apply_patch",
                                    "arguments": {"path": "src/main.rs", "old": "old\n", "new": "new\n"},
                                },
                            ],
                        },
                    }
                ],
                "agent_steps": [
                    {
                        "timestamp": ISO_TS_1,
                        "tool_calls": [
                        {
                            "id": "tool-1",
                            "name": "read_file",
                            "arguments": {"path": "src/main.rs"},
                        },
                        {
                            "id": "tool-edit",
                            "name": "apply_patch",
                            "arguments": {"path": "src/main.rs", "old": "old\n", "new": "new\n"},
                        }
                    ],
                        "tool_results": [
                            {"tool_call_id": "tool-1", "content": "file content"}
                        ],
                    }
                ],
                "final_result": "trae final output",
                "execution_time": 1.2,
            },
        )
        return path

    if agent == "qwen":
        path = root / ".qwen/telemetry.log"
        write_jsonl(
            path,
            [
                {
                    "name": "qwen-code.user_prompt",
                    "timestamp": ISO_TS,
                    "attributes": {
                        "prompt_id": "qwen-prompt",
                        "prompt_length": 11,
                        "prompt": "qwen prompt",
                        "auth_type": "oauth-personal",
                    },
                },
                {
                    "name": "qwen-code.tool_call",
                    "timestamp": ISO_TS_1,
                    "attributes": {
                        "prompt_id": "qwen-prompt",
                        "function_name": "read_file",
                        "function_args": "{\"path\":\"src/lib.rs\"}",
                        "duration_ms": 18,
                        "success": True,
                        "decision": "accept",
                        "tool_type": "native",
                    },
                },
                {
                    "name": "qwen-code.tool_call",
                    "timestamp": ISO_TS_1,
                    "attributes": {
                        "prompt_id": "qwen-prompt",
                        "function_name": "apply_patch",
                        "function_args": "{\"path\":\"src/lib.rs\",\"old\":\"old\\n\",\"new\":\"new\\n\"}",
                        "duration_ms": 22,
                        "success": True,
                        "decision": "accept",
                        "tool_type": "native",
                    },
                },
                {
                    "name": "qwen-code.tool_result",
                    "timestamp": ISO_TS_1,
                    "attributes": {
                        "prompt_id": "qwen-prompt",
                        "function_name": "read_file",
                        "result": "file content",
                        "success": True,
                    },
                },
                {
                    "name": "qwen-code.api_response",
                    "timestamp": ISO_TS_1,
                    "attributes": {
                        "prompt_id": "qwen-prompt",
                        "model": "qwen3-coder-plus",
                        "status_code": 200,
                        "input_token_count": 124,
                        "output_token_count": 76,
                        "cached_content_token_count": 5,
                        "thoughts_token_count": 9,
                        "tool_token_count": 4,
                        "total_token_count": 218,
                        "response_text": "qwen output",
                        "auth_type": "oauth-personal",
                    },
                },
            ],
        )
        write_jsonl(
            root / ".qwen/projects/project-one/chats/session-qwen.jsonl",
            [
                {
                    "uuid": "qwen-user-1",
                    "parentUuid": None,
                    "sessionId": "qwen-session",
                    "timestamp": ISO_TS,
                    "type": "user",
                    "cwd": "/workspace/project-one",
                    "version": "0.0.1",
                    "gitBranch": "main",
                    "message": {
                        "role": "user",
                        "parts": [{"text": "qwen prompt"}],
                    },
                },
                {
                    "uuid": "qwen-assistant-1",
                    "parentUuid": "qwen-user-1",
                    "sessionId": "qwen-session",
                    "timestamp": ISO_TS_1,
                    "type": "assistant",
                    "cwd": "/workspace/project-one",
                    "version": "0.0.1",
                    "gitBranch": "main",
                    "model": "qwen3-coder-plus",
                    "contextWindowSize": 262144,
                    "message": {
                        "role": "model",
                        "parts": [
                            {"text": "qwen output"},
                            {
                                "functionCall": {
                                    "id": "qwen-call-read",
                                    "name": "read_file",
                                    "args": {"path": "src/lib.rs"},
                                }
                            },
                            {
                                "functionCall": {
                                    "id": "qwen-call-edit",
                                    "name": "apply_patch",
                                    "args": {
                                        "path": "src/lib.rs",
                                        "old": "old\n",
                                        "new": "new\n",
                                    },
                                }
                            },
                        ],
                    },
                    "usageMetadata": {
                        "promptTokenCount": 120,
                        "candidatesTokenCount": 45,
                        "cachedContentTokenCount": 12,
                        "thoughtsTokenCount": 7,
                        "totalTokenCount": 184,
                    },
                },
                {
                    "uuid": "qwen-tool-1",
                    "parentUuid": "qwen-assistant-1",
                    "sessionId": "qwen-session",
                    "timestamp": ISO_TS_1,
                    "type": "tool_result",
                    "cwd": "/workspace/project-one",
                    "version": "0.0.1",
                    "gitBranch": "main",
                    "message": {
                        "role": "user",
                        "parts": [
                            {
                                "functionResponse": {
                                    "id": "qwen-call-read",
                                    "name": "read_file",
                                    "response": {"output": "file content"},
                                }
                            }
                        ],
                    },
                    "toolCallResult": {
                        "callId": "qwen-call-edit",
                        "resultDisplay": {
                            "fileName": "src/lib.rs",
                            "fileDiff": "@@ -1 +1 @@\n-old\n+new\n",
                            "originalContent": "old\n",
                            "newContent": "new\n",
                        },
                        "error": None,
                        "errorType": None,
                    },
                }
            ],
        )
        write_jsonl(
            root / ".qwen/usage_record.jsonl",
            [
                {
                    "version": 1,
                    "sessionId": "qwen-session",
                    "timestamp": 1781589600000,
                    "startTime": 1781589540000,
                    "project": "/workspace/project-one",
                    "durationMs": 60000,
                    "totalLatencyMs": 1234,
                    "models": {
                        "qwen3-coder-plus": {
                            "requests": 2,
                            "inputTokens": 180,
                            "outputTokens": 80,
                            "cachedTokens": 16,
                            "thoughtsTokens": 9,
                            "cost": 0.021,
                            "totalTokens": 285,
                            "totalLatencyMs": 1234,
                        }
                    },
                    "cost": 0.021,
                    "tools": {
                        "totalCalls": 2,
                        "totalSuccess": 2,
                        "totalFail": 0,
                        "byName": {
                            "read_file": {
                                "count": 1,
                                "success": 1,
                                "fail": 0,
                                "totalDurationMs": 18,
                            },
                            "apply_patch": {
                                "count": 1,
                                "success": 1,
                                "fail": 0,
                                "totalDurationMs": 22,
                            },
                        },
                    },
                    "files": {"linesAdded": 1, "linesRemoved": 1},
                }
            ],
        )
        write_jsonl(
            root / ".qwen/usage/token-usage-2026-06.jsonl",
            [
                {
                    "schemaVersion": 1,
                    "id": "qwen-token-usage-1",
                    "timestamp": ISO_TS_1,
                    "localDate": "2026-06-16",
                    "localMonth": "2026-06",
                    "sessionId": "qwen-token-session",
                    "model": "qwen3-coder-plus",
                    "authType": "oauth-personal",
                    "source": "main",
                    "inputTokens": 66,
                    "outputTokens": 27,
                    "cachedTokens": 8,
                    "thoughtsTokens": 5,
                    "cost": 0.009,
                    "totalTokens": 106,
                    "apiDurationMs": 1234,
                }
            ],
        )
        return path

    if agent == "antigravity":
        path = root / ".gemini/antigravity-cli/conversations/session-antigravity.db"
        prompt_text = "antigravity prompt"
        output_text = "antigravity output"
        read_tool_name = "read_file"
        edit_tool_name = "apply_patch"
        tool_result_status = 1
        conn = open_fixture_db(path)
        try:
            conn.executescript(
                "CREATE TABLE gen_metadata (idx INTEGER, data BLOB, size INTEGER);"
                "CREATE TABLE trajectory_metadata_blob (id TEXT, data BLOB);"
                "CREATE TABLE steps (idx INTEGER PRIMARY KEY, step_type INTEGER, status INTEGER, step_payload BLOB);"
            )
            conn.execute(
                "INSERT INTO gen_metadata VALUES (?, ?, ?)",
                (0, antigravity_gen_metadata_proto(prompt_text, output_text), 0),
            )
            conn.execute(
                "INSERT INTO trajectory_metadata_blob VALUES (?, ?)",
                ("main", antigravity_trajectory_proto()),
            )
            conn.execute(
                "INSERT INTO steps VALUES (?, ?, ?, ?)",
                (1, 15, tool_result_status, antigravity_text_step_proto(output_text)),
            )
            conn.execute(
                "INSERT INTO steps VALUES (?, ?, ?, ?)",
                (
                    2,
                    5,
                    tool_result_status,
                    antigravity_tool_step_proto(read_tool_name, {"path": "src/lib.rs"}),
                ),
            )
            conn.execute(
                "INSERT INTO steps VALUES (?, ?, ?, ?)",
                (
                    3,
                    5,
                    tool_result_status,
                    antigravity_tool_step_proto(
                        edit_tool_name,
                        {"path": "src/lib.rs", "old": "old\n", "new": "new\n"},
                    ),
                ),
            )
            conn.commit()
        finally:
            conn.close()
        return path

    if agent == "opencode":
        path = root / ".local/share/opencode/opencode.db"
        conn = open_fixture_db(path)
        try:
            conn.executescript(
                "CREATE TABLE session (id TEXT PRIMARY KEY, model TEXT, provider TEXT, account TEXT, "
                "workspace TEXT, input_tokens INTEGER, output_tokens INTEGER, cost REAL, created_at INTEGER);"
                "CREATE TABLE message (id TEXT PRIMARY KEY, session_id TEXT, role TEXT, data TEXT, created_at INTEGER);"
                "CREATE TABLE part (id TEXT PRIMARY KEY, message_id TEXT, session_id TEXT, data TEXT, created_at INTEGER);"
                "CREATE TABLE session_input (id TEXT PRIMARY KEY, session_id TEXT, prompt TEXT, workspace TEXT, created_at INTEGER);"
                "CREATE TABLE session_message (id TEXT PRIMARY KEY, session_id TEXT, type TEXT, data TEXT, created_at INTEGER);"
                "CREATE TABLE account (id TEXT PRIMARY KEY, providerID TEXT, email TEXT, model TEXT, data TEXT);"
            )
            conn.execute(
                "INSERT INTO session VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                (
                    "opencode-session",
                    "gpt-5",
                    "openai",
                    None,
                    "/tmp/opencode-workspace",
                    88,
                    22,
                    0.031,
                    EPOCH_MS,
                ),
            )
            conn.execute(
                "INSERT INTO account VALUES (?, ?, ?, ?, ?)",
                (
                    "opencode-account",
                    "openai",
                    "opencode@example.com",
                    "gpt-5",
                    json.dumps({"workspace": "/tmp/opencode-account-workspace"}, separators=(",", ":")),
                ),
            )
            conn.execute(
                "INSERT INTO message VALUES (?, ?, ?, ?, ?)",
                (
                    "opencode-user",
                    "opencode-session",
                    "user",
                    json.dumps(
                        {"role": "user", "content": "opencode prompt"},
                        separators=(",", ":"),
                    ),
                    EPOCH_MS,
                ),
            )
            conn.execute(
                "INSERT INTO message VALUES (?, ?, ?, ?, ?)",
                (
                    "opencode-assistant",
                    "opencode-session",
                    "assistant",
                    json.dumps(
                        {
                            "role": "assistant",
                            "content": "opencode output",
                            "tool_calls": [
                                {
                                    "id": "tool-1",
                                    "name": "read_file",
                                    "arguments": {"path": "src/lib.rs"},
                                }
                            ],
                        },
                        separators=(",", ":"),
                    ),
                    EPOCH_MS_1,
                ),
            )
            conn.execute(
                "INSERT INTO message VALUES (?, ?, ?, ?, ?)",
                (
                    "opencode-tool-result",
                    "opencode-session",
                    "tool_result",
                    json.dumps(
                        {
                            "role": "tool_result",
                            "tool_call_id": "tool-1",
                            "content": "file content",
                        },
                        separators=(",", ":"),
                    ),
                    EPOCH_MS,
                ),
            )
            conn.execute(
                "INSERT INTO part VALUES (?, ?, ?, ?, ?)",
                (
                    "opencode-orphan-part",
                    "missing-message",
                    "opencode-session",
                    json.dumps(
                        {
                            "type": "tool",
                            "tool": "read_file",
                            "callID": "orphan-tool",
                            "state": {"input": {"path": "src/orphan.rs"}, "output": "orphan file content"},
                        },
                        separators=(",", ":"),
                    ),
                    EPOCH_MS_1,
                ),
            )
            conn.execute(
                "INSERT INTO session_input VALUES (?, ?, ?, ?, ?)",
                (
                    "opencode-input",
                    "opencode-session",
                    "opencode session input prompt",
                    "/tmp/opencode-input-workspace",
                    EPOCH_MS,
                ),
            )
            conn.execute(
                "INSERT INTO session_message VALUES (?, ?, ?, ?, ?)",
                (
                    "opencode-session-message",
                    "opencode-session",
                    "assistant",
                    json.dumps(
                        {
                            "role": "assistant",
                            "content": [
                                {"type": "text", "text": "opencode session message output"},
                                {
                                    "type": "tool",
                                    "tool": "read_file",
                                    "callID": "session-message-tool",
                                    "state": {
                                        "input": {"path": "src/session-message.rs"},
                                        "output": "session message file content",
                                    },
                                },
                                {
                                    "type": "tool_result",
                                    "tool": "read_file",
                                    "tool_call_id": "session-message-result",
                                    "content": "session message explicit result",
                                },
                                {
                                    "type": "patch",
                                    "path": "src/session-message.rs",
                                    "old": "old\n",
                                    "new": "new\n",
                                },
                            ],
                            "input_tokens": 18,
                            "output_tokens": 7,
                        },
                        separators=(",", ":"),
                    ),
                    EPOCH_MS_1,
                ),
            )
            conn.commit()
        finally:
            conn.close()
        write_json(
            root / ".config/opencode/exports/session-opencode-message.json",
            {
                "info": {
                    "id": "opencode-export-session",
                    "directory": "/tmp/project",
                    "model": {"id": "gpt-5", "providerID": "openai"},
                    "tokens": {
                        "input": 120,
                        "output": 50,
                        "cache": {"read": 9, "write": 4},
                        "reasoning": 6,
                    },
                    "cost": 0.04,
                    "time": {"created": EPOCH_MS},
                },
                "messages": [
                    {
                        "info": {
                            "id": "opencode-export-user",
                            "sessionID": "opencode-export-session",
                            "role": "user",
                            "time": {"created": EPOCH_MS},
                        },
                        "parts": [{"type": "text", "text": "opencode export prompt"}],
                    },
                    {
                        "info": {
                            "id": "opencode-export-assistant",
                            "sessionID": "opencode-export-session",
                            "role": "assistant",
                            "time": {"created": EPOCH_MS_1},
                            "tokens": {
                                "input": 120,
                                "output": 50,
                                "cache": {"read": 9, "write": 4},
                                "reasoning": 6,
                            },
                        },
                        "parts": [
                            {"type": "text", "text": "opencode export output"},
                            {"type": "reasoning", "text": "opencode export reasoning"},
                            {
                                "type": "tool",
                                "tool": "read_file",
                                "callID": "export-tool-1",
                                "state": {"input": {"path": "src/lib.rs"}, "output": "file content"},
                            },
                            {
                                "type": "tool",
                                "tool": "apply_patch",
                                "callID": "export-tool-edit",
                                "state": {
                                    "input": {"path": "src/lib.rs", "old": "old\n", "new": "new\n"},
                                    "output": "patched",
                                },
                            },
                        ],
                    },
                ],
            },
        )
        return path

    if agent in {"qoder", "qoder-cn"}:
        hook_root = ".qoder" if agent == "qoder" else ".lingma"
        write_jsonl(
            root / hook_root / "hooks" / f"{agent}-hooks.jsonl",
            [
                {
                    "hook_event_name": "UserPromptSubmit",
                    "session_id": f"{agent}-hook-session",
                    "timestamp": ISO_TS,
                    "prompt": f"{agent} prompt",
                },
                {
                    "hook_event_name": "PreToolUse",
                    "session_id": f"{agent}-hook-session",
                    "timestamp": ISO_TS_1,
                    "tool_name": "read_file",
                    "tool_input": {"path": "src/lib.rs"},
                },
                {
                    "hook_event_name": "PreToolUse",
                    "session_id": f"{agent}-hook-session",
                    "timestamp": ISO_TS_1,
                    "tool_name": "apply_patch",
                    "tool_input": {"path": "src/lib.rs", "old": "old\n", "new": "new\n"},
                },
                {
                    "hook_event_name": "PostToolUse",
                    "session_id": f"{agent}-hook-session",
                    "timestamp": ISO_TS_1,
                    "tool_name": "read_file",
                    "tool_response": "file content",
                },
                {
                    "hook_event_name": "PostToolUse",
                    "session_id": f"{agent}-hook-session",
                    "timestamp": ISO_TS_1,
                    "tool_name": "apply_patch",
                    "tool_response": {"path": "src/lib.rs", "output": "patch applied"},
                },
                {
                    "hook_event_name": "PostToolUse",
                    "session_id": f"{agent}-hook-session",
                    "timestamp": ISO_TS_1,
                    "tool_name": "apply_patch",
                    "tool_response": {"path": "src/lib.rs", "output": "patch applied"},
                },
            ],
        )
        write_jsonl(
            root / hook_root / "projects/project-one/transcript" / f"{agent}-session.jsonl",
            [
                {
                    "type": "message",
                    "sessionId": f"{agent}-transcript-session",
                    "timestamp": ISO_TS,
                    "cwd": "/tmp/project",
                    "message": {"role": "user", "content": f"{agent} transcript prompt"},
                },
                {
                    "type": "message",
                    "sessionId": f"{agent}-transcript-session",
                    "timestamp": ISO_TS_1,
                    "message": {
                        "role": "assistant",
                        "content": [
                            {"type": "text", "text": f"{agent} transcript output"},
                            {
                                "type": "tool_use",
                                "id": "tool-1",
                                "name": "read_file",
                                "input": {"path": "src/lib.rs"},
                            },
                            {
                                "type": "tool_use",
                                "id": "tool-edit",
                                "name": "apply_patch",
                                "input": {"path": "src/lib.rs", "old": "old\n", "new": "new\n"},
                            },
                        ],
                        "usage_basis": usage_fixture_expected_usage_basis(agent, "transcript-jsonl"),
                        "usage": {
                            "input_tokens": 110,
                            "output_tokens": 44,
                            "cache_read_tokens": 9,
                            "reasoning_tokens": 6,
                            "cost": 0.014,
                        },
                        "model": "qoder-pro",
                        "provider": agent,
                    },
                },
                {
                    "type": "message",
                    "sessionId": f"{agent}-transcript-session",
                    "timestamp": ISO_TS_1,
                    "message": {
                        "role": "toolResult",
                        "toolUseID": "tool-1",
                        "toolUseResult": "file content",
                    },
                },
                {
                    "type": "message",
                    "sessionId": f"{agent}-transcript-session",
                    "timestamp": ISO_TS_1,
                    "message": {
                        "role": "toolResult",
                        "toolUseID": "tool-edit",
                        "toolUseResult": {"path": "src/lib.rs", "output": "patch applied"},
                    },
                },
            ],
        )
        product = "Qoder" if agent == "qoder" else "QoderCN"
        path = root / f"Library/Application Support/{product}/SharedClientCache/cache/db/local.db"
        conn = open_fixture_db(path)
        try:
            conn.execute(
                "CREATE TABLE chat_message (id TEXT, session_id TEXT, request_id TEXT, role TEXT, token_info TEXT, gmt_create INTEGER)"
            )
            conn.execute(
                "INSERT INTO chat_message VALUES (?, ?, ?, ?, ?, ?)",
                (
                    f"{agent}-user",
                    f"{agent}-session",
                    "req-1",
                    "user",
                    json.dumps(
                        {"type": "user", "content": f"{agent} prompt"},
                        separators=(",", ":"),
                    ),
                    EPOCH_MS,
                ),
            )
            conn.execute(
                "INSERT INTO chat_message VALUES (?, ?, ?, ?, ?, ?)",
                (
                    f"{agent}-msg",
                    f"{agent}-session",
                    "req-1",
                    "assistant",
                    json.dumps(
                        {
                            "type": "assistant",
                            "content": f"{agent} output",
                            "usage_basis": usage_fixture_expected_usage_basis(agent, "local-db"),
                            "prompt_tokens": 120,
                            "completion_tokens": 45,
                            "cached_tokens": 12,
                            "model": "qoder-pro",
                            "provider": agent,
                            "tool_calls": [
                                {"id": "tool-1", "name": "read_file", "arguments": {"path": "src/lib.rs"}},
                                {"id": "tool-edit", "name": "apply_patch", "arguments": {"path": "src/lib.rs", "old": "old\n", "new": "new\n"}},
                            ],
                        },
                        separators=(",", ":"),
                    ),
                    EPOCH_MS,
                ),
            )
            conn.execute(
                "INSERT INTO chat_message VALUES (?, ?, ?, ?, ?, ?)",
                (
                    f"{agent}-tool",
                    f"{agent}-session",
                    "req-1",
                    "tool",
                    json.dumps(
                        {"type": "tool_result", "tool_call_id": "tool-1", "content": "file content"},
                        separators=(",", ":"),
                    ),
                    EPOCH_MS,
                ),
            )
            conn.commit()
        finally:
            conn.close()
        return path

    if agent in {"qoder-work", "qoder-work-cn"}:
        dirname = ".qoderwork"
        write_jsonl(
            root / dirname / "hooks" / f"{agent}-hooks.jsonl",
            [
                {
                    "hook_event_name": "UserPromptSubmit",
                    "session_id": f"{agent}-hook-session",
                    "timestamp": ISO_TS,
                    "prompt": f"{agent} prompt",
                },
                {
                    "hook_event_name": "PreToolUse",
                    "session_id": f"{agent}-hook-session",
                    "timestamp": ISO_TS_1,
                    "tool_name": "read_file",
                    "tool_input": {"path": "src/lib.rs"},
                },
                {
                    "hook_event_name": "PreToolUse",
                    "session_id": f"{agent}-hook-session",
                    "timestamp": ISO_TS_1,
                    "tool_name": "apply_patch",
                    "tool_input": {"path": "src/lib.rs", "old": "old\n", "new": "new\n"},
                },
                {
                    "hook_event_name": "PostToolUse",
                    "session_id": f"{agent}-hook-session",
                    "timestamp": ISO_TS_1,
                    "tool_name": "read_file",
                    "tool_response": "file content",
                },
            ],
        )
        if agent == "qoder-work-cn":
            write_jsonl(
                root / ".qoderworkcn" / "hooks" / f"{agent}-legacy-hooks.jsonl",
                [
                    {
                        "hook_event_name": "UserPromptSubmit",
                        "session_id": f"{agent}-legacy-hook-session",
                        "timestamp": ISO_TS,
                        "prompt": f"{agent} legacy prompt",
                    }
                ],
            )
        path = root / dirname / "data/session-events.jsonl"
        write_jsonl(
            path,
            [
                {
                    "event.name": "llm.request",
                    "timestamp": ISO_TS,
                    "gen_ai.agent.type": agent,
                    "gen_ai.session.id": f"{agent}-session",
                    "gen_ai.request.model": "qwork-ultimate",
                    "gen_ai.input.messages_delta": [{"role": "user", "content": f"{agent} prompt"}],
                },
                {
                    "event.name": "llm.response",
                    "timestamp": ISO_TS_1,
                    "gen_ai.agent.type": agent,
                    "gen_ai.session.id": f"{agent}-session",
                    "gen_ai.response.model": "qwork-ultimate",
                    "usage_basis": usage_fixture_expected_usage_basis(agent, "trace-jsonl"),
                    "usage": {
                        "input_tokens": 140,
                        "output_tokens": 58,
                        "cache_read_tokens": 13,
                        "cache_write_tokens": 4,
                        "reasoning_tokens": 8,
                        "cost": 0.024,
                    },
                    "account": "local",
                    "gen_ai.output.messages": [{"role": "assistant", "content": f"{agent} output"}],
                },
                {
                    "event.name": "llm.tool_call",
                    "timestamp": ISO_TS_1,
                    "gen_ai.agent.type": agent,
                    "gen_ai.session.id": f"{agent}-session",
                    "gen_ai.tool.name": "read_file",
                    "gen_ai.tool.call.arguments": {"path": "src/lib.rs"},
                },
                {
                    "event.name": "llm.tool_result",
                    "timestamp": ISO_TS_1,
                    "gen_ai.agent.type": agent,
                    "gen_ai.session.id": f"{agent}-session",
                    "gen_ai.tool.call.id": "tool-1",
                    "gen_ai.tool.name": "read_file",
                    "gen_ai.tool.call.result": "file content",
                },
                {
                    "event.name": "llm.file_edit",
                    "timestamp": ISO_TS_1,
                    "gen_ai.agent.type": agent,
                    "gen_ai.session.id": f"{agent}-session",
                    "gen_ai.tool.name": "apply_patch",
                    "gen_ai.tool.call.id": "tool-edit",
                    "path": "src/lib.rs",
                    "old": "old\n",
                    "new": "new\n",
                },
            ],
        )
        write_jsonl(
            root / dirname / "logs/sessions" / f"{agent}-session" / "segments" / "0001.jsonl",
            [
                {
                    "event.name": "llm.response",
                    "timestamp": ISO_TS_1,
                    "gen_ai.session.id": f"{agent}-segment-session",
                    "gen_ai.response.model": "qwork-ultimate",
                    "usage_basis": usage_fixture_expected_usage_basis(agent, "trace-jsonl"),
                    "usage": {
                        "input_tokens": 50,
                        "output_tokens": 20,
                        "cache_read_tokens": 5,
                        "reasoning_tokens": 3,
                        "cost": 0.008,
                    },
                    "gen_ai.output.messages": [{"role": "assistant", "content": f"{agent} segment output"}],
                }
            ],
        )
        tool_result_path = root / dirname / "tool-results" / f"{agent}-session" / "tool-2.txt"
        tool_result_path.parent.mkdir(parents=True, exist_ok=True)
        tool_result_path.write_text("tool result file content", encoding="utf-8")
        db_path = root / dirname / "messages.db"
        conn = open_fixture_db(db_path)
        try:
            conn.executescript(
                "CREATE TABLE messages (id TEXT, session_id TEXT, role TEXT, content TEXT, "
                "model TEXT, provider TEXT, input_tokens INTEGER, output_tokens INTEGER, "
                "cache_read_tokens INTEGER, cache_write_tokens INTEGER, reasoning_tokens INTEGER, "
                "cost REAL, created_at TEXT);"
                "CREATE TABLE events (id TEXT, session_id TEXT, payload TEXT, created_at TEXT);"
            )
            conn.execute(
                "INSERT INTO messages VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                (
                    "qwork-db-user",
                    f"{agent}-db-session",
                    "user",
                    f"{agent} db prompt",
                    "qwork-ultimate",
                    agent,
                    30,
                    0,
                    0,
                    0,
                    0,
                    0.0,
                    ISO_TS,
                ),
            )
            conn.execute(
                "INSERT INTO messages VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                (
                    "qwork-db-assistant",
                    f"{agent}-db-session",
                    "assistant",
                    f"{agent} db output",
                    "qwork-ultimate",
                    agent,
                    120,
                    48,
                    11,
                    3,
                    7,
                    0.018,
                    ISO_TS_1,
                ),
            )
            conn.execute(
                "INSERT INTO events VALUES (?, ?, ?, ?)",
                (
                    "qwork-db-tool",
                    f"{agent}-db-session",
                    json.dumps(
                        {
                            "event.name": "llm.tool_call",
                            "gen_ai.session.id": f"{agent}-db-session",
                            "gen_ai.tool.call.id": "db-tool-1",
                            "gen_ai.tool.name": "read_file",
                            "gen_ai.tool.call.arguments": {"path": "src/db.rs"},
                        },
                        separators=(",", ":"),
                    ),
                    ISO_TS_1,
                ),
            )
            conn.execute(
                "INSERT INTO events VALUES (?, ?, ?, ?)",
                (
                    "qwork-db-tool-result",
                    f"{agent}-db-session",
                    json.dumps(
                        {
                            "event.name": "llm.tool_result",
                            "gen_ai.session.id": f"{agent}-db-session",
                            "gen_ai.tool.call.id": "db-tool-1",
                            "gen_ai.tool.name": "read_file",
                            "gen_ai.tool.call.result": "file content",
                        },
                        separators=(",", ":"),
                    ),
                    ISO_TS_1,
                ),
            )
            conn.execute(
                "INSERT INTO events VALUES (?, ?, ?, ?)",
                (
                    "qwork-db-edit",
                    f"{agent}-db-session",
                    json.dumps(
                        {
                            "event.name": "llm.file_edit",
                            "gen_ai.session.id": f"{agent}-db-session",
                            "gen_ai.tool.name": "apply_patch",
                            "gen_ai.tool.call.id": "db-tool-edit",
                            "path": "src/db.rs",
                            "old": "old\n",
                            "new": "new\n",
                        },
                        separators=(",", ":"),
                    ),
                    ISO_TS_1,
                ),
            )
            conn.commit()
        finally:
            conn.close()
        agents_db = root / dirname / "agents.db"
        conn = open_fixture_db(agents_db)
        try:
            conn.execute(
                "CREATE TABLE agents (id TEXT, model TEXT, provider TEXT, input_tokens INTEGER, output_tokens INTEGER, created_at TEXT)"
            )
            conn.execute(
                "INSERT INTO agents VALUES (?, ?, ?, ?, ?, ?)",
                (f"{agent}-agent-session", "qwork-ultimate", agent, None, None, ISO_TS),
            )
            conn.commit()
        finally:
            conn.close()
        return path

    if agent == "wukong":
        path = root / ".wukong/data/wukong.db"
        conn = open_fixture_db(path)
        try:
            conn.executescript(
                """
                CREATE TABLE sessions (
                    id TEXT PRIMARY KEY,
                    goal TEXT NOT NULL,
                    initial_goal TEXT,
                    status TEXT DEFAULT 'active',
                    user_id TEXT,
                    organization_id TEXT,
                    agent_type TEXT DEFAULT 'InteractiveAgent',
                    created_at TEXT,
                    updated_at TEXT
                );
                CREATE TABLE steps (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    session_id TEXT NOT NULL,
                    step_number INTEGER NOT NULL,
                    llm_prompt TEXT,
                    llm_response TEXT,
                    action TEXT NOT NULL,
                    reasoning TEXT,
                    selected_tool TEXT,
                    parameters TEXT,
                    step_result TEXT,
                    error_message TEXT,
                    status TEXT DEFAULT 'completed',
                    started_at TEXT,
                    completed_at TEXT,
                    execution_duration_ms INTEGER,
                    created_at TEXT,
                    updated_at TEXT
                );
                CREATE TABLE parallel_tool_calls (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    step_id INTEGER NOT NULL,
                    tool_id TEXT NOT NULL,
                    tool_name TEXT NOT NULL,
                    parameters TEXT NOT NULL,
                    status TEXT DEFAULT 'completed',
                    result TEXT,
                    error_message TEXT,
                    started_at TEXT,
                    completed_at TEXT,
                    execution_duration_ms INTEGER,
                    created_at TEXT,
                    updated_at TEXT
                );
                CREATE TABLE fork_agent_tasks (
                    id TEXT PRIMARY KEY,
                    parent_session_id TEXT NOT NULL,
                    parent_step_id INTEGER,
                    sub_session_id TEXT,
                    goal TEXT NOT NULL,
                    context_summary TEXT,
                    depth INTEGER NOT NULL,
                    status TEXT DEFAULT 'completed',
                    steps_executed INTEGER DEFAULT 0,
                    tokens_used INTEGER DEFAULT 0,
                    input_tokens INTEGER DEFAULT 0,
                    output_tokens INTEGER DEFAULT 0,
                    cache_read_tokens INTEGER DEFAULT 0,
                    cache_write_tokens INTEGER DEFAULT 0,
                    reasoning_tokens INTEGER DEFAULT 0,
                    source_cost REAL DEFAULT 0,
                    tools_called INTEGER DEFAULT 0,
                    started_at TEXT,
                    completed_at TEXT,
                    execution_duration_ms INTEGER,
                    created_at TEXT,
                    updated_at TEXT
                );
                CREATE TABLE todos (
                    id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL,
                    actual_tokens INTEGER,
                    input_tokens INTEGER DEFAULT 0,
                    output_tokens INTEGER DEFAULT 0,
                    cache_read_tokens INTEGER DEFAULT 0,
                    cache_write_tokens INTEGER DEFAULT 0,
                    reasoning_tokens INTEGER DEFAULT 0,
                    source_cost REAL DEFAULT 0,
                    created_at TEXT,
                    updated_at TEXT
                );
                """
            )
            conn.execute(
                "INSERT INTO sessions VALUES ('wk-session','wukong prompt','wukong prompt','active','local-user','local-org','InteractiveAgent',?,?)",
                (ISO_TS, ISO_TS_1),
            )
            conn.execute(
                "INSERT INTO steps (id, session_id, step_number, llm_prompt, llm_response, action, selected_tool, parameters, step_result, started_at, completed_at, created_at, updated_at) VALUES (1,'wk-session',1,'wukong prompt','wukong output','CallTool','apply_patch',?,? ,?,?,?,?)",
                (
                    json.dumps({"path": "src/lib.rs", "old": "old\n", "new": "new\n"}),
                    json.dumps({"ok": True, "path": "src/lib.rs"}),
                    ISO_TS,
                    ISO_TS_1,
                    ISO_TS,
                    ISO_TS_1,
                ),
            )
            conn.execute(
                "INSERT INTO parallel_tool_calls (step_id, tool_id, tool_name, parameters, result, started_at, completed_at, created_at, updated_at) VALUES (1,'tool-1','read_file',?,?,?,?,?,?)",
                (
                    json.dumps({"path": "src/lib.rs"}),
                    "file content",
                    ISO_TS,
                    ISO_TS_1,
                    ISO_TS,
                    ISO_TS_1,
                ),
            )
            conn.execute(
                "INSERT INTO fork_agent_tasks VALUES ('fork-1','wk-session',1,NULL,'sub task','context',1,'completed',2,230,140,60,16,4,10,0.032,3,?,?,20,?,?)",
                (ISO_TS, ISO_TS_1, ISO_TS, ISO_TS_1),
            )
            conn.execute(
                "INSERT INTO todos VALUES ('todo-1','wk-session',90,50,25,6,2,4,0.011,?,?)",
                (ISO_TS, ISO_TS_1),
            )
            conn.commit()
        finally:
            conn.close()
        return path

    if agent == "hermes":
        path = root / ".hermes/state.db"
        conn = open_fixture_db(path)
        try:
            conn.execute(
                "CREATE TABLE sessions (id TEXT, model TEXT, billing_provider TEXT, input_tokens INTEGER, output_tokens INTEGER, cache_read_tokens INTEGER, cache_write_tokens INTEGER, reasoning_tokens INTEGER, estimated_cost_usd REAL, actual_cost_usd REAL)"
            )
            conn.execute(
                "CREATE TABLE messages (id TEXT, session_id TEXT, role TEXT, content TEXT, tool_calls TEXT, tool_call_id TEXT, token_count INTEGER, created_at INTEGER, reasoning TEXT, reasoning_details TEXT)"
            )
            conn.execute(
                "INSERT INTO sessions VALUES ('hermes-session','claude-sonnet-4','anthropic',100,50,10,5,2,0.1,0.12)"
            )
            conn.execute(
                "INSERT INTO messages VALUES ('hermes-user','hermes-session','user','hermes prompt',NULL,NULL,12,?,NULL,NULL)",
                (EPOCH_MS,),
            )
            conn.execute(
                "INSERT INTO messages VALUES ('hermes-assistant','hermes-session','assistant','hermes output',?,NULL,18,?,?,?)",
                (
                    json.dumps(
                        [
                            {"id": "tool-1", "name": "read_file", "arguments": {"path": "src/lib.rs"}},
                            {"id": "tool-edit", "name": "apply_patch", "arguments": {"path": "src/lib.rs", "old": "old\n", "new": "new\n"}},
                        ],
                        separators=(",", ":"),
                    ),
                    EPOCH_MS,
                    "hermes reasoning text",
                    json.dumps({"summary": "hermes reasoning details"}, separators=(",", ":")),
                ),
            )
            conn.execute(
                "INSERT INTO messages VALUES ('hermes-tool','hermes-session','tool','file content',NULL,'tool-1',3,?,NULL,NULL)",
                (EPOCH_MS,),
            )
            conn.commit()
        finally:
            conn.close()
        return path

    if agent == "openclaw":
        sessions_dir = root / ".openclaw/agents/main/sessions"
        path = sessions_dir / "session-openclaw.jsonl"
        write_json(
            sessions_dir / "sessions.json",
            {
                "agent:main": {
                    "sessionId": "openclaw-session",
                    "sessionFile": str(path),
                    "lastAccountId": "openclaw-account",
                    "origin": {"accountId": "origin-account"},
                    "updatedAt": ISO_TS_1,
                    "chatType": "agent",
                    "channel": "main",
                    "displayName": "OpenClaw session",
                }
            },
        )
        write_jsonl(
            path,
            [
                {"type": "session", "id": "openclaw-session", "cwd": "/tmp/project", "timestamp": ISO_TS},
                {"type": "model_change", "modelId": "gpt-5", "provider": "openai", "timestamp": ISO_TS},
                {
                    "type": "message",
                    "sessionId": "openclaw-session",
                    "timestamp": ISO_TS,
                    "message": {"role": "user", "content": "openclaw prompt", "timestamp": EPOCH_MS},
                },
                {
                    "type": "message",
                    "sessionId": "openclaw-session",
                    "timestamp": ISO_TS_1,
                    "message": {
                        "role": "assistant",
                        "timestamp": EPOCH_MS,
                        "content": [
                            {"type": "text", "text": "openclaw output"},
                            {
                                "type": "toolCall",
                                "id": "tool-1",
                                "name": "read_file",
                                "arguments": {"path": "src/lib.rs"},
                            },
                            {
                                "type": "toolCall",
                                "id": "tool-edit",
                                "name": "edit",
                                "arguments": {
                                    "path": "src/lib.rs",
                                    "edits": [{"oldText": "old\n", "newText": "new\n"}],
                                },
                            },
                        ],
                        "usage": {
                            "input": 100,
                            "output": 40,
                            "cacheReadTokens": 8,
                            "cacheWriteTokens": 3,
                            "cost": {"total": 0.03},
                        },
                        "model": "gpt-5",
                        "provider": "openai",
                    },
                },
                {
                    "type": "assistant",
                    "sessionId": "openclaw-session",
                    "timestamp": ISO_TS_1,
                    "role": "assistant",
                    "content": [
                        {
                            "type": "toolResult",
                            "toolCallId": "tool-1",
                            "toolName": "read_file",
                            "content": "file content",
                            "timestamp": EPOCH_MS,
                        }
                    ],
                },
            ],
        )
        return path

    if agent == "gemini":
        path = root / ".gemini/telemetry.log"
        write_jsonl(
            path,
            [
                {
                    "name": "gemini_cli.user_prompt",
                    "timestamp": ISO_TS,
                    "attributes": {
                        "prompt_id": "gemini-prompt",
                        "prompt_length": 13,
                        "prompt": "gemini prompt",
                        "auth_type": "oauth-personal",
                    },
                },
                {
                    "name": "gemini_cli.tool_call",
                    "timestamp": ISO_TS_1,
                    "attributes": {
                        "prompt_id": "gemini-prompt",
                        "function_name": "read_file",
                        "function_args": "{\"path\":\"src/main.rs\"}",
                        "duration_ms": 12,
                        "success": True,
                        "decision": "accept",
                        "tool_type": "native",
                    },
                },
                {
                    "name": "gemini_cli.tool_call",
                    "timestamp": ISO_TS_1,
                    "attributes": {
                        "prompt_id": "gemini-prompt",
                        "function_name": "apply_patch",
                        "function_args": "{\"path\":\"src/main.rs\",\"old\":\"old\\n\",\"new\":\"new\\n\"}",
                        "duration_ms": 18,
                        "success": True,
                        "decision": "accept",
                        "tool_type": "native",
                    },
                },
                {
                    "name": "gemini_cli.tool_result",
                    "timestamp": ISO_TS_1,
                    "attributes": {
                        "prompt_id": "gemini-prompt",
                        "function_name": "read_file",
                        "result": "file content",
                        "success": True,
                    },
                },
                {
                    "name": "gemini_cli.api_response",
                    "timestamp": ISO_TS_1,
                    "attributes": {
                        "prompt_id": "gemini-prompt",
                        "model": "gemini-3-pro",
                        "status_code": 200,
                        "input_token_count": 100,
                        "output_token_count": 45,
                        "cached_content_token_count": 10,
                        "thoughts_token_count": 6,
                        "tool_token_count": 0,
                        "total_token_count": 161,
                        "response_text": "gemini output",
                        "auth_type": "oauth-personal",
                    },
                },
            ],
        )
        write_jsonl(
            root / ".gemini/tmp/project-one-hash/chats/session-2026-06-16-gemini.jsonl",
            [
                {
                    "sessionId": "gemini-session",
                    "projectHash": "project-one-hash",
                    "startTime": ISO_TS,
                    "lastUpdated": ISO_TS_1,
                    "kind": "main",
                    "directories": ["/workspace/project-one"],
                },
                {
                    "id": "gemini-user-1",
                    "timestamp": ISO_TS,
                    "type": "user",
                    "content": [{"text": "gemini prompt"}],
                },
                {
                    "id": "gemini-assistant-1",
                    "timestamp": ISO_TS_1,
                    "type": "gemini",
                    "model": "gemini-3-pro",
                    "content": "gemini output",
                    "toolCalls": [
                        {
                            "id": "gemini-call-read",
                            "name": "read_file",
                            "args": {"path": "src/main.rs"},
                            "result": [{"text": "file content"}],
                            "status": "success",
                            "timestamp": ISO_TS_1,
                        },
                        {
                            "id": "gemini-call-edit",
                            "name": "apply_patch",
                            "args": {
                                "path": "src/main.rs",
                                "old": "old\n",
                                "new": "new\n",
                            },
                            "resultDisplay": {
                                "fileName": "src/main.rs",
                                "fileDiff": "@@ -1 +1 @@\n-old\n+new\n",
                                "originalContent": "old\n",
                                "newContent": "new\n",
                            },
                            "status": "success",
                            "timestamp": ISO_TS_1,
                        },
                    ],
                    "tokens": {
                        "input": 100,
                        "output": 45,
                        "cached": 10,
                        "thoughts": 6,
                        "tool": 3,
                        "total": 164,
                    },
                },
            ],
        )
        return path

    if agent == "copilot":
        path = root / ".copilot/session-state/copilot-official-session/events.jsonl"
        write_jsonl(
            path,
            [
                {
                    "event": "session.context_changed",
                    "timestamp": ISO_TS,
                    "session_id": "copilot-official-session",
                    "model": "gpt-5",
                    "cwd": "/tmp/copilot-workspace",
                    "gitRoot": "/tmp/copilot-workspace",
                    "repository": "https://github.com/example/project",
                    "branch": "main",
                },
                {
                    "event": "user.message",
                    "timestamp": ISO_TS,
                    "session_id": "copilot-official-session",
                    "content": [{"type": "text", "text": "official copilot prompt"}],
                },
                {
                    "event": "assistant.message_delta",
                    "timestamp": ISO_TS_1,
                    "session_id": "copilot-official-session",
                    "deltaContent": "official copilot output",
                },
                {
                    "event": "tool.execution_start",
                    "timestamp": ISO_TS_1,
                    "session_id": "copilot-official-session",
                    "id": "tool-edit",
                    "name": "apply_patch",
                    "input": {"path": "src/lib.rs", "old": "old\n", "new": "new\n"},
                },
                {
                    "event": "tool.execution_complete",
                    "timestamp": ISO_TS_1,
                    "session_id": "copilot-official-session",
                    "id": "tool-edit",
                    "result": {"output": "patch applied", "file": "src/lib.rs"},
                },
                {
                    "event": "assistant.usage",
                    "timestamp": ISO_TS_1,
                    "session_id": "copilot-official-session",
                    "model": "gpt-5",
                    "usage": {
                        "input_tokens": 31,
                        "output_tokens": 17,
                        "cache_read_tokens": 5,
                        "reasoning_tokens": 3,
                        "cost": 0.0042,
                    },
                },
                {
                    "event": "session.shutdown",
                    "timestamp": ISO_TS_1,
                    "session_id": "copilot-official-session",
                    "codeChanges": [
                        {
                            "file": "src/lib.rs",
                            "diff": "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@\n-old\n+new\n",
                        }
                    ],
                },
            ],
        )
        write_jsonl(
            root / ".copilot/otel/events.jsonl",
            [
                {
                    "type": "span",
                    "name": "copilot.chat",
                    "startTime": [EPOCH_S, 0],
                    "attributes": {
                        "gen_ai.request.model": "gpt-5",
                        "gen_ai.response.model": "gpt-5",
                        "copilot_chat.session_id": "copilot-session",
                        "gen_ai.usage.input_tokens": 130,
                        "gen_ai.usage.output_tokens": 60,
                        "gen_ai.usage.cache_read.input_tokens": 14,
                        "gen_ai.usage.reasoning.output_tokens": 9,
                        "gen_ai.input.messages": [{"role": "user", "content": "copilot prompt"}],
                        "gen_ai.output.messages": [{"role": "assistant", "content": "copilot output"}],
                    },
                },
                {
                    "type": "span",
                    "name": "copilot.tool_call",
                    "startTime": [EPOCH_S, 1],
                    "attributes": {
                        "gen_ai.session.id": "copilot-session",
                        "gen_ai.tool.name": "read_file",
                        "gen_ai.tool.call.id": "tool-1",
                        "gen_ai.tool.call.arguments": {"path": "src/lib.rs"},
                    },
                },
                {
                    "type": "span",
                    "name": "copilot.tool_call",
                    "startTime": [EPOCH_S, 1],
                    "attributes": {
                        "gen_ai.session.id": "copilot-session",
                        "gen_ai.tool.name": "apply_patch",
                        "gen_ai.tool.call.id": "tool-edit",
                        "gen_ai.tool.call.arguments": {"path": "src/lib.rs", "old": "old\n", "new": "new\n"},
                    },
                },
                {
                    "type": "span",
                    "name": "copilot.tool_result",
                    "startTime": [EPOCH_S, 1],
                    "attributes": {
                        "gen_ai.session.id": "copilot-session",
                        "gen_ai.tool.call.id": "tool-1",
                        "gen_ai.tool.call.result": "file content",
                    },
                },
            ],
        )
        write_jsonl(
            root / ".copilot/session-state/copilot-session/events.jsonl",
            [
                {
                    "type": "session",
                    "sessionId": "copilot-session",
                    "provider": "copilot",
                    "model": "gpt-5",
                    "timestamp": ISO_TS,
                },
                {
                    "type": "user",
                    "sessionId": "copilot-session",
                    "content": "copilot prompt",
                    "timestamp": ISO_TS,
                },
                {
                    "type": "assistant",
                    "sessionId": "copilot-session",
                    "content": "copilot output",
                    "timestamp": ISO_TS_1,
                },
                {
                    "type": "tool_call",
                    "sessionId": "copilot-session",
                    "tool_name": "apply_patch",
                    "tool_arguments": {"path": "src/lib.rs", "old": "old\n", "new": "new\n"},
                    "timestamp": ISO_TS_1,
                },
                {
                    "type": "tool_result",
                    "sessionId": "copilot-session",
                    "tool_result": "file content",
                    "timestamp": ISO_TS_1,
                },
            ],
        )
        write_jsonl(
            root / ".config/github-copilot/ws/chat-sessions/copilot-config-session/session.jsonl",
            [
                {
                    "kind": "chatSession",
                    "v": {
                        "sessionId": "copilot-config-session",
                        "creationDate": ISO_TS,
                        "lastMessageDate": ISO_TS_1,
                        "inputState": {"selectedModel": "gpt-5"},
                        "requests": [
                            {
                                "timestamp": ISO_TS_1,
                                "modelId": "gpt-5",
                                "message": {
                                    "text": "config copilot prompt",
                                    "parts": [{"kind": "text", "text": "config copilot prompt"}],
                                },
                                "response": [{"value": "config copilot output"}],
                                "result": {
                                    "toolInvocationSerialized": {
                                        "toolName": "read_file",
                                        "toolCallId": "config-call-read",
                                        "args": {"path": "src/lib.rs"},
                                    },
                                    "toolResultSerialized": {
                                        "toolCallId": "config-call-read",
                                        "result": {"content": "synthetic file content"},
                                    },
                                    "textEditGroup": {
                                        "uri": "src/lib.rs",
                                        "edits": [{"oldText": "old\n", "newText": "new\n"}],
                                    },
                                },
                            }
                        ],
                    },
                }
            ],
        )
        opaque_dir = root / ".config/github-copilot/ws/chat-agent-sessions/copilot-config-session"
        opaque_dir.mkdir(parents=True, exist_ok=True)
        (opaque_dir / "00000000000.xd").write_bytes(bytes([0x82, 0x81, 0x82, 0x80]))
        (opaque_dir / "copilot-agent-sessions-nitrite.db").write_bytes(
            b"H:2,block:8,blockSize:1000"
        )
        edit_opaque_dir = root / ".config/github-copilot/ws/chat-edit-sessions/copilot-config-session"
        edit_opaque_dir.mkdir(parents=True, exist_ok=True)
        (edit_opaque_dir / "00000000000.xd").write_bytes(bytes([0x82, 0x81, 0x82, 0x80]))
        (edit_opaque_dir / "copilot-edit-sessions-nitrite.db").write_bytes(
            b"H:2,block:8,blockSize:1000"
        )
        store_db = root / ".copilot/session-store.db"
        conn = open_fixture_db(store_db)
        try:
            conn.executescript(
                """
                CREATE TABLE sessions (
                  id TEXT PRIMARY KEY,
                  provider TEXT,
                  model TEXT,
                  workspace TEXT,
                  cwd TEXT,
                  created_at TEXT,
                  promptTokens INTEGER,
                  completionTokens INTEGER,
                  cacheReadTokens INTEGER,
                  reasoningTokens INTEGER
                );
                CREATE TABLE messages (
                  id TEXT PRIMARY KEY,
                  session_id TEXT,
                  role TEXT,
                  content TEXT,
                  timestamp TEXT
                );
                CREATE TABLE tool_calls (
                  id TEXT PRIMARY KEY,
                  session_id TEXT,
                  tool_name TEXT,
                  arguments TEXT,
                  timestamp TEXT
                );
                CREATE TABLE tool_results (
                  id TEXT PRIMARY KEY,
                  session_id TEXT,
                  tool_call_id TEXT,
                  result TEXT,
                  timestamp TEXT
                );
                CREATE TABLE modified_files (
                  id TEXT PRIMARY KEY,
                  session_id TEXT,
                  path TEXT,
                  old_string TEXT,
                  new_string TEXT,
                  unified_diff TEXT,
                  timestamp TEXT
                );
                """
            )
            conn.execute(
                "INSERT INTO sessions VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                (
                    "copilot-session",
                    "copilot",
                    "gpt-5",
                    "/tmp/copilot-workspace",
                    "/tmp/copilot-workspace",
                    ISO_TS,
                    120,
                    55,
                    11,
                    7,
                ),
            )
            conn.execute(
                "INSERT INTO messages VALUES (?, ?, ?, ?, ?)",
                ("msg-user", "copilot-session", "user", "copilot prompt", ISO_TS),
            )
            conn.execute(
                "INSERT INTO messages VALUES (?, ?, ?, ?, ?)",
                ("msg-assistant", "copilot-session", "assistant", "copilot output", ISO_TS_1),
            )
            conn.execute(
                "INSERT INTO tool_calls VALUES (?, ?, ?, ?, ?)",
                (
                    "call-read",
                    "copilot-session",
                    "read_file",
                    json.dumps({"path": "src/lib.rs"}, separators=(",", ":")),
                    ISO_TS_1,
                ),
            )
            conn.execute(
                "INSERT INTO tool_calls VALUES (?, ?, ?, ?, ?)",
                (
                    "call-edit",
                    "copilot-session",
                    "apply_patch",
                    json.dumps({"path": "src/lib.rs", "old": "old\n", "new": "new\n"}, separators=(",", ":")),
                    ISO_TS_1,
                ),
            )
            conn.execute(
                "INSERT INTO tool_results VALUES (?, ?, ?, ?, ?)",
                ("result-read", "copilot-session", "call-read", "file content", ISO_TS_1),
            )
            conn.execute(
                "INSERT INTO modified_files VALUES (?, ?, ?, ?, ?, ?, ?)",
                (
                    "file-edit",
                    "copilot-session",
                    "src/lib.rs",
                    "old\n",
                    "new\n",
                    "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@\n-old\n+new\n",
                    ISO_TS_1,
                ),
            )
            conn.commit()
        finally:
            conn.close()
        state_db = root / "Library/Application Support/Code/User/workspaceStorage/copilot-workspace/state.vscdb"
        write_jsonl(
            state_db.parent / "chatSessions/copilot-session.jsonl",
            [
                {
                    "snapshot": {
                        "sessionId": "copilot-session",
                        "creationDate": ISO_TS,
                        "lastMessageDate": ISO_TS_1,
                        "inputState": {"selectedModel": "gpt-5"},
                        "requests": [
                            {
                                "timestamp": ISO_TS_1,
                                "modelId": "gpt-5",
                                "promptTokens": 100,
                                "completionTokens": 40,
                                "message": {
                                    "text": "copilot prompt",
                                    "parts": [{"kind": "text", "text": "copilot prompt"}],
                                },
                                "response": [{"value": "copilot output"}],
                                "result": {
                                    "toolInvocationSerialized": {
                                        "toolName": "read_file",
                                        "toolCallId": "copilot-tool-read",
                                        "args": {"path": "src/lib.rs"},
                                    },
                                    "textEditGroup": {
                                        "uri": "src/lib.rs",
                                        "edits": [{"oldText": "old\n", "newText": "new\n"}],
                                    },
                                },
                            }
                        ],
                    }
                }
            ],
        )
        conn = open_fixture_db(state_db)
        try:
            conn.execute("CREATE TABLE ItemTable (key TEXT PRIMARY KEY, value TEXT)")
            conn.execute(
                "INSERT INTO ItemTable VALUES (?, ?)",
                (
                    "chat.ChatSessionStore.index",
                    json.dumps([{"id": "copilot-session"}], separators=(",", ":")),
                ),
            )
            conn.execute(
                "INSERT INTO ItemTable VALUES (?, ?)",
                (
                    "agentSessions.model.cache",
                    json.dumps(
                        {"sessions": [{"id": "agent-copilot", "stats": {"fileCount": 1, "added": 1, "removed": 1}}]},
                        separators=(",", ":"),
                    ),
                ),
            )
            conn.execute(
                "INSERT INTO ItemTable VALUES (?, ?)",
                (
                    "interactive.sessions",
                    json.dumps(
                        [
                            {
                                "sessionId": "copilot-session",
                                "providerId": "copilot",
                                "requesterUsername": "local",
                                "creationDate": ISO_TS,
                                "lastMessageDate": ISO_TS_1,
                                "requests": [
                                    {
                                        "message": {
                                            "text": "copilot prompt",
                                            "parts": [{"kind": "text", "text": "copilot prompt"}],
                                        },
                                        "response": [{"value": "copilot output"}],
                                        "result": {
                                            "metadata": {
                                                "sessionId": "copilot-session",
                                                "responseId": "copilot-response",
                                            }
                                        },
                                    }
                                ],
                            }
                        ],
                        separators=(",", ":"),
                    ),
                ),
            )
            conn.commit()
        finally:
            conn.close()
        return path

    if agent == "cline":
        write_vscode_task_fixture(
            root,
            "saoudrizwan.claude-dev",
            "cline",
            "claude-sonnet-4",
            "anthropic",
        )
        return write_cline_current_fixture(root)

    if agent == "roo-code":
        return write_vscode_task_fixture(root, "rooveterinaryinc.roo-cline", agent, "claude-sonnet-4", "anthropic")

    if agent == "kilocode":
        sqlite_path = write_kilo_sqlite_fixture(root, agent, "/tmp/project", "gpt-5", "openai")
        write_kilo_storage_json_fixture(root, agent, "gpt-5", "openai")
        write_vscode_task_fixture(root, "kilocode.kilo-code", agent, "gpt-5", "openai")
        return sqlite_path

    if agent == "kiro":
        # Official-style hook JSONL events provide monitoring fields only; no
        # token usage rows are inferred from Kiro hook events.
        path = root / ".kiro/hooks/kiro-hooks.jsonl"
        write_jsonl(
            path,
            [
                {
                    "hook_event_name": "UserPromptSubmit",
                    "timestamp": ISO_TS,
                    "session_id": "kiro-hook-session",
                    "workspace_roots": ["/tmp/project"],
                    "model": "claude-sonnet-4",
                    "provider": "kiro",
                    "account": "local",
                    "prompt": "kiro prompt",
                    "usage": {
                        "input_tokens": 70,
                        "output_tokens": 28,
                        "cache_read_tokens": 5,
                        "cache_write_tokens": 2,
                        "reasoning_tokens": 3,
                        "cost": 0.01,
                    },
                },
                {
                    "hook_event_name": "PreToolUse",
                    "timestamp": ISO_TS_1,
                    "session_id": "kiro-hook-session",
                    "tool_name": "read_file",
                    "tool_input": {"path": "src/lib.rs"},
                },
                {
                    "hook_event_name": "PreToolUse",
                    "timestamp": ISO_TS_1,
                    "session_id": "kiro-hook-session",
                    "tool_name": "apply_patch",
                    "tool_input": {"path": "src/lib.rs", "old": "old\n", "new": "new\n"},
                },
                {
                    "hook_event_name": "PostToolUse",
                    "timestamp": ISO_TS_1,
                    "session_id": "kiro-hook-session",
                    "tool_name": "read_file",
                    "tool_response": "file content",
                },
                {
                    "hook_event_name": "afterAgentResponse",
                    "timestamp": ISO_TS_1,
                    "session_id": "kiro-hook-session",
                    "assistant_output": "kiro output",
                },
                {
                    "hook_event_name": "afterFileEdit",
                    "timestamp": ISO_TS_1,
                    "session_id": "kiro-hook-session",
                    "file_path": "src/lib.rs",
                    "old_string": "old\n",
                    "new_string": "new\n",
                },
            ],
        )
        sqlite_path = root / "Library/Application Support/kiro-cli/data.sqlite3"
        conn = open_fixture_db(sqlite_path)
        try:
            conn.execute(
                "CREATE TABLE conversations_v2 (conversation_id TEXT, value TEXT, created_at INTEGER)"
            )
            conn.execute(
                "INSERT INTO conversations_v2 VALUES (?, ?, ?)",
                (
                    "kiro-conversation",
                    json.dumps(
                        {
                            "session_state": {
                                "rts_model_state": {
                                    "model_info": {"model_id": "kiro-state-model"}
                                }
                            },
                            "provider": "anthropic",
                            "account": "kiro-local",
                            "messages": [
                                {"role": "user", "content": "kiro conversation prompt"},
                                {
                                    "role": "assistant",
                                    "content": "kiro conversation output",
                                },
                                {
                                    "role": "tool_call",
                                    "name": "read_file",
                                    "id": "kiro-read-call",
                                    "input": {
                                        "path": "src/kiro.rs",
                                    },
                                },
                                {
                                    "role": "tool_result",
                                    "tool_call_id": "kiro-read-call",
                                    "content": "file content",
                                },
                                {
                                    "role": "tool_call",
                                    "name": "apply_patch",
                                    "id": "kiro-edit-call",
                                    "input": {
                                        "path": "src/kiro.rs",
                                        "old": "old\n",
                                        "new": "new\n",
                                    },
                                },
                                {
                                    "role": "tool_result",
                                    "tool_call_id": "kiro-edit-call",
                                    "content": "patch applied",
                                },
                            ],
                            "input_tokens": 33,
                            "output_tokens": 12,
                            "cost": 0.033,
                        },
                        separators=(",", ":"),
                    ),
                    1781589605000,
                ),
            )
            conn.commit()
        finally:
            conn.close()
        write_json(
            root / ".kiro/sessions/cli/kiro-session.json",
            {
                "id": "kiro-cli-session",
                "session_id": "kiro-cli-session",
                "provider": "anthropic",
                "model": "claude-sonnet-4",
                "account": "kiro-local",
                "workspace": "/tmp/project",
                "session_state": {
                    "rts_model_state": {
                        "model_info": {"model_id": "claude-sonnet-4"}
                    }
                },
                "messages": [
                    {
                        "type": "user",
                        "timestamp": ISO_TS,
                        "content": "kiro cli prompt",
                    },
                    {
                        "type": "assistant",
                        "timestamp": ISO_TS_1,
                        "content": "kiro cli output",
                        "usage": {
                            "input_tokens": 80,
                            "output_tokens": 32,
                            "cache_read_tokens": 8,
                            "cache_write_tokens": 2,
                            "reasoning_tokens": 5,
                            "cost": 0.025,
                        },
                    },
                    {
                        "type": "tool_call",
                        "timestamp": ISO_TS_1,
                        "id": "kiro-cli-read",
                        "name": "read_file",
                        "arguments": {"path": "src/lib.rs"},
                    },
                    {
                        "type": "tool_call",
                        "timestamp": ISO_TS_1,
                        "id": "kiro-cli-edit",
                        "name": "apply_patch",
                        "arguments": {
                            "path": "src/lib.rs",
                            "old": "old\n",
                            "new": "new\n",
                            "diff": "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@\n-old\n+new\n",
                        },
                    },
                    {
                        "type": "tool_result",
                        "timestamp": ISO_TS_1,
                        "tool_call_id": "kiro-cli-read",
                        "tool_result": "file content",
                    },
                ],
                "events": [
                    {
                        "type": "tool_result",
                        "timestamp": ISO_TS_1,
                        "tool_call_id": "kiro-cli-edit",
                        "tool_result": "patch applied",
                    },
                    {
                        "type": "tool_call",
                        "timestamp": ISO_TS_1,
                        "id": "kiro-cli-edit-info",
                        "name": "edit",
                        "arguments": {
                            "path": "src/lib.rs",
                            "old": "old\n",
                            "new": "new\n",
                        },
                    },
                ],
            },
        )
        return path

    if agent == "zed":
        path = root / ".local/share/zed/threads/threads.db"
        conn = open_fixture_db(path)
        try:
            conn.execute(
                "CREATE TABLE threads (id TEXT, updated_at TEXT, created_at TEXT, folder_paths TEXT, folder_paths_order TEXT, data_type TEXT, data BLOB)"
            )
            conn.execute(
                "INSERT INTO threads VALUES (?, ?, ?, ?, ?, ?, ?)",
                (
                    "zed-thread",
                    ISO_TS,
                    ISO_TS,
                    json.dumps(["/tmp/project"]),
                    json.dumps([0]),
                    "zstd",
                    ZED_ZSTD_FIXTURE,
                ),
            )
            conn.commit()
        finally:
            conn.close()
        return path

    if agent == "goose":
        path = root / ".local/share/goose/sessions/sessions.db"
        conn = open_fixture_db(path)
        try:
            conn.execute(
                "CREATE TABLE sessions (id TEXT, model_config_json TEXT, provider_name TEXT, created_at TEXT, total_tokens INTEGER, input_tokens INTEGER, output_tokens INTEGER, accumulated_total_tokens INTEGER, accumulated_input_tokens INTEGER, accumulated_output_tokens INTEGER)"
            )
            conn.execute(
                "CREATE TABLE messages (id TEXT, session_id TEXT, role TEXT, content_json TEXT, tokens INTEGER, created_at TEXT)"
            )
            conn.execute(
                "INSERT INTO sessions VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                (
                    "goose-session",
                    json.dumps({"model_name": "claude-sonnet-4"}),
                    "anthropic",
                    ISO_TS,
                    160,
                    100,
                    50,
                    160,
                    100,
                    50,
                ),
            )
            conn.execute(
                "INSERT INTO messages VALUES ('goose-user','goose-session','user',?,12,?)",
                (
                    json.dumps([{"type": "Text", "text": "goose prompt"}], separators=(",", ":")),
                    ISO_TS,
                ),
            )
            conn.execute(
                "INSERT INTO messages VALUES ('goose-assistant','goose-session','assistant',?,18,?)",
                (
                    json.dumps(
                        [
                            {"type": "Text", "text": "goose output"},
                            {"type": "thinking", "thinking": "goose reasoning"},
                            {
                                "type": "ToolRequest",
                                "id": "tool-1",
                                "tool_call": {"name": "read_file", "arguments": {"path": "src/lib.rs"}},
                            },
                            {
                                "type": "ToolRequest",
                                "id": "tool-edit",
                                "tool_call": {"name": "apply_patch", "arguments": {"path": "src/lib.rs", "old": "old\n", "new": "new\n"}},
                            },
                        ],
                        separators=(",", ":"),
                    ),
                    ISO_TS_1,
                ),
            )
            conn.execute(
                "INSERT INTO messages VALUES ('goose-tool','goose-session','tool',?,3,?)",
                (
                    json.dumps(
                        [{"type": "ToolResponse", "tool_call_id": "tool-1", "content": "file content"}],
                        separators=(",", ":"),
                    ),
                    ISO_TS_1,
                ),
            )
            conn.commit()
        finally:
            conn.close()
        return path

    if agent == "amp":
        # Amp first-party evidence covers --stream-json runtime stdout events here, not
        # an official stable local persisted thread/history file path or schema.
        path = root / ".amp/sessions/session-amp.jsonl"
        write_jsonl(
            path,
            [
                {
                    "type": "system",
                    "subtype": "init",
                    "session_id": "amp-session",
                    "cwd": "/tmp/project",
                    "model": "amp-model",
                    "provider": "amp",
                    "tools": ["read_file", "apply_patch"],
                },
                {
                    "type": "user",
                    "session_id": "amp-session",
                    "message": {"role": "user", "content": [{"type": "text", "text": "amp prompt"}]},
                    "timestamp": ISO_TS,
                },
                {
                    "type": "assistant",
                    "session_id": "amp-session",
                    "message": {
                        "role": "assistant",
                        "content": [
                            {"type": "text", "text": "amp output"},
                            {"type": "tool_use", "id": "tool-1", "name": "read_file", "input": {"path": "src/lib.rs"}},
                            {
                                "type": "tool_use",
                                "id": "tool-edit",
                                "name": "apply_patch",
                                "input": {"path": "src/lib.rs", "old": "old\n", "new": "new\n"},
                            },
                        ],
                        "usage": {
                            "input_tokens": 100,
                            "output_tokens": 45,
                            "cache_read_input_tokens": 8,
                            "cache_creation_input_tokens": 4,
                            "reasoning_tokens": 6,
                            "cost": 0.019,
                        },
                        "usage_basis": usage_fixture_expected_usage_basis(agent, "threads-jsonl"),
                        "account": "local",
                    },
                    "timestamp": ISO_TS_1,
                },
                {
                    "type": "user",
                    "session_id": "amp-session",
                    "message": {
                        "role": "user",
                        "content": [{"type": "tool_result", "tool_use_id": "tool-1", "content": "file content"}],
                    },
                    "timestamp": ISO_TS_1,
                },
                {
                    "type": "result",
                    "session_id": "amp-session",
                    "result": "amp output",
                    "usage": {
                        "input_tokens": 100,
                        "output_tokens": 45,
                        "cache_read_input_tokens": 8,
                        "cache_creation_input_tokens": 4,
                        "reasoning_tokens": 6,
                        "cost": 0.019,
                    },
                    "usage_basis": usage_fixture_expected_usage_basis(agent, "threads-jsonl"),
                    "account": "local",
                    "timestamp": ISO_TS_1,
                },
            ],
        )
        return path

    if agent == "droid":
        path = root / ".factory/sessions/session-droid.settings.json"
        write_json(
            path,
            {
                "sessionId": "droid-session",
                "model": "claude-sonnet-4",
                "tokenUsage": {
                    "inputTokens": 100,
                    "outputTokens": 40,
                    "cacheReadTokens": 7,
                    "cacheCreationTokens": 3,
                    "thinkingTokens": 5,
                },
            },
        )
        write_jsonl(
            root / ".factory/sessions/session-droid.jsonl",
            [
                {
                    "type": "session_start",
                    "sessionId": "droid-session",
                    "cwd": "/tmp/project",
                    "timestamp": ISO_TS,
                },
                {
                    "jsonrpc": "2.0",
                    "method": "droid.session_notification",
                    "params": {
                        "sessionId": "droid-session",
                        "notification": {
                            "type": "message",
                            "message": {
                                "role": "user",
                                "content": [{"type": "text", "text": "droid prompt"}],
                            },
                        },
                    },
                    "timestamp": ISO_TS,
                },
                {
                    "jsonrpc": "2.0",
                    "method": "droid.session_notification",
                    "params": {
                        "sessionId": "droid-session",
                        "notification": {
                            "type": "message",
                            "message": {
                                "role": "assistant",
                                "content": [
                                    {"type": "text", "text": "droid output"},
                                    {"type": "tool_use", "id": "tool-1", "name": "read_file", "input": {"path": "src/lib.rs"}},
                                    {
                                        "type": "tool_use",
                                        "id": "tool-edit",
                                        "name": "apply_patch",
                                        "input": {"path": "src/lib.rs", "old": "old\n", "new": "new\n"},
                                    },
                                ],
                            },
                        },
                    },
                    "timestamp": ISO_TS_1,
                },
                {
                    "jsonrpc": "2.0",
                    "method": "droid.session_notification",
                    "params": {
                        "sessionId": "droid-session",
                        "notification": {
                            "type": "message",
                            "message": {
                                "role": "user",
                                "content": [{"type": "tool_result", "tool_use_id": "tool-1", "content": "file content"}],
                            },
                        },
                    },
                    "timestamp": ISO_TS_1,
                },
                {
                    "jsonrpc": "2.0",
                    "method": "droid.session_notification",
                    "params": {
                        "sessionId": "droid-session",
                        "notification": {
                            "type": "token_usage_update",
                            "tokenUsage": {
                                "inputTokens": 100,
                                "outputTokens": 40,
                                "cacheReadTokens": 7,
                                "cacheCreationTokens": 3,
                                "thinkingTokens": 5,
                            },
                        },
                    },
                    "timestamp": ISO_TS_1,
                },
            ],
        )
        return path

    if agent == "pi":
        path = root / ".pi/agent/sessions/session-pi.jsonl"
        write_jsonl(
            path,
            [
                {
                    "type": "session",
                    "version": 3,
                    "id": "pi-session",
                    "timestamp": ISO_TS,
                    "cwd": "/tmp/project",
                },
                {
                    "type": "model_change",
                    "id": "pi-model-entry",
                    "parentId": None,
                    "timestamp": ISO_TS,
                    "provider": "openai",
                    "modelId": "gpt-5",
                },
                {
                    "type": "message",
                    "id": "pi-user-entry",
                    "parentId": "pi-model-entry",
                    "timestamp": ISO_TS,
                    "message": {"role": "user", "content": "pi prompt", "timestamp": EPOCH_MS},
                },
                {
                    "type": "message",
                    "id": "pi-assistant-entry",
                    "parentId": "pi-user-entry",
                    "timestamp": ISO_TS_1,
                    "message": {
                        "role": "assistant",
                        "content": [
                            {"type": "text", "text": "pi output"},
                            {"type": "thinking", "thinking": "pi reasoning"},
                            {
                                "type": "toolCall",
                                "id": "tool-1",
                                "name": "read_file",
                                "arguments": {"path": "src/lib.rs"},
                            },
                            {
                                "type": "toolCall",
                                "id": "tool-edit",
                                "name": "apply_patch",
                                "arguments": {"path": "src/lib.rs", "old": "old\n", "new": "new\n"},
                            },
                        ],
                        "model": "gpt-5",
                        "provider": "openai",
                        "timestamp": EPOCH_MS,
                        "usage": {
                            "input": 100,
                            "output": 40,
                            "cacheRead": 6,
                            "cacheWrite": 2,
                            "cost": {"total": 0.031},
                        },
                    },
                },
                {
                    "type": "message",
                    "id": "pi-tool-entry",
                    "parentId": "pi-assistant-entry",
                    "timestamp": ISO_TS_1,
                    "message": {
                        "role": "toolResult",
                        "toolCallId": "tool-1",
                        "toolName": "read_file",
                        "content": [{"type": "text", "text": "file content"}],
                        "isError": False,
                        "timestamp": EPOCH_MS,
                    },
                },
            ],
        )
        return path

    if agent == "mux":
        session_dir = root / ".mux/sessions/workspace-one"
        path = session_dir / "session-usage.json"
        write_json(
            path,
            {
                "version": 1,
                "byModel": {
                    "anthropic:claude-opus-4-6": {
                        "input": {"tokens": 100, "cost_usd": 0.01},
                        "cached": {"tokens": 50, "cost_usd": 0.005},
                        "cacheCreate": {"tokens": 20, "cost_usd": 0.002},
                        "output": {"tokens": 30, "cost_usd": 0.003},
                        "reasoning": {"tokens": 7, "cost_usd": 0.001},
                    }
                },
                "lastRequest": {"timestamp": EPOCH_MS},
            },
        )
        write_jsonl(
            session_dir / "chat.jsonl",
            [
                {
                    "id": "mux-user",
                    "sessionId": "mux-session",
                    "role": "user",
                    "createdAt": EPOCH_MS,
                    "parts": [{"type": "text", "text": "mux prompt"}],
                    "metadata": {
                        "cwd": "/tmp/mux-project",
                        "workspace": "workspace-one",
                        "model": "claude-opus-4-6",
                        "routeProvider": "anthropic",
                    },
                },
                {
                    "id": "mux-assistant",
                    "sessionId": "mux-session",
                    "role": "assistant",
                    "createdAt": EPOCH_MS,
                    "parts": [
                        {"type": "text", "text": "mux output"},
                        {"type": "reasoning", "text": "mux reasoning"},
                        {"type": "file", "path": "src/lib.rs", "text": "fn main() {}"},
                        {
                            "type": "dynamic-tool",
                            "state": "output-available",
                            "toolCallId": "tool-1",
                            "toolName": "read_file",
                            "input": {"path": "src/lib.rs"},
                            "output": "file content",
                        },
                        {
                            "type": "dynamic-tool",
                            "state": "output-available",
                            "toolCallId": "tool-edit",
                            "toolName": "apply_patch",
                            "input": {"path": "src/lib.rs", "old": "old\n", "new": "new\n"},
                            "output": "patched",
                        },
                    ],
                    "metadata": {
                        "cwd": "/tmp/mux-project",
                        "workspace": "workspace-one",
                        "model": "claude-opus-4-6",
                        "routeProvider": "anthropic",
                        "usage": {"inputTokens": 100, "outputTokens": 30, "reasoningTokens": 7},
                    },
                },
            ],
        )
        write_jsonl(
            session_dir / "chat-archive.jsonl",
            [
                {
                    "id": "mux-archive-assistant",
                    "sessionId": "mux-session",
                    "role": "assistant",
                    "createdAt": EPOCH_MS,
                    "parts": [{"type": "text", "text": "mux archived output"}],
                    "metadata": {
                        "cwd": "/tmp/mux-project",
                        "workspace": "workspace-one",
                        "model": "claude-opus-4-6",
                        "routeProvider": "anthropic",
                    },
                }
            ],
        )
        write_json(
            session_dir / "partial.json",
            {
                "id": "mux-partial-assistant",
                "sessionId": "mux-session",
                "role": "assistant",
                "createdAt": EPOCH_MS,
                "parts": [{"type": "text", "text": "mux partial output"}],
                "metadata": {
                    "cwd": "/tmp/mux-project",
                    "workspace": "workspace-one",
                    "model": "claude-opus-4-6",
                    "routeProvider": "anthropic",
                },
            },
        )
        return path

    if agent == "crush":
        path = root / ".crush/crush.db"
        conn = open_fixture_db(path)
        try:
            conn.executescript(
                "CREATE TABLE sessions (id TEXT PRIMARY KEY, parent_session_id TEXT, title TEXT, message_count INTEGER, prompt_tokens INTEGER, completion_tokens INTEGER, cache_read_tokens INTEGER, cache_write_tokens INTEGER, reasoning_tokens INTEGER, cost REAL, updated_at INTEGER, created_at INTEGER);"
                "CREATE TABLE messages (id TEXT PRIMARY KEY, session_id TEXT, role TEXT, parts TEXT, model TEXT, provider TEXT, is_summary_message INTEGER, created_at INTEGER, updated_at INTEGER, finished_at INTEGER);"
                "CREATE TABLE files (id TEXT PRIMARY KEY, session_id TEXT, path TEXT, content TEXT, version INTEGER, created_at INTEGER, updated_at INTEGER);"
                "CREATE TABLE read_files (session_id TEXT, path TEXT, read_at INTEGER, PRIMARY KEY(path, session_id));"
            )
            conn.execute("INSERT INTO sessions VALUES ('crush-session', NULL, 'Checkout task', 2, 100, 45, 8, 3, 6, 0.25, ?, ?)", (EPOCH_S, EPOCH_S))
            conn.execute(
                "INSERT INTO messages VALUES ('crush-user', 'crush-session', 'user', ?, 'gpt-5', 'openai', 0, ?, ?, NULL)",
                (json.dumps([{"type": "text", "data": {"text": "crush prompt"}}], separators=(",", ":")), EPOCH_S, EPOCH_S),
            )
            conn.execute(
                "INSERT INTO messages VALUES ('crush-msg', 'crush-session', 'assistant', ?, 'gpt-5', 'openai', 0, ?, ?, ?)",
                (
                    json.dumps(
                        [
                            {"type": "text", "data": {"text": "crush output"}},
                            {"type": "reasoning", "data": {"text": "crush reasoning"}},
                            {
                                "type": "tool_call",
                                "data": {"id": "tool-1", "name": "read_file", "input": "{\"path\":\"src/lib.rs\"}"},
                            },
                            {
                                "type": "tool_call",
                                "data": {"id": "tool-edit", "name": "apply_patch", "input": "{\"path\":\"src/lib.rs\",\"old\":\"old\\n\",\"new\":\"new\\n\"}"},
                            },
                            {"type": "tool_result", "data": {"tool_call_id": "tool-1", "name": "read_file", "content": "file content"}},
                        ],
                        separators=(",", ":"),
                    ),
                    EPOCH_S,
                    EPOCH_S,
                    EPOCH_S,
                ),
            )
            conn.execute("INSERT INTO files VALUES ('crush-file', 'crush-session', 'src/crush-file.rs', 'file content', 1, ?, ?)", (EPOCH_S, EPOCH_S))
            conn.execute("INSERT INTO read_files VALUES ('crush-session', 'src/crush-read.rs', ?)", (EPOCH_S,))
            conn.commit()
        finally:
            conn.close()
        return path

    if agent == "codebuff":
        path = root / ".config/manicode/projects/project-one/chats/2026-06-16T14-00-00.000Z/chat-messages.json"
        write_json(
            path.parent / "run-state.json",
            {
                "traceSessionId": "codebuff-session",
                "sessionState": {
                    "mainAgentState": {
                        "creditsUsed": 0.42,
                        "directCreditsUsed": 0.31,
                        "fileContext": {
                            "cwd": "/tmp/project",
                            "projectRoot": "/tmp/project",
                        },
                    }
                },
                "output": {"type": "success", "message": "codebuff output"},
            },
        )
        write_json(
            path,
            [
                {
                    "id": "codebuff-user",
                    "variant": "user",
                    "timestamp": ISO_TS,
                    "content": "codebuff prompt",
                },
                {
                    "id": "codebuff-msg",
                    "variant": "ai",
                    "timestamp": ISO_TS,
                    "content": "codebuff output",
                    "blocks": [
                        {
                            "type": "tool",
                            "toolCallId": "tool-1",
                            "toolName": "read_file",
                            "input": {"path": "src/lib.rs"},
                            "output": "file content",
                        },
                        {
                            "type": "tool",
                            "toolCallId": "tool-edit",
                            "toolName": "apply_patch",
                            "input": {"path": "src/lib.rs", "old": "old\n", "new": "new\n"},
                            "outputRaw": {"ok": True},
                        },
                    ],
                },
                {
                    "id": "codebuff-run-state-only",
                    "variant": "ai",
                    "timestamp": ISO_TS_1,
                    "content": "codebuff run-state credits output",
                    "blocks": [
                        {
                            "type": "text",
                            "content": "codebuff run-state credits block",
                        },
                    ],
                },
            ],
        )
        return path

    if agent == "kilo":
        sqlite_path = write_kilo_sqlite_fixture(root, agent, "/tmp/project", "gpt-5", "openai")
        write_kilo_storage_json_fixture(root, agent, "gpt-5", "openai")
        return sqlite_path

    if agent == "kimi":
        path = root / ".kimi-code/sessions/project/kimi-code-session/agents/main/wire.jsonl"
        write_jsonl(
            root / ".kimi-code/session_index.jsonl",
            [
                {
                    "sessionId": "kimi-code-session",
                    "workDirKey": "project",
                    "cwd": "/tmp/project",
                    "updatedAt": ISO_TS_1,
                }
            ],
        )
        write_json(
            root / ".kimi-code/sessions/project/kimi-code-session/state.json",
            {
                "sessionId": "kimi-code-session",
                "cwd": "/tmp/project",
                "model": "kimi-k2",
                "provider": "kimi",
            },
        )
        write_jsonl(
            path,
            [
                {
                    "jsonrpc": "2.0",
                    "method": "event",
                    "params": {
                        "type": "TurnBegin",
                        "payload": {"user_input": "kimi prompt"},
                    },
                    "timestamp": ISO_TS,
                },
                {
                    "jsonrpc": "2.0",
                    "method": "event",
                    "params": {
                        "type": "ContentPart",
                        "payload": {"text": "kimi output"},
                    },
                    "timestamp": ISO_TS_1,
                },
                {
                    "jsonrpc": "2.0",
                    "id": "tool-1",
                    "method": "request",
                    "params": {
                        "type": "ToolCallRequest",
                        "payload": {"tool_call_id": "tool-1", "name": "read_file", "arguments": {"path": "src/lib.rs"}},
                    },
                    "timestamp": ISO_TS_1,
                },
                {
                    "jsonrpc": "2.0",
                    "id": "tool-edit",
                    "method": "request",
                    "params": {
                        "type": "ToolCallRequest",
                        "payload": {
                            "tool_call_id": "tool-edit",
                            "name": "apply_patch",
                            "arguments": {"path": "src/lib.rs", "old": "old\n", "new": "new\n"},
                        },
                    },
                    "timestamp": ISO_TS_1,
                },
                {
                    "jsonrpc": "2.0",
                    "id": "tool-1",
                    "result": {"tool_call_id": "tool-1", "return_value": "file content"},
                    "timestamp": ISO_TS_1,
                },
                {
                    "jsonrpc": "2.0",
                    "method": "event",
                    "params": {
                        "type": "StatusUpdate",
                        "payload": {
                            "token_usage": {
                                "input_other": 100,
                                "output": 45,
                                "input_cache_read": 10,
                                "input_cache_creation": 5,
                            }
                        },
                    },
                    "timestamp": ISO_TS_1,
                }
            ],
        )
        write_jsonl(
            root / ".kimi-code/sessions/project/kimi-code-session/agents/agent-1/wire.jsonl",
            [
                {
                    "jsonrpc": "2.0",
                    "method": "event",
                    "params": {"type": "ContentPart", "payload": {"text": "kimi subagent output"}},
                    "timestamp": ISO_TS_1,
                }
            ],
        )
        write_json(
            root / ".kimi/sessions/group/session/state.json",
            {
                "sessionId": "kimi-session",
                "cwd": "/tmp/legacy-project",
                "model": "kimi-k2",
                "provider": "kimi",
            },
        )
        write_jsonl(
            root / ".kimi/sessions/group/session/wire.jsonl",
            [
                {
                    "type": "TurnBegin",
                    "timestamp": ISO_TS,
                    "sessionId": "kimi-session",
                    "payload": {"user_input": "kimi legacy prompt"},
                }
            ],
        )
        return path

    if agent == "gjc":
        rows = [
            {
                "type": "session",
                "version": 3,
                "id": "gjc-session",
                "timestamp": ISO_TS,
                "cwd": "/tmp/project",
                "title": "GJC session",
                "titleSource": "user",
            },
            {
                "type": "message",
                "id": "gjc-msg",
                "message": {
                    "role": "user",
                    "content": "gjc prompt",
                    "timestamp": ISO_TS,
                },
            },
            {
                "type": "message",
                "id": "gjc-assistant",
                "message": {
                    "role": "assistant",
                    "content": [
                        {"type": "text", "text": "gjc output"},
                        {
                            "type": "toolCall",
                            "id": "tool-1",
                            "name": "read_file",
                            "arguments": {"path": "src/lib.rs"},
                        },
                        {
                            "type": "toolCall",
                            "id": "tool-edit",
                            "name": "apply_patch",
                            "arguments": {"path": "src/lib.rs", "old": "old\n", "new": "new\n"},
                        },
                    ],
                    "model": "gpt-5",
                    "provider": "openai",
                    "timestamp": EPOCH_MS,
                    "contextUsage": {"tokens": 9999},
                    "usage": {
                        "input": 100,
                        "output": 50,
                        "cacheRead": 7,
                        "cacheWrite": 3,
                        "cost": {"total": 0.02},
                    },
                },
            },
            {
                "type": "message",
                "id": "gjc-tool",
                "message": {
                    "role": "toolResult",
                    "toolCallId": "tool-1",
                    "toolName": "read_file",
                    "content": "file content",
                    "details": {"path": "src/lib.rs"},
                    "isError": False,
                    "timestamp": ISO_TS_1,
                },
            },
        ]
        path = root / ".gjc/agent/sessions/tmp-project/20260616T140000_gjc-session.jsonl"
        write_jsonl(path, rows)
        write_jsonl(root / ".gjc/_session-gjc-session/state/audit.jsonl", rows)
        return path

    if agent == "grok":
        path = root / ".grok/sessions/workspace/grok-session/events.jsonl"
        write_jsonl(
            path,
            [
                {
                    "method": "session/update",
                    "sessionId": "grok-session",
                    "timestamp": ISO_TS,
                    "params": {
                        "update": {
                            "sessionUpdate": "user_message",
                            "content": {"text": "grok prompt"},
                            "model": "grok-build-0.1",
                            "provider": "xai",
                        }
                    },
                },
                {
                    "method": "session/update",
                    "sessionId": "grok-session",
                    "timestamp": ISO_TS_1,
                    "params": {
                        "update": {
                            "sessionUpdate": "agent_message_chunk",
                            "content": {"type": "text", "text": "grok output"},
                        }
                    },
                },
                {
                    "method": "session/update",
                    "sessionId": "grok-session",
                    "timestamp": ISO_TS_1,
                    "params": {
                        "update": {
                            "sessionUpdate": "tool_call",
                            "toolCallId": "tool-edit",
                            "name": "edit",
                            "rawInput": {
                                "filePath": "src/lib.rs",
                                "search": "old\n",
                                "replace": "new\n",
                            },
                            "locations": [{"path": "src/lib.rs", "line": 1}],
                            "status": "started",
                            "meta": {"source": "acp"},
                        }
                    },
                },
                {
                    "method": "session/update",
                    "sessionId": "grok-session",
                    "timestamp": ISO_TS_1,
                    "params": {
                        "update": {
                            "sessionUpdate": "tool_call",
                            "toolCallId": "tool-1",
                            "name": "read_file",
                            "rawInput": {"filePath": "src/lib.rs"},
                            "locations": [{"path": "src/lib.rs", "line": 1}],
                            "status": "started",
                            "meta": {"source": "acp"},
                        }
                    },
                },
                {
                    "method": "session/update",
                    "sessionId": "grok-session",
                    "timestamp": ISO_TS_1,
                    "params": {
                        "update": {
                            "sessionUpdate": "tool_result",
                            "toolCallId": "tool-1",
                            "rawOutput": "file content",
                            "content": [{"type": "text", "text": "file content"}],
                            "status": "completed",
                            "meta": {"source": "acp"},
                        }
                    },
                },
                {
                    "method": "session/update",
                    "sessionId": "grok-session",
                    "timestamp": ISO_TS_1,
                    "params": {
                        "update": {
                            "sessionUpdate": "usage_update",
                            "model": "grok-build-0.1",
                            "provider": "xai",
                            "account": "local",
                            "usage_basis": usage_fixture_expected_usage_basis(agent, "sessions-jsonl"),
                            "tokens": {
                                "input_tokens": 105,
                                "output_tokens": 41,
                                "cache_read_tokens": 9,
                                "cache_write_tokens": 2,
                                "reasoning_tokens": 7,
                            },
                            "cost": 0.018,
                        }
                    },
                },
            ],
        )
        return path

    if agent == "synthetic":
        path = root / ".local/share/octofriend/sqlite.db"
        conn = open_fixture_db(path)
        try:
            conn.execute(
                "CREATE TABLE trees (id INTEGER PRIMARY KEY, name TEXT NOT NULL, cwd TEXT NOT NULL, updated_at INTEGER NOT NULL)"
            )
            conn.execute(
                "CREATE TABLE launches (id INTEGER PRIMARY KEY, dockerLaunchId INTEGER, localLaunchId INTEGER)"
            )
            conn.execute("CREATE TABLE llm_irs (id INTEGER PRIMARY KEY, json TEXT NOT NULL)")
            conn.execute("CREATE TABLE history_items (id INTEGER PRIMARY KEY, llm_ir_id INTEGER)")
            conn.execute(
                "CREATE TABLE tree_nodes (id INTEGER PRIMARY KEY, history_item_id INTEGER NOT NULL, tree_id INTEGER NOT NULL, parent_id INTEGER, is_leaf INTEGER NOT NULL, launch_id INTEGER NOT NULL)"
            )
            conn.execute(
                "INSERT INTO trees VALUES (?, ?, ?, ?)",
                (1, "synthetic-tree", "/tmp/project", EPOCH_MS),
            )
            conn.execute("INSERT INTO launches VALUES (?, ?, ?)", (1, None, 1))
            ir_rows = [
                {
                    "role": "user",
                    "content": [{"type": "text", "content": "synthetic prompt"}],
                },
                {
                    "role": "assistant",
                    "model": "claude-sonnet-4",
                    "provider": "anthropic",
                    "sourceCost": 0.02,
                    "content": "synthetic output",
                    "usage": {
                        "input": {"cached": 7, "uncached": 93, "total": 100},
                        "output": 40,
                    },
                    "toolCalls": [
                        {
                            "type": "tool-call",
                            "name": "read_file",
                            "toolCallId": "tool-1",
                            "parsed": {"filePath": "src/lib.rs"},
                            "original": {"filePath": "src/lib.rs"},
                        },
                        {
                            "type": "tool-call",
                            "name": "edit",
                            "toolCallId": "tool-edit",
                            "parsed": {
                                "filePath": "src/lib.rs",
                                "search": "old\n",
                                "replace": "new\n",
                            },
                            "original": {
                                "filePath": "src/lib.rs",
                                "search": "old\n",
                                "replace": "new\n",
                            },
                        },
                        {
                            "type": "tool-call",
                            "name": "rewrite",
                            "toolCallId": "tool-rewrite",
                            "parsed": {"filePath": "src/rewrite.rs", "text": "rewritten\n"},
                            "original": {"filePath": "src/rewrite.rs", "text": "rewritten\n"},
                        },
                        {
                            "type": "tool-call",
                            "name": "create",
                            "toolCallId": "tool-create",
                            "parsed": {"filePath": "src/new.rs", "content": "created\n"},
                            "original": {"filePath": "src/new.rs", "content": "created\n"},
                        },
                    ],
                },
                {
                    "role": "tool-output",
                    "toolCall": {
                        "type": "tool-call",
                        "name": "read_file",
                        "toolCallId": "tool-1",
                        "parsed": {"filePath": "src/lib.rs"},
                        "original": {"filePath": "src/lib.rs"},
                    },
                    "content": [{"type": "text", "content": "file content"}],
                },
                {
                    "role": "tool-runtime-error",
                    "toolCall": {
                        "type": "tool-call",
                        "name": "shell",
                        "toolCallId": "tool-runtime",
                        "parsed": {"cmd": "false"},
                        "original": {"cmd": "false"},
                    },
                    "error": "runtime failed",
                },
                {
                    "role": "tool-validation-error",
                    "toolCall": {
                        "type": "tool-call",
                        "name": "edit",
                        "toolCallId": "tool-invalid",
                        "parsed": {
                            "filePath": "src/lib.rs",
                            "search": "missing\n",
                            "replace": "new\n",
                        },
                        "original": {
                            "filePath": "src/lib.rs",
                            "search": "missing\n",
                            "replace": "new\n",
                        },
                    },
                    "error": "validation failed",
                    "aborted": False,
                },
                {
                    "role": "tool-skip-output",
                    "toolCall": {
                        "type": "tool-call",
                        "name": "create",
                        "toolCallId": "tool-skip",
                        "parsed": {"filePath": "src/skip.rs", "content": "skip\n"},
                        "original": {"filePath": "src/skip.rs", "content": "skip\n"},
                    },
                    "reason": "skipped after validation",
                },
                {
                    "role": "tool-invoke-subagent",
                    "toolCall": {
                        "type": "tool-call",
                        "name": "dispatch_subagent",
                        "toolCallId": "tool-subagent",
                        "parsed": {"task": "inspect repo"},
                        "original": {"task": "inspect repo"},
                    },
                    "subagent": "research",
                },
            ]
            for idx, ir in enumerate(ir_rows, start=1):
                conn.execute(
                    "INSERT INTO llm_irs VALUES (?, ?)",
                    (idx, json.dumps(ir, separators=(",", ":"))),
                )
                conn.execute("INSERT INTO history_items VALUES (?, ?)", (idx, idx))
                conn.execute(
                    "INSERT INTO tree_nodes VALUES (?, ?, ?, ?, ?, ?)",
                    (idx, idx, 1, idx - 1 if idx > 1 else None, 1 if idx == len(ir_rows) else 0, 1),
                )
            conn.commit()
        finally:
            conn.close()
        return path

    if agent == "warp":
        path = root / ".warp/warp.sqlite"
        conn = open_fixture_db(path)
        try:
            conn.execute(
                """CREATE TABLE agent_conversations (
                    id INTEGER PRIMARY KEY NOT NULL,
                    conversation_id TEXT NOT NULL,
                    active_task_id TEXT,
                    conversation_data TEXT NOT NULL,
                    last_modified_at TIMESTAMP NOT NULL
                )"""
            )
            conn.execute(
                """CREATE TABLE agent_tasks (
                    id INTEGER PRIMARY KEY NOT NULL,
                    conversation_id TEXT NOT NULL,
                    task_id TEXT NOT NULL,
                    task BLOB NOT NULL,
                    last_modified_at TIMESTAMP NOT NULL
                )"""
            )
            conn.execute(
                "INSERT INTO agent_conversations VALUES (?, ?, ?, ?, ?)",
                (
                    1,
                    "warp-conversation",
                    "warp-task",
                    json.dumps(
                        {
                            "conversation_usage_metadata": {
                                "credits_spent": 0.02,
                                "token_usage": [
                                    {
                                        "model_id": "claude-sonnet-4",
                                        "warp_tokens": 140,
                                        "warp_token_usage_by_category": {"primary_agent": 140},
                                    }
                                ],
                            }
                        },
                        separators=(",", ":"),
                    ),
                    ISO_TS,
                ),
            )
            conn.execute(
                "INSERT INTO agent_tasks VALUES (?, ?, ?, ?, ?)",
                (1, "warp-conversation", "warp-task", warp_task_proto(), ISO_TS_1),
            )
            conn.commit()
        finally:
            conn.close()
        return path

    if agent == "zcode":
        path = root / ".zcode/projects/project-zcode/session-zcode.jsonl"
        write_jsonl(
            path,
            [
                {
                    "type": "session",
                    "sessionId": "zcode-session",
                    "cwd": "/tmp/project",
                    "timestamp": ISO_TS,
                    "model": "glm-5.2",
                    "provider": "zhipu",
                },
                {
                    "type": "message",
                    "role": "user",
                    "sessionId": "zcode-session",
                    "timestamp": ISO_TS,
                    "content": "zcode prompt",
                },
                {
                    "type": "message",
                    "role": "assistant",
                    "sessionId": "zcode-session",
                    "timestamp": ISO_TS_1,
                    "content": "zcode output",
                    "usage_basis": usage_fixture_expected_usage_basis(agent, "projects-jsonl"),
                    "usage": {
                        "input_tokens": 100,
                        "output_tokens": 42,
                        "cache_read_tokens": 9,
                        "cache_write_tokens": 3,
                        "reasoning_tokens": 6,
                        "cost": 0.017,
                    },
                    "account": "local",
                    "tool_calls": [
                        {"id": "tool-1", "name": "read_file", "arguments": {"path": "src/lib.rs"}},
                        {
                            "id": "tool-edit",
                            "name": "apply_patch",
                            "arguments": {"path": "src/lib.rs", "old": "old\n", "new": "new\n"},
                        },
                    ],
                },
                {
                    "type": "tool_result",
                    "sessionId": "zcode-session",
                    "timestamp": ISO_TS_1,
                    "tool_call_id": "tool-1",
                    "tool_result": "file content",
                },
            ],
        )
        return path

    raise SystemExit(f"ERROR: no native usage fixture for {agent}")


def fixture_file_paths(root: Path) -> list[Path]:
    return sorted(path for path in root.rglob("*") if path.is_file())


def self_check_expected_sources() -> None:
    failures: list[str] = []
    checked = 0
    with tempfile.TemporaryDirectory(prefix="aitrack-local-usage-matrix-") as tmp:
        temp_root = Path(tmp)
        for agent, expected_rows in EXPECTED_SOURCES.items():
            root = temp_root / agent
            write_agent_fixture(root, agent)
            normalize_fixture_edit_paths(root, agent)
            fixture_paths = fixture_file_paths(root)
            fixture_path_strings = [str(path) for path in fixture_paths]
            fixture_path_display = [str(path.relative_to(root)) for path in fixture_paths]
            shared_path_hits = shared_edit_fixture_path_hits(root)
            if shared_path_hits:
                failures.append(
                    f"{agent}: shared edit fixture paths must be normalized; hits={shared_path_hits[:8]}"
                )

            for row in expected_rows:
                checked += 1
                source_name = f"{agent}/{row['label']}"
                path_substring = row["path_substring"]
                if not any(path_substring in path for path in fixture_path_strings):
                    candidates = [
                        path
                        for path in fixture_path_display
                        if agent in path or row["label"].split("-")[0] in path
                    ]
                    displayed = candidates[:8] or fixture_path_display[:8]
                    failures.append(
                        f"{source_name}: path_substring {path_substring!r} did not match "
                        f"generated fixture files; candidates={displayed}"
                    )

                required_usage_fields = row.get("required_usage_fields")
                if not isinstance(required_usage_fields, list) or not all(
                    isinstance(field, str) for field in required_usage_fields
                ):
                    failures.append(f"{source_name}: required_usage_fields must be a string list")
                elif row["expects_usage"] and not required_usage_fields:
                    failures.append(
                        f"{source_name}: expects_usage requires non-empty required_usage_fields"
                    )
                elif not row["expects_usage"] and required_usage_fields:
                    failures.append(
                        f"{source_name}: non-usage source should not declare required_usage_fields "
                        f"{required_usage_fields}"
                    )
                if row["expects_usage"]:
                    usage_basis = row.get("usage_basis")
                    if usage_basis not in {"native", "local_derived"}:
                        failures.append(
                            f"{source_name}: expects_usage requires source-level usage_basis"
                        )
                    elif usage_basis != usage_source_expected_usage_basis(agent, row["label"]):
                        failures.append(
                            f"{source_name}: usage_basis {usage_basis!r} does not match source label"
                        )
                    for field in ["message_count", "usage_basis", "day"]:
                        if field not in required_usage_fields:
                            failures.append(
                                f"{source_name}: expects_usage requires {field} in required_usage_fields"
                            )

                required_record_fields = row.get("required_record_fields")
                if not isinstance(required_record_fields, list) or not all(
                    isinstance(field, str) for field in required_record_fields
                ):
                    failures.append(f"{source_name}: required_record_fields must be a string list")
                elif row["expects_monitoring"] and not required_record_fields:
                    failures.append(
                        f"{source_name}: expects_monitoring requires non-empty required_record_fields"
                    )
                elif not row["expects_monitoring"] and required_record_fields:
                    failures.append(
                        f"{source_name}: non-monitoring source should not declare required_record_fields "
                        f"{required_record_fields}"
                    )
                if row["expects_monitoring"] and "timestamp_ms" not in required_record_fields:
                    failures.append(
                        f"{source_name}: expects_monitoring requires timestamp_ms in required_record_fields"
                    )

                required_event_types = row.get("required_event_types")
                if row["expects_monitoring"]:
                    if not isinstance(required_event_types, list) or not all(
                        isinstance(event_type, str) for event_type in required_event_types
                    ):
                        failures.append(
                            f"{source_name}: expects_monitoring requires required_event_types string list"
                        )
                    elif not required_event_types:
                        failures.append(
                            f"{source_name}: expects_monitoring requires non-empty required_event_types"
                        )
                elif required_event_types:
                    failures.append(
                        f"{source_name}: non-monitoring source should not declare required_event_types "
                        f"{required_event_types}"
                    )

    if failures:
        for failure in failures:
            print(f"FAIL {failure}", file=sys.stderr)
        raise SystemExit(1)
    print(f"PASS local_usage_matrix self-check ({checked} expected source entries)")


def main() -> None:
    if len(sys.argv) == 2 and sys.argv[1] == "--self-check":
        self_check_expected_sources()
        return

    if len(sys.argv) == 3 and sys.argv[1] == "--expected-sources":
        print(json.dumps(expected_sources_for_agent(sys.argv[2]), separators=(",", ":")))
        return
    if len(sys.argv) == 3 and sys.argv[1] == "--expected-usage-basis":
        print(usage_fixture_expected_usage_basis(sys.argv[2]))
        return
    if len(sys.argv) != 3:
        raise SystemExit("usage: local_usage_matrix.py <root> <agent>")
    root = Path(sys.argv[1])
    agent = sys.argv[2]
    path = write_agent_fixture(root, agent)
    normalize_fixture_edit_paths(root, agent)
    print(path)


if __name__ == "__main__":
    main()
