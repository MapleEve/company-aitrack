#!/usr/bin/env python3
"""Generate isolated local usage fixtures for the client e2e matrix."""

from __future__ import annotations

import json
import sqlite3
import sys
from pathlib import Path
from typing import Any


ISO_TS = "2026-06-16T14:00:00Z"
ISO_TS_1 = "2026-06-16T14:00:01Z"
EPOCH_S = 1_781_589_600
EPOCH_MS = 1_781_589_600_000


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


def write_vscode_task_fixture(root: Path, extension: str, agent: str, model: str, provider: str) -> Path:
    task_dir = root / ".config/Code/User/globalStorage" / extension / "tasks/task-1"
    task_dir.mkdir(parents=True, exist_ok=True)
    (task_dir / "api_conversation_history.json").write_text(
        f"<environment_details><model>{model}</model><name>{agent}</name></environment_details>",
        encoding="utf-8",
    )
    path = task_dir / "ui_messages.json"
    write_json(
        path,
        [
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
            }
        ],
    )
    return path


def write_agent_fixture(root: Path, agent: str) -> Path:
    if agent == "claude":
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
                        "content": "claude output",
                        "usage": {
                            "input_tokens": 100,
                            "output_tokens": 45,
                            "cache_read_input_tokens": 10,
                            "cache_creation_input_tokens": 5,
                        },
                    },
                },
            ],
        )
        return path

    if agent == "codex":
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
                        "type": "token_count",
                        "model": "gpt-5",
                        "info": {
                            "last_token_usage": {
                                "input_tokens": 120,
                                "output_tokens": 55,
                                "cached_input_tokens": 12,
                                "reasoning_output_tokens": 7,
                            }
                        },
                    },
                },
            ],
        )
        return path

    if agent == "cursor":
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
                            "prompt": "cursor prompt",
                            "assistant_output": "cursor output",
                            "input_tokens": 90,
                            "output_tokens": 33,
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
        path = root / "Library/Application Support/Trae/trajectories/trajectory_20260616_140000.json"
        write_json(
            path,
            {
                "task_id": "trae-session",
                "timestamp": "2026-06-16T14:00:00Z",
                "input_messages": [{"role": "user", "content": "trae prompt"}],
                "response": {
                    "role": "assistant",
                    "content": "trae output",
                    "usage": {
                        "input_tokens": 100,
                        "output_tokens": 44,
                        "cache_read_tokens": 8,
                        "cache_write_tokens": 4,
                    },
                    "tool_calls": [
                        {
                            "id": "tool-1",
                            "name": "read_file",
                            "arguments": {"path": "src/main.rs"},
                        }
                    ],
                },
                "agent_steps": [
                    {
                        "tool_calls": [
                            {
                                "id": "tool-1",
                                "name": "read_file",
                                "arguments": {"path": "src/main.rs"},
                            }
                        ],
                        "tool_results": [
                            {"tool_call_id": "tool-1", "content": "file content"}
                        ],
                    }
                ],
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
                    "model": "qwen3-coder-plus",
                    "provider": "qwen",
                    "sessionId": "qwen-session",
                    "timestamp": ISO_TS,
                    "usageMetadata": {
                        "promptTokenCount": 120,
                        "candidatesTokenCount": 45,
                        "cachedContentTokenCount": 12,
                        "thoughtsTokenCount": 7,
                    },
                }
            ],
        )
        return path

    if agent == "antigravity":
        path = root / ".gemini/antigravity-ide/sessions/session-antigravity.jsonl"
        write_jsonl(
            path,
            [
                {"type": "session_meta", "sessionId": "ag-session", "modelId": "claude-sonnet-4"},
                {
                    "type": "usage",
                    "sessionId": "ag-session",
                    "modelId": "claude-sonnet-4",
                    "providerId": "anthropic",
                    "timestamp": EPOCH_MS,
                    "input": 80,
                    "output": 21,
                    "cacheRead": 6,
                    "cacheWrite": 3,
                    "reasoning": 2,
                    "responseId": "ag-response",
                },
            ],
        )
        return path

    if agent == "opencode":
        path = root / ".local/share/opencode/opencode.db"
        conn = open_fixture_db(path)
        try:
            conn.executescript(
                "CREATE TABLE session (id TEXT PRIMARY KEY, directory TEXT);"
                "CREATE TABLE message (id TEXT, session_id TEXT, data TEXT);"
            )
            conn.execute("INSERT INTO session(id, directory) VALUES ('opencode-session', '/tmp/project')")
            conn.execute(
                "INSERT INTO message(id, session_id, data) VALUES (?, ?, ?)",
                (
                    "opencode-msg",
                    "opencode-session",
                    json.dumps(
                        {
                            "id": "opencode-msg",
                            "sessionID": "opencode-session",
                            "role": "assistant",
                            "modelID": "gpt-5",
                            "providerID": "openai",
                            "cost": 0.02,
                            "tokens": {
                                "input": 88,
                                "output": 22,
                                "reasoning": 3,
                                "cache": {"read": 7, "write": 2},
                            },
                            "time": {"created": EPOCH_MS},
                        },
                        separators=(",", ":"),
                    ),
                ),
            )
            conn.commit()
        finally:
            conn.close()
        return path

    if agent in {"qoder", "qoder-cn"}:
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
                    f"{agent}-msg",
                    f"{agent}-session",
                    "req-1",
                    "assistant",
                    json.dumps(
                        {
                            "prompt_tokens": 120,
                            "completion_tokens": 45,
                            "cached_tokens": 12,
                        },
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
        dirname = ".qoderwork" if agent == "qoder-work" else ".qoderworkcn"
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
                    "gen_ai.usage.input_tokens": 70,
                    "gen_ai.usage.output_tokens": 24,
                    "gen_ai.output.messages": [{"role": "assistant", "content": f"{agent} output"}],
                },
            ],
        )
        return path

    if agent == "wukong":
        path = root / ".wukong/agent-data/messages.json"
        write_json(
            path,
            {
                "messages": [
                    {
                        "id": "wk-msg",
                        "conversationId": "wk-session",
                        "role": "assistant",
                        "content": "wukong output",
                        "createdAt": EPOCH_MS,
                        "events": [
                            {
                                "type": "USAGE",
                                "prompt_tokens": 90,
                                "completion_tokens": 35,
                                "cached_tokens": 5,
                                "total_tokens": 130,
                            }
                        ],
                        "model": "gpt-5",
                        "provider": "wukong",
                    }
                ]
            },
        )
        return path

    if agent == "hermes":
        path = root / ".hermes/state.db"
        conn = open_fixture_db(path)
        try:
            conn.execute(
                "CREATE TABLE sessions (id TEXT, model TEXT, billing_provider TEXT, input_tokens INTEGER, output_tokens INTEGER, cache_read_tokens INTEGER, cache_write_tokens INTEGER, reasoning_tokens INTEGER, estimated_cost_usd REAL, actual_cost_usd REAL)"
            )
            conn.execute(
                "INSERT INTO sessions VALUES ('hermes-session','claude-sonnet-4','anthropic',100,50,10,5,2,0.1,0.12)"
            )
            conn.commit()
        finally:
            conn.close()
        return path

    if agent == "openclaw":
        path = root / ".openclaw/agents/main/session-openclaw.jsonl"
        write_jsonl(
            path,
            [
                {"type": "model_change", "modelId": "gpt-5", "provider": "openai"},
                {
                    "type": "message",
                    "message": {
                        "role": "assistant",
                        "timestamp": EPOCH_MS,
                        "usage": {
                            "input": 100,
                            "output": 40,
                            "cacheRead": 8,
                            "cacheWrite": 3,
                            "cost": {"total": 0.03},
                        },
                        "model": "gpt-5",
                        "provider": "openai",
                    },
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
        return path

    if agent == "copilot":
        path = root / ".copilot/otel/session-copilot.jsonl"
        write_jsonl(
            path,
            [
                {
                    "type": "span",
                    "name": "chat gpt-5",
                    "startTime": [EPOCH_S, 0],
                    "attributes": {
                        "gen_ai.request.model": "gpt-5",
                        "gen_ai.response.model": "gpt-5",
                        "copilot_chat.session_id": "copilot-session",
                        "gen_ai.usage.input_tokens": 130,
                        "gen_ai.usage.output_tokens": 60,
                        "gen_ai.usage.cache_read.input_tokens": 14,
                        "gen_ai.usage.reasoning.output_tokens": 9,
                    },
                }
            ],
        )
        return path

    if agent == "cline":
        return write_vscode_task_fixture(root, "saoudrizwan.claude-dev", agent, "claude-sonnet-4", "anthropic")

    if agent == "roo-code":
        return write_vscode_task_fixture(root, "rooveterinaryinc.roo-cline", agent, "claude-sonnet-4", "anthropic")

    if agent == "kilocode":
        return write_vscode_task_fixture(root, "kilocode.kilo-code", agent, "gpt-5", "openai")

    if agent == "kiro":
        path = root / ".kiro/sessions/cli/session-kiro.json"
        write_json(
            path,
            {
                "session_id": "kiro-session",
                "session_state": {
                    "rts_model_state": {
                        "model_info": {"model_id": "claude-sonnet-4", "context_window_tokens": 200000}
                    }
                },
                "turns": [
                    {
                        "timestamp": ISO_TS,
                        "input_token_count": 100,
                        "output_token_count": 40,
                        "response_size": 160,
                    }
                ],
            },
        )
        return path

    if agent == "zed":
        path = root / ".local/share/zed/threads/threads.db"
        conn = open_fixture_db(path)
        try:
            conn.execute(
                "CREATE TABLE threads (id TEXT, updated_at TEXT, created_at TEXT, folder_paths TEXT, folder_paths_order TEXT, data_type TEXT, data TEXT)"
            )
            conn.execute(
                "INSERT INTO threads VALUES (?, ?, ?, ?, ?, ?, ?)",
                (
                    "zed-thread",
                    ISO_TS,
                    ISO_TS,
                    json.dumps(["/tmp/project"]),
                    json.dumps([0]),
                    "json",
                    json.dumps(
                        {
                            "model": {"provider": "zed.dev", "model": "claude-sonnet-4"},
                            "request_token_usage": {
                                "input_tokens": 90,
                                "output_tokens": 35,
                                "cache_read_tokens": 8,
                            },
                            "message_count": 1,
                        },
                        separators=(",", ":"),
                    ),
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
            conn.commit()
        finally:
            conn.close()
        return path

    if agent == "amp":
        path = root / ".local/share/amp/threads/T-thread-amp.json"
        write_json(
            path,
            {
                "id": "thread-amp",
                "usageLedger": {
                    "events": [
                        {
                            "timestamp": ISO_TS,
                            "model": "claude-sonnet-4",
                            "tokens": {"input": 100, "output": 45, "cacheRead": 8, "cacheWrite": 4},
                        }
                    ]
                },
            },
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
        return path

    if agent == "pi":
        path = root / ".pi/agent/sessions/session-pi.jsonl"
        write_jsonl(
            path,
            [
                {"type": "session", "id": "pi-session", "cwd": "/tmp/project"},
                {
                    "type": "message",
                    "message": {
                        "role": "assistant",
                        "model": "gpt-5",
                        "provider": "openai",
                        "timestamp": EPOCH_MS,
                        "usage": {"input": 100, "output": 40, "cacheRead": 6, "cacheWrite": 2},
                    },
                },
            ],
        )
        return path

    if agent == "mux":
        path = root / ".mux/sessions/workspace-one/session-usage.json"
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
        return path

    if agent == "crush":
        path = root / ".local/share/crush/crush.db"
        conn = open_fixture_db(path)
        try:
            conn.executescript(
                "CREATE TABLE sessions (id TEXT, parent_session_id TEXT, message_count INTEGER, cost REAL, created_at INTEGER, updated_at INTEGER);"
                "CREATE TABLE messages (id TEXT, session_id TEXT, role TEXT, created_at INTEGER);"
            )
            conn.execute("INSERT INTO sessions VALUES ('crush-session', NULL, 1, 0.25, ?, ?)", (EPOCH_S, EPOCH_S))
            conn.execute("INSERT INTO messages VALUES ('crush-msg', 'crush-session', 'assistant', ?)", (EPOCH_S,))
            conn.commit()
        finally:
            conn.close()
        return path

    if agent == "codebuff":
        path = root / ".config/manicode/projects/project-one/chats/2026-06-16T14-00-00.000Z/chat-messages.json"
        write_json(
            path,
            [
                {
                    "id": "codebuff-msg",
                    "role": "assistant",
                    "createdAt": ISO_TS,
                    "metadata": {
                        "usage": {
                            "model": "gpt-5",
                            "input_tokens": 100,
                            "output_tokens": 50,
                            "cache_read_input_tokens": 5,
                            "cache_creation_input_tokens": 2,
                            "credits": 0.1,
                        }
                    },
                }
            ],
        )
        return path

    if agent == "kilo":
        path = root / ".local/share/kilo/kilo.db"
        conn = open_fixture_db(path)
        try:
            conn.execute("CREATE TABLE message (id TEXT, session_id TEXT, data TEXT)")
            conn.execute(
                "INSERT INTO message VALUES (?, ?, ?)",
                (
                    "kilo-msg",
                    "kilo-session",
                    json.dumps(
                        {
                            "id": "kilo-msg",
                            "session_id": "kilo-session",
                            "role": "assistant",
                            "modelID": "gpt-5",
                            "providerID": "openai",
                            "tokens": {
                                "input": 100,
                                "output": 40,
                                "reasoning": 4,
                                "cache": {"read": 8, "write": 2},
                            },
                            "time": {"created": EPOCH_MS},
                        },
                        separators=(",", ":"),
                    ),
                ),
            )
            conn.commit()
        finally:
            conn.close()
        return path

    if agent == "kimi":
        path = root / ".kimi/sessions/group/session/wire.jsonl"
        write_jsonl(
            path,
            [
                {
                    "type": "StatusUpdate",
                    "timestamp": ISO_TS,
                    "sessionId": "kimi-session",
                    "payload": {
                        "token_usage": {
                            "input_other": 100,
                            "output": 45,
                            "input_cache_read": 10,
                            "input_cache_creation": 5,
                        }
                    },
                }
            ],
        )
        return path

    if agent == "gjc":
        path = root / ".gjc/agent/sessions/project/session-gjc.jsonl"
        write_jsonl(
            path,
            [
                {"type": "session", "id": "gjc-session", "cwd": "/tmp/project"},
                {
                    "type": "message",
                    "id": "gjc-msg",
                    "message": {
                        "role": "assistant",
                        "model": "gpt-5",
                        "provider": "openai",
                        "timestamp": EPOCH_MS,
                        "usage": {
                            "input": 100,
                            "output": 50,
                            "cacheRead": 7,
                            "cacheWrite": 3,
                            "cost": {"total": 0.02},
                        },
                    },
                },
            ],
        )
        return path

    if agent == "grok":
        path = root / ".grok/sessions/workspace/grok-session/updates.jsonl"
        write_jsonl(
            path,
            [
                {"type": "user", "timestamp": ISO_TS, "content": "grok prompt", "model": "grok-code-fast"},
                {"type": "usage", "timestamp": ISO_TS_1, "totalTokens": 150, "model": "grok-code-fast"},
            ],
        )
        return path

    if agent == "synthetic":
        path = root / ".local/share/octofriend/sqlite.db"
        conn = open_fixture_db(path)
        try:
            conn.execute(
                "CREATE TABLE messages (id TEXT, model TEXT, input_tokens INTEGER, output_tokens INTEGER, cache_read_tokens INTEGER, cache_write_tokens INTEGER, reasoning_tokens INTEGER, cost REAL, timestamp INTEGER, session_id TEXT, provider TEXT)"
            )
            conn.execute(
                "INSERT INTO messages VALUES ('synthetic-msg','hf:test-model',100,40,0,0,0,0.01,?,'synthetic-session','synthetic')",
                (EPOCH_MS,),
            )
            conn.commit()
        finally:
            conn.close()
        return path

    if agent == "warp":
        path = root / ".config/aitrack/warp-cache/usage-2026-06-16.json"
        write_json(
            path,
            {
                "syncedAt": ISO_TS,
                "usage": {"requestsUsed": 3, "spendCents": 42},
                "workspaces": [],
            },
        )
        return path

    if agent == "zcode":
        path = root / ".zcode/cli/db/db.sqlite"
        conn = open_fixture_db(path)
        try:
            conn.executescript(
                "CREATE TABLE session (id TEXT, directory TEXT);"
                "CREATE TABLE message (id TEXT, session_id TEXT, model TEXT, provider TEXT, input_tokens INTEGER, output_tokens INTEGER, cache_read_tokens INTEGER, cache_write_tokens INTEGER, timestamp INTEGER);"
            )
            conn.execute("INSERT INTO session VALUES ('zcode-session', '/tmp/project')")
            conn.execute(
                "INSERT INTO message VALUES ('zcode-msg','zcode-session','gpt-5','openai',100,40,8,3,?)",
                (EPOCH_MS,),
            )
            conn.commit()
        finally:
            conn.close()
        return path

    raise SystemExit(f"ERROR: no native usage fixture for {agent}")


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: local_usage_matrix.py <root> <agent>")
    path = write_agent_fixture(Path(sys.argv[1]), sys.argv[2])
    print(path)


if __name__ == "__main__":
    main()
