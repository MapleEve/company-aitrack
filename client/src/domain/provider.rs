/// Return the agent-framework label to store in the `provider` field.
///
/// Records the agent/tool key, not the underlying LLM API provider. URL-based
/// heuristics are intentionally absent — the framework name is known at parse
/// time.
pub fn infer_provider(tool: &str) -> &str {
    tool
}

#[cfg(test)]
mod tests {
    use super::infer_provider;

    #[test]
    fn claude_returns_claude() {
        assert_eq!(infer_provider("claude"), "claude");
    }

    #[test]
    fn codex_returns_codex() {
        assert_eq!(infer_provider("codex"), "codex");
    }

    #[test]
    fn cursor_returns_cursor() {
        assert_eq!(infer_provider("cursor"), "cursor");
    }

    #[test]
    fn unknown_tool_passes_through() {
        assert_eq!(infer_provider("jetbrains"), "jetbrains");
    }
}
