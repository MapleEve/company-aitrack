use serde::Deserialize;
use serde_json::Value;
use crate::db::Record;
use crate::diff::compute_diff;

#[derive(Debug, Deserialize)]
struct CodexToolInput {
    old_string: Option<String>,
    new_string: Option<String>,
    file_path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CodexHookPayload {
    hook_event_name: Option<String>,
    tool_name: Option<String>,
    tool_input: Option<CodexToolInput>,
    #[serde(alias = "conversation_id")]
    session_id: Option<String>,
    model: Option<String>,
    generation_id: Option<String>,
    user_email: Option<String>,
    #[serde(alias = "workspace_roots")]
    cwd: Option<Value>,
}

pub fn parse(stdin_json: &str) -> Option<Record> {
    let payload: CodexHookPayload = serde_json::from_str(stdin_json)
        .map_err(|e| eprintln!("[aitrack] codex adapter parse error: {e}"))
        .ok()?;

    // Only process file edit tool calls
    let event = payload.hook_event_name.as_deref().unwrap_or("");
    if event != "postToolUse" {
        return None;
    }
    let tool_name = payload.tool_name.as_deref().unwrap_or("");
    if !tool_name.contains("Edit") && !tool_name.contains("Write") && !tool_name.contains("apply_patch") {
        return None;
    }

    let ti = payload.tool_input?;
    let old_string = ti.old_string.unwrap_or_default();
    let new_string = ti.new_string.unwrap_or_default();
    let file_path = ti.file_path.unwrap_or_default();

    let diff = compute_diff(&old_string, &new_string);

    let metadata = build_metadata(
        payload.generation_id.as_deref(),
        payload.user_email.as_deref(),
        &payload.cwd,
    );

    Some(Record {
        id: 0,
        tool: "codex".to_string(),
        tool_version: Some("codex-cli".to_string()),
        provider: "openai".to_string(),
        model: payload.model,
        session_id: payload.session_id.unwrap_or_default(),
        repo_url: String::new(),
        branch: String::new(),
        current_sha: String::new(),
        file_path,
        added_lines: diff.added,
        removed_lines: diff.removed,
        diff_hunk: if diff.hunk.is_empty() { None } else { Some(diff.hunk) },
        metadata,
        synced: 0,
        synced_at: None,
        retry_count: 0,
        timestamp: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        token_key: String::new(),
        device_id: String::new(),
        hostname: String::new(),
        record_sig: String::new(),
    })
}

fn build_metadata(
    generation_id: Option<&str>,
    user_email: Option<&str>,
    cwd: &Option<Value>,
) -> Option<String> {
    let mut map = serde_json::Map::new();
    if let Some(g) = generation_id {
        map.insert("generation_id".to_string(), Value::String(g.to_string()));
    }
    if let Some(e) = user_email {
        map.insert("user_email".to_string(), Value::String(e.to_string()));
    }
    if let Some(c) = cwd {
        map.insert("cwd".to_string(), c.clone());
    }
    if map.is_empty() {
        None
    } else {
        serde_json::to_string(&map).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::testkit::factories::{
        CodexHookPayloadFactory, malformed_json,
        codex_wrong_event, codex_non_edit_tool,
    };

    #[test]
    fn parse_valid_edit_payload_returns_record() {
        let json = CodexHookPayloadFactory::new(1)
            .with_old_string("fn old() {}\n")
            .with_new_string("fn new() {}\n")
            .with_file_path("src/lib.rs")
            .with_session_id("codex-sess-1")
            .build_json();
        let rec = parse(&json).expect("valid codex Edit should parse");
        assert_eq!(rec.tool, "codex");
        assert_eq!(rec.provider, "openai");
        assert_eq!(rec.file_path, "src/lib.rs");
        assert_eq!(rec.added_lines, 1);
        assert_eq!(rec.removed_lines, 1);
    }

    #[test]
    fn parse_wrong_event_name_returns_none() {
        let json = codex_wrong_event(2);
        assert!(parse(&json).is_none(), "preToolUse event must return None");
    }

    #[test]
    fn parse_non_edit_tool_name_returns_none() {
        let json = codex_non_edit_tool(3);
        assert!(parse(&json).is_none(), "ListFiles tool must return None");
    }

    #[test]
    fn parse_write_tool_accepted() {
        let json = CodexHookPayloadFactory::new(4)
            .with_tool_name("Write")
            .with_old_string("")
            .with_new_string("new file content\n")
            .build_json();
        let rec = parse(&json).expect("Write tool should be accepted");
        assert_eq!(rec.tool, "codex");
    }

    #[test]
    fn parse_apply_patch_tool_accepted() {
        let json = CodexHookPayloadFactory::new(5)
            .with_tool_name("apply_patch")
            .build_json();
        let rec = parse(&json).expect("apply_patch tool should be accepted");
        assert_eq!(rec.tool, "codex");
    }

    #[test]
    fn parse_malformed_json_returns_none() {
        assert!(parse(&malformed_json()).is_none());
    }

    #[test]
    fn parse_metadata_includes_generation_id() {
        let json = serde_json::json!({
            "hook_event_name": "postToolUse",
            "tool_name": "Edit",
            "conversation_id": "sess-meta",
            "model": "gpt-4o",
            "generation_id": "gen-abc123",
            "user_email": "user@example.com",
            "tool_input": {
                "old_string": "a\n",
                "new_string": "b\n",
                "file_path": "src/meta.rs"
            }
        }).to_string();
        let rec = parse(&json).expect("should parse with metadata");
        let meta = rec.metadata.expect("should have metadata");
        assert!(meta.contains("gen-abc123"));
        assert!(meta.contains("user@example.com"));
    }

    #[test]
    fn parse_no_metadata_when_all_optional_fields_absent() {
        let json = CodexHookPayloadFactory::new(6).build_json();
        let rec = parse(&json).expect("should parse");
        // Factory doesn't set generation_id or user_email → None
        assert!(rec.metadata.is_none());
    }

    #[test]
    fn parse_model_field_propagated() {
        let json = CodexHookPayloadFactory::new(7).build_json();
        let rec = parse(&json).expect("should parse");
        assert_eq!(rec.model, Some("gpt-4o".to_string()));
    }

    #[test]
    fn parse_tool_version_is_codex_cli() {
        let json = CodexHookPayloadFactory::new(8).build_json();
        let rec = parse(&json).expect("should parse");
        assert_eq!(rec.tool_version, Some("codex-cli".to_string()));
    }
}
