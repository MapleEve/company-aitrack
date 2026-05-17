use serde::Deserialize;
use crate::db::Record;
use crate::diff::compute_diff;

#[derive(Debug, Deserialize)]
struct CursorToolInput {
    file_path: Option<String>,
    content: Option<String>,
    old_str: Option<String>,
    new_str: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CursorHookPayload {
    tool_input: Option<CursorToolInput>,
    session_id: Option<String>,
    cursor_version: Option<String>,
}

pub fn parse(stdin_json: &str) -> Option<Record> {
    let payload: CursorHookPayload = serde_json::from_str(stdin_json)
        .map_err(|e| eprintln!("[aitrack] cursor adapter parse error: {e}"))
        .ok()?;

    let ti = payload.tool_input?;
    let file_path = ti.file_path.unwrap_or_default();
    let content = ti.content.unwrap_or_default();
    let old_text = ti.old_str.unwrap_or_default();
    let new_text = ti.new_str.unwrap_or(content);

    let diff = compute_diff(&old_text, &new_text);

    Some(Record {
        id: 0,
        tool: "cursor".to_string(),
        tool_version: payload.cursor_version,
        provider: "anthropic".to_string(),
        model: None,
        session_id: payload.session_id.unwrap_or_default(),
        repo_url: String::new(),
        branch: String::new(),
        current_sha: String::new(),
        file_path,
        added_lines: diff.added,
        removed_lines: diff.removed,
        diff_hunk: if diff.hunk.is_empty() { None } else { Some(diff.hunk) },
        metadata: None,
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

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::testkit::factories::{CursorHookPayloadFactory, malformed_json};

    #[test]
    fn parse_valid_payload_old_new_str() {
        let json = CursorHookPayloadFactory::new(1)
            .with_old_str("old content\n")
            .with_new_str("new content\nextra line\n")
            .with_file_path("src/cursor.rs")
            .with_session_id("cursor-sess-1")
            .build_json();
        let rec = parse(&json).expect("valid cursor payload should parse");
        assert_eq!(rec.tool, "cursor");
        assert_eq!(rec.file_path, "src/cursor.rs");
        assert_eq!(rec.session_id, "cursor-sess-1");
        // old has 1 line, new has 2 lines → removed=1, added=2
        assert_eq!(rec.added_lines, 2);
        assert_eq!(rec.removed_lines, 1);
        assert_eq!(rec.provider, "anthropic");
    }

    #[test]
    fn parse_content_field_used_when_no_new_str() {
        let json = serde_json::json!({
            "session_id": "sess-content",
            "cursor_version": "0.40.0",
            "tool_input": {
                "file_path": "src/content.rs",
                "content": "brand new file content\n"
            }
        }).to_string();
        let rec = parse(&json).expect("should parse using content field");
        assert_eq!(rec.file_path, "src/content.rs");
        // old_str is empty, new_text = content
        assert_eq!(rec.added_lines, 1);
    }

    #[test]
    fn parse_missing_tool_input_returns_none() {
        let json = serde_json::json!({
            "session_id": "sess-no-input",
            "cursor_version": "0.40.0"
        }).to_string();
        assert!(parse(&json).is_none(), "missing tool_input must return None");
    }

    #[test]
    fn parse_malformed_json_returns_none() {
        assert!(parse(&malformed_json()).is_none());
    }

    #[test]
    fn parse_cursor_version_propagated() {
        let json = CursorHookPayloadFactory::new(2).build_json();
        let rec = parse(&json).expect("should parse");
        assert_eq!(rec.tool_version, Some("0.40.0".to_string()));
    }

    #[test]
    fn parse_no_changes_yields_zero_diff() {
        let json = CursorHookPayloadFactory::new(3)
            .with_old_str("same content\n")
            .with_new_str("same content\n")
            .build_json();
        let rec = parse(&json).expect("should parse");
        assert_eq!(rec.added_lines, 0);
        assert_eq!(rec.removed_lines, 0);
        assert!(rec.diff_hunk.is_none());
    }

    #[test]
    fn parse_model_is_none_for_cursor() {
        let json = CursorHookPayloadFactory::new(4).build_json();
        let rec = parse(&json).expect("should parse");
        assert!(rec.model.is_none(), "cursor adapter sets no model");
    }

    #[test]
    fn parse_metadata_is_none_for_cursor() {
        let json = CursorHookPayloadFactory::new(5).build_json();
        let rec = parse(&json).expect("should parse");
        assert!(rec.metadata.is_none());
    }

    #[test]
    fn parse_seed_determinism() {
        // Same seed → same field values (session_id etc)
        let json1 = CursorHookPayloadFactory::new(42).build_json();
        let json2 = CursorHookPayloadFactory::new(42).build_json();
        let rec1 = parse(&json1).expect("parse 1");
        let rec2 = parse(&json2).expect("parse 2");
        assert_eq!(rec1.session_id, rec2.session_id);
        assert_eq!(rec1.file_path, rec2.file_path);
    }
}
