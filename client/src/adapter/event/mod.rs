pub mod claude;
pub mod codex;
pub mod cursor;

pub use claude::parse as parse_claude;
pub use codex::parse as parse_codex;
pub use cursor::parse as parse_cursor;

use crate::agent;
use crate::domain::model::Record;

pub fn parse_known_agent(tool: &str, stdin_json: &str) -> Option<Record> {
    match tool {
        "claude" => parse_claude(stdin_json),
        "codex" => parse_codex(stdin_json),
        "cursor" => parse_cursor(stdin_json),
        other => {
            if agent::is_known_agent(other) {
                eprintln!("[aitrack] known agent has no native edit-event adapter yet: {other}");
            } else {
                eprintln!("[aitrack] unsupported capture tool: {other}");
            }
            None
        }
    }
}
