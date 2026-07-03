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
    pub time_context: bool,
    pub model_provider_context: bool,
    pub account_context: bool,
    pub cost_usage: bool,
    pub reasoning_usage: bool,
    pub edit_diff: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalSourceSpec {
    pub agent: &'static str,
    pub kind: LocalSourceKind,
    pub label: &'static str,
    pub capabilities: LocalSourceCapabilities,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UsageBasis {
    None,
    Native,
    LocalDerived,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentFullDataCoverage {
    pub prompt_input: bool,
    pub assistant_output: bool,
    pub tool_call: bool,
    pub tool_result: bool,
    pub token_usage: bool,
    pub usage_basis: UsageBasis,
    pub session_context: bool,
    pub time_context: bool,
    pub model_provider_context: bool,
    pub account_context: bool,
    pub cost_usage: bool,
    pub reasoning_usage: bool,
    pub edit_diff: bool,
}

impl AgentFullDataCoverage {
    fn empty() -> Self {
        Self {
            prompt_input: false,
            assistant_output: false,
            tool_call: false,
            tool_result: false,
            token_usage: false,
            usage_basis: UsageBasis::None,
            session_context: false,
            time_context: false,
            model_provider_context: false,
            account_context: false,
            cost_usage: false,
            reasoning_usage: false,
            edit_diff: false,
        }
    }

    fn merge_source(&mut self, spec: &LocalSourceSpec) {
        let capabilities = spec.capabilities;
        let usage_basis = local_source_usage_basis(spec);
        self.prompt_input |= capabilities.prompt_input;
        self.assistant_output |= capabilities.assistant_output;
        self.tool_call |= capabilities.tool_call;
        self.tool_result |= capabilities.tool_result;
        self.token_usage |= capabilities.token_usage || usage_basis != UsageBasis::None;
        self.session_context |= capabilities.session_context;
        self.time_context |= capabilities.time_context;
        self.model_provider_context |= capabilities.model_provider_context;
        self.account_context |= capabilities.account_context || usage_basis != UsageBasis::None;
        self.cost_usage |= capabilities.cost_usage || usage_basis != UsageBasis::None;
        self.reasoning_usage |= capabilities.reasoning_usage || usage_basis != UsageBasis::None;
        self.edit_diff |= capabilities.edit_diff;
        self.usage_basis = merge_usage_basis(self.usage_basis, usage_basis);
    }

    fn has_required_fields(self) -> bool {
        self.prompt_input
            && self.assistant_output
            && self.tool_call
            && self.tool_result
            && self.token_usage
            && self.usage_basis != UsageBasis::None
            && self.session_context
            && self.time_context
            && self.model_provider_context
            && self.account_context
            && self.cost_usage
            && self.reasoning_usage
            && self.edit_diff
    }
}

const CURSOR_STATE_VSCDB_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: true,
    cost_usage: true,
    reasoning_usage: true,
    edit_diff: true,
};

const QODER_LOCAL_DB_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: false,
    cost_usage: false,
    reasoning_usage: false,
    edit_diff: true,
};

const ANTIGRAVITY_CONVERSATION_SQLITE_CAPABILITIES: LocalSourceCapabilities =
    LocalSourceCapabilities {
        prompt_input: true,
        assistant_output: true,
        tool_call: true,
        tool_result: true,
        token_usage: true,
        session_context: true,
        time_context: true,
        model_provider_context: true,
        account_context: false,
        cost_usage: false,
        reasoning_usage: true,
        edit_diff: true,
    };

fn merge_usage_basis(current: UsageBasis, next: UsageBasis) -> UsageBasis {
    match (current, next) {
        (UsageBasis::Native, _) | (_, UsageBasis::Native) => UsageBasis::Native,
        (UsageBasis::LocalDerived, _) | (_, UsageBasis::LocalDerived) => UsageBasis::LocalDerived,
        _ => UsageBasis::None,
    }
}

const NATIVE_EDIT_HOOK_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: true,
    cost_usage: true,
    reasoning_usage: true,
    edit_diff: true,
};

const CLAUDE_HOOK_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: false,
    session_context: true,
    time_context: true,
    model_provider_context: false,
    account_context: false,
    cost_usage: false,
    reasoning_usage: false,
    edit_diff: true,
};

const GEMINI_TELEMETRY_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: false,
    assistant_output: false,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: false,
    cost_usage: false,
    reasoning_usage: true,
    edit_diff: true,
};

const QWEN_TELEMETRY_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: false,
    assistant_output: false,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: false,
    cost_usage: false,
    reasoning_usage: false,
    edit_diff: true,
};

const OFFICIAL_TRANSCRIPT_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: false,
    cost_usage: false,
    reasoning_usage: false,
    edit_diff: true,
};

const OFFICIAL_SESSION_EXPORT_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: false,
    tool_result: false,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: false,
    cost_usage: true,
    reasoning_usage: true,
    edit_diff: false,
};

const OFFICIAL_PROMPT_TOOL_HOOK_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: false,
    tool_call: true,
    tool_result: true,
    token_usage: false,
    session_context: true,
    time_context: true,
    model_provider_context: false,
    account_context: false,
    cost_usage: false,
    reasoning_usage: false,
    edit_diff: true,
};

const KIRO_HOOK_JSONL_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: true,
    cost_usage: true,
    reasoning_usage: true,
    edit_diff: true,
};

const KIRO_CLI_SESSION_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: false,
    cost_usage: true,
    reasoning_usage: true,
    edit_diff: true,
};

const CODEBUFF_PROJECT_JSONL_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: false,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: false,
    cost_usage: true,
    reasoning_usage: false,
    edit_diff: true,
};

const WARP_SQLITE_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: false,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: false,
    cost_usage: true,
    reasoning_usage: false,
    edit_diff: true,
};

const OPENCODE_SQLITE_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: true,
    cost_usage: true,
    reasoning_usage: false,
    edit_diff: true,
};

const MUX_CHAT_JSONL_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: false,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: false,
    cost_usage: false,
    reasoning_usage: false,
    edit_diff: true,
};

const MUX_SESSION_USAGE_JSON_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: false,
    assistant_output: false,
    tool_call: false,
    tool_result: false,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: false,
    cost_usage: true,
    reasoning_usage: true,
    edit_diff: false,
};

const LOCAL_DERIVED_TRANSCRIPT_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: true,
    cost_usage: true,
    reasoning_usage: true,
    edit_diff: true,
};

const NATIVE_TRANSCRIPT_FULL_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: true,
    cost_usage: true,
    reasoning_usage: true,
    edit_diff: true,
};

const CLINE_FAMILY_TASK_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: false,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: false,
    cost_usage: false,
    reasoning_usage: false,
    edit_diff: true,
};

const CLINE_FAMILY_UI_USAGE_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: false,
    assistant_output: false,
    tool_call: false,
    tool_result: false,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: false,
    cost_usage: true,
    reasoning_usage: false,
    edit_diff: false,
};

const AMP_STREAM_JSONL_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: true,
    cost_usage: true,
    reasoning_usage: true,
    edit_diff: true,
};

const DROID_SESSION_JSONL_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: false,
    cost_usage: false,
    reasoning_usage: true,
    edit_diff: true,
};

const DROID_SETTINGS_JSON_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: false,
    assistant_output: false,
    tool_call: false,
    tool_result: false,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: false,
    cost_usage: false,
    reasoning_usage: true,
    edit_diff: false,
};

const CRUSH_SQLITE_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: false,
    cost_usage: true,
    reasoning_usage: true,
    edit_diff: true,
};

const KILO_FULL_LOCAL_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: true,
    cost_usage: true,
    reasoning_usage: true,
    edit_diff: true,
};

const KIMI_WIRE_JSONL_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: true,
    cost_usage: false,
    reasoning_usage: false,
    edit_diff: true,
};

const GJC_SESSION_JSONL_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: false,
    cost_usage: true,
    reasoning_usage: false,
    edit_diff: true,
};

const GROK_SESSION_JSONL_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: true,
    cost_usage: true,
    reasoning_usage: true,
    edit_diff: true,
};

const SYNTHETIC_SQLITE_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: true,
    cost_usage: true,
    reasoning_usage: false,
    edit_diff: true,
};

const ZCODE_PROJECT_JSONL_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: true,
    cost_usage: true,
    reasoning_usage: true,
    edit_diff: true,
};

const FIELD_LEVEL_EXTENDED_SESSION_CAPABILITIES: LocalSourceCapabilities =
    LocalSourceCapabilities {
        prompt_input: true,
        assistant_output: true,
        tool_call: true,
        tool_result: true,
        token_usage: true,
        session_context: true,
        time_context: true,
        model_provider_context: true,
        account_context: false,
        cost_usage: false,
        reasoning_usage: false,
        edit_diff: false,
    };

const FIELD_LEVEL_SESSION_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: false,
    cost_usage: false,
    reasoning_usage: false,
    edit_diff: false,
};

const SUPPLEMENTAL_USAGE_SUMMARY_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: false,
    assistant_output: false,
    tool_call: false,
    tool_result: false,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: false,
    cost_usage: true,
    reasoning_usage: true,
    edit_diff: false,
};

const CONDITIONAL_OTEL_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: false,
    assistant_output: false,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: false,
    cost_usage: false,
    reasoning_usage: true,
    edit_diff: false,
};

const SUPPLEMENTAL_STATE_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: false,
    assistant_output: false,
    tool_call: false,
    tool_result: false,
    token_usage: false,
    session_context: true,
    time_context: false,
    model_provider_context: false,
    account_context: false,
    cost_usage: false,
    reasoning_usage: false,
    edit_diff: false,
};

const QODER_WORK_TRACE_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: true,
    cost_usage: true,
    reasoning_usage: true,
    edit_diff: true,
};

const QODER_WORK_LOCAL_DB_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: true,
    cost_usage: true,
    reasoning_usage: true,
    edit_diff: true,
};

const LOCAL_TRANSCRIPT_EVENT_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: false,
    session_context: true,
    time_context: true,
    model_provider_context: false,
    account_context: false,
    cost_usage: false,
    reasoning_usage: false,
    edit_diff: false,
};

const COPILOT_OFFICIAL_RUNTIME_JSONL_CAPABILITIES: LocalSourceCapabilities =
    LocalSourceCapabilities {
        prompt_input: true,
        assistant_output: true,
        tool_call: true,
        tool_result: true,
        token_usage: true,
        session_context: true,
        time_context: true,
        model_provider_context: true,
        account_context: false,
        cost_usage: true,
        reasoning_usage: true,
        edit_diff: true,
    };

const COPILOT_SESSION_STORE_DB_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: false,
    cost_usage: false,
    reasoning_usage: true,
    edit_diff: true,
};

const WUKONG_SQLITE_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: false,
    cost_usage: true,
    reasoning_usage: true,
    edit_diff: true,
};

const HERMES_SQLITE_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: true,
    cost_usage: true,
    reasoning_usage: true,
    edit_diff: true,
};

const OPENCLAW_SESSION_CAPABILITIES: LocalSourceCapabilities = LocalSourceCapabilities {
    prompt_input: true,
    assistant_output: true,
    tool_call: true,
    tool_result: true,
    token_usage: true,
    session_context: true,
    time_context: true,
    model_provider_context: true,
    account_context: true,
    cost_usage: true,
    reasoning_usage: false,
    edit_diff: true,
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
        capabilities: OFFICIAL_TRANSCRIPT_CAPABILITIES,
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
        capabilities: LOCAL_DERIVED_TRANSCRIPT_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "cursor",
        kind: LocalSourceKind::SessionJsonl,
        label: "agent-transcripts-jsonl",
        capabilities: LOCAL_DERIVED_TRANSCRIPT_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "cursor",
        kind: LocalSourceKind::Sqlite,
        label: "state-vscdb",
        capabilities: CURSOR_STATE_VSCDB_CAPABILITIES,
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
        agent: "qwen",
        kind: LocalSourceKind::SessionJsonl,
        label: "project-chats-jsonl",
        capabilities: FIELD_LEVEL_SESSION_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qwen",
        kind: LocalSourceKind::SessionJsonl,
        label: "usage-record-jsonl",
        capabilities: SUPPLEMENTAL_USAGE_SUMMARY_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qwen",
        kind: LocalSourceKind::SessionJsonl,
        label: "token-usage-jsonl",
        capabilities: SUPPLEMENTAL_USAGE_SUMMARY_CAPABILITIES,
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
        capabilities: OPENCODE_SQLITE_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder",
        kind: LocalSourceKind::HookJsonl,
        label: "hook-jsonl",
        capabilities: OFFICIAL_PROMPT_TOOL_HOOK_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder",
        kind: LocalSourceKind::SessionJsonl,
        label: "transcript-jsonl",
        capabilities: NATIVE_TRANSCRIPT_FULL_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder",
        kind: LocalSourceKind::Sqlite,
        label: "local-db",
        capabilities: QODER_LOCAL_DB_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder-cn",
        kind: LocalSourceKind::HookJsonl,
        label: "hook-jsonl",
        capabilities: OFFICIAL_PROMPT_TOOL_HOOK_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder-cn",
        kind: LocalSourceKind::SessionJsonl,
        label: "transcript-jsonl",
        capabilities: NATIVE_TRANSCRIPT_FULL_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder-cn",
        kind: LocalSourceKind::Sqlite,
        label: "local-db",
        capabilities: QODER_LOCAL_DB_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder-work",
        kind: LocalSourceKind::HookJsonl,
        label: "hook-jsonl",
        capabilities: OFFICIAL_PROMPT_TOOL_HOOK_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder-work",
        kind: LocalSourceKind::SessionJsonl,
        label: "trace-jsonl",
        capabilities: QODER_WORK_TRACE_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder-work",
        kind: LocalSourceKind::Sqlite,
        label: "local-db",
        capabilities: QODER_WORK_LOCAL_DB_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder-work-cn",
        kind: LocalSourceKind::HookJsonl,
        label: "hook-jsonl",
        capabilities: OFFICIAL_PROMPT_TOOL_HOOK_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder-work-cn",
        kind: LocalSourceKind::SessionJsonl,
        label: "trace-jsonl",
        capabilities: QODER_WORK_TRACE_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "qoder-work-cn",
        kind: LocalSourceKind::Sqlite,
        label: "local-db",
        capabilities: QODER_WORK_LOCAL_DB_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "wukong",
        kind: LocalSourceKind::Sqlite,
        label: "sqlite",
        capabilities: WUKONG_SQLITE_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "hermes",
        kind: LocalSourceKind::Sqlite,
        label: "sqlite",
        capabilities: HERMES_SQLITE_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "openclaw",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: OPENCLAW_SESSION_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "gemini",
        kind: LocalSourceKind::TelemetryLog,
        label: "telemetry-log",
        capabilities: GEMINI_TELEMETRY_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "gemini",
        kind: LocalSourceKind::SessionJsonl,
        label: "tmp-chats-jsonl",
        capabilities: FIELD_LEVEL_EXTENDED_SESSION_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "copilot",
        kind: LocalSourceKind::IdeSnapshot,
        label: "otel-jsonl",
        capabilities: CONDITIONAL_OTEL_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "copilot",
        kind: LocalSourceKind::SessionJsonl,
        label: "official-copilot-runtime-jsonl",
        capabilities: COPILOT_OFFICIAL_RUNTIME_JSONL_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "copilot",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-state-jsonl",
        capabilities: LOCAL_TRANSCRIPT_EVENT_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "copilot",
        kind: LocalSourceKind::Sqlite,
        label: "session-store-db",
        capabilities: COPILOT_SESSION_STORE_DB_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "copilot",
        kind: LocalSourceKind::Sqlite,
        label: "vscode-chat-state",
        capabilities: LocalSourceCapabilities {
            prompt_input: true,
            assistant_output: true,
            tool_call: false,
            tool_result: false,
            token_usage: false,
            session_context: true,
            time_context: true,
            model_provider_context: false,
            account_context: false,
            cost_usage: false,
            reasoning_usage: false,
            edit_diff: false,
        },
    },
    LocalSourceSpec {
        agent: "cline",
        kind: LocalSourceKind::SessionJsonl,
        label: "vscode-tasks",
        capabilities: CLINE_FAMILY_TASK_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "cline",
        kind: LocalSourceKind::SessionJsonl,
        label: "vscode-ui-messages",
        capabilities: CLINE_FAMILY_UI_USAGE_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "cline",
        kind: LocalSourceKind::Sqlite,
        label: "sessions-db",
        capabilities: SUPPLEMENTAL_STATE_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "roo-code",
        kind: LocalSourceKind::SessionJsonl,
        label: "vscode-tasks",
        capabilities: CLINE_FAMILY_TASK_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "roo-code",
        kind: LocalSourceKind::SessionJsonl,
        label: "vscode-ui-messages",
        capabilities: CLINE_FAMILY_UI_USAGE_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "kiro",
        kind: LocalSourceKind::HookJsonl,
        label: "hook-jsonl",
        capabilities: KIRO_HOOK_JSONL_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "kiro",
        kind: LocalSourceKind::Sqlite,
        label: "data-sqlite",
        capabilities: FIELD_LEVEL_EXTENDED_SESSION_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "kiro",
        kind: LocalSourceKind::SessionJsonl,
        label: "cli-session-json",
        capabilities: KIRO_CLI_SESSION_CAPABILITIES,
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
        capabilities: AMP_STREAM_JSONL_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "droid",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: DROID_SESSION_JSONL_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "droid",
        kind: LocalSourceKind::SessionJsonl,
        label: "settings-json",
        capabilities: DROID_SETTINGS_JSON_CAPABILITIES,
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
        label: "chat-jsonl",
        capabilities: MUX_CHAT_JSONL_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "mux",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-usage-json",
        capabilities: MUX_SESSION_USAGE_JSON_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "crush",
        kind: LocalSourceKind::Sqlite,
        label: "sqlite",
        capabilities: CRUSH_SQLITE_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "codebuff",
        kind: LocalSourceKind::SessionJsonl,
        label: "project-jsonl",
        capabilities: CODEBUFF_PROJECT_JSONL_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "kilo",
        kind: LocalSourceKind::Sqlite,
        label: "sqlite",
        capabilities: KILO_FULL_LOCAL_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "kilo",
        kind: LocalSourceKind::SessionJsonl,
        label: "storage-json",
        capabilities: KILO_FULL_LOCAL_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "kilocode",
        kind: LocalSourceKind::Sqlite,
        label: "sqlite",
        capabilities: KILO_FULL_LOCAL_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "kilocode",
        kind: LocalSourceKind::SessionJsonl,
        label: "storage-json",
        capabilities: KILO_FULL_LOCAL_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "kilocode",
        kind: LocalSourceKind::SessionJsonl,
        label: "vscode-tasks",
        capabilities: LOCAL_TRANSCRIPT_EVENT_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "kilocode",
        kind: LocalSourceKind::SessionJsonl,
        label: "vscode-ui-messages",
        capabilities: CLINE_FAMILY_UI_USAGE_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "kimi",
        kind: LocalSourceKind::SessionJsonl,
        label: "wire-jsonl",
        capabilities: KIMI_WIRE_JSONL_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "gjc",
        kind: LocalSourceKind::SessionJsonl,
        label: "session-jsonl",
        capabilities: GJC_SESSION_JSONL_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "grok",
        kind: LocalSourceKind::SessionJsonl,
        label: "sessions-jsonl",
        capabilities: GROK_SESSION_JSONL_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "synthetic",
        kind: LocalSourceKind::Sqlite,
        label: "sqlite",
        capabilities: SYNTHETIC_SQLITE_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "warp",
        kind: LocalSourceKind::Sqlite,
        label: "warp-sqlite",
        capabilities: WARP_SQLITE_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "antigravity",
        kind: LocalSourceKind::Sqlite,
        label: "conversation-sqlite",
        capabilities: ANTIGRAVITY_CONVERSATION_SQLITE_CAPABILITIES,
    },
    LocalSourceSpec {
        agent: "zcode",
        kind: LocalSourceKind::SessionJsonl,
        label: "projects-jsonl",
        capabilities: ZCODE_PROJECT_JSONL_CAPABILITIES,
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

pub fn local_source_usage_basis(spec: &LocalSourceSpec) -> UsageBasis {
    if verified_full_capability_source(spec) {
        UsageBasis::Native
    } else if verified_local_derived_full_coverage_source(spec) || local_derived_usage_source(spec)
    {
        UsageBasis::LocalDerived
    } else if spec.capabilities.token_usage || spec.capabilities.cost_usage {
        UsageBasis::Native
    } else {
        UsageBasis::None
    }
}

/// Agent-level rollup used by local full-data upload checks.
///
/// The check is intentionally built from all declared local sources for an
/// agent because real tools often split transcript, usage, state, and edit
/// events across multiple local files or stores.
pub fn agent_full_data_coverage(agent: &str) -> Option<AgentFullDataCoverage> {
    let canonical = canonical_agent_name(agent);
    let mut coverage = AgentFullDataCoverage::empty();
    let mut found = false;
    for spec in LOCAL_SOURCE_SPECS
        .iter()
        .filter(|spec| spec.agent == canonical)
    {
        found = true;
        coverage.merge_source(spec);
    }
    if found {
        Some(coverage)
    } else {
        None
    }
}

/// Returns whether an agent has the required combined local sources for
/// full-data upload support.
pub fn agent_has_required_full_data_coverage(agent: &str) -> bool {
    match agent_full_data_coverage(agent) {
        Some(coverage) => coverage.has_required_fields(),
        None => false,
    }
}

pub fn agent_has_strict_full_data_coverage(agent: &str) -> bool {
    let canonical = canonical_agent_name(agent);
    LOCAL_SOURCE_SPECS
        .iter()
        .filter(|spec| spec.agent == canonical)
        .any(verified_full_capability_source)
}

pub fn source_has_full_upload_coverage(spec: &LocalSourceSpec) -> bool {
    verified_full_capability_source(spec)
}

fn verified_full_capability_source(spec: &LocalSourceSpec) -> bool {
    audited_native_source_label(spec)
        && source_has_strict_full_upload_capabilities(spec.capabilities)
}

fn verified_local_derived_full_coverage_source(spec: &LocalSourceSpec) -> bool {
    local_derived_usage_source(spec)
        && source_has_strict_full_upload_capabilities(spec.capabilities)
}

fn source_has_strict_full_upload_capabilities(capabilities: LocalSourceCapabilities) -> bool {
    capabilities.prompt_input
        && capabilities.assistant_output
        && capabilities.tool_call
        && capabilities.tool_result
        && capabilities.token_usage
        && capabilities.session_context
        && capabilities.time_context
        && capabilities.model_provider_context
        && capabilities.account_context
        && capabilities.cost_usage
        && capabilities.reasoning_usage
        && capabilities.edit_diff
}

fn audited_native_source_label(spec: &LocalSourceSpec) -> bool {
    matches!(
        (spec.agent, spec.label),
        ("cursor", "hook-jsonl")
            | ("cursor", "agent-transcripts-jsonl")
            | ("cursor", "state-vscdb")
            | ("codex", "hook-jsonl")
            | ("trae", "trajectory-json")
            | ("claude", "projects-jsonl")
            | ("codex", "rollout-jsonl")
            | ("qwen", "project-chats-jsonl")
            | ("qwen", "usage-record-jsonl")
            | ("qwen", "token-usage-jsonl")
            | ("qoder", "transcript-jsonl")
            | ("qoder-cn", "transcript-jsonl")
            | ("qoder-work", "trace-jsonl")
            | ("qoder-work", "local-db")
            | ("qoder-work-cn", "trace-jsonl")
            | ("qoder-work-cn", "local-db")
            | ("opencode", "export-json")
            | ("opencode", "sqlite")
            | ("openclaw", "session-jsonl")
            | ("gjc", "session-jsonl")
            | ("zed", "threads-db")
            | ("goose", "sessions-db")
            | ("hermes", "sqlite")
            | ("kiro", "data-sqlite")
            | ("kiro", "hook-jsonl")
            | ("pi", "session-jsonl")
            | ("mux", "chat-jsonl")
            | ("mux", "session-usage-json")
            | ("droid", "session-jsonl")
            | ("amp", "threads-jsonl")
            | ("kimi", "wire-jsonl")
            | ("gemini", "telemetry-log")
            | ("gemini", "tmp-chats-jsonl")
            | ("copilot", "otel-jsonl")
            | ("copilot", "official-copilot-runtime-jsonl")
            | ("crush", "sqlite")
            | ("kilo", "sqlite")
            | ("kilo", "storage-json")
            | ("kilocode", "sqlite")
            | ("kilocode", "storage-json")
            | ("codebuff", "project-jsonl")
            | ("synthetic", "sqlite")
            | ("warp", "warp-sqlite")
            | ("antigravity", "conversation-sqlite")
            | ("grok", "sessions-jsonl")
            | ("zcode", "projects-jsonl")
            | ("wukong", "sqlite")
    )
}

fn local_derived_usage_source(_spec: &LocalSourceSpec) -> bool {
    false
}

#[cfg(test)]
fn field_level_native_coverage_source(spec: &LocalSourceSpec) -> bool {
    matches!(
        (spec.agent, spec.label),
        ("trae", "trajectory-json")
            | ("claude", "projects-jsonl")
            | ("codex", "rollout-jsonl")
            | ("qwen", "project-chats-jsonl")
            | ("opencode", "sqlite")
            | ("openclaw", "session-jsonl")
            | ("gjc", "session-jsonl")
            | ("zed", "threads-db")
            | ("goose", "sessions-db")
            | ("hermes", "sqlite")
            | ("kiro", "data-sqlite")
            | ("pi", "session-jsonl")
            | ("mux", "chat-jsonl")
            | ("mux", "session-usage-json")
            | ("droid", "session-jsonl")
            | ("kimi", "wire-jsonl")
            | ("gemini", "tmp-chats-jsonl")
            | ("copilot", "official-copilot-runtime-jsonl")
            | ("cline", "vscode-tasks")
            | ("roo-code", "vscode-tasks")
            | ("crush", "sqlite")
            | ("kilo", "sqlite")
            | ("kilo", "storage-json")
            | ("kilocode", "sqlite")
            | ("kilocode", "storage-json")
            | ("codebuff", "project-jsonl")
            | ("synthetic", "sqlite")
            | ("warp", "warp-sqlite")
            | ("antigravity", "conversation-sqlite")
            | ("wukong", "sqlite")
    )
}

pub fn agent_by_name(name: &str) -> Option<&'static Agent> {
    REGISTERED_AGENTS.iter().find(|agent| agent.name == name)
}

pub fn is_known_agent(name: &str) -> bool {
    agent_by_name(canonical_agent_name(name)).is_some()
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
            let claude_config_dir = env_or_home(home, "CLAUDE_CONFIG_DIR", ".claude");
            roots.push(claude_config_dir.join("projects"));
            roots.push(claude_config_dir.join("transcripts"));
        }
        "codex" => {
            let codex_home = env_or_home(home, "CODEX_HOME", ".codex");
            roots.push(codex_home.join("sessions"));
        }
        "cursor" => {
            roots.push(home.join(".cursor/hooks"));
            roots.push(home.join(".cursor/projects"));
            roots.push(home.join("Library/Application Support/Cursor/User/globalStorage"));
            roots.push(home.join("Library/Application Support/Cursor/User/workspaceStorage"));
            roots.push(home.join(".config/Cursor/User/globalStorage"));
            roots.push(home.join(".config/Cursor/User/workspaceStorage"));
        }
        "trae" => {
            if let Ok(cwd) = std::env::current_dir() {
                roots.push(cwd.join("trajectories"));
            }
            roots.push(home.join("trajectories"));
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
            roots.push(home.join(".qoder/projects"));
            roots.push(home.join(".qoder/transcripts"));
            roots.push(home.join(".qoder/hooks"));
            roots.push(home.join("Library/Application Support/Qoder/SharedClientCache/cache/db"));
            roots.push(home.join(".config/Qoder/SharedClientCache/cache/db"));
        }
        "qoder-cn" => {
            roots.push(home.join(".lingma/hooks"));
            roots.push(home.join(".lingma/projects"));
            roots.push(home.join(".lingma/transcripts"));
            roots.push(home.join(".qoder-cn/projects"));
            roots.push(home.join(".qoder-cn/transcripts"));
            roots.push(home.join(".qoder-cn/hooks"));
            roots.push(home.join("Library/Application Support/QoderCN/SharedClientCache/cache/db"));
            roots.push(home.join(".config/QoderCN/SharedClientCache/cache/db"));
        }
        "qoder-work" => {
            roots.push(home.join(".qoderwork/hooks"));
            roots.push(home.join(".qoderwork/data"));
            roots.push(home.join(".qoderwork/logs/sessions"));
            roots.push(home.join(".qoderwork/tool-results"));
            roots.push(home.join(".qoderwork/agents.db"));
            roots.push(home.join(".qoderwork/messages.db"));
        }
        "qoder-work-cn" => {
            roots.push(home.join(".qoderwork/hooks"));
            roots.push(home.join(".qoderwork/data"));
            roots.push(home.join(".qoderwork/logs/sessions"));
            roots.push(home.join(".qoderwork/tool-results"));
            roots.push(home.join(".qoderwork/agents.db"));
            roots.push(home.join(".qoderwork/messages.db"));
            roots.push(home.join(".qoderworkcn/hooks"));
            roots.push(home.join(".qoderworkcn/data"));
            roots.push(home.join(".qoderworkcn/logs/sessions"));
            roots.push(home.join(".qoderworkcn/tool-results"));
            roots.push(home.join(".qoderworkcn/agents.db"));
            roots.push(home.join(".qoderworkcn/messages.db"));
        }
        "wukong" => {
            if let Some(path) = env_path("WUKONG_DB_PATH") {
                roots.push(path);
            }
            roots.push(home.join(".wukong/data/wukong.db"));
            roots.push(home.join(".wukong"));
        }
        "gemini" => {
            let gemini_home_root =
                env_path("GEMINI_CLI_HOME").unwrap_or_else(|| home.to_path_buf());
            let gemini_home = gemini_home_root.join(".gemini");
            roots.push(gemini_home.clone());
            roots.push(gemini_home.join("tmp"));
            if env_path("GEMINI_CLI_HOME").is_some() {
                roots.push(gemini_home_root.clone());
                roots.push(gemini_home_root.join("tmp"));
            }
        }
        "copilot" => {
            roots.push(home.join(".copilot").join("otel"));
            roots.push(home.join(".copilot").join("session-state"));
            roots.push(home.join(".copilot").join("session-store.db"));
            let github_copilot_config = xdg_config_home(home).join("github-copilot");
            roots.push(github_copilot_config.clone());
            for session_root in ["chat-sessions", "chat-agent-sessions", "chat-edit-sessions"] {
                roots.push(github_copilot_config.join(session_root));
                roots.push(github_copilot_config.join("ws").join(session_root));
            }
            roots.push(home.join("Library/Application Support/Code/User/workspaceStorage"));
            roots
                .push(home.join("Library/Application Support/Code/User/globalStorage/state.vscdb"));
            roots.push(home.join("Library/Application Support/Cursor/User/workspaceStorage"));
            roots.push(
                home.join("Library/Application Support/Cursor/User/globalStorage/state.vscdb"),
            );
            roots.push(home.join(".config/Code/User/workspaceStorage"));
            roots.push(home.join(".config/Code/User/globalStorage/state.vscdb"));
            roots.push(home.join(".config/Cursor/User/workspaceStorage"));
            roots.push(home.join(".config/Cursor/User/globalStorage/state.vscdb"));
            if let Some(path) = env_path("COPILOT_OTEL_FILE_EXPORTER_PATH") {
                roots.push(path);
            }
        }
        "cline" => {
            let cline_data = env_or_home(home, "CLINE_DATA_DIR", ".cline/data");
            if let Some(path) = env_path("CLINE_DIR") {
                roots.push(path);
            }
            if let Some(path) = env_path("CLINE_SESSION_DATA_DIR") {
                roots.push(path);
            }
            if let Some(path) = env_path("CLINE_DB_DATA_DIR") {
                roots.push(path);
            }
            roots.push(cline_data.join("db"));
            roots.push(cline_data.join("db/sessions.db"));
            roots.push(cline_data.join("sessions"));
            roots.push(home.join(".config/Code/User/globalStorage/saoudrizwan.claude-dev/tasks"));
            roots.push(home.join(".config/Cursor/User/globalStorage/saoudrizwan.claude-dev/tasks"));
            roots.push(
                home.join(
                    ".config/Code - Insiders/User/globalStorage/saoudrizwan.claude-dev/tasks",
                ),
            );
            roots.push(
                home.join(".config/VSCodium/User/globalStorage/saoudrizwan.claude-dev/tasks"),
            );
            roots.push(home.join(
                "Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/tasks",
            ));
            roots.push(home.join(
                "Library/Application Support/Code - Insiders/User/globalStorage/saoudrizwan.claude-dev/tasks",
            ));
            roots.push(home.join(
                "Library/Application Support/VSCodium/User/globalStorage/saoudrizwan.claude-dev/tasks",
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
                home.join(".config/Cursor/User/globalStorage/rooveterinaryinc.roo-cline/tasks"),
            );
            roots.push(home.join(
                ".config/Code - Insiders/User/globalStorage/rooveterinaryinc.roo-cline/tasks",
            ));
            roots.push(
                home.join(".config/VSCodium/User/globalStorage/rooveterinaryinc.roo-cline/tasks"),
            );
            roots.push(
                home.join(
                    "Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/tasks",
                ),
            );
            roots.push(home.join(
                "Library/Application Support/Code - Insiders/User/globalStorage/rooveterinaryinc.roo-cline/tasks",
            ));
            roots.push(home.join(
                "Library/Application Support/VSCodium/User/globalStorage/rooveterinaryinc.roo-cline/tasks",
            ));
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
            roots.push(xdg_data_home(home).join("kilo/kilo.db"));
            roots.push(xdg_data_home(home).join("kilo/storage/session"));
            roots.push(home.join("Library/Application Support/kilo/kilo.db"));
            roots.push(home.join("Library/Application Support/kilo/storage/session"));
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
            roots.push(xdg_data_home(home).join("kilo/kilo.db"));
            roots.push(xdg_data_home(home).join("kilo"));
            roots.push(xdg_data_home(home).join("kilo/storage/session"));
            roots.push(home.join("Library/Application Support/kilo/kilo.db"));
            roots.push(home.join("Library/Application Support/kilo/storage/session"));
        }
        "kiro" => {
            let kiro_home = env_or_home(home, "KIRO_HOME", ".kiro");
            roots.push(kiro_home.clone());
            roots.push(kiro_home.join("sessions"));
            roots.push(home.join(".kiro/sessions/cli"));
            roots.push(xdg_data_home(home).join("kiro-cli"));
            roots.push(xdg_data_home(home).join("kiro-cli/data.sqlite3"));
            roots.push(home.join("Library/Application Support/kiro-cli"));
            roots.push(home.join("Library/Application Support/kiro-cli/data.sqlite3"));
            roots.push(
                home.join("Library/Application Support/Kiro/User/globalStorage/kiro.kiroagent"),
            );
            roots.push(home.join(".config/Kiro/User/globalStorage/kiro.kiroagent"));
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
            roots.push(home.join(".amp"));
            roots.push(xdg_data_home(home).join("amp/threads"));
            roots.push(xdg_data_home(home).join("amp/sessions"));
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
            let qwen_home = env_or_home(home, "QWEN_HOME", ".qwen");
            let qwen_runtime = env_path("QWEN_RUNTIME_DIR").unwrap_or_else(|| qwen_home.clone());
            roots.push(qwen_home.clone());
            roots.push(qwen_home.join("projects"));
            roots.push(qwen_runtime.clone());
            roots.push(qwen_runtime.join("projects"));
            roots.push(qwen_runtime.join("usage"));
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
            if let Ok(cwd) = std::env::current_dir() {
                roots.push(cwd.join(".crush"));
                roots.push(cwd.join(".crush/crush.db"));
            }
            roots.push(home.join(".crush"));
            roots.push(xdg_data_home(home).join("crush"));
        }
        "hermes" => {
            roots.push(env_or_home(home, "HERMES_HOME", ".hermes"));
        }
        "antigravity" => {
            roots.push(home.join(".gemini/antigravity-cli/conversations"));
            roots.push(home.join(".gemini/antigravity-cli"));
        }
        "gjc" | "gajae-code" => {
            if let Ok(cwd) = std::env::current_dir() {
                roots.push(cwd.join(".gjc"));
            }
            let gjc_config_dir = env_or_home(home, "GJC_CONFIG_DIR", ".gjc");
            roots.push(gjc_config_dir.clone());
            let gjc_agent_dir = env_or_home(home, "GJC_CODING_AGENT_DIR", ".gjc/agent");
            roots.push(gjc_agent_dir.join("sessions"));
            if gjc_agent_dir.ends_with("sessions") {
                roots.push(gjc_agent_dir);
            }
            roots.push(gjc_config_dir.join("agent/sessions"));
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
            roots.push(home.join(".warp"));
            roots.push(home.join(".warp-preview"));
            roots.push(home.join(".warp-dev"));
            roots.push(home.join(".warp-oss"));
            roots.push(home.join("Library/Group Containers/2BBY89MBSN.dev.warp"));
            roots.push(home.join("Library/Application Support/dev.warp.Warp-Stable"));
            roots.push(home.join("Library/Application Support/dev.warp.Warp-Preview"));
            roots.push(home.join("Library/Application Support/dev.warp.Warp"));
            roots.push(xdg_data_home(home).join("warp-terminal"));
            roots.push(xdg_data_home(home).join("Warp-Terminal"));
        }
        "zcode" => {
            roots.push(home.join(".zcode/projects"));
            roots.push(home.join(".zcode"));
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
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn registered_agents_include_reference_frameworks_without_aliases() {
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
            "kilocode",
            "kilo",
            "mux",
            "crush",
            "goose",
            "antigravity",
            "zed",
            "kiro",
            "gjc",
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
        for name in ["roocode", "kilo-code", "gajae-code"] {
            assert!(
                is_known_agent(name),
                "{name} should be accepted as an alias"
            );
            assert!(
                !registered_agent_names().contains(&name),
                "{name} should not be a registered coverage agent"
            );
        }
        assert!(!default_scan_agent_names().contains(&"roocode"));
        assert!(!default_scan_agent_names().contains(&"kilo-code"));
        assert!(!default_scan_agent_names().contains(&"gajae-code"));
        for name in [
            "antigravity",
            "qoder-work",
            "qoder-work-cn",
            "roo-code",
            "synthetic",
        ] {
            assert!(
                default_scan_agent_names().contains(&name),
                "{name} should be a default canonical scan agent"
            );
        }
        assert_eq!(default_scan_agent_names().len(), 35);
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
                        || source_has_full_upload_coverage(spec)
                }),
                "{agent} source spec overclaims local fields that the source itself does not expose"
            );
        }

        assert!(!specs.iter().any(|spec| spec.agent == "baidu-comate"));
        assert!(!specs.iter().any(|spec| spec.agent == "wenxin"));
        assert!(specs
            .iter()
            .any(|spec| spec.agent == "qoder" && spec.kind == LocalSourceKind::Sqlite));
        assert!(specs
            .iter()
            .any(|spec| spec.agent == "qoder-cn" && spec.kind == LocalSourceKind::Sqlite));
        assert!(specs
            .iter()
            .any(|spec| spec.agent == "copilot" && spec.kind == LocalSourceKind::IdeSnapshot));
        let copilot_official = specs
            .iter()
            .find(|spec| spec.agent == "copilot" && spec.label == "official-copilot-runtime-jsonl")
            .expect("copilot official runtime source spec missing");
        assert!(
            field_level_native_coverage_source(copilot_official),
            "Copilot runtime event reader must stay field-level until local schema evidence is closed"
        );
        assert!(
            !verified_full_capability_source(copilot_official),
            "Copilot runtime event reader must not claim fields absent from runtime events"
        );
        assert!(specs
            .iter()
            .any(|spec| spec.agent == "cursor" && spec.label == "state-vscdb"));

        for (agent, label) in [
            ("copilot", "session-state-jsonl"),
            ("copilot", "session-store-db"),
            ("qoder", "local-db"),
            ("qoder-cn", "local-db"),
            ("gemini", "telemetry-log"),
            ("qwen", "telemetry-log"),
            ("cline", "vscode-tasks"),
            ("roo-code", "vscode-tasks"),
            ("kilocode", "vscode-tasks"),
        ] {
            let spec = specs
                .iter()
                .find(|spec| spec.agent == agent && spec.label == label)
                .unwrap_or_else(|| panic!("{agent}/{label} source spec missing"));
            assert!(
                !all_capabilities_enabled(spec.capabilities),
                "{agent}/{label} must not claim every local field without verified evidence"
            );
            assert!(
                !verified_full_capability_source(spec),
                "{agent}/{label} must not claim fields absent from the source"
            );
        }

        let gemini_telemetry = specs
            .iter()
            .find(|spec| spec.agent == "gemini" && spec.label == "telemetry-log")
            .expect("gemini telemetry source spec missing");
        assert_eq!(
            gemini_telemetry.capabilities,
            LocalSourceCapabilities {
                prompt_input: false,
                assistant_output: false,
                tool_call: true,
                tool_result: true,
                token_usage: true,
                session_context: true,
                time_context: true,
                model_provider_context: true,
                account_context: false,
                cost_usage: false,
                reasoning_usage: true,
                edit_diff: true,
            }
        );
        let gemini_chats = specs
            .iter()
            .find(|spec| spec.agent == "gemini" && spec.label == "tmp-chats-jsonl")
            .expect("gemini tmp chats source spec missing");
        assert_eq!(
            gemini_chats.capabilities,
            FIELD_LEVEL_EXTENDED_SESSION_CAPABILITIES
        );
        assert!(
            field_level_native_coverage_source(gemini_chats),
            "gemini tmp-chats-jsonl must stay field-level until local ChatRecording schema evidence is closed"
        );
        assert!(
            !verified_full_capability_source(gemini_chats),
            "gemini tmp-chats-jsonl must not claim fields absent from ChatRecording events"
        );

        let qwen_telemetry = specs
            .iter()
            .find(|spec| spec.agent == "qwen" && spec.label == "telemetry-log")
            .expect("qwen telemetry source spec missing");
        assert_eq!(qwen_telemetry.capabilities, QWEN_TELEMETRY_CAPABILITIES);

        let qwen_chats = specs
            .iter()
            .find(|spec| spec.agent == "qwen" && spec.label == "project-chats-jsonl")
            .expect("qwen project chats source spec missing");
        assert_eq!(qwen_chats.capabilities, FIELD_LEVEL_SESSION_CAPABILITIES);
        assert!(
            field_level_native_coverage_source(qwen_chats),
            "qwen project chats must stay field-level until provider/account and complete usage evidence is closed"
        );
        assert!(
            !verified_full_capability_source(qwen_chats),
            "qwen project chats must not claim fields absent from ChatRecording events"
        );
        let qwen_usage = specs
            .iter()
            .find(|spec| spec.agent == "qwen" && spec.label == "usage-record-jsonl")
            .expect("qwen usage summary source spec missing");
        assert_eq!(
            qwen_usage.capabilities,
            SUPPLEMENTAL_USAGE_SUMMARY_CAPABILITIES
        );
        let qwen_token_usage = specs
            .iter()
            .find(|spec| spec.agent == "qwen" && spec.label == "token-usage-jsonl")
            .expect("qwen token usage source spec missing");
        assert_eq!(
            qwen_token_usage.capabilities,
            SUPPLEMENTAL_USAGE_SUMMARY_CAPABILITIES
        );

        let codex_rollout = specs
            .iter()
            .find(|spec| spec.agent == "codex" && spec.label == "rollout-jsonl")
            .expect("codex rollout source spec missing");
        assert_eq!(codex_rollout.capabilities, OFFICIAL_TRANSCRIPT_CAPABILITIES);
        assert!(field_level_native_coverage_source(codex_rollout));
        assert!(!verified_full_capability_source(codex_rollout));

        let claude_projects = specs
            .iter()
            .find(|spec| spec.agent == "claude" && spec.label == "projects-jsonl")
            .expect("claude projects source spec missing");
        assert_eq!(
            claude_projects.capabilities,
            OFFICIAL_TRANSCRIPT_CAPABILITIES
        );
        assert!(field_level_native_coverage_source(claude_projects));
        assert!(!verified_full_capability_source(claude_projects));

        let opencode_sqlite = specs
            .iter()
            .find(|spec| spec.agent == "opencode" && spec.kind == LocalSourceKind::Sqlite)
            .expect("opencode sqlite source spec missing");
        assert_eq!(opencode_sqlite.capabilities, OPENCODE_SQLITE_CAPABILITIES);
        assert!(
            field_level_native_coverage_source(opencode_sqlite),
            "opencode sqlite keeps a field-level source contract while mapping official sessions/messages/files"
        );
        assert!(
            !verified_full_capability_source(opencode_sqlite),
            "opencode sqlite must not claim reasoning fields absent from the local DB"
        );

        let opencode_export = specs
            .iter()
            .find(|spec| spec.agent == "opencode" && spec.label == "export-json")
            .expect("opencode export source spec missing");
        assert_eq!(
            opencode_export.capabilities,
            OFFICIAL_SESSION_EXPORT_CAPABILITIES
        );

        let droid = specs
            .iter()
            .find(|spec| spec.agent == "droid" && spec.label == "session-jsonl")
            .expect("droid source spec missing");
        assert_eq!(droid.capabilities, DROID_SESSION_JSONL_CAPABILITIES);
        assert!(
            field_level_native_coverage_source(droid),
            "droid session-jsonl must keep the Factory/Droid field-level source contract"
        );
        assert!(
            !verified_full_capability_source(droid),
            "droid session-jsonl must not claim a single-source complete field set"
        );

        let kilo = specs
            .iter()
            .find(|spec| spec.agent == "kilo" && spec.label == "sqlite")
            .expect("kilo source spec missing");
        assert_eq!(kilo.capabilities, KILO_FULL_LOCAL_CAPABILITIES);
        assert!(
            field_level_native_coverage_source(kilo),
            "kilo sqlite must keep the Kilo field-level source contract"
        );
        assert!(
            verified_full_capability_source(kilo),
            "kilo sqlite must claim the full local field set proved by the Kilo DB reader"
        );
        let kilo_storage = specs
            .iter()
            .find(|spec| spec.agent == "kilo" && spec.label == "storage-json")
            .expect("kilo storage-json source spec missing");
        assert_eq!(kilo_storage.capabilities, KILO_FULL_LOCAL_CAPABILITIES);
        assert!(
            field_level_native_coverage_source(kilo_storage),
            "kilo storage-json must keep the Kilo storage field-level source contract"
        );
        assert!(
            verified_full_capability_source(kilo_storage),
            "kilo storage-json must claim the full local field set proved by the Kilo storage reader"
        );

        let kiro = specs
            .iter()
            .find(|spec| spec.agent == "kiro" && spec.label == "data-sqlite")
            .expect("kiro source spec missing");
        assert_eq!(kiro.capabilities, FIELD_LEVEL_EXTENDED_SESSION_CAPABILITIES);
        assert!(
            field_level_native_coverage_source(kiro),
            "kiro data-sqlite must keep the Kiro local DB field-level source contract"
        );
        assert!(
            !verified_full_capability_source(kiro),
            "kiro data-sqlite must not claim a single-source complete field set"
        );
        let kiro_hooks = specs
            .iter()
            .find(|spec| spec.agent == "kiro" && spec.label == "hook-jsonl")
            .expect("kiro hook-jsonl source spec missing");
        assert_eq!(kiro_hooks.capabilities, KIRO_HOOK_JSONL_CAPABILITIES);
        assert_eq!(local_source_usage_basis(kiro_hooks), UsageBasis::Native);
        assert!(
            verified_full_capability_source(kiro_hooks),
            "kiro hook-jsonl must carry native full-data coverage"
        );
        let kiro_cli = specs
            .iter()
            .find(|spec| spec.agent == "kiro" && spec.label == "cli-session-json")
            .expect("kiro cli-session-json source spec missing");
        assert_eq!(
            kiro_cli.capabilities,
            LocalSourceCapabilities {
                prompt_input: true,
                assistant_output: true,
                tool_call: true,
                tool_result: true,
                token_usage: true,
                session_context: true,
                time_context: true,
                model_provider_context: true,
                account_context: false,
                cost_usage: true,
                reasoning_usage: true,
                edit_diff: true,
            },
            "kiro cli-session-json scanner/fixture covers prompt, output, tools, usage, context, and edit data"
        );
        assert_eq!(local_source_usage_basis(kiro_cli), UsageBasis::Native);
        assert!(
            !verified_full_capability_source(kiro_cli),
            "kiro cli-session-json must not claim account fields absent from the source"
        );

        let zed = specs
            .iter()
            .find(|spec| spec.agent == "zed" && spec.label == "threads-db")
            .expect("zed source spec missing");
        assert_eq!(zed.capabilities, OFFICIAL_TRANSCRIPT_CAPABILITIES);
        assert!(field_level_native_coverage_source(zed));
        assert!(!verified_full_capability_source(zed));

        let goose = specs
            .iter()
            .find(|spec| spec.agent == "goose" && spec.label == "sessions-db")
            .expect("goose source spec missing");
        assert_eq!(goose.capabilities, OFFICIAL_TRANSCRIPT_CAPABILITIES);
        assert!(field_level_native_coverage_source(goose));
        assert!(!verified_full_capability_source(goose));

        let openclaw = specs
            .iter()
            .find(|spec| spec.agent == "openclaw" && spec.label == "session-jsonl")
            .expect("openclaw source spec missing");
        assert_eq!(openclaw.capabilities, OPENCLAW_SESSION_CAPABILITIES);
        assert!(field_level_native_coverage_source(openclaw));
        assert!(!verified_full_capability_source(openclaw));

        let pi = specs
            .iter()
            .find(|spec| spec.agent == "pi" && spec.label == "session-jsonl")
            .expect("pi source spec missing");
        assert_eq!(pi.capabilities, OFFICIAL_TRANSCRIPT_CAPABILITIES);
        assert!(field_level_native_coverage_source(pi));
        assert!(!verified_full_capability_source(pi));

        let mux_chat = specs
            .iter()
            .find(|spec| spec.agent == "mux" && spec.label == "chat-jsonl")
            .expect("mux chat source spec missing");
        assert_eq!(mux_chat.capabilities, MUX_CHAT_JSONL_CAPABILITIES);
        assert!(field_level_native_coverage_source(mux_chat));
        assert!(!verified_full_capability_source(mux_chat));
        let mux_usage = specs
            .iter()
            .find(|spec| spec.agent == "mux" && spec.label == "session-usage-json")
            .expect("mux session usage source spec missing");
        assert_eq!(mux_usage.capabilities, MUX_SESSION_USAGE_JSON_CAPABILITIES);
        assert!(field_level_native_coverage_source(mux_usage));
        assert!(!verified_full_capability_source(mux_usage));

        let kilocode = specs
            .iter()
            .find(|spec| spec.agent == "kilocode" && spec.label == "vscode-tasks")
            .expect("kilocode source spec missing");
        assert_eq!(kilocode.capabilities, LOCAL_TRANSCRIPT_EVENT_CAPABILITIES);
        assert!(
            !verified_full_capability_source(kilocode),
            "kilocode vscode-tasks contributes transcript events without claiming SQLite fields"
        );

        let kilocode_sqlite = specs
            .iter()
            .find(|spec| spec.agent == "kilocode" && spec.label == "sqlite")
            .expect("kilocode sqlite source spec missing");
        assert_eq!(kilocode_sqlite.capabilities, KILO_FULL_LOCAL_CAPABILITIES);
        assert!(
            field_level_native_coverage_source(kilocode_sqlite),
            "kilocode sqlite must keep the KiloCode field-level source contract"
        );
        assert!(
            verified_full_capability_source(kilocode_sqlite),
            "kilocode sqlite must claim the full local field set proved by the KiloCode DB reader"
        );
        let kilocode_storage = specs
            .iter()
            .find(|spec| spec.agent == "kilocode" && spec.label == "storage-json")
            .expect("kilocode storage-json source spec missing");
        assert_eq!(kilocode_storage.capabilities, KILO_FULL_LOCAL_CAPABILITIES);
        assert!(
            field_level_native_coverage_source(kilocode_storage),
            "kilocode storage-json must keep the KiloCode storage field-level source contract"
        );
        assert!(
            verified_full_capability_source(kilocode_storage),
            "kilocode storage-json must claim the full local field set proved by the KiloCode storage reader"
        );

        let kimi = specs
            .iter()
            .find(|spec| spec.agent == "kimi" && spec.label == "wire-jsonl")
            .expect("kimi source spec missing");
        assert_eq!(kimi.capabilities, KIMI_WIRE_JSONL_CAPABILITIES);
        assert!(
            field_level_native_coverage_source(kimi),
            "kimi wire-jsonl must keep the Kimi Code wire/state field-level source contract"
        );
        assert!(
            !verified_full_capability_source(kimi),
            "kimi wire-jsonl must not claim cost/reasoning fields absent from the source"
        );

        let gjc = specs
            .iter()
            .find(|spec| spec.agent == "gjc" && spec.label == "session-jsonl")
            .expect("gjc source spec missing");
        assert_eq!(gjc.capabilities, GJC_SESSION_JSONL_CAPABILITIES);
        assert!(field_level_native_coverage_source(gjc));
        assert!(!verified_full_capability_source(gjc));

        let warp = specs
            .iter()
            .find(|spec| spec.agent == "warp" && spec.label == "warp-sqlite")
            .expect("warp source spec missing");
        assert_eq!(
            warp.capabilities,
            LocalSourceCapabilities {
                prompt_input: true,
                assistant_output: true,
                tool_call: true,
                tool_result: true,
                token_usage: false,
                session_context: true,
                time_context: true,
                model_provider_context: true,
                account_context: false,
                cost_usage: true,
                reasoning_usage: false,
                edit_diff: true,
            },
            "warp aggregate/category token totals must not require a stable output token bucket"
        );
        assert!(
            !verified_full_capability_source(warp),
            "warp sqlite must keep aggregate/category token support explicit"
        );

        let zcode_projects = specs
            .iter()
            .find(|spec| spec.agent == "zcode" && spec.label == "projects-jsonl")
            .expect("zcode projects source spec missing");
        assert_eq!(
            zcode_projects.capabilities,
            ZCODE_PROJECT_JSONL_CAPABILITIES
        );
        assert_eq!(local_source_usage_basis(zcode_projects), UsageBasis::Native);
        assert!(
            verified_full_capability_source(zcode_projects),
            "zcode projects-jsonl must carry native full-data coverage"
        );

        let antigravity_conversation = specs
            .iter()
            .find(|spec| spec.agent == "antigravity" && spec.label == "conversation-sqlite")
            .expect("antigravity conversation source spec missing");
        assert_eq!(
            antigravity_conversation.capabilities,
            ANTIGRAVITY_CONVERSATION_SQLITE_CAPABILITIES,
            "antigravity conversation-sqlite extracts native protobuf token buckets plus typed transcript fields"
        );
        assert_eq!(
            local_source_usage_basis(antigravity_conversation),
            UsageBasis::Native
        );
        assert!(field_level_native_coverage_source(antigravity_conversation));
        assert!(
            !verified_full_capability_source(antigravity_conversation),
            "antigravity conversation-sqlite must not claim fields absent from the source"
        );

        let cursor_state = specs
            .iter()
            .find(|spec| spec.agent == "cursor" && spec.label == "state-vscdb")
            .expect("cursor state-vscdb source spec missing");
        assert_eq!(local_source_usage_basis(cursor_state), UsageBasis::Native);
        assert!(
            verified_full_capability_source(cursor_state),
            "cursor/state-vscdb must carry native full-data coverage"
        );

        for (agent, label) in [("qoder", "local-db"), ("qoder-cn", "local-db")] {
            let spec = specs
                .iter()
                .find(|spec| spec.agent == agent && spec.label == label)
                .unwrap_or_else(|| panic!("{agent}/{label} source spec missing"));
            assert_eq!(local_source_usage_basis(spec), UsageBasis::Native);
            assert!(
                !verified_full_capability_source(spec),
                "{agent}/{label} must not claim fields absent from the source"
            );
        }

        let qoder_work_local = specs
            .iter()
            .find(|spec| spec.agent == "qoder-work" && spec.label == "local-db")
            .expect("qoder-work local-db source spec missing");
        assert_eq!(
            qoder_work_local.capabilities, QODER_WORK_LOCAL_DB_CAPABILITIES,
            "qoder-work local-db contributes prompt, native usage, model/session context, and edits"
        );
        assert!(
            verified_full_capability_source(qoder_work_local),
            "qoder-work local-db must carry native full-data coverage"
        );

        let qoder_work_trace = specs
            .iter()
            .find(|spec| spec.agent == "qoder-work" && spec.label == "trace-jsonl")
            .expect("qoder-work trace source spec missing");
        assert_eq!(qoder_work_trace.capabilities, QODER_WORK_TRACE_CAPABILITIES);
        assert!(
            verified_full_capability_source(qoder_work_trace),
            "qoder-work trace-jsonl must carry native full-data coverage"
        );

        let qoder_work_cn_local = specs
            .iter()
            .find(|spec| spec.agent == "qoder-work-cn" && spec.label == "local-db")
            .expect("qoder-work-cn local-db source spec missing");
        assert_eq!(
            qoder_work_cn_local.capabilities, QODER_WORK_LOCAL_DB_CAPABILITIES,
            "qoder-work-cn local-db contributes prompt, native usage, model/session context, and edits"
        );
        assert!(
            verified_full_capability_source(qoder_work_cn_local),
            "qoder-work-cn local-db must carry native full-data coverage"
        );

        let qoder_work_cn_trace = specs
            .iter()
            .find(|spec| spec.agent == "qoder-work-cn" && spec.label == "trace-jsonl")
            .expect("qoder-work-cn trace source spec missing");
        assert_eq!(
            qoder_work_cn_trace.capabilities,
            QODER_WORK_TRACE_CAPABILITIES
        );
        assert!(
            verified_full_capability_source(qoder_work_cn_trace),
            "qoder-work-cn trace-jsonl must carry native full-data coverage"
        );

        let wukong_sqlite = specs
            .iter()
            .find(|spec| spec.agent == "wukong" && spec.label == "sqlite")
            .expect("wukong sqlite source spec missing");
        assert_eq!(
            wukong_sqlite.capabilities, WUKONG_SQLITE_CAPABILITIES,
            "wukong sqlite must keep field-level prompt/output/tool/token coverage"
        );
        assert_eq!(local_source_usage_basis(wukong_sqlite), UsageBasis::Native);
        assert!(
            !verified_full_capability_source(wukong_sqlite),
            "wukong sqlite must not claim fields absent from the local DB"
        );

        let synthetic_sqlite = specs
            .iter()
            .find(|spec| spec.agent == "synthetic" && spec.label == "sqlite")
            .expect("synthetic sqlite source spec missing");
        assert_eq!(synthetic_sqlite.capabilities, SYNTHETIC_SQLITE_CAPABILITIES);
        assert_eq!(
            local_source_usage_basis(synthetic_sqlite),
            UsageBasis::Native
        );
        assert!(
            !verified_full_capability_source(synthetic_sqlite),
            "synthetic sqlite must not claim fields absent from the local DB"
        );

        let codebuff_project_jsonl = specs
            .iter()
            .find(|spec| spec.agent == "codebuff" && spec.label == "project-jsonl")
            .expect("codebuff project-jsonl source spec missing");
        assert_eq!(
            codebuff_project_jsonl.capabilities,
            LocalSourceCapabilities {
                prompt_input: true,
                assistant_output: true,
                tool_call: true,
                tool_result: true,
                token_usage: false,
                session_context: true,
                time_context: true,
                model_provider_context: true,
                account_context: false,
                cost_usage: true,
                reasoning_usage: false,
                edit_diff: true,
            },
            "Codebuff project-jsonl must require credits/source_cost, not fabricated token buckets"
        );
        assert_eq!(
            local_source_usage_basis(codebuff_project_jsonl),
            UsageBasis::Native
        );
        assert!(
            !verified_full_capability_source(codebuff_project_jsonl),
            "Codebuff credits/source_cost evidence must not claim stable token buckets"
        );
    }

    #[test]
    fn copilot_default_scan_roots_include_github_copilot_config_tree() {
        let _guard = ENV_LOCK.lock().unwrap();
        {
            let dir = TempDir::new().unwrap();
            std::env::remove_var("XDG_CONFIG_HOME");
            let roots = default_scan_roots(dir.path(), "copilot");

            assert!(
                roots
                    .iter()
                    .any(|root| root == &dir.path().join(".config/github-copilot")),
                "Copilot default scan roots should cover the GitHub Copilot config session cache"
            );

            let xdg_config_home = dir.path().join("xdg-config");
            std::env::set_var("XDG_CONFIG_HOME", &xdg_config_home);
            let roots = default_scan_roots(dir.path(), "copilot");
            assert!(
                roots
                    .iter()
                    .any(|root| root == &xdg_config_home.join("github-copilot")),
                "Copilot default scan roots should honor XDG_CONFIG_HOME"
            );
            std::env::remove_var("XDG_CONFIG_HOME");
        }
    }

    #[test]
    fn agent_support_matrix_has_required_upload_sources() {
        let specs = local_source_specs();

        for agent in default_scan_agent_names() {
            let matching = specs
                .iter()
                .filter(|spec| spec.agent == agent)
                .collect::<Vec<_>>();
            assert!(!matching.is_empty(), "{agent} source spec missing");
            assert!(
                matching.iter().any(|spec| {
                    source_has_full_upload_coverage(spec)
                        || local_derived_usage_source(spec)
                        || field_level_native_coverage_source(spec)
                }),
                "{agent} must have a declared source that contributes to full-data upload"
            );
        }

        for (agent, label) in [
            ("cursor", "hook-jsonl"),
            ("cursor", "agent-transcripts-jsonl"),
            ("cursor", "state-vscdb"),
            ("kiro", "hook-jsonl"),
            ("amp", "threads-jsonl"),
            ("grok", "sessions-jsonl"),
            ("qoder", "transcript-jsonl"),
            ("qoder-cn", "transcript-jsonl"),
            ("qoder-work", "trace-jsonl"),
            ("qoder-work", "local-db"),
            ("qoder-work-cn", "trace-jsonl"),
            ("qoder-work-cn", "local-db"),
            ("zcode", "projects-jsonl"),
        ] {
            let spec = specs
                .iter()
                .find(|spec| spec.agent == agent && spec.label == label)
                .unwrap_or_else(|| panic!("{agent}/{label} source spec missing"));
            assert_eq!(
                local_source_usage_basis(spec),
                UsageBasis::Native,
                "{agent}/{label} must use native local usage fields"
            );
            assert!(
                source_has_full_upload_coverage(spec),
                "{agent}/{label} must carry source-level full upload coverage"
            );
            assert!(
                verified_full_capability_source(spec),
                "{agent}/{label} must be audited as native full coverage"
            );
        }
    }

    #[test]
    fn default_scan_agents_have_required_full_data_coverage() {
        let specs = local_source_specs();
        let mut missing_agents = Vec::new();

        for agent in default_scan_agent_names() {
            let matching = specs
                .iter()
                .filter(|spec| spec.agent == agent)
                .collect::<Vec<_>>();
            assert!(!matching.is_empty(), "{agent} source spec missing");
            if !agent_has_required_full_data_coverage(agent) {
                let coverage = agent_full_data_coverage(agent).expect("agent coverage");
                missing_agents.push(format!("{agent}: {:?}", missing_coverage_fields(coverage)));
            }
            assert!(
                matching
                    .iter()
                    .any(|spec| local_source_usage_basis(spec) != UsageBasis::None),
                "{agent} must declare a native or local-derived usage basis"
            );
        }
        assert!(
            missing_agents.is_empty(),
            "default scan agents missing required full-data coverage: {}",
            missing_agents.join("; ")
        );

        for agent in [
            "cursor",
            "qoder",
            "qoder-cn",
            "antigravity",
            "cline",
            "roo-code",
            "qoder-work",
            "qoder-work-cn",
        ] {
            assert_eq!(
                agent_full_data_coverage(agent)
                    .expect("agent coverage")
                    .usage_basis,
                UsageBasis::Native,
                "{agent} has a typed native usage source at agent level"
            );
        }
    }

    #[test]
    fn source_level_full_upload_coverage_requires_audited_label_and_strict_capabilities() {
        let strict_native_source = LocalSourceSpec {
            agent: "wukong",
            kind: LocalSourceKind::Sqlite,
            label: "sqlite",
            capabilities: strict_full_upload_capabilities(),
        };
        assert!(verified_full_capability_source(&strict_native_source));
        assert!(source_has_full_upload_coverage(&strict_native_source));
        assert_eq!(
            local_source_usage_basis(&strict_native_source),
            UsageBasis::Native
        );

        let strict_local_derived_source = LocalSourceSpec {
            agent: "cursor",
            kind: LocalSourceKind::SessionJsonl,
            label: "agent-transcripts-jsonl",
            capabilities: strict_full_upload_capabilities(),
        };
        assert!(!verified_local_derived_full_coverage_source(
            &strict_local_derived_source
        ));
        assert!(source_has_full_upload_coverage(
            &strict_local_derived_source
        ));
        assert_eq!(
            local_source_usage_basis(&strict_local_derived_source),
            UsageBasis::Native
        );

        let unaudited_source = LocalSourceSpec {
            agent: "unknown",
            kind: LocalSourceKind::SessionJsonl,
            label: "strict-jsonl",
            capabilities: strict_full_upload_capabilities(),
        };
        assert!(!verified_full_capability_source(&unaudited_source));
        assert!(!verified_local_derived_full_coverage_source(
            &unaudited_source
        ));
        assert!(!source_has_full_upload_coverage(&unaudited_source));

        let field_level_native = local_source_specs()
            .iter()
            .find(|spec| spec.agent == "wukong" && spec.label == "sqlite")
            .expect("wukong sqlite source spec missing");
        assert!(!verified_full_capability_source(field_level_native));
        assert!(!source_has_full_upload_coverage(field_level_native));
    }

    #[test]
    fn supplemental_sources_do_not_overclaim_single_source_complete_fields() {
        let specs = local_source_specs();
        for (agent, label) in [
            ("copilot", "otel-jsonl"),
            ("copilot", "session-state-jsonl"),
            ("copilot", "session-store-db"),
            ("copilot", "vscode-chat-state"),
            ("kiro", "cli-session-json"),
            ("antigravity", "conversation-sqlite"),
            ("openclaw", "session-jsonl"),
            ("gjc", "session-jsonl"),
            ("droid", "session-jsonl"),
            ("pi", "session-jsonl"),
            ("mux", "chat-jsonl"),
            ("mux", "session-usage-json"),
            ("kimi", "wire-jsonl"),
            ("zed", "threads-db"),
            ("goose", "sessions-db"),
            ("qwen", "project-chats-jsonl"),
            ("opencode", "sqlite"),
            ("qoder", "hook-jsonl"),
            ("qoder", "local-db"),
            ("qoder-cn", "hook-jsonl"),
            ("qoder-cn", "local-db"),
            ("qoder-work", "hook-jsonl"),
            ("qoder-work-cn", "hook-jsonl"),
        ] {
            let spec = specs
                .iter()
                .find(|spec| spec.agent == agent && spec.label == label)
                .unwrap_or_else(|| panic!("{agent}/{label} source spec missing"));
            assert!(
                !all_capabilities_enabled(spec.capabilities),
                "{agent}/{label} must not claim fields that the source itself does not expose"
            );
        }
    }

    fn all_capabilities_enabled(capabilities: LocalSourceCapabilities) -> bool {
        capabilities.prompt_input
            && capabilities.assistant_output
            && capabilities.tool_call
            && capabilities.tool_result
            && capabilities.token_usage
            && capabilities.session_context
            && capabilities.time_context
            && capabilities.model_provider_context
            && capabilities.account_context
            && capabilities.cost_usage
            && capabilities.reasoning_usage
            && capabilities.edit_diff
    }

    fn missing_coverage_fields(coverage: AgentFullDataCoverage) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if !coverage.prompt_input {
            missing.push("prompt_input");
        }
        if !coverage.assistant_output {
            missing.push("assistant_output");
        }
        if !coverage.tool_call {
            missing.push("tool_call");
        }
        if !coverage.tool_result {
            missing.push("tool_result");
        }
        if !coverage.token_usage {
            missing.push("token_usage");
        }
        if coverage.usage_basis == UsageBasis::None {
            missing.push("usage_basis");
        }
        if !coverage.session_context {
            missing.push("session_context");
        }
        if !coverage.time_context {
            missing.push("time_context");
        }
        if !coverage.model_provider_context {
            missing.push("model_provider_context");
        }
        if !coverage.account_context {
            missing.push("account_context");
        }
        if !coverage.cost_usage {
            missing.push("cost_usage");
        }
        if !coverage.reasoning_usage {
            missing.push("reasoning_usage");
        }
        if !coverage.edit_diff {
            missing.push("edit_diff");
        }
        missing
    }

    fn strict_full_upload_capabilities() -> LocalSourceCapabilities {
        LocalSourceCapabilities {
            prompt_input: true,
            assistant_output: true,
            tool_call: true,
            tool_result: true,
            token_usage: true,
            session_context: true,
            time_context: true,
            model_provider_context: true,
            account_context: true,
            cost_usage: true,
            reasoning_usage: true,
            edit_diff: true,
        }
    }

    #[test]
    fn default_scan_roots_cover_local_agent_storage() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        std::env::set_var("CLAUDE_CONFIG_DIR", home.join("custom-claude"));
        std::env::set_var("CODEX_HOME", home.join("custom-codex"));
        std::env::set_var("GEMINI_CLI_HOME", home.join("custom-gemini"));
        std::env::set_var("QWEN_HOME", home.join("custom-qwen"));
        std::env::set_var("QWEN_RUNTIME_DIR", home.join("custom-qwen-runtime"));
        std::env::set_var("CLINE_DATA_DIR", home.join(".cline/data"));
        std::env::set_var("KIMI_CODE_HOME", home.join(".custom-kimi-code"));
        std::env::set_var("KIRO_HOME", home.join(".custom-kiro"));
        std::env::set_var("GJC_CODING_AGENT_DIR", home.join("custom-gjc-agent"));
        std::env::set_var(
            "COPILOT_OTEL_FILE_EXPORTER_PATH",
            home.join("copilot.jsonl"),
        );

        assert!(default_scan_roots(home, "claude")
            .iter()
            .any(|p| p.ends_with("custom-claude/projects")));
        assert!(default_scan_roots(home, "claude")
            .iter()
            .any(|p| p.ends_with("custom-claude/transcripts")));
        assert!(default_scan_roots(home, "codex")
            .iter()
            .any(|p| p.ends_with("custom-codex/sessions")));
        assert!(!default_scan_roots(home, "codex")
            .iter()
            .any(|p| p == &home.join("custom-codex")));
        let cursor_roots = default_scan_roots(home, "cursor");
        assert!(cursor_roots.iter().any(|p| p.ends_with(".cursor/projects")));
        assert!(cursor_roots
            .iter()
            .any(|p| p.to_string_lossy().contains("Cursor/User/workspaceStorage")));
        let qwen_roots = default_scan_roots(home, "qwen");
        assert!(qwen_roots.iter().any(|p| p == &home.join("custom-qwen")));
        assert!(qwen_roots
            .iter()
            .any(|p| p == &home.join("custom-qwen/projects")));
        assert!(qwen_roots
            .iter()
            .any(|p| p == &home.join("custom-qwen-runtime")));
        assert!(qwen_roots
            .iter()
            .any(|p| p == &home.join("custom-qwen-runtime/projects")));
        assert!(qwen_roots
            .iter()
            .any(|p| p == &home.join("custom-qwen-runtime/usage")));
        assert!(default_scan_roots(home, "gemini")
            .iter()
            .any(|p| p.ends_with("custom-gemini/.gemini")));
        assert!(default_scan_roots(home, "gemini")
            .iter()
            .any(|p| p.ends_with("custom-gemini/.gemini/tmp")));
        assert!(default_scan_roots(home, "gemini")
            .iter()
            .any(|p| p.ends_with("custom-gemini/tmp")));
        assert!(default_scan_roots(home, "copilot")
            .iter()
            .any(|p| p.ends_with("copilot.jsonl")));
        assert!(default_scan_roots(home, "copilot")
            .iter()
            .any(|p| p.ends_with(".copilot/session-state")));
        assert!(default_scan_roots(home, "copilot")
            .iter()
            .any(|p| p.ends_with(".copilot/session-store.db")));
        let copilot_roots = default_scan_roots(home, "copilot");
        for suffix in [
            ".config/github-copilot",
            ".config/github-copilot/chat-sessions",
            ".config/github-copilot/chat-agent-sessions",
            ".config/github-copilot/chat-edit-sessions",
            ".config/github-copilot/ws/chat-sessions",
            ".config/github-copilot/ws/chat-agent-sessions",
            ".config/github-copilot/ws/chat-edit-sessions",
        ] {
            assert!(
                copilot_roots.iter().any(|p| p.ends_with(suffix)),
                "Copilot default scan roots should include {suffix}"
            );
        }
        assert!(default_scan_roots(home, "cline")
            .iter()
            .any(|p| p.ends_with(".cline/data/sessions")));
        assert!(default_scan_roots(home, "cline").iter().any(|p| {
            p.to_string_lossy().contains(
                "Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/tasks",
            )
        }));
        assert!(default_scan_roots(home, "cline").iter().any(|p| {
            p.to_string_lossy()
                .contains(".config/Cursor/User/globalStorage/saoudrizwan.claude-dev/tasks")
        }));
        assert!(default_scan_roots(home, "cline").iter().any(|p| {
            p.to_string_lossy()
                .contains(".config/VSCodium/User/globalStorage/saoudrizwan.claude-dev/tasks")
        }));
        assert!(default_scan_roots(home, "zed")
            .iter()
            .any(|p| p.to_string_lossy().contains("Zed/threads")));
        assert!(default_scan_roots(home, "qoder")
            .iter()
            .any(|p| p.to_string_lossy().contains("SharedClientCache/cache/db")));
        assert!(default_scan_roots(home, "qoder-work")
            .iter()
            .any(|p| p.ends_with(".qoderwork/logs/sessions")));
        assert!(default_scan_roots(home, "qoder-work")
            .iter()
            .any(|p| p.ends_with(".qoderwork/tool-results")));
        assert!(default_scan_roots(home, "qoder-work")
            .iter()
            .any(|p| p.ends_with(".qoderwork/messages.db")));
        assert!(default_scan_roots(home, "qoder-work-cn")
            .iter()
            .any(|p| p.ends_with(".qoderwork/hooks")));
        assert!(default_scan_roots(home, "qoder-work-cn")
            .iter()
            .any(|p| p.ends_with(".qoderwork/messages.db")));
        assert!(default_scan_roots(home, "qoder-work-cn")
            .iter()
            .any(|p| p.ends_with(".qoderworkcn/hooks")));
        assert!(default_scan_roots(home, "roo-code")
            .iter()
            .any(|p| p.to_string_lossy().contains("roo-cline/tasks")));
        assert!(default_scan_roots(home, "roo-code").iter().any(|p| p
            .to_string_lossy()
            .contains("Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/tasks")));
        assert!(default_scan_roots(home, "roo-code").iter().any(|p| p
            .to_string_lossy()
            .contains(".config/Cursor/User/globalStorage/rooveterinaryinc.roo-cline/tasks")));
        assert!(default_scan_roots(home, "roo-code").iter().any(|p| p
            .to_string_lossy()
            .contains(".config/VSCodium/User/globalStorage/rooveterinaryinc.roo-cline/tasks")));
        assert!(default_scan_roots(home, "kilocode")
            .iter()
            .any(|p| p.to_string_lossy().contains("kilo-code/tasks")));
        assert!(default_scan_roots(home, "kilocode")
            .iter()
            .any(|p| p.ends_with(".kilocode/cli/global/tasks")));
        assert!(default_scan_roots(home, "kilocode")
            .iter()
            .any(|p| p.ends_with(".kilocode/cli/workspaces")));
        assert!(default_scan_roots(home, "kilocode")
            .iter()
            .any(|p| p.ends_with(".local/share/kilo/kilo.db")));
        assert!(default_scan_roots(home, "kilocode")
            .iter()
            .any(|p| p.ends_with(".local/share/kilo/storage/session")));
        assert!(default_scan_roots(home, "kilocode").iter().any(|p| {
            p.to_string_lossy()
                .contains("Library/Application Support/kilo/kilo.db")
        }));
        assert!(default_scan_roots(home, "kilocode").iter().any(|p| {
            p.to_string_lossy()
                .contains("Library/Application Support/kilo/storage/session")
        }));
        assert!(default_scan_roots(home, "kilo")
            .iter()
            .any(|p| p.ends_with(".local/share/kilo/kilo.db")));
        assert!(default_scan_roots(home, "kilo").iter().any(|p| {
            p.to_string_lossy()
                .contains("Library/Application Support/kilo/kilo.db")
        }));
        assert!(default_scan_roots(home, "kilo")
            .iter()
            .any(|p| p.ends_with(".local/share/kilo/storage/session")));
        assert!(default_scan_roots(home, "kilo").iter().any(|p| {
            p.to_string_lossy()
                .contains("Library/Application Support/kilo/storage/session")
        }));
        assert!(default_scan_roots(home, "crush")
            .iter()
            .any(|p| p.ends_with(".crush/crush.db")));
        assert!(default_scan_roots(home, "crush")
            .iter()
            .any(|p| p.ends_with(".crush")));
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
            .any(|p| p.ends_with(".zcode/projects")));

        let previous_cwd = std::env::current_dir().unwrap();
        let project_cwd = home.join("gjc-project");
        fs::create_dir_all(&project_cwd).unwrap();
        std::env::set_current_dir(&project_cwd).unwrap();
        let gjc_roots = default_scan_roots(home, "gjc");
        std::env::set_current_dir(previous_cwd).unwrap();
        let project_gjc_root = fs::canonicalize(&project_cwd).unwrap().join(".gjc");
        assert!(
            gjc_roots.iter().any(|p| p == &project_gjc_root),
            "gjc must scan cwd-local .gjc session roots"
        );
        assert!(
            gjc_roots.iter().any(|p| p.ends_with(".gjc/agent/sessions")),
            "gjc must keep legacy home/env session fallback"
        );
        assert!(
            gjc_roots
                .iter()
                .any(|p| p == &home.join("custom-gjc-agent/sessions")),
            "gjc must scan sessions under GJC_CODING_AGENT_DIR"
        );

        std::env::remove_var("CLAUDE_CONFIG_DIR");
        std::env::remove_var("CODEX_HOME");
        std::env::remove_var("GEMINI_CLI_HOME");
        std::env::remove_var("QWEN_HOME");
        std::env::remove_var("QWEN_RUNTIME_DIR");
        std::env::remove_var("CLINE_DATA_DIR");
        std::env::remove_var("KIMI_CODE_HOME");
        std::env::remove_var("KIRO_HOME");
        std::env::remove_var("GJC_CODING_AGENT_DIR");
        std::env::remove_var("COPILOT_OTEL_FILE_EXPORTER_PATH");
    }
}
