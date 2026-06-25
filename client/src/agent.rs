use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Agent {
    pub name: &'static str,
    marker: &'static str,
    pub has_native_edit_adapter: bool,
    pub has_native_prompt_hook: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalSourceKind {
    HookJsonl,
    SessionJsonl,
    TelemetryLog,
    Sqlite,
    IdeSnapshot,
    GenericCache,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalSourceCapabilities {
    pub prompt_input: bool,
    pub assistant_output: bool,
    pub tool_call: bool,
    pub tool_result: bool,
    pub token_usage: bool,
    pub session_context: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalSourceSpec {
    pub agent: &'static str,
    pub kind: LocalSourceKind,
    pub label: &'static str,
    pub capabilities: LocalSourceCapabilities,
}

const NATIVE_EDIT_HOOK_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: false,
    tool_call: false,
    tool_result: false,
    token_usage: false,
    session_context: true,
};

const CLAUDE_HOOK_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: false,
    tool_call: false,
    tool_result: false,
    token_usage: false,
    session_context: true,
};

const GEMINI_TELEMETRY_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: false,
    token_usage: true,
    session_context: true,
};

const QWEN_TELEMETRY_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: false,
    token_usage: true,
    session_context: true,
};

const LOCAL_USAGE_STATS_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: false,
    assistant_output: false,
    tool_call: false,
    tool_result: false,
    token_usage: true,
    session_context: true,
};

const OFFICIAL_TRANSCRIPT_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
};

const OFFICIAL_SESSION_EXPORT_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: false,
    tool_result: false,
    token_usage: true,
    session_context: true,
};

const OFFICIAL_SESSION_TEXT_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: false,
    tool_result: false,
    token_usage: false,
    session_context: true,
};

const OFFICIAL_SESSION_ACTION_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: false,
    session_context: true,
};

const OFFICIAL_HOOK_ACTION_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: false,
    session_context: true,
};

const OFFICIAL_PROMPT_TOOL_HOOK_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: false,
    tool_call: true,
    tool_result: true,
    token_usage: false,
    session_context: true,
};

const OFFICIAL_STREAM_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: false,
    token_usage: false,
    session_context: false,
};

const OFFICIAL_SESSION_USAGE_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: false,
    tool_result: false,
    token_usage: true,
    session_context: true,
};

const OFFICIAL_SESSION_TOOL_USAGE_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: false,
    token_usage: true,
    session_context: true,
};

const OFFICIAL_OUTPUT_TOOL_EVENT_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: false,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: false,
    session_context: false,
};

const OFFICIAL_TELEMETRY_USAGE_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: false,
    assistant_output: false,
    tool_call: true,
    tool_result: false,
    token_usage: true,
    session_context: true,
};

const OFFICIAL_PROMPT_OUTPUT_SESSION_CAPABILITIES: LocalSourceCapabilities =
    LocalSourceCapabilities {
        prompt_input: true,
        assistant_output: true,
        tool_call: false,
        tool_result: false,
        token_usage: false,
        session_context: true,
    };

const OFFICIAL_TOOL_RESULT_USAGE_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: false,
    tool_result: true,
    token_usage: true,
    session_context: true,
};

const CONDITIONAL_OTEL_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: false,
    assistant_output: false,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
};

impl Agent {
    pub fn marker_path(&self, home: &Path) -> PathBuf {
        home.join(self.marker)
    }
}

pub const REGISTERED_AGENTS: &[Agent] = &[
    Agent {
        name: "claude",
        marker: ".claude",
        has_native_edit_adapter: true,
        has_native_prompt_hook: true,
    },
    Agent {
        name: "codex",
        marker: ".codex",
        has_native_edit_adapter: true,
        has_native_prompt_hook: true,
    },
    Agent {
        name: "cursor",
        marker: ".cursor",
        has_native_edit_adapter: true,
        has_native_prompt_hook: true,
    },
    Agent {
        name: "trae",
        marker: ".trae",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "qwen",
        marker: ".qwen",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "antigravity",
        marker: ".antigravity",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "opencode",
        marker: ".opencode",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "qoder",
        marker: ".qoder",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "qoder-cn",
        marker: ".qoder-cn",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "qoder-work",
        marker: ".qoderwork",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "qoder-work-cn",
        marker: ".qoderworkcn",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "wukong",
        marker: ".wukong",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "hermes",
        marker: ".hermes",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "openclaw",
        marker: ".openclaw",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "gemini",
        marker: ".gemini",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "copilot",
        marker: ".copilot",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "cline",
        marker: ".cline",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "roo-code",
        marker: ".roo-code",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "roocode",
        marker: ".roo-code",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "kiro",
        marker: ".kiro",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "zed",
        marker: ".zed",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "goose",
        marker: ".goose",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "amp",
        marker: ".amp",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "droid",
        marker: ".factory",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "pi",
        marker: ".pi",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "mux",
        marker: ".mux",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "crush",
        marker: ".crush",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "codebuff",
        marker: ".codebuff",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "kilo",
        marker: ".kilo",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "kilocode",
        marker: ".kilo-code",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "kilo-code",
        marker: ".kilo-code",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "kimi",
        marker: ".kimi",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "gjc",
        marker: ".gjc",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "gajae-code",
        marker: ".gjc",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "grok",
        marker: ".grok",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "synthetic",
        marker: ".synthetic",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "warp",
        marker: ".warp",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "zcode",
        marker: ".zcode",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
];

const LOCAL_SOURCE_SPECS: &[LocalSourceSpec] = &[
    LocalSourceSpec {
        agent: "claude",
        kind: LocalSourceKind::HookJsonl,
        label: "hook-jsonl",
        capabilities: CLAUDE_HOOK_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "claude",
        kind: LocalSourceKind::SessionJsonl,
        label: "projects-jsonl",
        capabilities: OFFICIAL_SESSION_ACTION_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "codex",
        kind: LocalSourceKind::HookJsonl,
        label: "hook-jsonl",
        capabilities: NATIVE_EDIT_HOOK_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "codex",
        kind: LocalSourceKind::SessionJsonl,
        label: "rollout-jsonl",
        capabilities: OFFICIAL_TRANSCRIPT_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "cursor",
        kind: LocalSourceKind::HookJsonl,
        label: "hook-jsonl",
        capabilities: NATIVE_EDIT_HOOK_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "trae",
        kind: LocalSourceKind::SessionJsonl,
        label: "trajectory-json",
        capabilities: OFFICIAL_TRANSCRIPT_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qwen",
        kind: LocalSourceKind::TelemetryLog,
        label: "telemetry-log",
        capabilities: QWEN_TELEMETRY_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "opencode",
        kind: LocalSourceKind::SessionJsonl,
        label: "export-json",
        capabilities: OFFICIAL_SESSION_EXPORT_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "opencode",
        kind: LocalSourceKind::Sqlite,
        label: "sqlite",
        capabilities: LOCAL_USAGE_STATS_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder",
        kind: LocalSourceKind::HookJsonl,
        label: "hook-jsonl",
        capabilities: OFFICIAL_PROMPT_TOOL_HOOK_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder-cn",
        kind: LocalSourceKind::HookJsonl,
        label: "hook-jsonl",
        capabilities: OFFICIAL_PROMPT_TOOL_HOOK_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "wukong",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: OFFICIAL_SESSION_USAGE_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "hermes",
        kind: LocalSourceKind::Sqlite,
        label: "sqlite",
        capabilities: OFFICIAL_SESSION_USAGE_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "openclaw",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: OFFICIAL_TRANSCRIPT_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "gemini",
        kind: LocalSourceKind::TelemetryLog,
        label: "telemetry-log",
        capabilities: GEMINI_TELEMETRY_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "copilot",
        kind: LocalSourceKind::IdeSnapshot,
        label: "otel-jsonl",
        capabilities: CONDITIONAL_OTEL_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "cline",
        kind: LocalSourceKind::SessionJsonl,
        label: "vscode-tasks",
        capabilities: OFFICIAL_TRANSCRIPT_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "kiro",
        kind: LocalSourceKind::Sqlite,
        label: "sessions-db",
        capabilities: OFFICIAL_HOOK_ACTION_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "zed",
        kind: LocalSourceKind::Sqlite,
        label: "threads-db",
        capabilities: OFFICIAL_TRANSCRIPT_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "goose",
        kind: LocalSourceKind::Sqlite,
        label: "sessions-db",
        capabilities: OFFICIAL_TRANSCRIPT_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "amp",
        kind: LocalSourceKind::SessionJsonl,
        label: "threads-jsonl",
        capabilities: OFFICIAL_STREAM_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "droid",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: OFFICIAL_TELEMETRY_USAGE_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "pi",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: OFFICIAL_TRANSCRIPT_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "mux",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: LOCAL_USAGE_STATS_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "crush",
        kind: LocalSourceKind::Sqlite,
        label: "sqlite",
        capabilities: OFFICIAL_SESSION_TOOL_USAGE_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "codebuff",
        kind: LocalSourceKind::SessionJsonl,
        label: "project-jsonl",
        capabilities: OFFICIAL_OUTPUT_TOOL_EVENT_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "kilo",
        kind: LocalSourceKind::Sqlite,
        label: "sqlite",
        capabilities: OFFICIAL_SESSION_EXPORT_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "kilocode",
        kind: LocalSourceKind::SessionJsonl,
        label: "vscode-tasks",
        capabilities: OFFICIAL_SESSION_TEXT_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "kimi",
        kind: LocalSourceKind::SessionJsonl,
        label: "wire-jsonl",
        capabilities: OFFICIAL_SESSION_ACTION_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "gjc",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: OFFICIAL_TOOL_RESULT_USAGE_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "grok",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: OFFICIAL_PROMPT_OUTPUT_SESSION_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "warp",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: OFFICIAL_PROMPT_OUTPUT_SESSION_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "zcode",
        kind: LocalSourceKind::Sqlite,
        label: "sqlite",
        capabilities: LOCAL_USAGE_STATS_CAPABILITIES,
    },
];

pub fn registered_agents() -> &'static [Agent] {
    REGISTERED_AGENTS
}

pub fn local_source_specs() -> &'static [LocalSourceSpec] {
    LOCAL_SOURCE_SPECS
}

pub fn registered_agent_names() -> Vec<&'static str> {
    REGISTERED_AGENTS.iter().map(|agent| agent.name).collect()
}

pub fn default_scan_agent_names() -> Vec<&'static str> {
    let mut names = Vec::new();
    for spec in LOCAL_SOURCE_SPECS {
        if !is_default_scan_excluded(spec.agent) && !names.contains(&spec.agent) {
            names.push(spec.agent);
        }
    }
    names
}

pub fn agent_by_name(name: &str) -> Option<&'static Agent> {
    REGISTERED_AGENTS.iter().find(|agent| agent.name == name)
}

pub fn is_known_agent(name: &str) -> bool {
    agent_by_name(name).is_some()
}

pub fn canonical_agent_name(name: &str) -> &str {
    match name {
        "roocode" => "roo-code",
        "kilo-code" => "kilocode",
        "gajae-code" => "gjc",
        other => other,
    }
}

fn is_default_scan_excluded(name: &str) -> bool {
    matches!(name, "roocode" | "kilo-code" | "gajae-code")
}

pub fn default_scan_roots(home: &Path, tool: &str) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    match tool {
        "claude" => {
            roots.push(home.join(".claude").join("projects"));
            roots.push(home.join(".claude").join("transcripts"));
        }
        "codex" => {
            let codex_home = env_or_home(home, "CODEX_HOME", ".codex");
            roots.push(codex_home.join("sessions"));
        }
        "cursor" => {
            roots.push(home.join("Library/Application Support/Cursor/User/globalStorage"));
            roots.push(home.join(".config/Cursor/User/globalStorage"));
        }
        "trae" => {
            roots.push(home.join("Library/Application Support/Trae"));
            roots.push(home.join("Library/Application Support/TRAE SOLO"));
            roots.push(home.join(".config/Trae"));
            roots.push(home.join(".config/trae"));
        }
        "opencode" => {
            roots.push(xdg_data_home(home).join("opencode"));
            roots.push(home.join(".config/opencode"));
        }
        "qoder" => {
            roots.push(home.join(".qoder"));
            roots.push(home.join("Library/Application Support/Qoder"));
            roots.push(home.join(".config/Qoder"));
            roots.push(home.join("Library/Application Support/Qoder/SharedClientCache/cache/db"));
            roots.push(home.join(".config/Qoder/SharedClientCache/cache/db"));
        }
        "qoder-cn" => {
            roots.push(home.join(".qoder-cn"));
            roots.push(home.join("Library/Application Support/QoderCN"));
            roots.push(home.join(".config/QoderCN"));
            roots.push(home.join("Library/Application Support/QoderCN/SharedClientCache/cache/db"));
            roots.push(home.join(".config/QoderCN/SharedClientCache/cache/db"));
        }
        "qoder-work" => {
            roots.push(home.join(".qoderwork"));
            roots.push(home.join(".qoderwork/data"));
        }
        "qoder-work-cn" => {
            roots.push(home.join(".qoderworkcn"));
            roots.push(home.join(".qoderworkcn/data"));
        }
        "wukong" => {
            roots.push(home.join(".wukong"));
        }
        "gemini" => {
            let gemini_home = env_or_home(home, "GEMINI_CLI_HOME", ".gemini");
            roots.push(gemini_home.clone());
            roots.push(gemini_home.join("tmp"));
        }
        "copilot" => {
            roots.push(home.join(".copilot").join("otel"));
            roots.push(home.join(".copilot").join("session-state"));
            if let Some(path) = env_path("COPILOT_OTEL_FILE_EXPORTER_PATH") {
                roots.push(path);
            }
        }
        "cline" => {
            roots.push(env_or_home(home, "CLINE_DATA_DIR", ".cline/data").join("sessions"));
            roots.push(home.join(".config/Code/User/globalStorage/saoudrizwan.claude-dev/tasks"));
            roots.push(home.join(
                "Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/tasks",
            ));
            roots.push(
                home.join(
                    "Library/Application Support/Cursor/User/globalStorage/saoudrizwan.claude-dev/tasks",
                ),
            );
            roots.push(
                home.join(".vscode-server/data/User/globalStorage/saoudrizwan.claude-dev/tasks"),
            );
        }
        "roo-code" | "roocode" => {
            roots.push(
                home.join(".config/Code/User/globalStorage/rooveterinaryinc.roo-cline/tasks"),
            );
            roots.push(
                home.join(
                    "Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/tasks",
                ),
            );
            roots.push(
                home.join(
                    "Library/Application Support/Cursor/User/globalStorage/rooveterinaryinc.roo-cline/tasks",
                ),
            );
            roots.push(
                home.join(
                    ".vscode-server/data/User/globalStorage/rooveterinaryinc.roo-cline/tasks",
                ),
            );
        }
        "kilocode" | "kilo-code" => {
            roots.push(home.join(".config/Code/User/globalStorage/kilocode.kilo-code/tasks"));
            roots.push(home.join(
                "Library/Application Support/Code/User/globalStorage/kilocode.kilo-code/tasks",
            ));
            roots.push(home.join(
                "Library/Application Support/Cursor/User/globalStorage/kilocode.kilo-code/tasks",
            ));
            roots
                .push(home.join(".vscode-server/data/User/globalStorage/kilocode.kilo-code/tasks"));
            roots.push(home.join(".kilocode/cli/global/tasks"));
            roots.push(home.join(".kilocode/cli/workspaces"));
        }
        "kilo" => {
            roots.push(xdg_data_home(home).join("kilo"));
            roots.push(xdg_data_home(home).join("kilo/storage/session"));
            roots.push(home.join("Library/Application Support/kilo/storage/session"));
        }
        "kiro" => {
            let kiro_home = env_or_home(home, "KIRO_HOME", ".kiro");
            roots.push(kiro_home.clone());
            roots.push(kiro_home.join("sessions"));
            roots.push(home.join(".kiro/sessions/cli"));
            roots.push(xdg_data_home(home).join("kiro-cli"));
            roots.push(home.join("Library/Application Support/kiro-cli"));
            roots.push(home.join(".config/Code/User/globalStorage/kiro.kiro-agent"));
            roots.push(
                home.join("Library/Application Support/Code/User/globalStorage/kiro.kiro-agent"),
            );
        }
        "zed" => {
            roots.push(xdg_data_home(home).join("zed/threads"));
            roots.push(home.join("Library/Application Support/Zed/threads"));
        }
        "goose" => {
            if let Some(path) = env_path("GOOSE_PATH_ROOT") {
                roots.push(path);
            }
            roots.push(xdg_data_home(home).join("goose/sessions"));
            roots.push(home.join("Library/Application Support/goose/sessions"));
            roots.push(home.join(".local/share/Block/goose/sessions"));
        }
        "amp" => {
            roots.push(xdg_data_home(home).join("amp/threads"));
        }
        "droid" => {
            roots.push(home.join(".factory/sessions"));
        }
        "openclaw" => {
            roots.push(home.join(".openclaw/agents"));
        }
        "pi" => {
            roots.push(home.join(".pi/agent/sessions"));
            roots.push(home.join(".omp/agent/sessions"));
        }
        "kimi" => {
            let kimi_home = env_or_home(home, "KIMI_CODE_HOME", ".kimi-code");
            roots.push(home.join(".kimi/sessions"));
            roots.push(kimi_home.join("sessions"));
            roots.push(kimi_home.join("session_index.jsonl"));
        }
        "qwen" => {
            roots.push(home.join(".qwen"));
            roots.push(home.join(".qwen/projects"));
        }
        "codebuff" => {
            roots.push(env_or_home(home, "CODEBUFF_DATA_DIR", ".config/manicode").join("projects"));
            roots.push(home.join(".config/manicode-dev/projects"));
            roots.push(home.join(".config/manicode-staging/projects"));
        }
        "mux" => {
            roots.push(home.join(".mux/sessions"));
        }
        "crush" => {
            roots.push(xdg_data_home(home).join("crush"));
        }
        "hermes" => {
            roots.push(env_or_home(home, "HERMES_HOME", ".hermes"));
        }
        "antigravity" => {
            roots.push(xdg_config_home(home).join("aitrack/antigravity-cache/sessions"));
            roots.push(home.join(".gemini/antigravity-ide"));
            roots.push(home.join(".gemini/antigravity"));
            roots.push(home.join(".gemini/antigravity-backup"));
        }
        "gjc" | "gajae-code" => {
            roots.push(env_or_home(
                home,
                "GJC_CODING_AGENT_DIR",
                ".gjc/agent/sessions",
            ));
            roots.push(env_or_home(home, "GJC_CONFIG_DIR", ".gjc").join("agent/sessions"));
            roots.push(env_or_home(home, "PI_CONFIG_DIR", ".gjc").join("agent/sessions"));
            roots.push(xdg_data_home(home).join("gjc/sessions"));
        }
        "grok" => {
            roots.push(env_or_home(home, "GROK_HOME", ".grok").join("sessions"));
        }
        "synthetic" => {
            roots.push(xdg_data_home(home).join("octofriend"));
        }
        "warp" => {
            roots.push(xdg_config_home(home).join("aitrack/warp-cache"));
            roots.push(home.join(".warp"));
        }
        "zcode" => {
            roots.push(home.join(".zcode/cli/db"));
            roots.push(home.join(".zcode/cli"));
        }
        _ => {
            if !is_known_agent(tool) {
                roots.push(home.join(format!(".{tool}")));
            }
        }
    }

    dedup_paths(roots)
}

fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn env_or_home(home: &Path, var: &str, fallback: &str) -> PathBuf {
    env_path(var).unwrap_or_else(|| home.join(fallback))
}

fn xdg_data_home(home: &Path) -> PathBuf {
    env_path("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share"))
}

fn xdg_config_home(home: &Path) -> PathBuf {
    env_path("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config"))
}

fn dedup_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for path in paths {
        if !out.iter().any(|existing| existing == &path) {
            out.push(path);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::ENV_LOCK;
    use tempfile::TempDir;

    #[test]
    fn registered_agents_include_reference_frameworks_and_aliases() {
        for name in [
            "claude",
            "codex",
            "cursor",
            "opencode",
            "trae",
            "copilot",
            "gemini",
            "amp",
            "codebuff",
            "droid",
            "openclaw",
            "hermes",
            "pi",
            "kimi",
            "qwen",
            "roo-code",
            "roocode",
            "kilocode",
            "kilo-code",
            "kilo",
            "mux",
            "crush",
            "goose",
            "antigravity",
            "zed",
            "kiro",
            "gjc",
            "gajae-code",
            "grok",
            "synthetic",
            "qoder",
            "qoder-cn",
            "qoder-work",
            "qoder-work-cn",
            "wukong",
            "zcode",
        ] {
            assert!(is_known_agent(name), "{name} should be registered");
        }
        assert!(!default_scan_agent_names().contains(&"roocode"));
        assert!(!default_scan_agent_names().contains(&"kilo-code"));
        assert!(!default_scan_agent_names().contains(&"gajae-code"));
        assert!(!default_scan_agent_names().contains(&"antigravity"));
        assert!(!default_scan_agent_names().contains(&"qoder-work"));
        assert!(!default_scan_agent_names().contains(&"qoder-work-cn"));
        assert!(!default_scan_agent_names().contains(&"roo-code"));
        assert!(!default_scan_agent_names().contains(&"synthetic"));
        assert_eq!(default_scan_agent_names().len(), 30);
        assert_eq!(canonical_agent_name("roocode"), "roo-code");
        assert_eq!(canonical_agent_name("kilo-code"), "kilocode");
        assert_eq!(canonical_agent_name("gajae-code"), "gjc");
        assert!(
            agent_by_name("codex")
                .expect("codex registered")
                .has_native_prompt_hook,
            "Codex official hooks include UserPromptSubmit"
        );
        assert!(
            agent_by_name("cursor")
                .expect("cursor registered")
                .has_native_prompt_hook,
            "Cursor official hooks include beforeSubmitPrompt"
        );
    }

    #[test]
    fn local_source_specs_do_not_overclaim_undocumented_local_fields() {
        let specs = local_source_specs();

        for agent in default_scan_agent_names() {
            let matching = specs
                .iter()
                .filter(|spec| spec.agent == agent)
                .collect::<Vec<_>>();
            assert!(!matching.is_empty(), "{agent} source spec missing");
            assert!(
                matching.iter().all(|spec| {
                    !all_capabilities_enabled(spec.capabilities)
                        || official_full_capability_source(spec)
                }),
                "{agent} source spec overclaims every local field without official evidence"
            );
        }

        assert!(!specs.iter().any(|spec| spec.agent == "baidu-comate"));
        assert!(!specs.iter().any(|spec| spec.agent == "wenxin"));
        assert!(!specs.iter().any(|spec| spec.agent == "synthetic"));
        assert!(!specs.iter().any(|spec| spec.agent == "antigravity"));
        assert!(!specs.iter().any(|spec| spec.agent == "qoder-work"));
        assert!(!specs.iter().any(|spec| spec.agent == "qoder-work-cn"));
        assert!(!specs.iter().any(|spec| spec.agent == "roo-code"));
        assert!(!specs
            .iter()
            .any(|spec| spec.agent == "qoder" && spec.kind == LocalSourceKind::Sqlite));
        assert!(specs
            .iter()
            .any(|spec| spec.agent == "copilot" && spec.kind == LocalSourceKind::IdeSnapshot));

        let gemini = specs
            .iter()
            .find(|spec| spec.agent == "gemini")
            .expect("gemini source spec missing");
        assert_eq!(
            gemini.capabilities,
            LocalSourceCapabilities {
                prompt_input: true,
                assistant_output: true,
                tool_call: true,
                tool_result: false,
                token_usage: true,
                session_context: true,
            }
        );

        let qwen = specs
            .iter()
            .find(|spec| spec.agent == "qwen")
            .expect("qwen source spec missing");
        assert_eq!(
            qwen.capabilities,
            LocalSourceCapabilities {
                prompt_input: true,
                assistant_output: true,
                tool_call: true,
                tool_result: false,
                token_usage: true,
                session_context: true,
            }
        );

        let codex_rollout = specs
            .iter()
            .find(|spec| spec.agent == "codex" && spec.label == "rollout-jsonl")
            .expect("codex rollout source spec missing");
        assert_eq!(codex_rollout.capabilities, OFFICIAL_TRANSCRIPT_CAPABILITIES);

        let claude_projects = specs
            .iter()
            .find(|spec| spec.agent == "claude" && spec.label == "projects-jsonl")
            .expect("claude projects source spec missing");
        assert_eq!(
            claude_projects.capabilities,
            OFFICIAL_SESSION_ACTION_CAPABILITIES
        );

        let opencode_sqlite = specs
            .iter()
            .find(|spec| spec.agent == "opencode" && spec.kind == LocalSourceKind::Sqlite)
            .expect("opencode sqlite source spec missing");
        assert_eq!(
            opencode_sqlite.capabilities,
            LocalSourceCapabilities {
                prompt_input: false,
                assistant_output: false,
                tool_call: false,
                tool_result: false,
                token_usage: true,
                session_context: true,
            }
        );

        let opencode_export = specs
            .iter()
            .find(|spec| spec.agent == "opencode" && spec.label == "export-json")
            .expect("opencode export source spec missing");
        assert_eq!(
            opencode_export.capabilities,
            OFFICIAL_SESSION_EXPORT_CAPABILITIES
        );

        let kilo = specs
            .iter()
            .find(|spec| spec.agent == "kilo" && spec.label == "sqlite")
            .expect("kilo source spec missing");
        assert_eq!(kilo.capabilities, OFFICIAL_SESSION_EXPORT_CAPABILITIES);

        let kiro = specs
            .iter()
            .find(|spec| spec.agent == "kiro" && spec.label == "sessions-db")
            .expect("kiro source spec missing");
        assert_eq!(kiro.capabilities, OFFICIAL_HOOK_ACTION_CAPABILITIES);

        let zed = specs
            .iter()
            .find(|spec| spec.agent == "zed" && spec.label == "threads-db")
            .expect("zed source spec missing");
        assert_eq!(zed.capabilities, OFFICIAL_TRANSCRIPT_CAPABILITIES);

        let openclaw = specs
            .iter()
            .find(|spec| spec.agent == "openclaw" && spec.label == "session-jsonl")
            .expect("openclaw source spec missing");
        assert_eq!(openclaw.capabilities, OFFICIAL_TRANSCRIPT_CAPABILITIES);

        let pi = specs
            .iter()
            .find(|spec| spec.agent == "pi" && spec.label == "session-jsonl")
            .expect("pi source spec missing");
        assert_eq!(pi.capabilities, OFFICIAL_TRANSCRIPT_CAPABILITIES);

        let kilocode = specs
            .iter()
            .find(|spec| spec.agent == "kilocode" && spec.label == "vscode-tasks")
            .expect("kilocode source spec missing");
        assert_eq!(kilocode.capabilities, OFFICIAL_SESSION_TEXT_CAPABILITIES);

        let kimi = specs
            .iter()
            .find(|spec| spec.agent == "kimi" && spec.label == "wire-jsonl")
            .expect("kimi source spec missing");
        assert_eq!(kimi.capabilities, OFFICIAL_SESSION_ACTION_CAPABILITIES);
    }

    fn all_capabilities_enabled(capabilities: LocalSourceCapabilities) -> bool {
        capabilities.prompt_input
            && capabilities.assistant_output
            && capabilities.tool_call
            && capabilities.tool_result
            && capabilities.token_usage
            && capabilities.session_context
    }

    fn official_full_capability_source(spec: &LocalSourceSpec) -> bool {
        matches!(
            (spec.agent, spec.label),
            ("codex", "rollout-jsonl")
                | ("trae", "trajectory-json")
                | ("cline", "vscode-tasks")
                | ("openclaw", "session-jsonl")
                | ("zed", "threads-db")
                | ("goose", "sessions-db")
                | ("pi", "session-jsonl")
        )
    }

    #[test]
    fn default_scan_roots_cover_local_agent_storage() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        std::env::set_var("CODEX_HOME", home.join("custom-codex"));
        std::env::set_var("GEMINI_CLI_HOME", home.join("custom-gemini"));
        std::env::set_var("CLINE_DATA_DIR", home.join(".cline/data"));
        std::env::set_var("KIMI_CODE_HOME", home.join(".custom-kimi-code"));
        std::env::set_var("KIRO_HOME", home.join(".custom-kiro"));
        std::env::set_var(
            "COPILOT_OTEL_FILE_EXPORTER_PATH",
            home.join("copilot.jsonl"),
        );

        assert!(default_scan_roots(home, "codex")
            .iter()
            .any(|p| p.ends_with("custom-codex/sessions")));
        assert!(!default_scan_roots(home, "codex")
            .iter()
            .any(|p| p == &home.join("custom-codex")));
        assert!(default_scan_roots(home, "qwen")
            .iter()
            .any(|p| p == &home.join(".qwen")));
        assert!(default_scan_roots(home, "gemini")
            .iter()
            .any(|p| p.ends_with("custom-gemini")));
        assert!(default_scan_roots(home, "gemini")
            .iter()
            .any(|p| p.ends_with("custom-gemini/tmp")));
        assert!(default_scan_roots(home, "copilot")
            .iter()
            .any(|p| p.ends_with("copilot.jsonl")));
        assert!(default_scan_roots(home, "cline")
            .iter()
            .any(|p| p.ends_with(".cline/data/sessions")));
        assert!(default_scan_roots(home, "cline")
            .iter()
            .any(|p| p.to_string_lossy().contains(
                "Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/tasks"
            )));
        assert!(default_scan_roots(home, "zed")
            .iter()
            .any(|p| p.to_string_lossy().contains("Zed/threads")));
        assert!(default_scan_roots(home, "qoder")
            .iter()
            .any(|p| p.to_string_lossy().contains("SharedClientCache/cache/db")));
        assert!(default_scan_roots(home, "roo-code")
            .iter()
            .any(|p| p.to_string_lossy().contains("roo-cline/tasks")));
        assert!(default_scan_roots(home, "roo-code").iter().any(|p| p
            .to_string_lossy()
            .contains("Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/tasks")));
        assert!(default_scan_roots(home, "kilocode")
            .iter()
            .any(|p| p.to_string_lossy().contains("kilo-code/tasks")));
        assert!(default_scan_roots(home, "kilocode")
            .iter()
            .any(|p| p.ends_with(".kilocode/cli/global/tasks")));
        assert!(default_scan_roots(home, "kilocode")
            .iter()
            .any(|p| p.ends_with(".kilocode/cli/workspaces")));
        assert!(default_scan_roots(home, "kilo")
            .iter()
            .any(|p| p.ends_with(".local/share/kilo/storage/session")));
        assert!(default_scan_roots(home, "kilo").iter().any(|p| p
            .to_string_lossy()
            .contains("Library/Application Support/kilo/storage/session")));
        assert!(default_scan_roots(home, "kimi")
            .iter()
            .any(|p| p.ends_with(".custom-kimi-code/sessions")));
        assert!(default_scan_roots(home, "kimi")
            .iter()
            .any(|p| p.ends_with(".custom-kimi-code/session_index.jsonl")));
        assert!(default_scan_roots(home, "kiro")
            .iter()
            .any(|p| p == &home.join(".custom-kiro")));
        assert!(default_scan_roots(home, "kiro")
            .iter()
            .any(|p| p.ends_with(".custom-kiro/sessions")));
        assert!(default_scan_roots(home, "zcode")
            .iter()
            .any(|p| p.ends_with(".zcode/cli/db")));

        std::env::remove_var("CODEX_HOME");
        std::env::remove_var("GEMINI_CLI_HOME");
        std::env::remove_var("CLINE_DATA_DIR");
        std::env::remove_var("KIMI_CODE_HOME");
        std::env::remove_var("KIRO_HOME");
        std::env::remove_var("COPILOT_OTEL_FILE_EXPORTER_PATH");
    }
}
