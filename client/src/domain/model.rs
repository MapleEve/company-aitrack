/// A full record row from the `records` table.
#[derive(Debug, Clone)]
pub struct Record {
    pub id: i64,
    pub tool: String,
    pub tool_version: Option<String>,
    pub provider: String,
    pub model: Option<String>,
    pub session_id: String,
    pub repo_url: String,
    pub branch: String,
    pub current_sha: String,
    pub file_path: String,
    pub added_lines: i64,
    pub removed_lines: i64,
    pub diff_hunk: Option<String>,
    pub metadata: Option<String>,
    #[allow(dead_code)]
    pub synced: i64,
    #[allow(dead_code)]
    pub synced_at: Option<String>,
    #[allow(dead_code)]
    pub retry_count: i64,
    pub timestamp: String,
    pub token_key: String,
    pub device_id: String,
    pub hostname: String,
    pub record_sig: String,
    pub prompt_summary: Option<String>,
}

pub const MAX_STORED_DIFF_HUNK_CHARS: usize = 8192;
pub const MAX_STORED_METADATA_CHARS: usize = 4096;
pub const MAX_STORED_PROMPT_CHARS: usize = 4096;

pub fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

pub fn truncate_optional_chars(value: &Option<String>, max_chars: usize) -> Option<String> {
    value.as_deref().map(|s| truncate_chars(s, max_chars))
}

pub fn sanitize_sig_bound_record_fields(record: &mut Record) {
    record.diff_hunk = truncate_optional_chars(&record.diff_hunk, MAX_STORED_DIFF_HUNK_CHARS);
}

pub fn sanitize_non_sig_record_fields(record: &mut Record) {
    record.metadata = truncate_optional_chars(&record.metadata, MAX_STORED_METADATA_CHARS);
    record.prompt_summary =
        truncate_optional_chars(&record.prompt_summary, MAX_STORED_PROMPT_CHARS);
}

/// A lightweight row returned by the `inspect` command.
#[derive(Debug)]
pub struct InspectRow {
    pub id: i64,
    pub tool: String,
    pub model: Option<String>,
    pub file_path: String,
    pub added_lines: i64,
    pub removed_lines: i64,
    pub synced: i64,
    pub retry_count: i64,
    #[allow(dead_code)]
    pub token_key: String,
    pub timestamp: String,
}
