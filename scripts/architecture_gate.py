#!/usr/bin/env python3
"""Repository architecture gate for local agent usage collection."""

from __future__ import annotations

import ast
import json
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]

REQUIRED_SOURCE_ROWS = [
    ("claude", "hook-jsonl", "HookJsonl"),
    ("claude", "projects-jsonl", "SessionJsonl"),
    ("codex", "hook-jsonl", "HookJsonl"),
    ("codex", "rollout-jsonl", "SessionJsonl"),
    ("cursor", "hook-jsonl", "HookJsonl"),
    ("cursor", "agent-transcripts-jsonl", "SessionJsonl"),
    ("cursor", "state-vscdb", "Sqlite"),
    ("trae", "trajectory-json", "SessionJsonl"),
    ("qwen", "telemetry-log", "TelemetryLog"),
    ("qwen", "project-chats-jsonl", "SessionJsonl"),
    ("qwen", "usage-record-jsonl", "SessionJsonl"),
    ("qwen", "token-usage-jsonl", "SessionJsonl"),
    ("opencode", "export-json", "SessionJsonl"),
    ("opencode", "sqlite", "Sqlite"),
    ("qoder", "hook-jsonl", "HookJsonl"),
    ("qoder", "transcript-jsonl", "SessionJsonl"),
    ("qoder", "local-db", "Sqlite"),
    ("qoder-cn", "hook-jsonl", "HookJsonl"),
    ("qoder-cn", "transcript-jsonl", "SessionJsonl"),
    ("qoder-cn", "local-db", "Sqlite"),
    ("qoder-work", "hook-jsonl", "HookJsonl"),
    ("qoder-work", "trace-jsonl", "SessionJsonl"),
    ("qoder-work", "local-db", "Sqlite"),
    ("qoder-work-cn", "hook-jsonl", "HookJsonl"),
    ("qoder-work-cn", "trace-jsonl", "SessionJsonl"),
    ("qoder-work-cn", "local-db", "Sqlite"),
    ("wukong", "sqlite", "Sqlite"),
    ("hermes", "sqlite", "Sqlite"),
    ("openclaw", "session-jsonl", "SessionJsonl"),
    ("gemini", "telemetry-log", "TelemetryLog"),
    ("gemini", "tmp-chats-jsonl", "SessionJsonl"),
    ("copilot", "otel-jsonl", "IdeSnapshot"),
    ("copilot", "official-copilot-runtime-jsonl", "SessionJsonl"),
    ("copilot", "session-state-jsonl", "SessionJsonl"),
    ("copilot", "session-store-db", "Sqlite"),
    ("copilot", "vscode-chat-state", "Sqlite"),
    ("cline", "vscode-tasks", "SessionJsonl"),
    ("cline", "vscode-ui-messages", "SessionJsonl"),
    ("cline", "sessions-db", "Sqlite"),
    ("roo-code", "vscode-tasks", "SessionJsonl"),
    ("roo-code", "vscode-ui-messages", "SessionJsonl"),
    ("kiro", "hook-jsonl", "HookJsonl"),
    ("kiro", "data-sqlite", "Sqlite"),
    ("kiro", "cli-session-json", "SessionJsonl"),
    ("zed", "threads-db", "Sqlite"),
    ("goose", "sessions-db", "Sqlite"),
    ("amp", "threads-jsonl", "SessionJsonl"),
    ("droid", "session-jsonl", "SessionJsonl"),
    ("droid", "settings-json", "SessionJsonl"),
    ("pi", "session-jsonl", "SessionJsonl"),
    ("mux", "chat-jsonl", "SessionJsonl"),
    ("mux", "session-usage-json", "SessionJsonl"),
    ("crush", "sqlite", "Sqlite"),
    ("codebuff", "project-jsonl", "SessionJsonl"),
    ("kilo", "sqlite", "Sqlite"),
    ("kilo", "storage-json", "SessionJsonl"),
    ("kilocode", "sqlite", "Sqlite"),
    ("kilocode", "storage-json", "SessionJsonl"),
    ("kilocode", "vscode-tasks", "SessionJsonl"),
    ("kilocode", "vscode-ui-messages", "SessionJsonl"),
    ("kimi", "wire-jsonl", "SessionJsonl"),
    ("gjc", "session-jsonl", "SessionJsonl"),
    ("grok", "sessions-jsonl", "SessionJsonl"),
    ("synthetic", "sqlite", "Sqlite"),
    ("warp", "warp-sqlite", "Sqlite"),
    ("antigravity", "conversation-sqlite", "Sqlite"),
    ("zcode", "projects-jsonl", "SessionJsonl"),
]

CANONICAL_ALIAS_AGENT_NAMES = {
    "roocode": "roo-code",
    "kilo-code": "kilocode",
    "gajae-code": "gjc",
}
ALIAS_AGENT_NAMES = set(CANONICAL_ALIAS_AGENT_NAMES)

SOURCE_LEVEL_PARTIAL_FIELD_SOURCES = [
    ("copilot", "otel-jsonl"),
    ("copilot", "session-state-jsonl"),
    ("copilot", "session-store-db"),
    ("copilot", "vscode-chat-state"),
    ("qoder", "hook-jsonl"),
    ("qoder", "local-db"),
    ("qoder-cn", "hook-jsonl"),
    ("qoder-cn", "local-db"),
    ("qoder-work", "hook-jsonl"),
    ("qoder-work-cn", "hook-jsonl"),
    ("kiro", "cli-session-json"),
    ("cline", "vscode-ui-messages"),
    ("roo-code", "vscode-ui-messages"),
    ("antigravity", "conversation-sqlite"),
    ("gemini", "telemetry-log"),
    ("qwen", "telemetry-log"),
    ("kilocode", "vscode-tasks"),
]

DERIVED_USAGE_COVERAGE_SOURCES = []

LOCAL_DERIVED_USAGE_BASIS_SOURCES = []

FIELD_LEVEL_NATIVE_COVERAGE_SOURCES = [
    ("claude", "projects-jsonl"),
    ("codex", "rollout-jsonl"),
    ("trae", "trajectory-json"),
    ("qwen", "project-chats-jsonl"),
    ("opencode", "sqlite"),
    ("kiro", "data-sqlite"),
    ("cline", "vscode-ui-messages"),
    ("roo-code", "vscode-ui-messages"),
    ("openclaw", "session-jsonl"),
    ("gjc", "session-jsonl"),
    ("zed", "threads-db"),
    ("goose", "sessions-db"),
    ("pi", "session-jsonl"),
    ("mux", "chat-jsonl"),
    ("mux", "session-usage-json"),
    ("crush", "sqlite"),
    ("kilo", "sqlite"),
    ("kilo", "storage-json"),
    ("kilocode", "sqlite"),
    ("kilocode", "storage-json"),
    ("droid", "session-jsonl"),
    ("kimi", "wire-jsonl"),
    ("gemini", "tmp-chats-jsonl"),
    ("copilot", "official-copilot-runtime-jsonl"),
    ("codebuff", "project-jsonl"),
    ("synthetic", "sqlite"),
    ("warp", "warp-sqlite"),
    ("antigravity", "conversation-sqlite"),
    ("wukong", "sqlite"),
]

FIELD_LEVEL_NATIVE_FULL_SOURCES = {
    ("kilo", "sqlite"),
    ("kilo", "storage-json"),
    ("kilocode", "sqlite"),
    ("kilocode", "storage-json"),
}

CAPABILITY_FIELDS = (
    "prompt_input",
    "assistant_output",
    "tool_call",
    "tool_result",
    "token_usage",
    "session_context",
    "time_context",
    "model_provider_context",
    "account_context",
    "cost_usage",
    "reasoning_usage",
    "edit_diff",
)

CORE_CAPABILITY_FIELDS = (
    "prompt_input",
    "assistant_output",
    "tool_call",
    "tool_result",
    "token_usage",
    "session_context",
)

SOURCE_LEVEL_CAPABILITY_FIELD_REQUIREMENTS = {
    ("cline", "vscode-ui-messages"): (
        "token_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "cost_usage",
    ),
    ("roo-code", "vscode-ui-messages"): (
        "token_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "cost_usage",
    ),
    ("mux", "chat-jsonl"): (
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "session_context",
        "time_context",
        "model_provider_context",
        "edit_diff",
    ),
    ("mux", "session-usage-json"): (
        "token_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "cost_usage",
        "reasoning_usage",
    ),
    ("codebuff", "project-jsonl"): (
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "session_context",
        "time_context",
        "model_provider_context",
        "cost_usage",
        "edit_diff",
    ),
    ("warp", "warp-sqlite"): (
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "session_context",
        "time_context",
        "model_provider_context",
        "cost_usage",
        "edit_diff",
    ),
}

EXPECTED_SOURCE_FIELD_KEYS = (
    "required_usage_fields",
    "required_record_fields",
    "optional_record_fields",
)

CAPABILITY_USAGE_FIELD_REQUIREMENTS = {
    "token_usage": ("tokens_in", "tokens_out", "message_count", "usage_basis"),
    "time_context": ("day",),
    "cost_usage": ("source_cost",),
    "reasoning_usage": ("reasoning",),
}

CAPABILITY_RECORD_FIELD_REQUIREMENTS = {
    "prompt_input": ("prompt_summary",),
    "assistant_output": ("assistant_output",),
    "tool_call": ("tool_name", "tool_arguments"),
    "tool_result": ("tool_result",),
    "edit_diff": ("file_path_or_diff",),
    "model_provider_context": ("provider", "model"),
    "session_context": ("session_id",),
    "time_context": ("timestamp_ms",),
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


def production_rust_text(text: str) -> str:
    return text.split("#[cfg(test)]", 1)[0]


def rust_function_body(text: str, name: str, *, production_only: bool = True) -> str:
    source = production_rust_text(text) if production_only else text
    match = re.search(rf"(?:pub\s+)?fn\s+{re.escape(name)}\s*\(", source)
    if not match:
        fail(f"{name} function not found")
    open_brace = source.find("{", match.end())
    if open_brace == -1:
        fail(f"{name} function body not found")
    depth = 0
    for index in range(open_brace, len(source)):
        char = source[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return source[match.start() : index + 1]
    fail(f"{name} function body is not closed")


def source_label_pairs(body: str) -> set[tuple[str, str]]:
    return set(re.findall(r'\("([^"]+)",\s*"([^"]+)"\)', body))


def assert_bool_helper_not_constant(name: str, body: str) -> None:
    inner = body[body.find("{") + 1 : body.rfind("}")].strip()
    if re.fullmatch(r"(?:return\s+)?(?:true|false)\s*;?", inner):
        fail(f"{name} must not be a constant {inner.rstrip(';')} helper")


def capability_flags(body: str) -> dict[str, bool] | None:
    flags: dict[str, bool] = {}
    for field in CAPABILITY_FIELDS:
        match = re.search(rf"\b{field}:\s*(true|false)", body)
        if not match:
            return None
        flags[field] = match.group(1) == "true"
    return flags


def local_source_capability_constants(text: str) -> dict[str, dict[str, bool]]:
    constants: dict[str, dict[str, bool]] = {}
    pattern = (
        r"const\s+([A-Z0-9_]+):\s*LocalSourceCapabilities\s*="
        r"\s*LocalSourceCapabilities\s*\{([\s\S]*?)\n\s*\};"
    )
    for match in re.finditer(pattern, text):
        flags = capability_flags(match.group(2))
        if flags is not None:
            constants[match.group(1)] = flags
    return constants


def local_source_spec_block(text: str, agent: str, label: str) -> str | None:
    for raw_block in production_rust_text(text).split("LocalSourceSpec {")[1:]:
        block = "LocalSourceSpec {" + raw_block.split("\n    },", 1)[0] + "\n    },"
        if re.search(rf'agent:\s*"{re.escape(agent)}"', block) and re.search(
            rf'label:\s*"{re.escape(label)}"', block
        ):
            return block
    return None


def local_source_spec_blocks(text: str) -> list[tuple[str, str, str]]:
    specs: list[tuple[str, str, str]] = []
    for raw_block in production_rust_text(text).split("LocalSourceSpec {")[1:]:
        block = "LocalSourceSpec {" + raw_block.split("\n    },", 1)[0] + "\n    },"
        agent_match = re.search(r'agent:\s*"([^"]+)"', block)
        label_match = re.search(r'label:\s*"([^"]+)"', block)
        if agent_match and label_match:
            specs.append((agent_match.group(1), label_match.group(1), block))
    return specs


def local_source_spec_entries(text: str) -> list[tuple[str, str, str]]:
    entries: list[tuple[str, str, str]] = []
    for agent, label, block in local_source_spec_blocks(text):
        kind_match = re.search(r"kind:\s*LocalSourceKind::([A-Za-z0-9_]+)", block)
        if not kind_match:
            fail(f"{agent}/{label} source spec kind missing")
        entries.append((agent, label, kind_match.group(1)))
    return entries


def required_fields_for_capabilities(
    flags: dict[str, bool],
    requirements: dict[str, tuple[str, ...]],
) -> set[str]:
    fields: set[str] = set()
    for capability, required in requirements.items():
        if flags.get(capability, False):
            fields.update(required)
    return fields


def capability_fields_for_expected_fields(
    fields: set[str],
    requirements: dict[str, tuple[str, ...]],
) -> set[str]:
    required: set[str] = set()
    for capability, mapped_fields in requirements.items():
        if any(field in fields for field in mapped_fields):
            required.add(capability)
    return required


def usage_capabilities_required_by_expected_fields(fields: set[str]) -> set[str]:
    required: set[str] = set()
    # tokens_in can be an aggregate-only local usage signal. Full token bucket
    # capability is only proven when output/completion tokens are required too.
    if "tokens_out" in fields:
        required.add("token_usage")
    if "day" in fields:
        required.add("time_context")
    if "source_cost" in fields:
        required.add("cost_usage")
    if "reasoning" in fields:
        required.add("reasoning_usage")
    return required


def local_source_spec_all_capabilities_enabled(
    text: str, constants: dict[str, dict[str, bool]], agent: str, label: str
) -> bool:
    return local_source_spec_capabilities_enabled_for(
        text, constants, agent, label, CAPABILITY_FIELDS
    )


def local_source_spec_core_capabilities_enabled(
    text: str, constants: dict[str, dict[str, bool]], agent: str, label: str
) -> bool:
    return local_source_spec_capabilities_enabled_for(
        text, constants, agent, label, required_capability_fields_for_source(agent, label)
    )


def required_capability_fields_for_source(agent: str, label: str) -> tuple[str, ...]:
    return SOURCE_LEVEL_CAPABILITY_FIELD_REQUIREMENTS.get(
        (agent, label), CORE_CAPABILITY_FIELDS
    )


def local_source_spec_capabilities_enabled_for(
    text: str,
    constants: dict[str, dict[str, bool]],
    agent: str,
    label: str,
    fields: tuple[str, ...],
) -> bool:
    block = local_source_spec_block(text, agent, label)
    if block is None:
        fail(f"{agent}/{label} source spec missing")
    constant_match = re.search(r"capabilities:\s*([A-Z][A-Z0-9_]+)\s*,", block)
    if constant_match:
        constant_name = constant_match.group(1)
        if constant_name not in constants:
            fail(f"{agent}/{label} uses unknown capability constant {constant_name}")
        return all(constants[constant_name].get(field, False) for field in fields)
    inline_match = re.search(
        r"capabilities:\s*LocalSourceCapabilities\s*\{([\s\S]*?)\n\s*\}", block
    )
    if not inline_match:
        fail(f"{agent}/{label} capability expression is not recognized")
    flags = capability_flags(inline_match.group(1))
    if flags is None:
        fail(f"{agent}/{label} inline capabilities are incomplete")
    return all(flags.get(field, False) for field in fields)


def local_source_spec_capability_flags(
    constants: dict[str, dict[str, bool]], agent: str, label: str, block: str
) -> dict[str, bool]:
    constant_match = re.search(r"capabilities:\s*([A-Z][A-Z0-9_]+)\s*,", block)
    if constant_match:
        constant_name = constant_match.group(1)
        if constant_name not in constants:
            fail(f"{agent}/{label} uses unknown capability constant {constant_name}")
        return constants[constant_name]
    inline_match = re.search(
        r"capabilities:\s*LocalSourceCapabilities\s*\{([\s\S]*?)\n\s*\}", block
    )
    if not inline_match:
        fail(f"{agent}/{label} capability expression is not recognized")
    flags = capability_flags(inline_match.group(1))
    if flags is None:
        fail(f"{agent}/{label} inline capabilities are incomplete")
    return flags


def verified_full_capability_source_body(text: str) -> str:
    return rust_function_body(text, "verified_full_capability_source")


def verified_local_derived_full_coverage_source_body(text: str) -> str:
    return rust_function_body(text, "verified_local_derived_full_coverage_source")


def local_derived_usage_source_body(text: str) -> str:
    return rust_function_body(text, "local_derived_usage_source")


def source_has_full_upload_coverage_body(text: str) -> str:
    return rust_function_body(text, "source_has_full_upload_coverage")


def source_has_strict_full_upload_capabilities_body(text: str) -> str:
    return rust_function_body(text, "source_has_strict_full_upload_capabilities")


def audited_native_source_label_body(text: str) -> str:
    return rust_function_body(text, "audited_native_source_label")


def verified_native_source_pairs(text: str) -> set[tuple[str, str]]:
    verified_body = verified_full_capability_source_body(text)
    pairs = source_label_pairs(verified_body)
    if "audited_native_source_label(spec)" in verified_body:
        pairs |= source_label_pairs(audited_native_source_label_body(text))
    return pairs


def verified_local_derived_full_source_pairs(text: str) -> set[tuple[str, str]]:
    verified_body = verified_local_derived_full_coverage_source_body(text)
    pairs = source_label_pairs(verified_body)
    if "local_derived_usage_source(spec)" in verified_body:
        pairs |= source_label_pairs(local_derived_usage_source_body(text))
    return pairs


def strict_full_upload_source_pairs(text: str) -> set[tuple[str, str]]:
    return verified_native_source_pairs(text) | verified_local_derived_full_source_pairs(text)


def assert_source_level_full_upload_helpers(text: str) -> None:
    strict_body = source_has_strict_full_upload_capabilities_body(text)
    assert_bool_helper_not_constant("source_has_strict_full_upload_capabilities", strict_body)
    if "||" in strict_body:
        fail("strict full upload capabilities must require every strict field with AND semantics")
    for field in CAPABILITY_FIELDS:
        if f"capabilities.{field}" not in strict_body:
            fail(f"strict full upload coverage must check extended capability field {field}")
    if strict_body.count("&&") < len(CAPABILITY_FIELDS) - 1:
        fail("strict full upload capabilities must combine every strict field")

    full_upload_body = source_has_full_upload_coverage_body(text)
    assert_bool_helper_not_constant("source_has_full_upload_coverage", full_upload_body)
    for helper in [
        "verified_full_capability_source(spec)",
    ]:
        if helper not in full_upload_body:
            fail(f"source_has_full_upload_coverage must use {helper}")
    if "local_derived_usage_source(spec)" in full_upload_body:
        fail("local-derived usage sources must not automatically imply full upload coverage")

    verified_body = verified_full_capability_source_body(text)
    assert_bool_helper_not_constant("verified_full_capability_source", verified_body)
    if "source_has_strict_full_upload_capabilities(spec.capabilities)" not in verified_body:
        fail("verified_full_capability_source must require strict full upload capabilities")
    if (
        "audited_native_source_label(spec)" not in verified_body
        and not source_label_pairs(verified_body)
    ):
        fail("verified_full_capability_source must use audited native labels or an explicit source set")

    local_derived_full_body = verified_local_derived_full_coverage_source_body(text)
    assert_bool_helper_not_constant(
        "verified_local_derived_full_coverage_source", local_derived_full_body
    )
    if "source_has_strict_full_upload_capabilities(spec.capabilities)" not in local_derived_full_body:
        fail("verified_local_derived_full_coverage_source must require strict full upload capabilities")
    if (
        "local_derived_usage_source(spec)" not in local_derived_full_body
        and not source_label_pairs(local_derived_full_body)
    ):
        fail(
            "verified_local_derived_full_coverage_source must use audited local-derived labels "
            "or an explicit source set"
        )

    audited_body = audited_native_source_label_body(text)
    assert_bool_helper_not_constant("audited_native_source_label", audited_body)
    if not source_label_pairs(audited_body):
        fail("audited_native_source_label must enumerate audited source labels")


def assert_agent_level_coverage_helpers(text: str) -> None:
    production_text = production_rust_text(text)
    struct_match = re.search(
        r"pub\s+struct\s+AgentFullDataCoverage\s*\{([\s\S]*?)\n\}",
        production_text,
    )
    if not struct_match:
        fail("AgentFullDataCoverage struct not found")
    struct_body = struct_match.group(1)
    for field in [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "usage_basis",
        "session_context",
        "time_context",
        "model_provider_context",
        "account_context",
        "cost_usage",
        "reasoning_usage",
        "edit_diff",
    ]:
        if f"pub {field}:" not in struct_body:
            fail(f"AgentFullDataCoverage missing field {field}")

    merge_body = rust_function_body(text, "merge_source")
    for field in [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "account_context",
        "cost_usage",
        "reasoning_usage",
        "edit_diff",
    ]:
        if f"capabilities.{field}" not in merge_body:
            fail(f"AgentFullDataCoverage::merge_source must merge {field}")
    if "local_source_usage_basis(spec)" not in merge_body:
        fail("AgentFullDataCoverage::merge_source must merge source usage basis")
    if "usage_basis != UsageBasis::None" not in merge_body:
        fail("AgentFullDataCoverage::merge_source must derive upload payload fields from usage basis")

    required_body = rust_function_body(text, "has_required_fields")
    assert_bool_helper_not_constant("AgentFullDataCoverage::has_required_fields", required_body)
    for field in [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "usage_basis",
        "session_context",
        "time_context",
        "model_provider_context",
        "account_context",
        "cost_usage",
        "reasoning_usage",
        "edit_diff",
    ]:
        if f"self.{field}" not in required_body:
            fail(f"AgentFullDataCoverage::has_required_fields must check {field}")

    coverage_body = rust_function_body(text, "agent_full_data_coverage")
    for needle in [
        "canonical_agent_name(agent)",
        "AgentFullDataCoverage::empty()",
        "LOCAL_SOURCE_SPECS",
        "coverage.merge_source(spec)",
        "Some(coverage)",
        "None",
    ]:
        if needle not in coverage_body:
            fail(f"agent_full_data_coverage missing {needle}")

    required_agent_body = rust_function_body(text, "agent_has_required_full_data_coverage")
    assert_bool_helper_not_constant("agent_has_required_full_data_coverage", required_agent_body)
    for needle in [
        "agent_full_data_coverage(agent)",
        "coverage.has_required_fields()",
        "None => false",
    ]:
        if needle not in required_agent_body:
            fail(f"agent_has_required_full_data_coverage missing {needle}")

    test_match = re.search(
        r"fn\s+default_scan_agents_have_required_full_data_coverage"
        r"[\s\S]*?(?=\n\s*#\[test\]|\Z)",
        text,
    )
    if not test_match:
        fail("missing default scan full-data coverage test")
    test_body = test_match.group(0)
    for needle in [
        "default_scan_agent_names()",
        "agent_has_required_full_data_coverage(agent)",
        "agent_full_data_coverage(agent)",
    ]:
        if needle not in test_body:
            fail(f"default scan full-data coverage test missing {needle}")


def assert_partial_sources_do_not_overclaim_fields(text: str) -> None:
    constants = local_source_capability_constants(text)
    strict_source_pairs = strict_full_upload_source_pairs(text)
    for agent, label in SOURCE_LEVEL_PARTIAL_FIELD_SOURCES:
        if local_source_spec_all_capabilities_enabled(text, constants, agent, label):
            fail(
                f"{agent}/{label} enables every local capability although the source is partial"
            )
        if (agent, label) in strict_source_pairs and local_source_spec_all_capabilities_enabled(
            text, constants, agent, label
        ):
            fail(f"{agent}/{label} overclaims a single-source complete field set")


def assert_no_unverified_all_true_capability_specs(text: str) -> None:
    constants = local_source_capability_constants(text)
    strict_source_pairs = strict_full_upload_source_pairs(text)
    for agent, label, block in local_source_spec_blocks(text):
        flags = local_source_spec_capability_flags(constants, agent, label, block)
        if not all(flags.get(field, False) for field in CAPABILITY_FIELDS):
            continue
        if (agent, label) not in strict_source_pairs:
            fail(
                f"{agent}/{label} enables every local capability without source evidence"
            )


def assert_all_default_agents_have_declared_upload_source(text: str) -> None:
    native_verified_pairs = verified_native_source_pairs(text)
    local_derived_full_pairs = verified_local_derived_full_source_pairs(text)
    derived_body = local_derived_usage_source_body(text)
    derived_pairs = source_label_pairs(derived_body)
    full_upload_body = source_has_full_upload_coverage_body(text)
    assert_source_level_full_upload_helpers(text)
    assert_agent_level_coverage_helpers(text)
    if "local_derived_usage_source(spec)" in full_upload_body:
        fail("local-derived usage sources must not automatically imply native source coverage")
    assert_no_unverified_all_true_capability_specs(text)
    for agent, label in DERIVED_USAGE_COVERAGE_SOURCES:
        if (agent, label) not in derived_pairs:
            fail(f"{agent}/{label} must provide local-derived usage coverage")
        if (agent, label) in native_verified_pairs:
            fail(f"{agent}/{label} must keep local-derived usage basis")
    for agent, label in LOCAL_DERIVED_USAGE_BASIS_SOURCES:
        if (agent, label) in native_verified_pairs:
            fail(f"{agent}/{label} must keep local-derived usage basis")
        if (agent, label) in local_derived_full_pairs and local_source_spec_all_capabilities_enabled(
            text, local_source_capability_constants(text), agent, label
        ):
            fail(f"{agent}/{label} overclaims a single-source complete field set")
    for agent, label in FIELD_LEVEL_NATIVE_COVERAGE_SOURCES:
        if (
            (agent, label) in native_verified_pairs
            and (agent, label) not in FIELD_LEVEL_NATIVE_FULL_SOURCES
            and local_source_spec_all_capabilities_enabled(
                text, local_source_capability_constants(text), agent, label
            )
        ):
            fail(f"{agent}/{label} overclaims a single-source complete field set")
        if (agent, label) in derived_pairs:
            fail(f"{agent}/{label} must keep native field-level usage basis")
        required_fields = required_capability_fields_for_source(agent, label)
        if not local_source_spec_capabilities_enabled_for(
            text,
            local_source_capability_constants(text),
            agent,
            label,
            required_fields,
        ):
            fail(
                f"{agent}/{label} must keep source-level parser capabilities enabled: "
                f"{', '.join(required_fields)}"
            )

    registered = re.findall(r'name:\s*"([^"]+)"', text)
    alias_registered = sorted(set(registered) & ALIAS_AGENT_NAMES)
    if alias_registered:
        fail(
            "registered agent list must contain canonical keys only, found alias "
            + ", ".join(alias_registered)
        )
    default_agents = registered
    for agent in default_agents:
        spec_blocks = [
            "LocalSourceSpec {" + block
            for block in text.split("LocalSourceSpec {")[1:]
            if re.search(rf'agent:\s*"{re.escape(agent)}"', block)
        ]
        if not spec_blocks:
            fail(f"missing local source spec for default agent {agent}")
        has_coverage = False
        for block in spec_blocks:
            label_match = re.search(r'label:\s*"([^"]+)"', block)
            if label_match is None:
                continue
            label = label_match.group(1)
            key = (agent, label)
            if (
                key in native_verified_pairs
                or key in derived_pairs
                or key in FIELD_LEVEL_NATIVE_COVERAGE_SOURCES
            ):
                has_coverage = True
                break
        if not has_coverage:
            fail(f"{agent} lacks a declared upload source via verified, derived, or field-level native usage")


def assert_alias_agents_are_canonicalized(text: str) -> None:
    registered = set(re.findall(r'name:\s*"([^"]+)"', text))
    for alias, canonical in CANONICAL_ALIAS_AGENT_NAMES.items():
        if alias in registered:
            fail(f"alias {alias} must not be registered/default coverage agent")
        if canonical not in registered:
            fail(f"alias {alias} canonical target {canonical} is not registered")
        if f'"{alias}" => "{canonical}"' not in text:
            fail(f"alias {alias} must canonicalize to {canonical}")


def ast_string_values(node: ast.AST) -> set[str]:
    if isinstance(node, ast.Constant) and isinstance(node.value, str):
        return {node.value}
    if isinstance(node, (ast.List, ast.Set, ast.Tuple)):
        values: set[str] = set()
        for element in node.elts:
            values.update(ast_string_values(element))
        return values
    return set()


def ast_string_pairs(node: ast.AST) -> set[tuple[str, str]]:
    if (
        isinstance(node, ast.Tuple)
        and len(node.elts) == 2
        and all(isinstance(element, ast.Constant) and isinstance(element.value, str) for element in node.elts)
    ):
        return {(node.elts[0].value, node.elts[1].value)}
    if isinstance(node, (ast.List, ast.Set, ast.Tuple)):
        values: set[tuple[str, str]] = set()
        for element in node.elts:
            values.update(ast_string_pairs(element))
        return values
    return set()


def is_agent_name(node: ast.AST) -> bool:
    return isinstance(node, ast.Name) and node.id == "agent"


def fixture_branch_matches_agent(test: ast.AST, agent: str) -> bool:
    if isinstance(test, ast.BoolOp):
        return any(fixture_branch_matches_agent(value, agent) for value in test.values)
    if not isinstance(test, ast.Compare) or len(test.ops) != 1 or len(test.comparators) != 1:
        return False
    op = test.ops[0]
    left = test.left
    right = test.comparators[0]
    if isinstance(op, ast.Eq):
        return (is_agent_name(left) and agent in ast_string_values(right)) or (
            is_agent_name(right) and agent in ast_string_values(left)
        )
    if isinstance(op, ast.In):
        return is_agent_name(left) and agent in ast_string_values(right)
    return False


def fixture_branch_for_agent(fixture_text: str, agent: str) -> str | None:
    tree = ast.parse(fixture_text)
    lines = fixture_text.splitlines(keepends=True)
    for node in ast.walk(tree):
        if not isinstance(node, ast.FunctionDef) or node.name != "write_agent_fixture":
            continue
        for child in ast.walk(node):
            if not isinstance(child, ast.If):
                continue
            if not fixture_branch_matches_agent(child.test, agent):
                continue
            end_lineno = getattr(child, "end_lineno", None)
            if end_lineno is None:
                fail(f"cannot determine fixture branch range for {agent}")
            return "".join(lines[child.lineno - 1 : end_lineno])
    return None


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
        "UsageBasis",
        "local_source_specs",
        "local_source_usage_basis",
        "source_has_full_upload_coverage",
        "source_has_strict_full_upload_capabilities",
        "verified_local_derived_full_coverage_source",
        "local_derived_usage_source",
        "AgentFullDataCoverage",
        "agent_full_data_coverage",
        "agent_has_required_full_data_coverage",
        "default_scan_agents_have_required_full_data_coverage",
        "FIELD_LEVEL_SESSION_CAPABILITIES",
        "OFFICIAL_TRANSCRIPT_CAPABILITIES",
        "FIELD_LEVEL_EXTENDED_SESSION_CAPABILITIES",
        "SUPPLEMENTAL_USAGE_SUMMARY_CAPABILITIES",
        "OFFICIAL_PROMPT_TOOL_HOOK_CAPABILITIES",
        "LOCAL_DERIVED_TRANSCRIPT_CAPABILITIES",
        "CONDITIONAL_OTEL_CAPABILITIES",
        "SUPPLEMENTAL_STATE_CAPABILITIES",
        "LOCAL_TRANSCRIPT_EVENT_CAPABILITIES",
        "agent_support_matrix_has_required_upload_sources",
    ]:
        if symbol not in text:
            fail(f"missing agent source spec symbol {symbol}")
    assert_partial_sources_do_not_overclaim_fields(text)
    assert_all_default_agents_have_declared_upload_source(text)
    assert_alias_agents_are_canonicalized(text)
    verified_body = verified_full_capability_source_body(text)
    cursor_state = local_source_spec_block(text, "cursor", "state-vscdb")
    if cursor_state is None:
        fail("cursor/state-vscdb source spec missing")
    if "LocalSourceKind::Sqlite" not in cursor_state:
        fail("cursor/state-vscdb must remain a typed SQLite source")
    if "CURSOR_STATE_VSCDB_CAPABILITIES" not in cursor_state:
        fail("cursor/state-vscdb must use the typed Cursor state capability constant")
    cursor_caps = local_source_capability_constants(text).get("CURSOR_STATE_VSCDB_CAPABILITIES")
    if not cursor_caps:
        fail("CURSOR_STATE_VSCDB_CAPABILITIES missing")
    for required in CAPABILITY_FIELDS:
        if not cursor_caps.get(required):
            fail(f"cursor/state-vscdb typed capability missing {required}: true")
    if "UNVERIFIED_LOCAL_CAPABILITIES" in text:
        fail("unverified local capabilities must not be listed as default support evidence")
    if re.search(r"LocalSourceSpec\s*\{[\s\S]{0,220}probe", text, flags=re.I):
        fail("structure probe must not be represented as a LocalSourceSpec capability")
    if "supplemental_sources_do_not_overclaim_single_source_complete_fields" not in text:
        fail("missing regression that supplemental sources do not overclaim fields")
    qwen_chats = local_source_spec_block(text, "qwen", "project-chats-jsonl")
    if qwen_chats is None:
        fail("qwen/project-chats-jsonl source spec missing for ChatRecording JSONL")
    if "tmp-chats-jsonl" in qwen_chats:
        fail("qwen source specs must use current project-chats-jsonl wording, not tmp-chats-jsonl")
    if "FIELD_LEVEL_SESSION_CAPABILITIES" not in qwen_chats:
        fail("qwen/project-chats-jsonl must stay field-level until real local token/schema evidence is closed")
    if "FIELD_LEVEL_EXTENDED_SESSION_CAPABILITIES" in qwen_chats:
        fail("qwen/project-chats-jsonl must not use extended session capabilities")
    opencode_sqlite = local_source_spec_block(text, "opencode", "sqlite")
    if opencode_sqlite is None:
        fail("opencode/sqlite source spec missing")
    if "OPENCODE_SQLITE_CAPABILITIES" not in opencode_sqlite:
        fail("opencode/sqlite must use the typed official SQLite capability contract")
    if "FIELD_LEVEL_EXTENDED_SESSION_CAPABILITIES" in opencode_sqlite:
        fail("opencode/sqlite must not use extended session capabilities")
    qwen_token_usage = local_source_spec_block(text, "qwen", "token-usage-jsonl")
    if qwen_token_usage is None:
        fail("qwen/token-usage-jsonl source spec missing for supplemental runtime token usage JSONL")
    if "SUPPLEMENTAL_USAGE_SUMMARY_CAPABILITIES" not in qwen_token_usage:
        fail("qwen/token-usage-jsonl must stay a usage-only supplemental source")
    gemini_chats = local_source_spec_block(text, "gemini", "tmp-chats-jsonl")
    if gemini_chats is None:
        fail("gemini/tmp-chats-jsonl source spec missing for ChatRecording JSONL")
    if "LocalSourceKind::SessionJsonl" not in gemini_chats:
        fail("gemini/tmp-chats-jsonl must be a SessionJsonl source")
    if not local_source_spec_core_capabilities_enabled(
        text,
        local_source_capability_constants(text),
        "gemini",
        "tmp-chats-jsonl",
    ):
        fail("gemini/tmp-chats-jsonl must keep ChatRecording field coverage enabled")
    if '("gemini", "tmp-chats-jsonl")' in verified_full_capability_source_body(text):
        fail("gemini/tmp-chats-jsonl must not claim fields absent from ChatRecording events")
    warp_sqlite = local_source_spec_block(text, "warp", "warp-sqlite")
    if warp_sqlite is None:
        fail("warp/warp-sqlite source spec missing for warp.sqlite")
    if "LocalSourceKind::Sqlite" not in warp_sqlite:
        fail("warp/warp-sqlite must be a Sqlite source")
    if not local_source_spec_core_capabilities_enabled(
        text,
        local_source_capability_constants(text),
        "warp",
        "warp-sqlite",
    ):
        fail("warp/warp-sqlite must keep typed SQLite/protobuf field coverage enabled")
    if '("warp", "warp-sqlite")' in verified_full_capability_source_body(text):
        fail("warp/warp-sqlite must keep aggregate/category token support explicit")
    kilocode_sqlite = local_source_spec_block(text, "kilocode", "sqlite")
    if kilocode_sqlite is None:
        fail("kilocode/sqlite source spec missing for Kilo Code kilo.db")
    if "LocalSourceKind::Sqlite" not in kilocode_sqlite:
        fail("kilocode/sqlite must be a Sqlite source")
    if not local_source_spec_core_capabilities_enabled(
        text,
        local_source_capability_constants(text),
        "kilocode",
        "sqlite",
    ):
        fail("kilocode/sqlite must keep typed Kilo Code SQLite field coverage enabled")
    native_verified_pairs = verified_native_source_pairs(text)
    if ("kilocode", "sqlite") not in native_verified_pairs:
        fail("kilocode/sqlite must claim the full local DB field set proved by tests")
    if '("kilocode", "vscode-tasks")' in verified_body:
        fail("kilocode/vscode-tasks must not claim fields absent from task transcripts")
    for agent in ["kilo", "kilocode"]:
        storage = local_source_spec_block(text, agent, "storage-json")
        if storage is None:
            fail(f"{agent}/storage-json source spec missing for Kilo storage JSON")
        if "LocalSourceKind::SessionJsonl" not in storage:
            fail(f"{agent}/storage-json must be a SessionJsonl source")
        if not local_source_spec_core_capabilities_enabled(
            text,
            local_source_capability_constants(text),
            agent,
            "storage-json",
        ):
            fail(f"{agent}/storage-json must keep typed storage JSON field coverage enabled")
        if (agent, "storage-json") not in native_verified_pairs:
            fail(f"{agent}/storage-json must claim the full local storage field set proved by tests")
    synthetic_sqlite = local_source_spec_block(text, "synthetic", "sqlite")
    if synthetic_sqlite is None:
        fail("synthetic/sqlite source spec missing for Octofriend sqlite.db")
    if "LocalSourceKind::Sqlite" not in synthetic_sqlite:
        fail("synthetic/sqlite must be a Sqlite source")
    if not local_source_spec_core_capabilities_enabled(
        text,
        local_source_capability_constants(text),
        "synthetic",
        "sqlite",
    ):
        fail("synthetic/sqlite must keep Octofriend SQLite field coverage enabled")
    if '("synthetic", "sqlite")' in verified_body:
        fail("synthetic/sqlite must not claim fields absent from the local DB")
    codebuff_project_jsonl = local_source_spec_block(text, "codebuff", "project-jsonl")
    if codebuff_project_jsonl is None:
        fail("codebuff/project-jsonl source spec missing for chat-messages.json plus run-state.json")
    if "LocalSourceKind::SessionJsonl" not in codebuff_project_jsonl:
        fail("codebuff/project-jsonl must be a SessionJsonl source")
    if not local_source_spec_core_capabilities_enabled(
        text,
        local_source_capability_constants(text),
        "codebuff",
        "project-jsonl",
    ):
        fail("codebuff/project-jsonl must keep Codebuff chat/run-state field coverage enabled")
    if '("codebuff", "project-jsonl")' in verified_body:
        fail("codebuff/project-jsonl must not claim stable token buckets")
    copilot_official_runtime = local_source_spec_block(
        text, "copilot", "official-copilot-runtime-jsonl"
    )
    if copilot_official_runtime is None:
        fail("copilot/official-copilot-runtime-jsonl source spec missing")
    if "LocalSourceKind::SessionJsonl" not in copilot_official_runtime:
        fail("copilot/official-copilot-runtime-jsonl must be a SessionJsonl source")
    if not local_source_spec_core_capabilities_enabled(
        text,
        local_source_capability_constants(text),
        "copilot",
        "official-copilot-runtime-jsonl",
    ):
        fail("copilot/official-copilot-runtime-jsonl must keep runtime event field coverage enabled")
    if '("copilot", "official-copilot-runtime-jsonl")' in verified_body:
        fail("copilot/official-copilot-runtime-jsonl must not claim fields absent from runtime events")
    for forbidden in [
        '("copilot", "otel-jsonl")',
        '("copilot", "generic-otel-jsonl")',
        '("copilot", "session-state-jsonl")',
        '("copilot", "session-store-db")',
        '("copilot", "vscode-chat-state")',
    ]:
        if forbidden in verified_body:
            fail(f"{forbidden} must not enter verified_full_capability_source")
    for needle in [
        'label: "official-copilot-runtime-jsonl"',
        'home.join(".copilot").join("session-state")',
        'p.ends_with(".copilot/session-state")',
        'label: "session-store-db"',
        'home.join(".copilot").join("session-store.db")',
        'p.ends_with(".copilot/session-store.db")',
        'xdg_config_home(home).join("github-copilot")',
        'roots.push(github_copilot_config.clone())',
        '"chat-sessions"',
        '"chat-agent-sessions"',
        '"chat-edit-sessions"',
        'github_copilot_config.join("ws").join(session_root)',
        '".config/github-copilot/ws/chat-agent-sessions"',
        '".config/github-copilot/ws/chat-edit-sessions"',
    ]:
        if needle not in text:
            fail(f"copilot session-store.db source/default roots missing {needle}")
    for needle in [
        'roots.push(home.join(".qoderwork/hooks"))',
        'roots.push(home.join(".qoderworkcn/hooks"))',
        'p.ends_with(".qoderwork/hooks")',
        'p.ends_with(".qoderworkcn/hooks")',
    ]:
        if needle not in text:
            fail(f"qoder-work-cn default roots must include official and legacy hook paths: {needle}")
    for agent, label in [
        ("cline", "sessions-db"),
    ]:
        pattern = (
            rf'agent:\s*"{re.escape(agent)}"[\s\S]{{0,120}}'
            rf'label:\s*"{re.escape(label)}"[\s\S]{{0,160}}'
            r"capabilities:\s*([^,]+),"
        )
        match = re.search(pattern, text)
        if not match or "SUPPLEMENTAL_STATE_CAPABILITIES" not in match.group(1):
            fail(f"{agent}/{label} must keep the supplemental state-only source contract")
    for agent, label, capability in [
        ("qoder-work", "trace-jsonl", "QODER_WORK_TRACE_CAPABILITIES"),
        ("qoder-work-cn", "trace-jsonl", "QODER_WORK_TRACE_CAPABILITIES"),
        ("qoder-work", "local-db", "QODER_WORK_LOCAL_DB_CAPABILITIES"),
        ("qoder-work-cn", "local-db", "QODER_WORK_LOCAL_DB_CAPABILITIES"),
    ]:
        block = local_source_spec_block(text, agent, label)
        if block is None or capability not in block:
            fail(f"{agent}/{label} must use {capability}")
    kiro_cli = local_source_spec_block(text, "kiro", "cli-session-json")
    if kiro_cli is None:
        fail("kiro/cli-session-json source spec missing")
    if "LocalSourceKind::SessionJsonl" not in kiro_cli:
        fail("kiro/cli-session-json must remain a typed session JSON source")
    if "KIRO_CLI_SESSION_CAPABILITIES" not in kiro_cli:
        fail("kiro/cli-session-json must use the typed Kiro CLI session capability constant")
    kiro_caps = local_source_capability_constants(text).get("KIRO_CLI_SESSION_CAPABILITIES")
    if not kiro_caps:
        fail("KIRO_CLI_SESSION_CAPABILITIES missing")
    for required in [
        "prompt_input",
        "assistant_output",
        "tool_call",
        "tool_result",
        "token_usage",
        "session_context",
        "time_context",
        "model_provider_context",
        "cost_usage",
        "reasoning_usage",
        "edit_diff",
    ]:
        if not kiro_caps.get(required):
            fail(f"kiro/cli-session-json typed capability missing {required}: true")
    if kiro_caps.get("account_context"):
        fail("kiro/cli-session-json must not claim absent account_context: true")
    if '("kiro", "cli-session-json")' in verified_body:
        fail("kiro/cli-session-json must not claim absent account fields")
    wukong_sqlite = re.search(
        r'agent:\s*"wukong"[\s\S]{0,120}kind:\s*LocalSourceKind::Sqlite[\s\S]{0,120}label:\s*"sqlite"',
        text,
    )
    if not wukong_sqlite:
        fail("wukong source spec must use official SQLite local adapter source")
    if not local_source_spec_core_capabilities_enabled(
        text,
        local_source_capability_constants(text),
        "wukong",
        "sqlite",
    ):
        fail("wukong/sqlite must keep sessions/steps/tool/token field coverage enabled")
    if '("wukong", "sqlite")' in verified_body:
        fail("wukong/sqlite must not claim fields absent from the local DB")
    if '("wukong", "sqlite")' in local_derived_usage_source_body(text):
        fail("wukong/sqlite must not be classified as local-derived after native token evidence is proven")

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

    parsed_source_rows = set(local_source_spec_entries(text))
    for agent, label, kind in REQUIRED_SOURCE_ROWS:
        if (agent, label, kind) not in parsed_source_rows:
            fail(f"missing source matrix cell {agent}/{label}/{kind}")

    registered = re.findall(r'name:\s*"([^"]+)"', text)
    alias_registered = sorted(set(registered) & ALIAS_AGENT_NAMES)
    if alias_registered:
        fail(
            "registered agent list must contain canonical keys only, found alias "
            + ", ".join(alias_registered)
        )
    default_agents = registered
    for agent in default_agents:
        if re.search(rf'agent:\s*"{re.escape(agent)}"', text) is None:
            fail(f"missing local source spec for default agent {agent}")
    for agent in ALIAS_AGENT_NAMES:
        if re.search(rf'agent:\s*"{re.escape(agent)}"', text) is not None:
            fail(f"alias {agent} must inherit its canonical source spec instead of duplicating one")
    for agent in ["baidu-comate", "wenxin"]:
        if re.search(rf'agent:\s*"{re.escape(agent)}"', text):
            fail(f"{agent} has no verified default local root and must not have a default source spec")


def assert_usage_parser_surface() -> None:
    text = read("client/src/usage/mod.rs")
    for symbol in [
        "DEFAULT_SCAN_LOOKBACK_DAYS",
        "MAX_SCAN_WINDOW_DAYS",
        "MAX_SCAN_FILES_PER_RUN",
        "MAX_SCAN_CANDIDATES_PER_RUN",
        "MAX_SCAN_CANDIDATES_PER_AGENT",
        "MAX_SCAN_DIR_ENTRIES_PER_RUN",
        "MAX_SCAN_DIR_ENTRIES_PER_AGENT",
        "IMPORT_MANIFEST_FILE",
        "MAX_IMPORT_MANIFEST_BYTES",
        "MAX_IMPORT_MANIFEST_ENTRIES",
        "MAX_USAGE_SCAN_FILE_CACHE_ROWS",
        "MAX_USAGE_MONITORING_SEEN_ROWS",
        "MAX_USAGE_ROLLUP_SOURCE_ROWS",
        "MAX_USAGE_OUTBOX_ROWS",
        "MAX_USAGE_OUTBOX_PAYLOAD_BYTES",
        "MAX_OUTBOX_RETRY_COUNT",
        "USAGE_OUTBOX_PENDING_TTL_DAYS",
        "MAX_JSONL_LINES_PER_FILE",
        "MAX_CSV_ROWS_PER_FILE",
        "MAX_SQLITE_TABLES_PER_FILE",
        "MAX_SQLITE_ROWS_PER_FILE",
        "MAX_SQLITE_ROWS_PER_TABLE",
        "MAX_SIDE_CAR_FILES_PER_SOURCE",
        "MAX_RECURSIVE_JSON_DEPTH",
        "MAX_EVENTS_PER_FILE",
        "STRUCTURE_PROBE_OUTPUT_KIND",
        "DEFAULT_STRUCTURE_PROBE_AGENTS",
        "MAX_STRUCTURE_PROBE_BYTES_PER_FILE",
        "UsageProbeOptions",
        "UsageProbeReport",
        "StructureProbeAgentReport",
        "probe_now",
        "selected_probe_tools",
        "probe_source_structure",
        "probe_sqlite_file",
        "probe_json_file",
        "redacted_file_ref",
        "redacted_json_key_segment",
        "unknown_probe_source",
        "ScanWindow",
        "FileScanPlan",
        "ScanCandidate",
        "usage_scan_file_cache",
        "ensure_usage_scan_file_cache_schema",
        "usage_rollup_sources",
        "ensure_usage_rollup_sources_schema",
        "replace_rollup_source",
        "usage_outbox",
        "cleanup_usage_outbox",
        "delete_synced_outbox_rows",
        "prune_failed_usage_outbox",
        "prune_expired_usage_outbox",
        "prune_usage_outbox_rows",
        "prune_usage_outbox_payload_bytes",
        "rollup_item_payload_sha256",
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
        "ImportSourcesManifest",
        "collect_import_manifest_files",
        "import_manifest_entries",
        "resolve_import_manifest_path",
        "is_disallowed_native_file",
        "scan_native_sqlite_file",
        "scan_typed_native_sqlite_file",
        "scan_session_message_sqlite_file",
        "scan_opencode_kilo_sqlite_file",
        "scan_opencode_kilo_plural_sqlite_file",
        "scan_kilo_storage_file",
        "kilo_storage_layout",
        "is_kilo_storage_session_entry_file",
        "collect_kilo_storage_session_diff",
        "emit_opencode_kilo_embedded_parts",
        "scan_opencode_files_table",
        "scan_opencode_kilo_part_table",
        "scan_opencode_kilo_session_input_table",
        "scan_opencode_kilo_session_message_table",
        "emit_opencode_kilo_session_message_events",
        "merge_opencode_kilo_account_contexts",
        "scan_hermes_sqlite_file",
        "scan_goose_sqlite_file",
        "scan_crush_sqlite_file",
        "scan_zed_threads_sqlite_file",
        "emit_structured_content_events",
        "normalize_structured_content_event",
        "normalize_sqlite_message_event",
        "collect_native_json_value",
        "collect_native_transcript_value",
        "scan_native_text_json_file",
        "scan_qoder_transcript_file",
        "collect_qoder_transcript_value",
        "scan_qoder_work_trace_file",
        "collect_qoder_work_trace_value",
        "scan_qoder_work_tool_result_file",
        "scan_qoder_work_sqlite_file",
        "scan_qoder_work_sqlite_event_table",
        "scan_qoder_chat_message_sqlite_file",
        "scan_codex_rollout_file",
        "CodexRolloutContext",
        "collect_codex_rollout_value",
        "normalize_codex_response_items",
        "scan_local_telemetry_file",
        "collect_local_telemetry_value",
        "scan_hook_jsonl_file",
        "collect_hook_event_value",
        "scan_cursor_file",
        "scan_qoder_family_file",
        "scan_trae_trajectory_file",
        "collect_trae_llm_interaction",
        "collect_trae_agent_step",
        "scan_claude_project_file",
        "collect_claude_project_value",
        "normalize_claude_content_block",
        "apply_native_event_context",
        "scan_mux_text_file",
        "collect_mux_message_value",
        "normalize_mux_message_part",
        "scan_pi_session_file",
        "collect_pi_session_value",
        "collect_pi_agent_message",
        "normalize_pi_message_events",
        "pi_text_from_content",
        "scan_cline_family_file",
        "collect_cline_family_history_message",
        "collect_cline_family_ui_message",
        "cline_family_event_context",
        "scan_cline_family_index_or_history_file",
        "collect_cline_task_dirs_from_value",
        "collect_cline_family_task_dir",
        "scan_openclaw_session_file",
        "collect_openclaw_session_value",
        "scan_opencode_export_file",
        "collect_opencode_export_value",
        "collect_opencode_export_message",
        "scan_gjc_session_file",
        "collect_gjc_session_value",
        "collect_gjc_agent_message_value",
        "normalize_gjc_edit_event",
        "scan_wukong_sqlite_file",
        "scan_wukong_steps_table",
        "scan_wukong_parallel_tool_calls_table",
        "scan_wukong_fork_agent_tasks_table",
        "scan_wukong_todos_table",
        "scan_amp_thread_file",
        "collect_amp_thread_message",
        "normalize_amp_thread_event",
        "scan_droid_settings_file",
        "collect_droid_session_event",
        "normalize_droid_session_event",
        "scan_codebuff_chat_file",
        "read_codebuff_run_state",
        "codebuff_run_state_source_cost",
        "codebuff_run_state_main_agent_state",
        "creditsUsed",
        "directCreditsUsed",
        "fileContext",
        "projectRoot",
        "scan_kimi_wire_file",
        "kimi_wire_envelopes",
        "kimi_wire_context_from_path",
        "normalize_kimi_wire_event",
        "scan_grok_session_file",
        "collect_grok_session_value",
        "collect_grok_usage_update",
        "normalize_grok_session_event",
        "usage_update",
        "rawInput",
        "rawOutput",
        "collect_simple_transcript_value",
        "normalize_simple_event_value",
        "is_message_wrapper",
        "local-sources",
        "aitrack-sources.json",
        "import_root_without_manifest_skips_loose_files",
        "import_manifest_allows_explicit_relative_files",
        "import_manifest_rejects_escape_and_absolute_paths",
        "import_manifest_cannot_bypass_extension_window_or_max_bytes",
        "import_manifest_limits_entries_and_parse_errors",
        'Some("other")',
    ]:
        if symbol not in text:
            fail(f"usage parser missing {symbol}")
    for needle in [
        "usage_basis TEXT NOT NULL DEFAULT 'native'",
        "usage_basis: UsageBasis",
        "UsageBasis::Native",
        "UsageBasis::LocalDerived",
        "derive_local_usage",
    ]:
        if needle not in text:
            fail(f"usage parser missing usage_basis/local-derived rollup support {needle}")
    for forbidden in [
        "inject_native_monitoring_fixture",
        "monitoring_event_values",
        "aitrack_fixture_events",
        "CREATE TABLE IF NOT EXISTS aitrack_fixture_events",
    ]:
        if forbidden in text:
            fail(f"usage parser/test matrix must not use injected monitoring proof: {forbidden}")
    if "collect_files_with_extension" in text:
        fail("codex quota reader must not use unbounded extension recursion")
    collect_supported_match = re.search(
        r"fn collect_supported_files\([\s\S]*?\n}\n\nfn collect_import_manifest_files",
        text,
    )
    if not collect_supported_match:
        fail("collect_supported_files must hand import roots to manifest helper")
    collect_supported_body = collect_supported_match.group(0)
    if "root.kind == ScanRootKind::Import" not in collect_supported_body:
        fail("import roots must be detected before native recursive directory traversal")
    if "collect_import_manifest_files(tool, &root.path, plan, collector)" not in collect_supported_body:
        fail("import roots must use aitrack-sources.json manifest collection")
    import_helper_match = re.search(
        r"fn collect_import_manifest_files\([\s\S]*?\n}\n\nfn import_manifest_entries",
        text,
    )
    if not import_helper_match:
        fail("import manifest collection helper missing")
    import_helper_body = import_helper_match.group(0)
    if "fs::read_dir" in import_helper_body:
        fail("import manifest helper must not recursively scan import root directories")
    for needle in [
        "root.join(IMPORT_MANIFEST_FILE)",
        "read_small_text_file(path, MAX_IMPORT_MANIFEST_BYTES)?",
        "manifest.files.len() > MAX_IMPORT_MANIFEST_ENTRIES",
        "relative.is_absolute()",
        "Component::ParentDir",
        "canonical.starts_with(root_canonical)",
        "is_import_file(&candidate)",
    ]:
        if needle not in text:
            fail(f"import manifest safety contract missing {needle}")
    for needle in [
        "struct SourceFile",
        "kind: ScanRootKind",
        "fn scan_source_file(tool: &str, file: &SourceFile)",
        "fn scan_text_json_file(tool: &str, path: &Path, source_kind: ScanRootKind)",
        "fn scan_sqlite_file(tool: &str, path: &Path, source_kind: ScanRootKind)",
        "fn collect_from_json_value(",
        "source_kind: ScanRootKind",
        "if source_kind == ScanRootKind::Native",
        "ScanRootKind::Import",
    ]:
        if needle not in text:
            fail(f"native/import scan split missing {needle}")
    native_json_match = re.search(
        r"fn collect_from_json_value\([\s\S]*?\n}\n\nfn collect_native_json_value",
        text,
    )
    if not native_json_match:
        fail("collect_from_json_value native split body not found")
    native_json_body = native_json_match.group(0)
    if "collect_usage_recursive(" in native_json_body.split("if source_kind == ScanRootKind::Native", 1)[1].split("return;", 1)[0]:
        fail("native JSON source path must not call generic usage recursion before native dispatch")
    native_dispatch_match = re.search(
        r"fn scan_native_text_json_file\([\s\S]*?\n}\n\n#\[derive\(Default\)\]",
        text,
    )
    if not native_dispatch_match:
        fail("scan_native_text_json_file typed dispatch body not found")
    if "scan_native_text_json_values(" in native_dispatch_match.group(0):
        fail("native JSON typed dispatch must not call generic native JSON value scanning")
    if "fn scan_native_text_json_values" in text or "scan_native_text_json_values(" in text:
        fail("generic native JSON value scanner must not exist; add typed agent parsers instead")
    sqlite_match = re.search(r"fn scan_sqlite_file\([\s\S]*?\n}\n\nfn scan_native_sqlite_file", text)
    if not sqlite_match:
        fail("scan_sqlite_file body not found")
    sqlite_body = sqlite_match.group(0)
    if "if source_kind == ScanRootKind::Native {\n        return Ok(result);" not in sqlite_body:
        fail("native SQLite path must return empty instead of falling back to generic table scan")
    native_sqlite_match = re.search(
        r"fn scan_native_sqlite_file\([\s\S]*?\n}\n\nfn scan_typed_native_sqlite_file",
        text,
    )
    if not native_sqlite_match:
        fail("scan_native_sqlite_file typed-only body not found")
    native_sqlite_body = native_sqlite_match.group(0)
    if "scan_typed_native_sqlite_file(tool, path, conn, result)" not in native_sqlite_body:
        fail("native SQLite path must route only through typed readers")
    for forbidden in ["fn native_sqlite_tables", "fn scan_native_sqlite_table", "scan_native_sqlite_table("]:
        if forbidden in text:
            fail(f"native SQLite path must not keep generic table fallback: {forbidden}")
    if "scan_grok_sqlite_file" in text or "grok.db" in text:
        fail("grok native support must not claim an unevidenced grok.db SQLite schema")
    codex_reader_match = re.search(
        r"fn scan_native_text_json_file\([\s\S]*?\n}\n\n#\[derive\(Default\)\]",
        text,
    )
    if not codex_reader_match or '"codex" => Ok(scan_codex_rollout_file' not in codex_reader_match.group(0):
        fail("codex native JSONL must use the stateful rollout reader")
    native_text_body = codex_reader_match.group(0)
    native_text_function_body = native_text_body.split("\n}\n\n#[derive(Default)]", 1)[0]
    default_arm = (
        native_text_function_body.rsplit("_ =>", 1)[-1]
        if "_ =>" in native_text_function_body
        else ""
    )
    if "scan_native_text_json_values" in default_arm:
        fail("unknown native text/JSON agent path must return empty, not generic JSON fallback")
    if "_ => Ok(ScanResult::default())" not in native_text_function_body:
        fail("unknown native text/JSON agent path must explicitly return an empty scan result")
    for needle in [
        '"custom_tool_call"',
        '"custom_tool_call_output"',
        '"web_search_call"',
        '"tool_search_call"',
        '"tool_search_output"',
        '"reasoning"',
        '"user_message"',
        '"agent_message"',
        '"agent_reasoning"',
        '"patch_apply_end"',
        "normalize_codex_patch_apply_events",
        "codex_patch_apply_event",
        "emit_codex_token_snapshot_event",
        "DIFF_HUNK_KEYS",
        "CONTEXT_WINDOW_KEYS",
        "model_context_window",
        '"last_token_usage"',
        '"total_token_usage"',
        '"rate_limits"',
        "count_unified_diff_hunk_lines",
        '"call_id"',
    ]:
        if needle not in text:
            fail(f"codex rollout reader must preserve real transcript event shape: {needle}")
    if '"claude" => Ok(scan_claude_project_file' not in codex_reader_match.group(0):
        fail("claude native JSONL must use the typed project transcript reader")
    for needle in ['"thinking"', '"redacted_thinking"', '"agent_reasoning"']:
        if needle not in text:
            fail(f"claude transcript reasoning coverage missing {needle}")
    gemini_reader_match = re.search(
        r'"gemini"\s*=>\s*\{([\s\S]*?)\n\s*\}\s*\n\s*"qwen"\s*=>',
        codex_reader_match.group(0),
    )
    if gemini_reader_match is None:
        fail("gemini native text reader must split telemetry.log from ChatRecording JSONL")
    gemini_reader_body = gemini_reader_match.group(1)
    if re.search(
        r'if\s+lowered\.ends_with\("/telemetry\.log"\)\s*\{\s*'
        r'Ok\(scan_local_telemetry_file\(\s*"gemini"\s*,',
        gemini_reader_body,
    ) is None:
        fail('gemini telemetry.log must route to scan_local_telemetry_file("gemini", ...)')
    if "fn is_gemini_chat_record_path" not in text:
        fail("gemini ChatRecording path helper is_gemini_chat_record_path is missing")
    for needle in [
        "struct GeminiChatRecordContext",
        "merge_gemini_chat_record_context",
        "gemini_chat_record_tokens",
        "gemini_record_content_role",
        "emit_gemini_tool_call_record_events",
        '"gemini"',
        '"toolCalls"',
        '"tokens"',
    ]:
        if needle not in text:
            fail(f"gemini ChatRecording reader must cover current official JSONL shape: {needle}")
    if re.search(
        r"else\s+if\s+is_gemini_chat_record_path\(&lowered,\s*ext\)\s*\{\s*"
        r"Ok\(scan_gemini_chat_record_file\(\s*path,\s*text,\s*ext,\s*modified_ms\s*\)\)",
        gemini_reader_body,
    ) is None:
        fail(
            "gemini ChatRecording JSONL must route through is_gemini_chat_record_path "
            "to scan_gemini_chat_record_file"
        )
    if '"qwen" => Ok(scan_qwen_text_file' not in codex_reader_match.group(0):
        fail("qwen native text reader must split telemetry.log from project chat JSONL")
    for needle in [
        "fn scan_qwen_text_file",
        'lowered.ends_with("/telemetry.log")',
        'lowered.ends_with("/usage_record.jsonl")',
        "is_qwen_token_usage_record_path",
        "fn scan_qwen_token_usage_record_file",
        "fn collect_qwen_token_usage_record_value",
        "schemaVersion",
        "inputTokens",
        "outputTokens",
        "cachedTokens",
        "thoughtsTokens",
        "fn scan_qwen_chat_record_file",
        "fn collect_qwen_chat_record_value",
        "fn scan_qwen_usage_record_file",
        "fn collect_qwen_usage_record_value",
        'lowered.contains("/projects/")',
        'lowered.contains("/chats/")',
    ]:
        if needle not in text:
            fail(f"qwen native reader missing session jsonl path support: {needle}")
    if '"copilot" => Ok(scan_copilot_text_file' not in codex_reader_match.group(0):
        fail("copilot native text files must route through scan_copilot_text_file")
    for needle in [
        "fn scan_copilot_official_runtime_event_file",
        "fn collect_copilot_official_runtime_value",
        "fn is_copilot_official_runtime_event_name",
        "fn emit_copilot_official_tool_start",
        "fn scan_copilot_session_state_file",
        '"deltaContent"',
        "scan_local_telemetry_file(\"copilot\"",
    ]:
        if needle not in text:
            fail(f"copilot native text reader missing {needle}")
    if "collect_copilot_official_runtime_value" not in text.split("fn scan_copilot_text_file", 1)[1].split("fn scan_copilot_chat_session_file", 1)[0]:
        fail("copilot official runtime events must route through typed parser before generic telemetry")
    if '"cursor" => Ok(scan_cursor_file' not in codex_reader_match.group(0):
        fail("cursor native JSON must use scan_cursor_file")
    agent_text = read("client/src/agent.rs")
    cursor_transcript = local_source_spec_block(agent_text, "cursor", "agent-transcripts-jsonl")
    if cursor_transcript is None:
        fail("cursor/agent-transcripts-jsonl source spec missing")
    if "LocalSourceKind::SessionJsonl" not in cursor_transcript:
        fail("cursor/agent-transcripts-jsonl must be a SessionJsonl source")
    if "LOCAL_DERIVED_TRANSCRIPT_CAPABILITIES" not in cursor_transcript:
        fail("cursor/agent-transcripts-jsonl must stay local-derived until a native usage schema is proven")
    for needle in [
        "scan_cursor_agent_transcript_file",
        "collect_cursor_agent_transcript_record",
        'lowered.contains("/.cursor/projects/")',
        'lowered.contains("/agent-transcripts/")',
        'lowered.contains("/workspacestorage/")',
    ]:
        if needle not in text:
            fail(f"cursor native matcher must cover official local storage path: {needle}")
    if '"trae" => Ok(scan_trae_trajectory_file' not in codex_reader_match.group(0):
        fail("trae native JSON must use scan_trae_trajectory_file")
    cursor_roots_match = re.search(
        r'"cursor"\s*=>\s*\{(?P<body>[\s\S]*?)\n\s*\}\s*\n\s*"trae"\s*=>',
        agent_text,
    )
    if not cursor_roots_match:
        fail("cursor default scan roots branch missing")
    cursor_roots_body = cursor_roots_match.group("body")
    for needle in [
        '".cursor/projects"',
        '"Library/Application Support/Cursor/User/workspaceStorage"',
        '".config/Cursor/User/workspaceStorage"',
    ]:
        if needle not in cursor_roots_body:
            fail(f"cursor default scan roots must cover official local storage path: {needle}")
    trae_roots_match = re.search(
        r'"trae"\s*=>\s*\{(?P<body>[\s\S]*?)\n\s*\}\s*\n\s*"opencode"\s*=>',
        agent_text,
    )
    if not trae_roots_match:
        fail("trae default scan roots branch missing")
    trae_roots_body = trae_roots_match.group("body")
    for needle in ['std::env::current_dir()', 'cwd.join("trajectories")', 'home.join("trajectories")']:
        if needle not in trae_roots_body:
            fail(f"trae default scan roots must cover official cwd-relative trajectories: {needle}")
    qwen_roots_match = re.search(
        r'"qwen"\s*=>\s*\{(?P<body>[\s\S]*?)\n\s*\}\s*\n\s*"codebuff"\s*=>',
        agent_text,
    )
    if not qwen_roots_match:
        fail("qwen default scan roots branch missing")
    qwen_roots_body = qwen_roots_match.group("body")
    for needle in [
        '"QWEN_HOME"',
        '"QWEN_RUNTIME_DIR"',
        'qwen_runtime.join("projects")',
        'qwen_runtime.join("usage")',
    ]:
        if needle not in qwen_roots_body:
            fail(f"qwen default scan roots must cover official runtime storage: {needle}")
    if '"opencode" => Ok(scan_opencode_export_file' not in codex_reader_match.group(0):
        fail("opencode export JSON must use scan_opencode_export_file")
    opencode_sqlite = re.search(
        r"fn scan_opencode_kilo_sqlite_file\([\s\S]*?\n}\n\n#\[derive",
        text,
    )
    if not opencode_sqlite:
        fail("opencode/kilo typed sqlite reader body not found")
    for needle in [
        '"account"',
        "merge_opencode_kilo_account_contexts",
        'matches!(tool, "opencode" | "kilo" | "kilocode" | "kilo-code")',
        '"session_message"',
        "scan_opencode_kilo_session_message_table",
    ]:
        if needle not in opencode_sqlite.group(0):
            fail(f"opencode sqlite reader must consume v2 official table/context: {needle}")
    session_message_reader = re.search(
        r"fn scan_opencode_kilo_session_message_table\([\s\S]*?\n}\n\nfn emit_opencode_kilo_session_message_events",
        text,
    )
    if not session_message_reader:
        fail("opencode session_message reader body not found")
    if "emit_simple_transcript_events_only" in session_message_reader.group(0):
        fail("opencode session_message must not use simple transcript event fallback")
    session_message_events = re.search(
        r"fn emit_opencode_kilo_session_message_events\([\s\S]*?\n}\n\nfn sqlite_json_data_object",
        text,
    )
    if not session_message_events:
        fail("opencode session_message event emitter body not found")
    for needle in ['"content"', '"parts"', "normalize_opencode_kilo_part_events", "collect_opencode_kilo_embedded_tool_calls"]:
        if needle not in session_message_events.group(0):
            fail(f"opencode session_message event emitter must parse v2 content/parts: {needle}")
    account_merge = re.search(
        r"fn merge_opencode_kilo_account_contexts\([\s\S]*?\n}\n\nfn merge_missing_sqlite_context",
        text,
    )
    if not account_merge:
        fail("opencode/kilo account context merge helper not found")
    for needle in ['"providerID"', '"accountID"', "merge_missing_sqlite_context"]:
        if needle not in account_merge.group(0):
            fail(f"opencode/kilo account helper must preserve provider/account context: {needle}")
    if '"kiro" => Ok(scan_kiro_session_file' not in codex_reader_match.group(0):
        fail("kiro native JSON must use scan_kiro_session_file")
    if '"qoder" | "qoder-cn" | "qoder-work" | "qoder-work-cn" =>' not in codex_reader_match.group(0):
        fail("qoder family native JSON must use scan_qoder_family_file")
    if '"mux" => Ok(scan_mux_text_file' not in codex_reader_match.group(0):
        fail("mux native JSONL must use the typed chat/session reader")
    for needle in [
        'lowered.ends_with("/session-usage.json")',
        "fn scan_mux_session_usage_file",
        "fn mux_usage_bucket_tokens",
        "fn mux_usage_bucket_cost",
        'lowered.ends_with("/chat.jsonl")',
        'lowered.ends_with("/chat-archive.jsonl")',
        'lowered.ends_with("/partial.json")',
        '"chat.jsonl" | "chat-archive.jsonl" | "partial.json" | "session-usage.json"',
    ]:
        if needle not in text:
            fail(f"mux native official file coverage missing {needle}")
    if 'lowered.ends_with("/devtools.jsonl")' in text:
        fail("mux native official file filter must not accept devtools.jsonl")
    if '"pi" => Ok(scan_pi_session_file' not in codex_reader_match.group(0):
        fail("pi native JSONL must use the official session-format reader")
    if '"cline" | "roo-code" | "roocode" =>' not in codex_reader_match.group(0):
        fail("cline/roo native JSON must use the typed task transcript reader")
    if '"kilocode" | "kilo-code" =>' not in codex_reader_match.group(0):
        fail("kilocode native JSON must branch between typed task and Kilo storage readers")
    if "scan_kilo_storage_file(tool, path, text, ext, modified_ms)" not in codex_reader_match.group(0):
        fail("kilocode/kilo storage JSON must use the typed Kilo storage reader")
    if '"opencode" | "kilo" | "kilocode" | "kilo-code" =>' not in text:
        fail("kilocode native sqlite must route through the typed Kilo SQLite reader")
    if '"wukong" => scan_wukong_sqlite_file' not in text:
        fail("wukong native sqlite must use typed official sessions/steps reader")
    if '"antigravity" => scan_antigravity_cli_sqlite_file' not in text:
        fail("antigravity native sqlite must use typed CLI conversation reader")
    if '"zcode" => Ok(scan_zcode_project_file' not in codex_reader_match.group(0):
        fail("zcode native transcript must use projects JSONL reader")
    zcode_reader = re.search(
        r"fn scan_zcode_project_file\([\s\S]*?\n}\n\nfn collect_zcode_project_value",
        text,
    )
    if not zcode_reader:
        fail("zcode typed projects JSONL reader body not found")
    if "collect_simple_transcript_value" in zcode_reader.group(0):
        fail("zcode typed projects JSONL reader must not fall back to generic transcript parsing")
    zcode_typed = re.search(
        r"fn collect_zcode_project_value\([\s\S]*?\n}\n\nfn normalize_zcode_project_event",
        text,
    )
    if not zcode_typed:
        fail("zcode typed project collector not found")
    if "collect_zcode_project_value" not in zcode_reader.group(0):
        fail("zcode project reader must call collect_zcode_project_value")
    for forbidden in ['"zhipu"', '"glm-5.2"', 'Some("local".to_string())']:
        if forbidden in zcode_reader.group(0) or forbidden in zcode_typed.group(0):
            fail(f"zcode typed reader must not hardcode provider/model/account: {forbidden}")
    for needle in ['"tool_calls"', '"toolCalls"', '"tool_result"', '"patch"']:
        if needle not in text:
            fail(f"zcode typed parser/fixture must cover {needle}")
    for agent, reader in {
        "openclaw": "scan_openclaw_session_file",
        "amp": "scan_amp_thread_file",
        "droid": "scan_droid_settings_file",
        "codebuff": "scan_codebuff_chat_file",
        "kimi": "scan_kimi_wire_file",
    }.items():
        if f'"{agent}" => Ok({reader}' not in codex_reader_match.group(0):
            fail(f"{agent} native transcript must use {reader}")
    if '"wukong" => Ok(scan_wukong_session_file' in codex_reader_match.group(0):
        fail("wukong must not use generic messages.json/jsonl transcript reader")
    if '"antigravity" => Ok(scan_antigravity_session_file' in codex_reader_match.group(0):
        fail("antigravity must not use fabricated JSON/JSONL transcript reader")
    if '"grok" => Ok(scan_grok_session_file' not in codex_reader_match.group(0):
        fail("grok native session source must use scan_grok_session_file")
    grok_usage = re.search(
        r"fn collect_grok_usage_update\([\s\S]*?\n}\n\nfn normalize_grok_session_event",
        text,
    )
    if not grok_usage:
        fail("grok usage update handler not found")
    for needle in [
        "push_native_usage_message",
        "NativeUsageParts",
        "token_buckets_from_object",
        "source_cost",
    ]:
        if needle not in grok_usage.group(0):
            fail(f"grok usage_update must emit native usage rollup: {needle}")
    for forbidden in ["UsageBasis::LocalDerived", "collect_local_derived_usage_value"]:
        if forbidden in grok_usage.group(0):
            fail(f"grok usage_update must not fall back to local-derived usage: {forbidden}")
    for needle in ['"usage_update_unverified"', "emit_native_monitoring_event"]:
        if needle not in grok_usage.group(0):
            fail(f"grok usage_update must retain monitoring context: {needle}")
    if '"gjc" | "gajae-code" => Ok(scan_gjc_session_file' not in codex_reader_match.group(0):
        fail("gjc native transcript must use scan_gjc_session_file")
    agent_text = read("client/src/agent.rs")
    claude_roots_match = re.search(
        r'"claude"\s*=>\s*\{(?P<body>[\s\S]*?)\n\s*\}\s*\n\s*"codex"\s*=>',
        agent_text,
    )
    if not claude_roots_match:
        fail("claude default scan roots branch missing")
    claude_roots_body = claude_roots_match.group("body")
    for needle in ['"CLAUDE_CONFIG_DIR"', '.join("projects")', '.join("transcripts")']:
        if needle not in claude_roots_body:
            fail(f"claude default scan roots must cover configured transcript roots: {needle}")
    gjc_roots_match = re.search(
        r'"gjc" \| "gajae-code" => \{(?P<body>[\s\S]*?)\n\s*"grok" =>',
        agent_text,
    )
    if not gjc_roots_match:
        fail("gjc default scan roots branch missing")
    gjc_roots_body = gjc_roots_match.group("body")
    if '"GJC_CODING_AGENT_DIR"' not in gjc_roots_body or '.join("sessions")' not in gjc_roots_body:
        fail("GJC_CODING_AGENT_DIR must scan its sessions child, not the agent dir root")
    token_key_blocks = re.findall(r"const\s+\w*TOKEN_KEYS:\s*&\[&str\]\s*=\s*&\[[\s\S]*?\];", text)
    if any('"contextUsage"' in block or '"context_usage"' in block for block in token_key_blocks):
        fail("contextUsage tokens are context window metadata, not billing usage tokens")
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
        "outbox_upload_success_deletes_synced_payload_rows",
        "outbox_cleanup_prunes_failed_expired_and_payload_overflow",
        "old_rollup_payload_ack_does_not_clear_new_dirty",
        "latest_rollup_payload_ack_clears_dirty",
        "json_scan_extracts_output_tool_call_and_tool_result_monitoring_records",
        "json_scan_extracts_skill_approval_and_explicit_other_agent_events",
        "discovery_helpers_cover_roots_supported_files_and_skipped_dirs",
        "changed_source_replaces_previous_rollup_contribution",
        "source_rollup_keeps_database_bounded_for_many_messages",
        "goose_sqlite_reader_maps_string_and_structured_content_events",
        "zed_threads_sqlite_reader_maps_structured_messages_usage_and_edit",
        "kilo_official_sqlite_reader_maps_singular_schema_and_dedupes_step_finish_usage",
        "kilo_official_sqlite_step_finish_tokens_are_fallback_when_session_has_no_usage",
        "kilo_storage_json_reader_aggregates_session_messages_parts_and_diff",
        "kilocode_storage_json_reader_maps_legacy_session_info_layout",
        "kilocode_official_sqlite_reader_maps_singular_kilo_db_schema",
        "hermes_state_db_reader_maps_official_sessions_messages_tool_calls_and_tokens",
        "opencode_export_reader_maps_info_messages_parts_usage_and_tools",
        "mux_official_session_files_map_typed_local_fields",
        "pi_session_reader_maps_official_message_entries_usage_and_tool_results",
        "crush_sqlite_reader_emits_files_and_read_files_tables",
        "scan_budget_resumes_after_cached_frontier",
        "default_scan_uses_global_recent_queue_so_late_agents_are_not_starved",
        "copilot_session_store_db_maps",
        "structure_probe_redacts_json_sqlite_values_and_paths",
    ]:
        if test_name not in text:
            fail(f"missing usage parser regression test {test_name}")
    for event_type in ['"prompt"', '"output"', '"tool"', '"tool_result"', '"edit"']:
        if event_type not in text:
            fail(f"native matrix must assert monitoring event type {event_type}")
    if "collect_openclaw_or_gjc_value" in text:
        fail("openclaw and gjc must use distinct typed collectors, not a shared fallback")
    for needle in [
        '"accumulated_cost"',
        '"model_config_json"',
        '"thinking"',
        '"agent_reasoning"',
        '"reasoning_content"',
        '"reasoning_details"',
        "emit_hermes_reasoning_events",
    ]:
        if needle not in text:
            fail(f"official SQLite reasoning/cost coverage missing {needle}")
    if '"copilot" => scan_copilot_sqlite_file' not in text:
        fail("copilot native sqlite must dispatch through the typed Copilot sqlite reader")
    if "scan_copilot_vscode_state_sqlite_file(tool, path, conn, result)" not in text:
        fail("copilot VS Code chat state must still use typed ItemTable reader")
    for needle in [
        "fn scan_copilot_session_store_sqlite_file",
        "COPILOT_SESSION_STORE_TABLES",
        '"session-store.db"',
        '"sessions"',
        '"session"',
        '"messages"',
        '"message"',
        '"tool_calls"',
        '"tool_call"',
        '"tool_results"',
        '"tool_result"',
        '"modified_files"',
        '"files"',
        "scan_copilot_session_store_session_table",
        "scan_copilot_session_store_event_table",
        "copilot_session_store_row_payload",
    ]:
        if needle not in text:
            fail(f"copilot session-store.db typed reader missing {needle}")
    for needle in [
        "interactive.sessions",
        "chat.ChatSessionStore.index",
        "agentSessions.model.cache",
        "collect_copilot_chat_session_store_index",
        "scan_copilot_chat_session_file",
        "toolInvocationSerialized",
        "textEditGroup",
        "copilot_message_text",
    ]:
        if needle not in text:
            fail(f"copilot typed chat state reader missing {needle}")
    openclaw_collector = re.search(
        r"fn collect_openclaw_session_value\([\s\S]*?\n}\n\nfn emit_openclaw_content_block_events",
        text,
    )
    if not openclaw_collector:
        fail("openclaw typed collector not found")
    openclaw_body = openclaw_collector.group(0)
    if "resolve_openclaw_session_file" not in text:
        fail("openclaw sessions.json reader must safely resolve sessionFile")
    for needle in ["emit_local_source_gap_event", '"local_source_gap"', '"unreadable_session_file"']:
        if needle not in text:
            fail(f"openclaw sessions.json reader must report missing transcript gaps: {needle}")
    for needle in ['"lastAccountId"', '"accountId"']:
        if needle not in text:
            fail(f"openclaw sessions.json reader must preserve official account context: {needle}")
    resolver = re.search(
        r"fn resolve_openclaw_session_file\([\s\S]*?\n}\n\nfn scan_gjc_session_file",
        text,
    )
    if not resolver:
        fail("openclaw sessionFile resolver body not found")
    for needle in ["raw_path.is_absolute()", "starts_with(sessions_dir)", "ParentDir"]:
        if needle not in resolver.group(0):
            fail(f"openclaw sessionFile resolver must safely allow in-dir absolute paths: {needle}")
    for needle in [
        'Some("tool_call")',
        "normalize_tool_call_value",
    ]:
        if needle not in openclaw_body:
            fail(f"openclaw typed collector missing top-level tool call support: {needle}")
    if "normalize_openclaw_content_block" not in text:
        fail("openclaw typed collector missing content block normalization helper")
    openclaw_block_helper = re.search(
        r"fn normalize_openclaw_content_block\([\s\S]*?\n}\n\nfn enrich_openclaw_edit_event_from_tool_arguments",
        text,
    )
    if not openclaw_block_helper:
        fail("openclaw content block normalization helper not found")
    for needle in ['"toolCall"', '"toolResult"']:
        if needle not in openclaw_block_helper.group(0):
            fail(f"openclaw content block helper missing {needle}")
    if '"synthetic" => scan_synthetic_sqlite_file' not in text:
        fail("synthetic native sqlite must use typed llm_irs reader")
    if '"warp" => scan_warp_sqlite_file' not in text:
        fail("warp native sqlite must use typed warp.sqlite reader")
    antigravity_reader = re.search(
        r"fn scan_antigravity_cli_sqlite_file\([\s\S]*?\n}\n\nfn collect_antigravity_gen_metadata",
        text,
    )
    if not antigravity_reader:
        fail("antigravity CLI sqlite reader body not found")
    for needle in ["gen_metadata", "trajectory_metadata_blob", "steps", "step_payload"]:
        if needle not in antigravity_reader.group(0):
            fail(f"antigravity CLI sqlite reader must consume {needle}")
    for needle in [
        "proto_first_message(chat_model, 4)",
        "proto_first_message(chat_model, 9)",
        "proto_message_fields(chat_model, 2)",
        "antigravity_step_tool_call",
        "antigravity_is_tool_step_type",
    ]:
        if needle not in text:
            fail(f"antigravity CLI sqlite reader must decode protobuf field path: {needle}")
    warp_reader = re.search(
        r"fn scan_warp_sqlite_file\([\s\S]*?\n}\n\n#\[allow\(clippy::too_many_arguments\)\]\nfn collect_warp_conversation_usage",
        text,
    )
    if not warp_reader:
        fail("warp native sqlite reader body not found")
    for needle in ["agent_conversations", "agent_tasks", "conversation_data", "task"]:
        if needle not in warp_reader.group(0):
            fail(f"warp native sqlite reader must consume Warp table/column {needle}")
    warp_usage_reader = re.search(
        r"fn collect_warp_conversation_usage\([\s\S]*?\n}\n\nfn warp_model_usage_total_tokens",
        text,
    )
    if not warp_usage_reader:
        fail("warp conversation usage reader body not found")
    for needle in [
        "conversation_usage_metadata",
        "token_usage",
        "warp_model_usage_total_tokens",
    ]:
        if needle not in warp_usage_reader.group(0):
            fail(f"warp native sqlite reader must consume Warp usage field {needle}")
    warp_usage_total = re.search(
        r"fn warp_model_usage_total_tokens\([\s\S]*?\n}\n\nfn collect_warp_task_proto",
        text,
    )
    if not warp_usage_total:
        fail("warp token total helper body not found")
    for needle in [
        "warp_tokens",
        "byok_tokens",
        "custom_endpoint_tokens",
        "warp_token_usage_by_category",
        "byok_token_usage_by_category",
        "custom_endpoint_token_usage_by_category",
    ]:
        if needle not in warp_usage_total.group(0):
            fail(f"warp native sqlite reader must preserve Warp token total/category field {needle}")
    for needle in [
        "fn collect_warp_task_proto",
        "fn proto_fields",
        "warp_tool_call_event",
        "warp_tool_result_event",
        "apply_file_diffs",
        "warp_input_context_workspace",
    ]:
        if needle not in text:
            fail(f"warp native sqlite reader must decode protobuf task messages: {needle}")
    synthetic_reader = re.search(
        r"fn scan_synthetic_sqlite_file\([\s\S]*?\n}\n\nfn collect_synthetic_llm_ir_value",
        text,
    )
    if not synthetic_reader:
        fail("synthetic native sqlite reader body not found")
    for needle in ["history_items", "tree_nodes", "trees", "llm_irs"]:
        if needle not in synthetic_reader.group(0):
            fail(f"synthetic native sqlite reader must join Octofriend {needle}")
    synthetic_ir_collector = re.search(
        r"fn collect_synthetic_llm_ir_value\([\s\S]*?\n}\n\n#\[allow\(clippy::too_many_arguments\)\]\nfn collect_synthetic_compiler_usage",
        text,
    )
    if not synthetic_ir_collector:
        fail("synthetic llm_ir collector not found")
    for needle in [
        "tool-output",
        "tool-runtime-error",
        "tool-validation-error",
        "tool-skip-output",
        "tool-invoke-subagent",
    ]:
        if needle not in synthetic_ir_collector.group(0):
            fail(f"synthetic llm_ir collector must handle Octofriend role {needle}")
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
        "MAX_SCAN_CANDIDATES_PER_RUN: usize = 800": "global candidate cap must stay bounded",
        "MAX_SCAN_CANDIDATES_PER_AGENT: usize = 800": "per-agent candidate cap must stay bounded",
        "MAX_SCAN_DIR_ENTRIES_PER_RUN: usize = 5000": "global directory traversal cap must stay bounded",
        "MAX_SCAN_DIR_ENTRIES_PER_AGENT: usize = 5000": "directory traversal cap must stay bounded",
        "MAX_IMPORT_MANIFEST_BYTES: u64 = 64 * 1024": "import manifest byte cap must stay bounded",
        "MAX_IMPORT_MANIFEST_ENTRIES: usize = 200": "import manifest entry cap must stay bounded",
        "MAX_USAGE_SCAN_FILE_CACHE_ROWS: usize = 20_000": "scan file cache row cap must stay bounded",
        "MAX_USAGE_MONITORING_SEEN_ROWS: usize = 50_000": "monitoring seen row cap must stay bounded",
        "MAX_USAGE_ROLLUP_SOURCE_ROWS: usize = 20_000": "rollup source row cap must stay bounded",
        "MAX_USAGE_OUTBOX_ROWS: i64 = 2_000": "usage outbox row cap must stay bounded",
        "MAX_USAGE_OUTBOX_PAYLOAD_BYTES: i64 = 16 * 1024 * 1024": "usage outbox payload byte cap must stay bounded",
        "MAX_OUTBOX_RETRY_COUNT: i64 = 5": "usage outbox failed retry cap must stay bounded",
        "USAGE_OUTBOX_PENDING_TTL_DAYS: i64 = 7": "usage outbox pending TTL must stay bounded",
        "MAX_JSONL_LINES_PER_FILE: usize = 2000": "jsonl line cap must stay bounded",
        "MAX_CSV_ROWS_PER_FILE: usize = 2000": "csv row cap must stay bounded",
        "MAX_SQLITE_TABLES_PER_FILE: usize = 10": "sqlite table cap must stay bounded",
        "MAX_SQLITE_ROWS_PER_FILE: usize = 5000": "sqlite row cap per file must stay bounded",
        "MAX_SQLITE_ROWS_PER_TABLE: usize = 2000": "sqlite row cap per table must stay bounded",
        "MAX_SIDE_CAR_FILES_PER_SOURCE: usize = 128": "sidecar fan-out cap must stay bounded",
        "MAX_RECURSIVE_JSON_DEPTH: usize = 32": "recursive JSON collector depth must stay bounded",
        "MAX_EVENTS_PER_FILE: usize = 200": "monitoring events per file must stay bounded",
    }
    for needle, message in expected_limits.items():
        if needle not in text:
            fail(message)
    for forbidden in [
        "fs::read_to_string(run_state_path)",
        "fs::read_to_string(&state_path)",
        "fs::read_to_string(settings_path)",
        "fs::read_to_string(path).ok()?",
    ]:
        if forbidden in text:
            fail(f"sidecar or manifest reader must use read_small_text_file, found {forbidden}")
    for needle in [
        "read_small_text_file(path, MAX_IMPORT_MANIFEST_BYTES)?",
        "read_small_text_file(&settings_path, MAX_JSON_BYTES)?",
        "read_small_text_file(&run_state_path, MAX_JSON_BYTES)",
        "read_small_text_file(&state_path, MAX_JSON_BYTES)",
        "fn collect_usage_recursive_with_depth",
        "fn collect_events_recursive_with_depth",
        "if depth > MAX_RECURSIVE_JSON_DEPTH",
    ]:
        if needle not in text:
            fail(f"bounded sidecar/recursive scan contract missing {needle}")
    native_file_match = re.search(
        r"fn is_native_file\([\s\S]*?\n}\n\nfn is_kiro_native_candidate_file",
        text,
    )
    if not native_file_match:
        fail("is_native_file must dispatch to explicit Kiro/Amp native candidate helpers")
    native_file_body = native_file_match.group(0)
    if '"kiro"' not in native_file_body or '"zed"' not in native_file_body:
        fail("kiro native file matcher branch not found")
    kiro_native_branch_text = native_file_body.split('"kiro"', 1)[1].split('"zed"', 1)[0]
    for forbidden in [
        'lowered.ends_with(".jsonl")',
        'lowered.contains("/kiro.kiroagent/")',
        'lowered.contains("/kiro.kiro-agent/")',
    ]:
        if forbidden in kiro_native_branch_text:
            fail(
                f"kiro native matcher must not use over-broad persisted source proof: {forbidden}"
            )
    if "is_kiro_native_candidate_file(&lowered, ext)" not in kiro_native_branch_text:
        fail("kiro native matcher must use explicit candidate file helper")
    kiro_helper = re.search(
        r"fn is_kiro_native_candidate_file\([\s\S]*?\n}\n\nfn is_amp_runtime_surface_file",
        text,
    )
    if not kiro_helper:
        fail("kiro explicit native candidate helper missing")
    for needle in [
        'lowered_path.ends_with("/sessions.db")',
        'lowered_path.ends_with("/data.sqlite3")',
        'ext == "json" && lowered_path.contains("/sessions/cli/")',
    ]:
        if needle not in kiro_helper.group(0):
            fail(f"kiro native helper missing explicit candidate {needle}")
    for forbidden in ['kiro.kiroagent', 'kiro.kiro-agent', 'ends_with(".jsonl")']:
        if forbidden in kiro_helper.group(0):
            fail(f"kiro native helper must not accept broad Kiro/globalStorage paths: {forbidden}")

    if '"amp"' not in native_file_body or '"droid"' not in native_file_body:
        fail("amp native file matcher branch not found")
    amp_native_branch_text = native_file_body.split('"amp"', 1)[1].split('"droid"', 1)[0]
    for forbidden in [
        'lowered.contains("/amp/")',
        'lowered.contains("/.amp/")',
        'lowered.contains("/threads/") && ext == "json"',
    ]:
        if forbidden in amp_native_branch_text:
            fail(f"amp native matcher must not use over-broad /amp/ proof: {forbidden}")
    if "is_amp_runtime_surface_file(&lowered, ext)" not in amp_native_branch_text:
        fail("amp native matcher must use explicit runtime surface helper")
    amp_helper = re.search(
        r"fn is_amp_runtime_surface_file\([\s\S]*?\n}\n\nfn is_gemini_chat_record_path",
        text,
    )
    if not amp_helper:
        fail("amp explicit runtime surface helper missing")
    for needle in [
        'lowered_path.contains("/.amp/sessions/")',
        'lowered_path.contains("/amp/sessions/")',
        'lowered_path.contains("/.amp/threads/")',
        'lowered_path.contains("/amp/threads/")',
        'matches!(ext, "jsonl" | "ndjson" | "log")',
    ]:
        if needle not in amp_helper.group(0):
            fail(f"amp runtime surface helper missing scoped candidate {needle}")
    for forbidden in ['lowered.contains("/amp/")', 'lowered.contains("/.amp/")']:
        if forbidden in amp_helper.group(0):
            fail(f"amp runtime surface helper must not use broad /amp/ matcher: {forbidden}")

    kiro_sqlite_match = re.search(
        r"fn scan_kiro_sqlite_file\([\s\S]*?\n}\n\nfn scan_antigravity_cli_sqlite_file",
        text,
    )
    if not kiro_sqlite_match:
        fail("kiro SQLite reader missing")
    for needle in [
        '"conversations_v2"',
        "emit_simple_transcript_events_only",
        "collect_simple_transcript_value",
    ]:
        if needle not in kiro_sqlite_match.group(0):
            fail(f"kiro SQLite reader must cover conversations_v2 payloads: {needle}")
    for forbidden in [
        "unwrap_or_else(|| Value::Object(row.clone()))",
        "payload.as_object().cloned().unwrap_or_else(|| row.clone())",
    ]:
        if forbidden in kiro_sqlite_match.group(0):
            fail(
                "kiro SQLite reader must not treat an arbitrary whole row as "
                f"transcript proof: {forbidden}"
            )
    for needle in [
        "updated_at: String",
        "payload_sha256: String",
        "row.payload_sha256 != rollup_item_payload_sha256(&row)",
        "delete_synced_outbox_rows(conn)",
        "prune_usage_outbox_payload_bytes(conn, MAX_USAGE_OUTBOX_PAYLOAD_BYTES)",
    ]:
        if needle not in text:
            fail(f"usage outbox/rollup ack bounded contract missing {needle}")

    schema_text = read("client/src/adapter/sqlite/schema.rs")
    if "idx_record_sig" not in schema_text:
        fail("records sqlite schema must index record_sig for bounded dedup lookups")


def assert_structure_probe_contract() -> None:
    usage = read("client/src/usage/mod.rs")
    cli = read("client/src/cli.rs")
    main_rs = read("client/src/main.rs")
    if "Probe(UsageProbeArgs)" not in cli:
        fail("usage probe CLI subcommand missing")
    if 'a == "usage" && second_arg.as_deref() == Some("probe")' not in main_rs:
        fail("usage probe must skip the banner so stdout remains JSON")
    match = re.search(
        r"DEFAULT_STRUCTURE_PROBE_AGENTS:\s*&\[&str\]\s*=\s*&\[([\s\S]*?)\];",
        usage,
    )
    if not match:
        fail("DEFAULT_STRUCTURE_PROBE_AGENTS constant not found")
    defaults = match.group(1)
    for agent in [
        "cursor",
        "copilot",
        "cline",
        "roo-code",
        "kiro",
        "zed",
        "codebuff",
        "amp",
        "openclaw",
        "gjc",
        "droid",
        "kimi",
        "mux",
        "warp",
        "grok",
        "zcode",
        "synthetic",
        "antigravity",
        "qoder",
        "qoder-cn",
        "qoder-work",
        "qoder-work-cn",
        "wukong",
    ]:
        if f'"{agent}"' not in defaults:
            fail(f"structure probe default agents missing {agent}")
    for needle in [
        "scan_roots(&home, &selection.agent)",
        "collect_supported_files(&selection.agent",
        "source_format: probe_source_format(path).to_string()",
        "filename_sha256: stable_hash(file_name)",
        "Value::String(_) => \"string\"",
        "row_count_capped",
        "key_paths_limit_reached",
    ]:
        if needle not in usage:
            fail(f"structure probe safety contract missing {needle}")


def assert_data_volume_gates() -> None:
    sqlite_queries = read("client/src/adapter/sqlite/queries.rs")
    sqlite_mod = read("client/src/adapter/sqlite/mod.rs")
    uploader = read("client/src/uploader.rs")
    client_lib = read("client/src/lib.rs")
    capture_match = re.search(
        r"async fn handle_capture\([\s\S]*?\n}\n\nfn parse_capture_event",
        client_lib,
    )
    if not capture_match:
        fail("rust capture path handle_capture function not found")
    if "prune_local_record_storage(&conn)?" not in capture_match.group(0):
        fail("rust capture path must prune local record storage after capture")

    prompt_capture_match = re.search(
        r"async fn handle_prompt_capture\([\s\S]*?\n}\n\nfn prompt_capture_text",
        client_lib,
    )
    if not prompt_capture_match:
        fail("rust prompt-capture path handle_prompt_capture function not found")
    if "prune_local_record_storage(&conn)?" not in prompt_capture_match.group(0):
        fail("rust prompt-capture path must prune local record storage after prompt capture")

    for needle in [
        "MAX_SYNCED_RECORD_ROWS: i64 = 10_000",
        "MAX_PENDING_RECORD_ROWS: i64 = 1_000",
        "MAX_UNUPLOADABLE_PENDING_RECORD_ROWS: i64 = 250",
        "MAX_RETRY_EXHAUSTED_PENDING_RECORD_ROWS: i64 = 100",
        "MAX_PROMPT_CONTEXT_ROWS: i64 = 256",
        "strip_retry_exhausted_pending_original_text",
        "PRUNED_DIFF_HUNK_MARKER",
        "PRUNED_METADATA_MARKER",
        "PRUNED_PROMPT_MARKER",
        "pub fn prune_local_record_storage(conn: &Connection) -> Result<usize>",
    ]:
        if needle not in sqlite_queries:
            fail(f"rust local records retention contract missing {needle}")
    mark_synced_match = re.search(
        r"pub fn mark_synced\(conn: &Connection, ids: &\[i64\]\) -> Result<\(\)> \{[\s\S]*?\n\}\n\npub fn increment_retry",
        sqlite_queries,
    )
    if not mark_synced_match:
        fail("rust mark_synced function not found")
    mark_synced_body = mark_synced_match.group(0)
    for needle in [
        "SET synced = 1",
        "synced_at = datetime('now')",
        "diff_hunk = CASE",
        "metadata = CASE",
        "prompt_summary = CASE",
        "PRUNED_DIFF_HUNK_MARKER",
        "PRUNED_METADATA_MARKER",
        "PRUNED_PROMPT_MARKER",
    ]:
        if needle not in mark_synced_body:
            fail(f"rust mark_synced must atomically mark and strip raw text: {needle}")
    if "strip_synced_original_text" in sqlite_queries:
        fail("rust mark_synced raw stripping must not use a second update helper")
    if "prune_local_record_storage" not in sqlite_mod:
        fail("rust local records retention helper must be exported from sqlite adapter")
    for test_name in [
        "prune_local_record_storage_caps_unuploadable_pending_records",
        "prune_local_record_storage_strips_retry_exhausted_pending_text",
        "retained unuploadable row {idx} should keep diff_hunk",
        "retained unuploadable row {idx} should keep metadata",
        "retained unuploadable row {idx} should keep prompt_summary",
        "mark_synced_strips_original_text_fields",
        "increment_retry_keeps_pending_original_text_fields",
        "prompt_context_is_truncated_and_pruned",
    ]:
        if test_name not in sqlite_mod:
            fail(f"rust local records retention tests missing {test_name}")
    for test_name in [
        "flush_accepted_response_marks_synced",
        "flush_flagged_response_marks_synced",
        "flush_marks_synced_when_response_unparseable",
        "original_text_fields",
        "assert_original_text_fields_pruned",
        "[aitrack-pruned:diff_hunk]",
        "aitrack_pruned",
        "[aitrack-pruned:prompt_summary]",
    ]:
        if test_name not in uploader:
            fail(f"rust upload ack raw retention tests missing {test_name}")

    go_ingest = read("server-go/internal/application/ingest_usecase.go")
    go_port = read("server-go/internal/domain/port/edit_record_port.go")
    go_repo = read("server-go/internal/adapter/db/edit_record_adapter.go")
    go_ingest_test = read("server-go/internal/application/ingest_usecase_test.go")
    go_repo_test = read("server-go/internal/adapter/db/edit_record_retention_test.go")
    for needle in [
        "maxStoredDiffHunkChars = 8192",
        "maxStoredMetadataChars = 4096",
        "maxStoredPromptChars   = 4096",
        "applyEditRawRetention",
        "port.EditRecordRetentionPort",
        "ApplyRawRetention(time.Now().UTC())",
    ]:
        if needle not in go_ingest:
            fail(f"go ingest raw retention contract missing {needle}")
    for needle in [
        "type EditRecordRetentionPort interface",
        "ApplyRawRetention(now time.Time) (int64, error)",
    ]:
        if needle not in go_port:
            fail(f"go retention port missing {needle}")
    for needle in [
        "RawFieldStrippedMarker",
        "EditRawRetentionPolicy",
        "DefaultEditRawRetentionPolicy",
        "RawRetentionWindowDays: 30",
        "MaxRawRows:             10_000",
        "ApplyRawRetentionWithPolicy",
        "received_at <",
        "ORDER BY received_at DESC, id DESC",
        "diff_hunk = CASE",
        "metadata = CASE",
        "prompt_summary = CASE",
    ]:
        if needle not in go_repo:
            fail(f"go db raw retention helper missing {needle}")
    for test_name in [
        "TestIngest_AppliesRawRetentionAfterSuccessfulSave",
        "TestIngest_DuplicateRecordSigIsIdempotent",
        "TestApplyRawRetentionWithPolicyStripsOverflowRows",
    ]:
        if test_name not in (go_ingest_test + "\n" + go_repo_test):
            fail(f"go raw retention tests missing {test_name}")

    java_ingest = read("server-java/src/main/java/com/aitrack/server/application/IngestService.java")
    java_port = read("server-java/src/main/java/com/aitrack/server/domain/port/EditRecordPort.java")
    java_repo = read("server-java/src/main/java/com/aitrack/server/adapter/db/EditRecordRepository.java")
    java_view = read("server-java/src/main/java/com/aitrack/server/domain/model/EditRecordView.java")
    java_edit_migrator = read("server-java/src/main/java/com/aitrack/server/infrastructure/config/EditRecordSchemaMigrator.java")
    java_ingest_test = read("server-java/src/test/java/com/aitrack/server/application/IngestServiceTest.java")
    java_repo_ingest_test = read("server-java/src/test/java/com/aitrack/server/application/RepositoryIngestServiceTest.java")
    java_view_test = read("server-java/src/test/java/com/aitrack/server/domain/model/EditRecordViewTest.java")
    java_edit_migrator_test = read("server-java/src/test/java/com/aitrack/server/infrastructure/config/EditRecordSchemaMigratorTest.java")
    for needle in [
        "MAX_STORED_DIFF_HUNK_CHARS = 8192",
        "MAX_STORED_METADATA_CHARS = 4096",
        "MAX_STORED_PROMPT_CHARS = 4096",
        "boolean savedAny = false",
        "editRecordPort.applyRawRetention(Instant.now())",
    ]:
        if needle not in java_ingest:
            fail(f"java ingest raw retention contract missing {needle}")
    if "int applyRawRetention(Instant now)" not in java_port:
        fail("java retention port missing applyRawRetention")
    for needle in [
        "RAW_FIELD_STRIPPED_MARKER",
        "DEFAULT_RAW_RETENTION_DAYS = 30",
        "DEFAULT_MAX_RAW_ROWS = 10_000",
        "applyRawRetentionWithPolicy",
        "stripRawFieldsOlderThan",
        "stripRawFieldsBeyondNewestRows",
        "ORDER BY received_at DESC, id DESC LIMIT :maxRows",
        "diff_hunk = CASE",
        "metadata = CASE",
        "prompt_summary = CASE",
    ]:
        if needle not in java_repo:
            fail(f"java repository raw retention helper missing {needle}")
    for test_name in [
        "ingest_successfulSave_appliesRawRetention",
        "ingest_duplicateRecordSig_isAcceptedButNotPersistedAgain",
        "ingestAppliesRawRetentionAfterSuccessfulSaveAndKeepsRecentTruncatedText",
        "repositoryRetentionPolicyStripsRowsOutsideNewestWindowWithoutDeletingRows",
    ]:
        if test_name not in (java_ingest_test + "\n" + java_repo_ingest_test):
            fail(f"java raw retention tests missing {test_name}")
    for needle in [
        '@JsonProperty("prompt_summary")',
        "private String promptSummary",
        "v.promptSummary = e.getPromptSummary()",
    ]:
        if needle not in java_view:
            fail(f"java edit record view must expose prompt_summary like Go: {needle}")
    if "fromEntityIncludesPromptSummary" not in java_view_test:
        fail("java edit record view prompt_summary test missing")
    for needle in [
        "@Component(\"editRecordSchemaMigrator\")",
        "ALTER TABLE ",
        "ADD COLUMN IF NOT EXISTS prompt_summary TEXT",
        "EditRecordSchemaMigrationOrder",
    ]:
        if needle not in java_edit_migrator:
            fail(f"java edit_records additive migration missing {needle}")
    for test_name in [
        "addsPromptSummaryColumnToExistingEditRecordsTable",
        "postgresMigrationSqlMentionsPromptSummary",
    ]:
        if test_name not in java_edit_migrator_test:
            fail(f"java edit record schema migrator test missing {test_name}")

    java_usage_service = read("server-java/src/main/java/com/aitrack/server/application/UsageService.java")
    java_usage_repo = read("server-java/src/main/java/com/aitrack/server/adapter/db/UsageDailyRollupRepository.java")
    java_summary_test_path = ROOT / "server-java/src/test/java/com/aitrack/server/application/UsageServiceSummaryTest.java"
    if not java_summary_test_path.exists():
        fail("java usage summary bounded aggregation tests are missing")
    java_summary_test = java_summary_test_path.read_text(encoding="utf-8")
    summary_match = re.search(
        r"public UsageSummary summary\([\s\S]*?\n    private static",
        java_usage_service,
    )
    if not summary_match:
        fail("java usage summary method not found")
    summary_body = summary_match.group(0)
    if "dailyRollups.findByFilters(" in summary_body:
        fail("java usage summary must not use unpaged findByFilters for bounded summary aggregation")
    if "grouped.values().stream()" in summary_body or "Comparator.comparingLong(UsageSummaryItem::getTotalTokens)" in summary_body:
        fail("java usage summary must not do in-memory grouped stream sorting/limit")
    ingest_rollups_match = re.search(
        r"public void ingestRollups\([\s\S]*?\n    @Transactional\n    public void ingestSubscription",
        java_usage_service,
    )
    if not ingest_rollups_match:
        fail("java usage rollup ingest method not found")
    ingest_rollups_body = ingest_rollups_match.group(0)
    if ".findByTokenKeyAndDeviceIdAndDayAndAgentAndModelAndAccountAndUsageBasis(" in ingest_rollups_body:
        fail("java usage rollup ingest must not do per-item findBy upsert reads")
    for needle in [
        "jdbcTemplate.batchUpdate(rollupUpsertSql()",
        "ON CONFLICT (token_key, device_id, \"day\", agent, model, account, usage_basis)",
        "MERGE INTO usage_daily_rollups",
        "KEY(token_key, device_id, \"day\", agent, model, account, usage_basis)",
    ]:
        if needle not in java_usage_service:
            fail(f"java usage rollup ingest must use bounded batch upsert: {needle}")
    for needle in [
        "findSummaryTotals(",
        "findSummaryItems(",
        "Pageable",
        "SUM(",
        "GROUP BY",
        "ORDER BY",
    ]:
        if needle not in java_usage_repo:
            fail(f"java usage summary repository missing bounded aggregate marker {needle}")
    for needle in [
        "dailyRollups.findSummaryTotals(",
        "dailyRollups.findSummaryItems(",
        "PageRequest.of(0, capped)",
    ]:
        if needle not in java_usage_service:
            fail(f"java usage summary service missing bounded aggregate call {needle}")
    for test_name in [
        "repositoryAggregatesTotalsAndPagesSummaryItems",
        "summaryLimitOneKeepsTotalsAcrossAllMatchingGroupsAndAppliesFilters",
        "ingestRollupsBatchUpsertsWithoutGrowingDuplicateIdentityRows",
    ]:
        if test_name not in java_summary_test:
            fail(f"java usage summary bounded aggregation tests missing {test_name}")

    go_usage = read("server-go/internal/adapter/handler/usage.go")
    go_usage_test = read("server-go/internal/adapter/handler/usage_test.go")
    if "io.ReadAll(reader)" in go_usage:
        fail("go usage gzip decoding must not call bare io.ReadAll(reader)")
    for needle in [
        "io.ReadAll(io.LimitReader(reader, maxBodyBytes+1))",
        "len(rawJSON) > maxBodyBytes",
        "http.StatusRequestEntityTooLarge",
    ]:
        if needle not in go_usage:
            fail(f"go usage gzip decoded-size cap missing {needle}")
    for needle in [
        "TestUsageRollupRejectsGzipBodyAboveDecodedLimit",
        "signedGzipUsageRequest",
        "http.StatusRequestEntityTooLarge",
    ]:
        if needle not in go_usage_test:
            fail(f"go usage gzip decompressed overflow test missing {needle}")

    java_usage_controller = read("server-java/src/main/java/com/aitrack/server/adapter/handler/UsageController.java")
    java_usage_controller_test = read("server-java/src/test/java/com/aitrack/server/adapter/handler/UsageControllerTest.java")
    if re.search(r"GZIPInputStream\s+\w+[\s\S]{0,240}\.readAllBytes\(\)", java_usage_controller):
        fail("java usage gzip decoding must not call bare GZIPInputStream.readAllBytes()")
    java_gzip_match = re.search(
        r"if \(!\"gzip\"\.equalsIgnoreCase[\s\S]*?\n    private TokenEntity",
        java_usage_controller,
    )
    if not java_gzip_match:
        fail("java usage gzip decoding branch not found")
    java_gzip_body = java_gzip_match.group(0)
    if "props.getMaxRequestBodyBytes()" not in java_gzip_body:
        fail("java usage gzip decoded path must apply max-request-body-bytes after decompression")
    if "HttpStatus.PAYLOAD_TOO_LARGE" not in java_gzip_body:
        fail("java usage gzip decoded overflow must return payload-too-large")
    for needle in [
        "rollup_gzipDecodedBodyExceedsLimit_413",
        "props.setMaxRequestBodyBytes(512)",
        "signedUsageRequest(\"/api/v1/ai-track/usage/rollup\", body, true)",
        "status().isPayloadTooLarge()",
    ]:
        if needle not in java_usage_controller_test:
            fail(f"java usage gzip decompressed overflow test missing {needle}")


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
    for needle in [
        "DETAIL_ROWS",
        "SOURCE_ROWS",
        "ROLLUP_ROWS",
        "OUTBOX_ROWS",
        "OUTBOX_PAYLOAD_BYTES",
        "OUTBOX_SYNCED_PAYLOAD_ROWS",
        "OUTBOX_FAILED_PAYLOAD_ROWS",
        "usage_rollup_sources",
        "usage_daily_model_rollups",
        "usage_outbox",
    ]:
        if needle not in text:
            fail(f"client e2e matrix must verify bounded local rollup storage: missing {needle}")

    registered = [
        name
        for name in re.findall(r'name:\s*"([^"]+)"', agent_text)
        if name not in ALIAS_AGENT_NAMES
    ]
    for agent in registered:
        if re.search(rf"^\s*{re.escape(agent)}\s*$", text, re.MULTILINE) is None:
            fail(f"client e2e matrix missing agent {agent}")
    for agent in ALIAS_AGENT_NAMES:
        if re.search(rf"^\s*{re.escape(agent)}\s*$", text, re.MULTILINE) is not None:
            fail(f"client e2e matrix must not duplicate alias key {agent}")
    combined = text + "\n" + fixture_text
    for forbidden in [
        "matrix-${agent}",
        "prompt for local collection",
        "write_usage_jsonl_fixture",
        "write_usage_sqlite_fixture",
        "write_usage_json_fixture",
        "CREATE TABLE messages (data TEXT)",
        "CREATE TABLE messages (id TEXT, session_id TEXT, role TEXT, content TEXT, tool_name",
        "inject_native_monitoring_fixture",
        "aitrack_fixture_events",
    ]:
        if forbidden in combined:
            fail(f"client e2e matrix still accepts generic fixture proof: {forbidden}")
    for needle in [
        "local_usage_matrix.py",
        "usage_fixture_expects_usage",
        "usage_fixture_requires_positive_tokens",
        "expected_usage_min_monitoring_events",
        "expected_usage_required_event_types",
        "usage_fixture_expects_reasoning_event",
        "prompt_summary",
        "assistant_output",
        "tool_name",
        "tool_arguments",
        "missing provider",
        "missing model",
        "agent_reasoning",
    ]:
        if needle not in text:
            fail(f"client e2e matrix missing harness marker {needle}")
    for needle in [
        "LOCAL_DERIVED_USAGE_SOURCES",
        "usage_fixture_expected_usage_basis",
        "EXPECTED_SOURCES",
        "expected_sources_for_agent",
        "--expected-sources",
        "path_substring",
        "expects_usage",
        "expects_monitoring",
        "session_id",
        "required_event_types",
        '"local_derived"',
        '"native"',
    ]:
        if needle not in fixture_text:
            fail(f"client e2e fixture missing usage_basis expectation marker {needle}")
    for needle in [
        "expected_usage_sources_json",
        "validate_expected_usage_sources",
        "expected_usage_min_monitoring_events",
        "expected_usage_required_event_types",
        "--expected-sources",
        "EXPECTED_SOURCES_JSON",
        "path_substring",
        "usage_rollup_sources",
        "source_path_matches",
        "message_count",
        "records.db",
        "session_id",
        "required_event_types",
        "BLOCKER",
    ]:
        if needle not in text:
            fail(f"client e2e matrix missing source-level expected-source check {needle}")
    for forbidden in [
        "usage_fixture_min_monitoring_events()",
        "usage_fixture_required_event_types()",
    ]:
        if forbidden in text:
            fail(f"client e2e matrix must derive monitoring expectations from expected-source rows, not {forbidden}")
    source_check_pos = text.find('validate_expected_usage_sources "${AITRACK_HOME}"')
    sync_pos = text.find('"${AITRACK_BIN}" usage sync --tool')
    pruning_pos = text.find("PRUNED_RECORD_ROWS_AFTER_SYNC")
    if source_check_pos == -1 or sync_pos == -1:
        fail("client e2e matrix must run source-bound expected-source validation before usage sync")
    if source_check_pos > sync_pos:
        fail("client e2e source-bound validation must run before usage sync")
    if pruning_pos != -1 and pruning_pos < source_check_pos:
        fail("client e2e post-sync pruning checks must not replace raw source-bound validation")
    source_path_match = re.search(
        r"def source_path_matches\([\s\S]*?\n\n\ndef nonzero_number",
        text,
    )
    if not source_path_match:
        fail("client e2e matrix missing source_path_matches helper body")
    source_path_body = source_path_match.group(0)
    for forbidden in [
        " in normalized_row",
        "endswith(",
        "path LIKE",
    ]:
        if forbidden in source_path_body:
            fail(f"source-level expected-source path check must not use loose fallback {forbidden}")
    registered_specs = {
        (agent, label, kind)
        for agent, label, kind in local_source_spec_entries(agent_text)
        if agent not in ALIAS_AGENT_NAMES
    }
    required_source_rows = set(REQUIRED_SOURCE_ROWS)
    if required_source_rows != registered_specs:
        missing = sorted(registered_specs - required_source_rows)
        extra = sorted(required_source_rows - registered_specs)
        fail(f"REQUIRED_SOURCE_ROWS do not match LocalSourceSpec entries: missing={missing} extra={extra}")
    constants = local_source_capability_constants(agent_text)
    capability_expectations = {
        (agent, label): local_source_spec_capability_flags(constants, agent, label, block)
        for agent, label, block in local_source_spec_blocks(agent_text)
        if agent not in ALIAS_AGENT_NAMES
    }
    expected_specs: set[tuple[str, str, str]] = set()
    for agent in registered:
        result = subprocess.run(
            [sys.executable, "-B", "e2e/local_usage_matrix.py", "--expected-sources", agent],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        if result.returncode != 0:
            fail(f"expected-sources command failed for {agent}: {result.stderr.strip()}")
        try:
            rows = json.loads(result.stdout)
        except json.JSONDecodeError as exc:
            fail(f"expected-sources command emitted invalid JSON for {agent}: {exc}")
        if not isinstance(rows, list) or not rows:
            fail(f"expected-sources command returned no rows for {agent}")
        for row in rows:
            if "blocker" in row:
                label = row.get("label", "<missing-label>")
                fail(f"expected-sources row {agent}/{label} must not contain blocker")
            for key in [
                "agent",
                "label",
                "kind",
                "path_substring",
                "expects_usage",
                "expects_monitoring",
                "session_id",
                *EXPECTED_SOURCE_FIELD_KEYS,
            ]:
                if key not in row:
                    fail(f"expected-sources row for {agent} missing {key}")
            if row["agent"] != agent:
                fail(f"expected-sources row for {agent} contains mismatched agent {row['agent']}")
            for key in EXPECTED_SOURCE_FIELD_KEYS:
                if not isinstance(row[key], list) or not all(
                    isinstance(item, str) for item in row[key]
                ):
                    fail(f"expected-sources row {agent}/{row['label']} {key} must be a string list")
            if row["expects_monitoring"] and not row.get("required_event_types"):
                fail(f"expected-sources row {agent}/{row['label']} lacks required_event_types")
            capability_flags_for_row = capability_expectations.get((row["agent"], row["label"]))
            if capability_flags_for_row is None:
                fail(f"expected-sources row {agent}/{row['label']} has no LocalSourceSpec")
            required_usage_fields = set(row["required_usage_fields"])
            required_record_fields = set(row["required_record_fields"])
            missing_usage_fields = (
                sorted(
                    required_fields_for_capabilities(
                        capability_flags_for_row, CAPABILITY_USAGE_FIELD_REQUIREMENTS
                    )
                    - required_usage_fields
                )
                if row["expects_usage"]
                else []
            )
            missing_record_fields = (
                sorted(
                    required_fields_for_capabilities(
                        capability_flags_for_row, CAPABILITY_RECORD_FIELD_REQUIREMENTS
                    )
                    - required_record_fields
                )
                if row["expects_monitoring"]
                else []
            )
            if missing_usage_fields:
                fail(
                    f"expected-sources row {agent}/{row['label']} omits usage fields "
                    f"implied by LocalSourceCapabilities: {missing_usage_fields}"
                )
            if missing_record_fields:
                fail(
                    f"expected-sources row {agent}/{row['label']} omits record fields "
                    f"implied by LocalSourceCapabilities: {missing_record_fields}"
                )
            expected_usage_capabilities = usage_capabilities_required_by_expected_fields(
                required_usage_fields
            )
            expected_record_capabilities = capability_fields_for_expected_fields(
                required_record_fields, CAPABILITY_RECORD_FIELD_REQUIREMENTS
            )
            unclaimed_usage_capabilities = sorted(
                capability
                for capability in expected_usage_capabilities
                if not capability_flags_for_row.get(capability, False)
            )
            unclaimed_record_capabilities = sorted(
                capability
                for capability in expected_record_capabilities
                if not capability_flags_for_row.get(capability, False)
            )
            if unclaimed_usage_capabilities:
                fail(
                    f"expected-sources row {agent}/{row['label']} requires usage capabilities "
                    f"not declared by LocalSourceCapabilities: {unclaimed_usage_capabilities}"
                )
            if unclaimed_record_capabilities:
                fail(
                    f"expected-sources row {agent}/{row['label']} requires record capabilities "
                    f"not declared by LocalSourceCapabilities: {unclaimed_record_capabilities}"
                )
            expected_specs.add((row["agent"], row["label"], row["kind"]))
    if expected_specs != registered_specs:
        missing = sorted(registered_specs - expected_specs)
        extra = sorted(expected_specs - registered_specs)
        fail(f"expected-sources rows do not match LocalSourceSpec entries: missing={missing} extra={extra}")
    fixture_tree = ast.parse(fixture_text)
    derived_sources: set[tuple[str, str]] = set()
    for node in fixture_tree.body:
        if isinstance(node, ast.Assign):
            for target in node.targets:
                if isinstance(target, ast.Name) and target.id == "LOCAL_DERIVED_USAGE_SOURCES":
                    derived_sources = ast_string_pairs(node.value)
    expected_derived_sources = source_label_pairs(local_derived_usage_source_body(agent_text))
    if derived_sources != expected_derived_sources:
        fail(
            "client e2e fixture local-derived usage_basis sources mismatch: "
            f"expected {sorted(expected_derived_sources)}, got {sorted(derived_sources)}"
        )
    derived_agents = {agent for agent, _ in derived_sources}
    expected_derived_agents = {agent for agent, _ in expected_derived_sources}
    if derived_agents != expected_derived_agents:
        fail(
            "client e2e fixture local-derived usage_basis agents mismatch: "
            f"expected {sorted(expected_derived_agents)}, got {sorted(derived_agents)}"
        )
    for needle in [
        "state.vscdb",
        "interactive.sessions",
        "chat.ChatSessionStore.index",
        "chatSessions/copilot-session.jsonl",
        "toolInvocationSerialized",
        "textEditGroup",
        "session-store.db",
        "CREATE TABLE sessions",
        "CREATE TABLE messages",
        "CREATE TABLE tool_calls",
        "CREATE TABLE tool_results",
        "CREATE TABLE modified_files",
        "/tmp/copilot-workspace",
        "ui_messages.json",
        "api_conversation_history.json",
        "session-usage.json",
        "chat-archive.jsonl",
        "partial.json",
        "dynamic-tool",
        "chat_message",
        "token_info",
        "content_json",
        "parts TEXT",
        "tool_calls TEXT",
        "token_count",
        "opencode.db",
        "session-opencode-message.json",
        "threads.db",
        "sessions.db",
        "crush.db",
        "kilo.db",
        "write_kilo_storage_json_fixture",
        ".local/share/kilo/storage",
        "session_diff",
        "storage prompt",
        ".zcode/projects",
        "session-zcode.jsonl",
        "warp.sqlite",
        "wukong.db",
        "parallel_tool_calls",
        "fork_agent_tasks",
        "session-events.jsonl",
        "session-amp.jsonl",
        "chat-messages.json",
        "wire.jsonl",
        "events.jsonl",
        "session/update",
        "usage_record.jsonl",
        "token-usage-2026-06.jsonl",
        "usageMetadata",
        "candidatesTokenCount",
        "functionCall",
        "functionResponse",
        "toolCallResult",
        "contextWindowSize",
        '"models"',
        '"schemaVersion": 1',
        "tokensIn",
        "cacheReads",
        "conversation_usage_metadata",
        "credits_spent",
        "warp_tokens",
        "llm_irs",
    ]:
        if needle not in fixture_text:
            fail(f"client e2e native fixture coverage missing {needle}")
    for agent in registered:
        if agent in ALIAS_AGENT_NAMES:
            continue
        if f'"{agent}"' not in fixture_text:
            fail(f"client e2e fixture generator missing real fixture branch for {agent}")
    if 'source_dir="${root}/sources/${agent}"' in text:
        fail("client e2e matrix still uses one generic sources/<agent> fixture")
    for needle in [
        "local-sources/claude/hook-events.jsonl",
        "local-sources/codex/hook-events.jsonl",
    ]:
        if needle not in fixture_text:
            fail(f"client e2e fixture missing registered source-level fixture path {needle}")
    qoder_work_cn_branch = fixture_branch_for_agent(fixture_text, "qoder-work-cn")
    if qoder_work_cn_branch is None:
        fail("client e2e fixture generator missing qoder-work-cn branch")
    for needle in [
        'dirname = ".qoderwork"',
        'root / ".qoderworkcn" / "hooks" / f"{agent}-legacy-hooks.jsonl"',
    ]:
        if needle not in qoder_work_cn_branch:
            fail(f"qoder-work-cn fixture must cover official and legacy roots: {needle}")
    if 'else ".qoderworkcn"' in qoder_work_cn_branch:
        fail("qoder-work-cn full fixture must not live only under legacy .qoderworkcn")
    min_events_body = re.search(
        r"expected_usage_min_monitoring_events\(\)[\s\S]*?\n}",
        text,
    ).group(0)
    required_events_body = re.search(
        r"expected_usage_required_event_types\(\)[\s\S]*?\n}",
        text,
    ).group(0)
    for needle in ['row.get("expects_monitoring")', 'row.get("required_event_types")']:
        if needle not in min_events_body:
            fail(f"client e2e monitoring event count must derive from expected-source rows: {needle}")
        if needle not in required_events_body:
            fail(f"client e2e monitoring required types must derive from expected-source rows: {needle}")
    for event_type in ["prompt", "output", "tool", "tool_result", "edit"]:
        if f'"{event_type}"' not in fixture_text:
            fail(f"client e2e fixture monitoring field expectation missing {event_type}")
    token_required_body = re.search(
        r"usage_fixture_requires_positive_tokens\(\)[\s\S]*?\n}",
        text,
    ).group(0)
    expects_usage_body = re.search(
        r"usage_fixture_expects_usage\(\)[\s\S]*?\n}",
        text,
    ).group(0)
    default_agents = []
    for raw_agent in re.search(
        r"REQUIRED_LOCAL_SOURCE_AGENTS=\(\n(?P<body>[\s\S]*?)\n\)",
        text,
    ).group("body").splitlines():
        agent = raw_agent.strip()
        if agent:
            default_agents.append(agent)
    default_agent_pattern = r"^\s*({})\)".format(
        "|".join(re.escape(agent) for agent in default_agents)
    )
    if "return 1" in expects_usage_body:
        fail("client e2e usage expectation must not exempt default agents from rollup/source checks")
    if re.search(default_agent_pattern, expects_usage_body, flags=re.MULTILINE):
        fail("client e2e usage expectation must not branch around default agents")
    if re.search(r'echo\s+["\']?0["\']?', min_events_body):
        fail("client e2e monitoring expectation must not allow zero events for default agents")
    if re.search(default_agent_pattern, min_events_body, flags=re.MULTILINE):
        fail("client e2e monitoring expectation must not branch around default agents")
    if re.search(default_agent_pattern, required_events_body, flags=re.MULTILINE):
        fail("client e2e monitoring required event types must not branch around default agents")
    if "crush)" in token_required_body:
        fail("crush official sessions schema exposes prompt/completion tokens and must not be exempt")
    if "warp)" in token_required_body:
        fail("warp native conversation_usage_metadata exposes token totals and must not be exempt")
    if "codebuff)" not in token_required_body:
        fail("codebuff official ChatMessage exposes credits, not token buckets, and must use cost aggregation")
    if "antigravity-ide/sessions" in fixture_text or "session-antigravity.jsonl" in fixture_text:
        fail("antigravity fixture must not use fabricated JSONL session data")
    if ".gemini/antigravity-cli/conversations/session-antigravity.db" not in fixture_text:
        fail("antigravity fixture must use the known CLI sqlite conversation location")
    for needle in [
        "CREATE TABLE gen_metadata",
        "CREATE TABLE trajectory_metadata_blob",
        "CREATE TABLE steps",
        "antigravity_gen_metadata_proto",
        "antigravity_tool_step_proto",
    ]:
        if needle not in fixture_text:
            fail(f"antigravity e2e fixture must prove CLI sqlite protobuf shape: {needle}")
    kiro_branch = fixture_branch_for_agent(fixture_text, "kiro")
    if kiro_branch is None:
        fail("client e2e fixture generator missing kiro branch")
    for needle in [
        "Official-style hook JSONL events",
        ".kiro/hooks/kiro-hooks.jsonl",
        '"hook_event_name": "UserPromptSubmit"',
        '"hook_event_name": "PreToolUse"',
        '"hook_event_name": "PostToolUse"',
        '"hook_event_name": "afterAgentResponse"',
        '"hook_event_name": "afterFileEdit"',
        '"prompt": "kiro prompt"',
        '"assistant_output": "kiro output"',
        '"tool_input"',
        '"tool_response": "file content"',
        '"old_string": "old\\n"',
        '"new_string": "new\\n"',
        "data.sqlite3",
        "CREATE TABLE conversations_v2",
        '"kiro conversation prompt"',
        '"kiro conversation output"',
        '"kiro-edit-call"',
        '"input_tokens": 33',
        '"output_tokens": 12',
    ]:
        if needle not in kiro_branch:
            fail(f"kiro fixture must prove hook JSONL plus conversations_v2 SQLite coverage: {needle}")
    for forbidden in [
        '"usageLedger"',
        '"input_token_count"',
        '"output_token_count"',
        '"token_usage"',
    ]:
        if forbidden in kiro_branch:
            fail(f"kiro fixture must not imply token usage support from hook-only data: {forbidden}")
    for agent, needles in {
        "cursor": [
            '"prompt": "cursor prompt"',
            '"response": "cursor output"',
            '"tool_name": "read_file"',
            '"tool_name": "apply_patch"',
            '"response": "file content"',
            '"afterFileEdit"',
            '"usage_basis"',
        ],
        "qoder": [
            '"hook_event_name": "UserPromptSubmit"',
            '"role": "user"',
            '"role": "assistant"',
            '"type": "tool_use"',
            '"role": "toolResult"',
            '"usage_basis"',
        ],
        "qoder-cn": [
            '"hook_event_name": "UserPromptSubmit"',
            '"role": "user"',
            '"role": "assistant"',
            '"type": "tool_use"',
            '"role": "toolResult"',
            '"usage_basis"',
        ],
        "qoder-work": [
            '"event.name": "llm.request"',
            '"event.name": "llm.response"',
            '"event.name": "llm.tool_call"',
            '"event.name": "llm.tool_result"',
            '"event.name": "llm.file_edit"',
            '"usage_basis"',
        ],
        "qoder-work-cn": [
            '"event.name": "llm.request"',
            '"event.name": "llm.response"',
            '"event.name": "llm.tool_call"',
            '"event.name": "llm.tool_result"',
            '"event.name": "llm.file_edit"',
            '"usage_basis"',
        ],
        "wukong": [
            "llm_prompt TEXT",
            "llm_response TEXT",
            "selected_tool TEXT",
            "step_result TEXT",
            "parallel_tool_calls",
            "fork_agent_tasks",
            "tokens_used INTEGER",
            "actual_tokens INTEGER",
        ],
        "antigravity": [
            "gen_metadata",
            "trajectory_metadata_blob",
            "steps",
            "antigravity prompt",
            "antigravity output",
            "antigravity_tool_step_proto",
            "apply_patch",
        ],
        "amp": [
            '"type": "user"',
            '"type": "assistant"',
            '"type": "tool_use"',
            '"type": "tool_result"',
            '"type": "result"',
            '"usage_basis"',
        ],
        "grok": [
            '"method": "session/update"',
            '"sessionUpdate": "user_message"',
            '"sessionUpdate": "agent_message_chunk"',
            '"sessionUpdate": "tool_call"',
            '"sessionUpdate": "tool_result"',
            '"sessionUpdate": "usage_update"',
            '"usage_basis"',
            '"tokens"',
            '"input_tokens"',
            '"output_tokens"',
            '"cache_read_tokens"',
            '"cache_write_tokens"',
            '"reasoning_tokens"',
            '"cost"',
            '"account": "local"',
        ],
        "zcode": [
            '"type": "session"',
            '"role": "user"',
            '"role": "assistant"',
            '"tool_calls"',
            '"type": "tool_result"',
            '"usage_basis"',
        ],
    }.items():
        branch = fixture_branch_for_agent(fixture_text, agent)
        if branch is None:
            fail(f"client e2e fixture generator missing local-derived {agent} branch")
        for needle in needles:
            if needle not in branch:
                fail(f"{agent} local-derived fixture coverage missing {needle}")
    for agent in [
        "codex",
        "qwen",
        "gemini",
        "copilot",
        "cursor",
        "trae",
        "openclaw",
        "opencode",
        "wukong",
        "amp",
        "droid",
        "pi",
        "mux",
        "codebuff",
        "gjc",
        "kimi",
        "crush",
        "synthetic",
        "warp",
    ]:
        branch = fixture_branch_for_agent(fixture_text, agent)
        if branch is None:
            fail(f"client e2e fixture generator missing {agent} branch")
        if agent == "codex":
            for needle in [
                '"custom_tool_call"',
                '"custom_tool_call_output"',
                '"web_search_call"',
                '"tool_search_call"',
                '"tool_search_output"',
                '"reasoning"',
                '"user_message"',
                '"agent_message"',
                '"agent_reasoning"',
                '"patch_apply_end"',
                '"event_msg"',
                '"task_started"',
                '"model_context_window"',
                '"last_token_usage"',
                '"total_token_usage"',
                '"rate_limits"',
                '"success"',
                '"status"',
                '"unified_diff"',
                '"move_path"',
                '"call_id"',
                '"name": "apply_patch"',
            ]:
                if needle not in branch:
                    fail(f"codex fixture must prove real rollout event shape: {needle}")
            if re.search(
                r'"type": "response_item"[\s\S]{0,180}"type": "patch_apply_end"',
                branch,
            ):
                fail("codex fixture must not put patch_apply_end under response_item")
        elif agent == "copilot":
            for needle in [
                ".copilot/session-state/copilot-official-session/events.jsonl",
                '"event": "user.message"',
                '"event": "assistant.message_delta"',
                '"deltaContent": "official copilot output"',
                '"event": "tool.execution_start"',
                '"event": "tool.execution_complete"',
                '"event": "assistant.usage"',
                '"event": "session.context_changed"',
                '"event": "session.shutdown"',
                '"codeChanges"',
                '"official copilot prompt"',
                '"official copilot output"',
                "session-store.db",
                "CREATE TABLE sessions",
                "CREATE TABLE messages",
                "CREATE TABLE tool_calls",
                "CREATE TABLE tool_results",
                "CREATE TABLE modified_files",
                "promptTokens",
                "completionTokens",
                "cacheReadTokens",
                "reasoningTokens",
                "/tmp/copilot-workspace",
                ".config/github-copilot/ws/chat-sessions",
                ".config/github-copilot/ws/chat-agent-sessions",
                ".config/github-copilot/ws/chat-edit-sessions",
                "00000000000.xd",
                "copilot-agent-sessions-nitrite.db",
                "copilot-edit-sessions-nitrite.db",
                '"gen_ai.input.messages"',
                '"gen_ai.output.messages"',
                '"gen_ai.tool.call.arguments"',
                '"gen_ai.tool.call.result"',
            ]:
                if needle not in branch:
                    fail(f"copilot fixture must prove session-store.db shape: {needle}")
        elif agent == "qwen":
            for needle in [
                ".qwen/telemetry.log",
                ".qwen/projects/project-one/chats/session-qwen.jsonl",
                ".qwen/usage_record.jsonl",
                ".qwen/usage/token-usage-2026-06.jsonl",
                '"functionCall"',
                '"functionResponse"',
                '"toolCallResult"',
                '"contextWindowSize"',
                '"usageMetadata"',
                '"candidatesTokenCount"',
                '"models"',
                '"schemaVersion": 1',
                '"authType": "oauth-personal"',
                '"apiDurationMs": 1234',
            ]:
                if needle not in branch:
                    fail(f"qwen fixture must prove telemetry, ChatRecord, usage_record, and token usage JSONL: {needle}")
        elif agent == "gemini":
            for needle in [
                ".gemini/telemetry.log",
                ".gemini/tmp/project-one-hash/chats/session-2026-06-16-gemini.jsonl",
                '"sessionId": "gemini-session"',
                '"projectHash": "project-one-hash"',
                '"directories": ["/workspace/project-one"]',
                '"type": "gemini"',
                '"content": "gemini output"',
                '"toolCalls"',
                '"tokens"',
                '"cached": 10',
                '"thoughts": 6',
                '"tool": 3',
            ]:
                if needle not in branch:
                    fail(f"gemini fixture must prove telemetry plus ChatRecording JSONL: {needle}")
        elif agent == "cursor":
            for needle in [
                ".cursor/projects/project-one/agent-transcripts/cursor-session.jsonl",
                '"cursor transcript prompt"',
                '"cursor transcript output"',
                '"cursor transcript file content"',
                '"preToolUse"',
                '"postToolUse"',
                '"afterAgentResponse"',
                '"afterFileEdit"',
                '"conversation_id"',
                '"generation_id"',
                '"workspace_roots"',
                '"transcript_path"',
            ]:
                if needle not in branch:
                    fail(f"cursor fixture must prove official hook event shape: {needle}")
        elif agent == "trae":
            for needle in [
                "trajectories/trajectory_20260616_140000.json",
                '"llm_interactions"',
                '"input_messages"',
                '"response"',
                '"usage"',
                '"cache_creation_input_tokens"',
                '"cache_read_input_tokens"',
                '"agent_steps"',
                '"tool_results"',
                '"final_result"',
                '"execution_time"',
            ]:
                if needle not in branch:
                    fail(f"trae fixture must prove official trajectory shape: {needle}")
            if re.search(
                r'"task_id": "trae-session"[\s\S]{0,240}"input_messages"[\s\S]{0,240}"response"',
                branch,
            ):
                fail("trae fixture must not prove only the old root-level flat shape")
        elif agent == "openclaw":
            for needle in [
                '"sessionFile"',
                '"lastAccountId"',
                '"accountId"',
                '"type": "toolCall"',
                '"type": "toolResult"',
                '"name": "edit"',
                '"edits"',
            ]:
                if needle not in branch:
                    fail(f"openclaw fixture must prove sessions.json plus transcript tool/edit blocks: {needle}")
        elif agent == "opencode":
            for needle in [
                "opencode.db",
                "CREATE TABLE session",
                "CREATE TABLE message",
                "CREATE TABLE part",
                "CREATE TABLE session_input",
                "CREATE TABLE session_message",
                "CREATE TABLE account",
                "providerID",
                "opencode@example.com",
                "opencode session input prompt",
                "opencode session message output",
                "session message explicit result",
                "src/session-message.rs",
                "session-opencode-message.json",
                '"info"',
                '"messages"',
                '"parts"',
                '"sessionID": "opencode-export-session"',
                '"type": "tool"',
                '"type": "tool_result"',
                '"type": "reasoning"',
                "opencode export reasoning",
                '"callID": "export-tool-1"',
                '"tool_call_id": "session-message-result"',
                '"cache": {"read": 9, "write": 4}',
            ]:
                if needle not in branch:
                    fail(f"opencode fixture must prove sqlite plus export JSON shape: {needle}")
        elif agent == "wukong":
            for needle in [
                ".wukong/data/wukong.db",
                "CREATE TABLE sessions",
                "CREATE TABLE steps",
                "llm_prompt TEXT",
                "llm_response TEXT",
                "selected_tool TEXT",
                "step_result TEXT",
                "CREATE TABLE parallel_tool_calls",
                "CREATE TABLE fork_agent_tasks",
                "CREATE TABLE todos",
                "tokens_used INTEGER",
                "actual_tokens INTEGER",
                "input_tokens INTEGER DEFAULT 0",
                "output_tokens INTEGER DEFAULT 0",
                "cache_read_tokens INTEGER DEFAULT 0",
                "cache_write_tokens INTEGER DEFAULT 0",
                "reasoning_tokens INTEGER DEFAULT 0",
                "source_cost REAL DEFAULT 0",
                "230,140,60,16,4,10,0.032,3",
                "'todo-1','wk-session',90,50,25,6,2,4,0.011",
                "wukong prompt",
                "wukong output",
                "read_file",
                "file content",
                "apply_patch",
            ]:
                if needle not in branch:
                    fail(f"wukong fixture must prove official SQLite local adapter shape: {needle}")
            for forbidden in [".wukong/agent-data/messages.json", '"messages": [']:
                if forbidden in branch:
                    fail(f"wukong fixture must not use generic transcript shape: {forbidden}")
        elif agent == "amp":
            for needle in [
                "Amp first-party evidence covers --stream-json runtime stdout events here",
                "official stable local persisted thread/history",
                ".amp/sessions/session-amp.jsonl",
                '"type": "system"',
                '"subtype": "init"',
                '"type": "assistant"',
                '"type": "tool_use"',
                '"type": "tool_result"',
                '"type": "result"',
                '"session_id": "amp-session"',
                '"usage_basis"',
            ]:
                if needle not in branch:
                    fail(f"amp fixture must prove runtime --stream-json shape, not local thread history: {needle}")
            for forbidden in ['"usageLedger"', '"messages": [']:
                if forbidden in branch:
                    fail(f"amp fixture must not use generic local thread/history JSON shape: {forbidden}")
        elif agent == "droid":
            for needle in [
                ".factory/sessions/session-droid.settings.json",
                ".factory/sessions/session-droid.jsonl",
                '"type": "session_start"',
                '"method": "droid.session_notification"',
                '"notification"',
                '"type": "message"',
                '"type": "tool_use"',
                '"type": "tool_result"',
                '"type": "token_usage_update"',
                '"tokenUsage"',
            ]:
                if needle not in branch:
                    fail(f"droid fixture must prove session notification shape: {needle}")
            for pattern, forbidden in [
                (r'^\s*\{\s*\n\s*"type": "user"', '{"type": "user"'),
                (r'^\s*\{\s*\n\s*"type": "tool_call"', '{"type": "tool_call"'),
                (r'^\s*\{\s*\n\s*"type": "tool_result"', '{"type": "tool_result"'),
            ]:
                if re.search(pattern, branch, flags=re.MULTILINE):
                    fail(f"droid fixture must not use generic transcript rows: {forbidden}")
        elif agent == "pi":
            for needle in [
                ".pi/agent/sessions/session-pi.jsonl",
                '"type": "session"',
                '"version": 3',
                '"type": "model_change"',
                '"type": "message"',
                '"role": "user"',
                '"role": "assistant"',
                '"type": "thinking"',
                '"pi reasoning"',
                '"type": "toolCall"',
                '"role": "toolResult"',
                '"cacheRead"',
                '"cacheWrite"',
                '"cost": {"total": 0.031}',
            ]:
                if needle not in branch:
                    fail(f"pi fixture must prove official session/message/thinking shape: {needle}")
            for forbidden in ['"type": "tool_result"', '"type": "edit"']:
                if forbidden in branch:
                    fail(f"pi fixture must not use generic transcript shape: {forbidden}")
        elif agent == "mux":
            for needle in [
                ".mux/sessions/workspace-one",
                "session-usage.json",
                "chat.jsonl",
                "chat-archive.jsonl",
                "partial.json",
                '"byModel"',
                '"anthropic:claude-opus-4-6"',
                '"reasoning": {"tokens": 7, "cost_usd": 0.001}',
                '"role": "user"',
                '"role": "assistant"',
                '"type": "reasoning"',
                '"mux reasoning"',
                '"type": "dynamic-tool"',
                '"state": "output-available"',
                '"toolName": "read_file"',
            ]:
                if needle not in branch:
                    fail(f"mux fixture must prove official session usage and chat part shape: {needle}")
            for forbidden in ['"type": "tool_result"', '"messages": [']:
                if forbidden in branch:
                    fail(f"mux fixture must not use generic transcript shape: {forbidden}")
        elif agent == "codebuff":
            for needle in [
                "chat-messages.json",
                "run-state.json",
                '"traceSessionId"',
                '"creditsUsed"',
                '"directCreditsUsed"',
                '"fileContext"',
                '"projectRoot"',
                '"variant": "user"',
                '"variant": "ai"',
                '"blocks"',
                '"type": "tool"',
                '"toolCallId"',
                '"toolName"',
                '"codebuff-run-state-only"',
                '"outputRaw"',
            ]:
                if needle not in branch:
                    fail(f"codebuff fixture must prove official ChatMessage block shape: {needle}")
            for forbidden in ['"role": "assistant"', '"tool_calls"', '"metadata":']:
                if forbidden in branch:
                    fail(f"codebuff fixture must not use generic transcript shape: {forbidden}")
        elif agent == "gjc":
            for needle in [
                ".gjc/agent/sessions/tmp-project/20260616T140000_gjc-session.jsonl",
                ".gjc/_session-gjc-session/state/audit.jsonl",
                '"type": "session"',
                '"version": 3',
                '"titleSource"',
                '"type": "message"',
                '"role": "user"',
                '"role": "assistant"',
                '"type": "toolCall"',
                '"role": "toolResult"',
                '"toolCallId"',
                '"cacheRead"',
                '"cacheWrite"',
                '"usage"',
                '"cost": {"total": 0.02}',
                '"contextUsage": {"tokens": 9999}',
            ]:
                if needle not in branch:
                    fail(f"gjc fixture must prove official session/message shape: {needle}")
            for forbidden in ['"type": "tool_result"', '"type": "edit"']:
                if forbidden in branch:
                    fail(f"gjc fixture must not use generic transcript shape: {forbidden}")
        elif agent == "kimi":
            for needle in [
                ".kimi-code/session_index.jsonl",
                ".kimi-code/sessions/project/kimi-code-session/state.json",
                "agents/main/wire.jsonl",
                "agents/agent-1/wire.jsonl",
                '"jsonrpc": "2.0"',
                '"method": "event"',
                '"method": "request"',
                '"result": {"tool_call_id": "tool-1", "return_value": "file content"}',
                '"type": "ToolCallRequest"',
                '"type": "TurnBegin"',
                '"type": "ContentPart"',
                '"type": "StatusUpdate"',
                '"payload"',
                '"token_usage"',
            ]:
                if needle not in branch:
                    fail(f"kimi fixture must prove current kimi-code wire/session shape: {needle}")
            for forbidden in [
                '"message": {"type": "TurnBegin"',
                '"message": {"type": "ToolCall"',
            ]:
                if forbidden in branch:
                    fail(f"kimi fixture must not use generic message-wrapper shape: {forbidden}")
        elif agent == "crush":
            for needle in [
                "prompt_tokens INTEGER",
                "completion_tokens INTEGER",
                "title TEXT",
                "model TEXT",
                "provider TEXT",
                "version INTEGER",
                "CREATE TABLE files",
                "CREATE TABLE read_files",
                "read_at INTEGER",
                '"data":',
                '"input":',
                "src/crush-file.rs",
                "src/crush-read.rs",
                '"type": "reasoning"',
                "crush reasoning",
            ]:
                if needle not in branch:
                    fail(f"crush fixture must prove official token columns: {needle}")
        elif agent == "synthetic":
            for needle in [
                "CREATE TABLE trees",
                "CREATE TABLE history_items",
                "CREATE TABLE tree_nodes",
                "CREATE TABLE llm_irs",
                "updated_at INTEGER",
                "llm_ir_id INTEGER",
                "history_item_id INTEGER",
                "tree_id INTEGER",
                "parent_id INTEGER",
                "is_leaf INTEGER",
                "launch_id INTEGER",
                '"filePath"',
                '"search"',
                '"replace"',
                '"name": "edit"',
                '"role": "tool-output"',
                '"role": "tool-runtime-error"',
                '"role": "tool-validation-error"',
                '"role": "tool-skip-output"',
                '"role": "tool-invoke-subagent"',
                '"usage":',
                '"model": "claude-sonnet-4"',
                '"provider": "anthropic"',
                '"sourceCost": 0.02',
            ]:
                if needle not in branch:
                    fail(f"synthetic fixture must prove Octofriend native shape: {needle}")
            for forbidden in ['"session_id": "synthetic-session"', '"messages": [']:
                if forbidden in branch:
                    fail(f"synthetic fixture must not use generic transcript shape: {forbidden}")
        elif agent == "warp":
            for needle in [
                "CREATE TABLE agent_conversations",
                "CREATE TABLE agent_tasks",
                "conversation_usage_metadata",
                "warp_task_proto()",
                "warp.sqlite",
            ]:
                if needle not in branch:
                    fail(f"warp fixture must prove Warp native sqlite/protobuf shape: {needle}")
            for needle in [
                "warp prompt",
                "warp output",
                "read_call",
                "read_result",
                "apply_call",
                "apply_result",
            ]:
                if needle not in fixture_text:
                    fail(f"warp task proto fixture must cover tool call/result shape: {needle}")
            for forbidden in ["aitrack/warp-cache", "usage-2026-06-16.json"]:
                if forbidden in branch:
                    fail(f"warp fixture must not use aitrack self-cache shape: {forbidden}")


def assert_e2e_diagnostics_gate() -> None:
    for path in ["e2e/run.sh", "e2e/run-client-e2e.sh"]:
        text = read(path)
        if "tail " + "-5" in text or re.search(r"docker build[\s\S]{0,160}\|\s*tail", text):
            fail(f"{path} must not truncate docker build logs")
        if "--progress=plain" not in text:
            fail(f"{path} must use plain docker progress for CI diagnostics")
        if path.endswith("run-client-e2e.sh"):
            for needle in [
                "EXTERNAL_DEPENDENCY_FAILURE",
                "Premature end of Content-Length",
                "Could not transfer artifact",
                "repo.maven.apache.org",
            ]:
                if needle not in text:
                    fail(f"{path} must classify Maven dependency download failures: {needle}")

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
        "30 个默认",
        "默认 30",
        "扫描默认 30",
        "30 / 30",
        "3" + "7 个默认",
        "默认 " + "3" + "7",
        "扫描默认 " + "3" + "7",
        "3" + "7 / " + "3" + "7",
        "公开" + "文档",
        "其他注册工具可用 --tool",
        "本地来源 E2E 矩阵覆盖 35 / 35",
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
        "antigravity",
        "opencode",
        "qoder",
        "qoder-cn",
        "qoder-work",
        "qoder-work-cn",
        "wukong",
        "hermes",
        "openclaw",
        "gemini",
        "copilot",
        "cline",
        "roo-code",
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
        "synthetic",
        "warp",
        "zcode",
    ]
    support_doc = read("docs/AGENT_SUPPORT.md")
    default_list_match = re.search(
        r"以下 35 个规范 key：\n\n(.+?)。",
        support_doc,
        flags=re.S,
    )
    if not default_list_match:
        fail("AGENT_SUPPORT default scan list must explicitly name 35 default scan keys")
    default_list = default_list_match.group(1)
    for agent in expected_agents:
        if f"`{agent}`" not in default_list:
            fail(f"AGENT_SUPPORT default scan list missing {agent}")

    support_rows = {}
    for row_name in [
        "Agent 合并完整覆盖",
        "字段级原生读取来源",
        "本地派生读取来源",
        "辅助状态/用量来源",
    ]:
        match = re.search(rf"^\| {re.escape(row_name)} \| (.+?) \|", support_doc, flags=re.M)
        if not match:
            fail(f"AGENT_SUPPORT source support row missing: {row_name}")
        support_rows[row_name] = match.group(1)
    for agent in expected_agents:
        if f"`{agent}`" not in support_rows["Agent 合并完整覆盖"]:
            fail(f"AGENT_SUPPORT agent-level full coverage row missing {agent}")
    for source_id in [
        "claude/projects-jsonl",
        "codex/rollout-jsonl",
        "kiro/data-sqlite",
        "hermes/sqlite",
        "crush/sqlite",
        "kilo/sqlite",
        "kilo/storage-json",
        "kilocode/sqlite",
        "kilocode/storage-json",
        "trae/trajectory-json",
        "qwen/project-chats-jsonl",
        "opencode/sqlite",
        "openclaw/session-jsonl",
        "cline/vscode-ui-messages",
        "roo-code/vscode-ui-messages",
        "gjc/session-jsonl",
        "zed/threads-db",
        "goose/sessions-db",
        "pi/session-jsonl",
        "mux/chat-jsonl",
        "mux/session-usage-json",
        "droid/session-jsonl",
        "kimi/wire-jsonl",
        "gemini/tmp-chats-jsonl",
        "copilot/official-copilot-runtime-jsonl",
        "codebuff/project-jsonl",
        "synthetic/sqlite",
        "warp/warp-sqlite",
        "antigravity/conversation-sqlite",
        "wukong/sqlite",
    ]:
        source_marker = f"`{source_id}`"
        if source_marker not in support_rows["字段级原生读取来源"]:
            fail(f"AGENT_SUPPORT field-level native row missing {source_id}")
        for row_name in ["Agent 合并完整覆盖", "本地派生读取来源", "辅助状态/用量来源"]:
            if source_marker in support_rows[row_name]:
                fail(f"AGENT_SUPPORT {row_name} row must not include field-level native {source_id}")
    for source_id in [
        "cursor/hook-jsonl",
        "cursor/agent-transcripts-jsonl",
        "kiro/hook-jsonl",
        "amp/threads-jsonl",
        "grok/sessions-jsonl",
        "zcode/projects-jsonl",
        "qoder/transcript-jsonl",
        "qoder-cn/transcript-jsonl",
        "qoder-work/trace-jsonl",
        "qoder-work-cn/trace-jsonl",
    ]:
        source_marker = f"`{source_id}`"
        if source_marker not in support_rows["本地派生读取来源"]:
            fail(f"AGENT_SUPPORT local-derived row missing {source_id}")
        for row_name in ["Agent 合并完整覆盖", "字段级原生读取来源"]:
            if source_marker in support_rows[row_name]:
                fail(f"AGENT_SUPPORT {row_name} row must not include local-derived {source_id}")
    for forbidden in [
        "".join(("待", "结构", "闭", "合")),
        "".join(("结构", "闭", "合前")),
        "".join(("不", "计入", "完整", "覆盖")),
    ]:
        if forbidden in support_doc:
            fail(f"AGENT_SUPPORT must not keep pending-source wording: {forbidden}")
    for source_id in [
        "copilot/otel-jsonl",
        "copilot/session-state-jsonl",
        "copilot/session-store-db",
        "copilot/vscode-chat-state",
        "qwen/token-usage-jsonl",
    ]:
        source_marker = f"`{source_id}`"
        if source_marker not in support_rows["辅助状态/用量来源"]:
            fail(f"AGENT_SUPPORT auxiliary row missing {source_id}")
        if source_marker in support_rows["Agent 合并完整覆盖"]:
            fail(f"AGENT_SUPPORT agent row must list agents, not source ids: {source_id}")


def assert_ci_gate() -> None:
    text = read(".github/workflows/ci.yml")
    required = {
        "architecture job": "Architecture gate",
        "architecture script": "python3 scripts/architecture_gate.py",
        "rust fmt": "cargo fmt --manifest-path client/Cargo.toml -- --check",
        "rust clippy": "cargo clippy --manifest-path client/Cargo.toml -- -D warnings",
        "go vet": "go vet ./...",
        "rust coverage gate": "cargo llvm-cov --manifest-path client/Cargo.toml --fail-under-lines 90",
        "java coverage gate": "mvn verify -q",
        "go coverage profile": "go test -p 1 ./... -coverprofile=cover.out",
        "go coverage threshold": 'if(val+0 < 90.0)',
        "server e2e": "bash e2e/run.sh both",
        "client local-source e2e": "bash e2e/run-client-e2e.sh both",
    }
    for label, needle in required.items():
        if needle not in text:
            fail(f"CI is missing {label}: {needle}")


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
    required_patterns = [
        "*.profraw",
        "*.profdata",
        ".coverage",
        ".coverage.*",
        "coverage.xml",
        "htmlcov/",
        "coverage/",
        "lcov.info",
        "__pycache__/",
        "*.py[cod]",
        "!.env.example",
    ]
    for pattern in required_patterns:
        if pattern not in gitignore:
            fail(f".gitignore must ignore generated artifact pattern {pattern}")
    tracked = [
        path
        for path in git_ls_files()
        if path.endswith(
            (
                ".profraw",
                ".profdata",
                ".coverage",
                ".pyc",
                ".pyo",
                ".pyd",
                "coverage.xml",
                "lcov.info",
            )
        )
        or Path(path).name.startswith(".coverage.")
        or "/__pycache__/" in path
        or path.startswith("__pycache__/")
        or "/htmlcov/" in path
        or path.startswith("htmlcov/")
        or "/coverage/" in path
        or path.startswith("coverage/")
    ]
    if tracked:
        fail(f"generated coverage artifacts are tracked: {', '.join(tracked)}")


def main() -> None:
    assert_private_paths_are_untracked()
    assert_agent_source_specs()
    assert_usage_parser_surface()
    assert_structure_probe_contract()
    assert_data_volume_gates()
    assert_e2e_matrix_gate()
    assert_e2e_diagnostics_gate()
    assert_public_docs_support_counts()
    assert_ci_gate()
    assert_client_dependency_freeze()
    assert_generated_coverage_hygiene()
    print("Architecture gate passed")


if __name__ == "__main__":
    main()
