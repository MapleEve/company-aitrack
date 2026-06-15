use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Agent {
    pub name: &'static str,
    marker: &'static str,
    pub has_native_edit_adapter: bool,
    pub has_native_prompt_hook: bool,
}

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
        name: "kimi",
        marker: ".kimi",
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
        name: "warp",
        marker: ".warp",
        has_native_edit_adapter: false,
        has_native_prompt_hook: false,
    },
];

pub fn registered_agents() -> &'static [Agent] {
    REGISTERED_AGENTS
}

pub fn registered_agent_names() -> Vec<&'static str> {
    REGISTERED_AGENTS.iter().map(|agent| agent.name).collect()
}

pub fn agent_by_name(name: &str) -> Option<&'static Agent> {
    REGISTERED_AGENTS.iter().find(|agent| agent.name == name)
}

pub fn is_known_agent(name: &str) -> bool {
    agent_by_name(name).is_some()
}
