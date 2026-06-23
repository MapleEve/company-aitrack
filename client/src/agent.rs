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

const LOCAL_MONITORING_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
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
        has_native_prompt_hook: false,
    },
    Agent {
        name: "cursor",
        marker: ".cursor",
        has_native_edit_adapter: true,
        has_native_prompt_hook: false,
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
        name: "baidu-comate",
        marker: ".baidu-comate",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
    Agent {
        name: "wenxin",
        marker: ".wenxin",
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
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "codex",
        kind: LocalSourceKind::HookJsonl,
        label: "hook-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "cursor",
        kind: LocalSourceKind::HookJsonl,
        label: "hook-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "trae",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qwen",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "antigravity",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "opencode",
        kind: LocalSourceKind::HookJsonl,
        label: "hook-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "opencode",
        kind: LocalSourceKind::Sqlite,
        label: "sqlite",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder",
        kind: LocalSourceKind::HookJsonl,
        label: "hook-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder-cn",
        kind: LocalSourceKind::HookJsonl,
        label: "hook-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder-work",
        kind: LocalSourceKind::HookJsonl,
        label: "hook-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder-work-cn",
        kind: LocalSourceKind::HookJsonl,
        label: "hook-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder",
        kind: LocalSourceKind::Sqlite,
        label: "sqlite",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder-cn",
        kind: LocalSourceKind::Sqlite,
        label: "sqlite",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder-work",
        kind: LocalSourceKind::Sqlite,
        label: "sqlite",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder-work-cn",
        kind: LocalSourceKind::Sqlite,
        label: "sqlite",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder",
        kind: LocalSourceKind::IdeSnapshot,
        label: "ide-snapshot",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder-cn",
        kind: LocalSourceKind::IdeSnapshot,
        label: "ide-snapshot",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder-work",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder-cn",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder-work-cn",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "wukong",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "hermes",
        kind: LocalSourceKind::Sqlite,
        label: "sqlite",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "openclaw",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "gemini",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "copilot",
        kind: LocalSourceKind::IdeSnapshot,
        label: "otel-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "cline",
        kind: LocalSourceKind::SessionJsonl,
        label: "vscode-tasks",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "roo-code",
        kind: LocalSourceKind::SessionJsonl,
        label: "vscode-tasks",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "kiro",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "zed",
        kind: LocalSourceKind::Sqlite,
        label: "threads-db",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "goose",
        kind: LocalSourceKind::Sqlite,
        label: "sessions-db",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "amp",
        kind: LocalSourceKind::SessionJsonl,
        label: "threads-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "droid",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "pi",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "mux",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "crush",
        kind: LocalSourceKind::Sqlite,
        label: "sqlite",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "codebuff",
        kind: LocalSourceKind::SessionJsonl,
        label: "project-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "kilo",
        kind: LocalSourceKind::Sqlite,
        label: "sqlite",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "kilocode",
        kind: LocalSourceKind::SessionJsonl,
        label: "vscode-tasks",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "kimi",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "gjc",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "grok",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "synthetic",
        kind: LocalSourceKind::Sqlite,
        label: "sqlite",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "warp",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "zcode",
        kind: LocalSourceKind::Sqlite,
        label: "sqlite",
        capabilities: LOCAL_MONITORING_CAPABILITIES,
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
    REGISTERED_AGENTS
        .iter()
        .map(|agent| agent.name)
        .filter(|name| !is_default_scan_excluded(name))
        .collect()
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
    matches!(
        name,
        "roocode" | "kilo-code" | "gajae-code" | "baidu-comate" | "wenxin"
    )
}

pub fn default_scan_roots(home: &Path, tool: &str) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    match tool {
        "claude" => {
            roots.push(home.join(".claude").join("projects"));
            roots.push(home.join(".claude").join("transcripts"));
        }
        "codex" => {
            roots.push(env_or_home(home, "CODEX_HOME", ".codex").join("sessions"));
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
            roots.push(env_or_home(home, "GEMINI_CLI_HOME", ".gemini").join("tmp"));
        }
        "copilot" => {
            roots.push(home.join(".copilot").join("otel"));
            roots.push(home.join(".copilot").join("session-state"));
            if let Some(path) = env_path("COPILOT_OTEL_FILE_EXPORTER_PATH") {
                roots.push(path);
            }
        }
        "cline" => {
            roots.push(home.join(".config/Code/User/globalStorage/saoudrizwan.claude-dev/tasks"));
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
                    ".vscode-server/data/User/globalStorage/rooveterinaryinc.roo-cline/tasks",
                ),
            );
        }
        "kilocode" | "kilo-code" => {
            roots.push(home.join(".config/Code/User/globalStorage/kilocode.kilo-code/tasks"));
            roots
                .push(home.join(".vscode-server/data/User/globalStorage/kilocode.kilo-code/tasks"));
        }
        "kilo" => {
            roots.push(xdg_data_home(home).join("kilo"));
        }
        "kiro" => {
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
            roots.push(home.join(".kimi/sessions"));
            roots.push(env_or_home(home, "KIMI_CODE_HOME", ".kimi-code").join("sessions"));
        }
        "qwen" => {
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
        assert!(!default_scan_agent_names().contains(&"baidu-comate"));
        assert!(!default_scan_agent_names().contains(&"wenxin"));
        assert_eq!(default_scan_agent_names().len(), 35);
        assert_eq!(canonical_agent_name("roocode"), "roo-code");
        assert_eq!(canonical_agent_name("kilo-code"), "kilocode");
        assert_eq!(canonical_agent_name("gajae-code"), "gjc");
    }

    #[test]
    fn local_source_specs_cover_default_local_collection_matrix() {
        let specs = local_source_specs();

        for agent in default_scan_agent_names() {
            let spec = specs
                .iter()
                .find(|spec| spec.agent == agent)
                .unwrap_or_else(|| panic!("{agent} source spec missing"));
            assert!(
                spec.capabilities.prompt_input,
                "{agent} {:?} prompt missing",
                spec.kind
            );
            assert!(
                spec.capabilities.assistant_output,
                "{agent} {:?} output missing",
                spec.kind
            );
            assert!(
                spec.capabilities.tool_call,
                "{agent} {:?} tool call missing",
                spec.kind
            );
            assert!(
                spec.capabilities.tool_result,
                "{agent} {:?} tool result missing",
                spec.kind
            );
            assert!(
                spec.capabilities.token_usage,
                "{agent} {:?} token missing",
                spec.kind
            );
            assert!(
                spec.capabilities.session_context,
                "{agent} {:?} session context missing",
                spec.kind
            );
        }
        assert!(!specs.iter().any(|spec| spec.agent == "baidu-comate"));
        assert!(!specs.iter().any(|spec| spec.agent == "wenxin"));
        assert!(specs
            .iter()
            .any(|spec| spec.agent == "qoder" && spec.kind == LocalSourceKind::Sqlite));
        assert!(specs
            .iter()
            .any(|spec| spec.agent == "copilot" && spec.kind == LocalSourceKind::IdeSnapshot));
    }

    #[test]
    fn default_scan_roots_cover_local_agent_storage() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        std::env::set_var("CODEX_HOME", home.join("custom-codex"));
        std::env::set_var("GEMINI_CLI_HOME", home.join("custom-gemini"));
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
        assert!(default_scan_roots(home, "gemini")
            .iter()
            .any(|p| p.ends_with("custom-gemini/tmp")));
        assert!(default_scan_roots(home, "copilot")
            .iter()
            .any(|p| p.ends_with("copilot.jsonl")));
        assert!(default_scan_roots(home, "zed")
            .iter()
            .any(|p| p.to_string_lossy().contains("Zed/threads")));
        assert!(default_scan_roots(home, "qoder")
            .iter()
            .any(|p| p.to_string_lossy().contains("SharedClientCache/cache/db")));
        assert!(default_scan_roots(home, "roo-code")
            .iter()
            .any(|p| p.to_string_lossy().contains("roo-cline/tasks")));
        assert!(default_scan_roots(home, "kilocode")
            .iter()
            .any(|p| p.to_string_lossy().contains("kilo-code/tasks")));
        assert!(default_scan_roots(home, "zcode")
            .iter()
            .any(|p| p.ends_with(".zcode/cli/db")));
        assert!(default_scan_roots(home, "baidu-comate").is_empty());

        std::env::remove_var("CODEX_HOME");
        std::env::remove_var("GEMINI_CLI_HOME");
        std::env::remove_var("COPILOT_OTEL_FILE_EXPORTER_PATH");
    }
}
