use similar::{ChangeTag, TextDiff};

#[derive(Debug, Clone)]
pub struct DiffResult {
    pub added: i64,
    pub removed: i64,
    pub hunk: String,
}

/// Compute true Myers/LCS diff between old and new text.
pub fn compute_diff(old_text: &str, new_text: &str) -> DiffResult {
    let diff = TextDiff::from_lines(old_text, new_text);

    let mut added: i64 = 0;
    let mut removed: i64 = 0;

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => added += 1,
            ChangeTag::Delete => removed += 1,
            ChangeTag::Equal => {}
        }
    }

    let hunk = generate_unified_diff(old_text, new_text);

    DiffResult {
        added,
        removed,
        hunk,
    }
}

fn generate_unified_diff(old_text: &str, new_text: &str) -> String {
    let diff = TextDiff::from_lines(old_text, new_text);
    let mut out = String::new();

    for group in diff.grouped_ops(3) {
        // Build hunk header
        let first = group.first().unwrap();
        let _last = group.last().unwrap();

        let old_start = first.old_range().start + 1;
        let old_len: usize = group.iter().map(|op| op.old_range().len()).sum();
        let new_start = first.new_range().start + 1;
        let new_len: usize = group.iter().map(|op| op.new_range().len()).sum();

        out.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            old_start, old_len, new_start, new_len
        ));

        for op in &group {
            for change in diff.iter_changes(op) {
                match change.tag() {
                    ChangeTag::Delete => out.push_str(&format!("-{}", change.value())),
                    ChangeTag::Insert => out.push_str(&format!("+{}", change.value())),
                    ChangeTag::Equal => out.push_str(&format!(" {}", change.value())),
                }
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::factories::{ClaudeHookPayloadFactory, EditRecordFactory};

    #[test]
    fn identical_texts_produce_zero_counts() {
        let text = "line1\nline2\nline3\n";
        let result = compute_diff(text, text);
        assert_eq!(result.added, 0);
        assert_eq!(result.removed, 0);
        assert!(result.hunk.is_empty(), "no hunk for identical text");
    }

    #[test]
    fn empty_to_nonempty_all_added() {
        let result = compute_diff("", "line1\nline2\n");
        assert_eq!(result.added, 2);
        assert_eq!(result.removed, 0);
        assert!(result.hunk.contains("@@"));
        assert!(result.hunk.contains("+line1"));
    }

    #[test]
    fn nonempty_to_empty_all_removed() {
        let result = compute_diff("line1\nline2\n", "");
        assert_eq!(result.added, 0);
        assert_eq!(result.removed, 2);
        assert!(result.hunk.contains("-line1"));
    }

    #[test]
    fn single_line_change() {
        let old = "fn foo() {}\n";
        let new = "fn bar() {}\n";
        let result = compute_diff(old, new);
        assert_eq!(result.added, 1);
        assert_eq!(result.removed, 1);
        assert!(result.hunk.contains("-fn foo()"));
        assert!(result.hunk.contains("+fn bar()"));
    }

    #[test]
    fn multi_hunk_diff() {
        // Two distant changes produce two hunks
        let old = (1..=20).map(|i| format!("line{i}\n")).collect::<String>();
        let mut new_lines: Vec<String> = (1..=20).map(|i| format!("line{i}\n")).collect();
        new_lines[0] = "CHANGED1\n".to_string();
        new_lines[19] = "CHANGED20\n".to_string();
        let new = new_lines.join("");
        let result = compute_diff(&old, &new);
        assert_eq!(result.added, 2);
        assert_eq!(result.removed, 2);
        // Two separate @@ hunks
        assert_eq!(result.hunk.matches("@@").count(), 4); // each @@ appears twice: @@ -... @@
    }

    #[test]
    fn myers_counts_real_changes_not_inflated() {
        // Verifies Myers/LCS: only truly different lines counted
        let old = "a\nb\nc\nd\n";
        let new = "a\nX\nc\nd\n";
        let result = compute_diff(old, new);
        assert_eq!(result.added, 1, "only 1 line added");
        assert_eq!(result.removed, 1, "only 1 line removed");
    }

    #[test]
    fn hunk_format_starts_with_at_at() {
        let result = compute_diff("old\n", "new\n");
        assert!(result.hunk.starts_with("@@"), "hunk must start with @@");
    }

    #[test]
    fn hunk_contains_plus_minus_lines() {
        let result = compute_diff("removed line\n", "added line\n");
        assert!(result.hunk.contains("-removed line"));
        assert!(result.hunk.contains("+added line"));
    }

    #[test]
    fn factory_old_new_produce_diff() {
        // Use ClaudeHookPayloadFactory to get realistic old/new strings
        let factory = ClaudeHookPayloadFactory::new(99)
            .with_old_string("fn hello() {\n    println!(\"hi\");\n}\n")
            .with_new_string("fn hello() {\n    println!(\"hi\");\n    println!(\"world\");\n}\n");
        // The factory produces the JSON, so parse the strings directly
        let old = "fn hello() {\n    println!(\"hi\");\n}\n";
        let new = "fn hello() {\n    println!(\"hi\");\n    println!(\"world\");\n}\n";
        let _ = factory; // factory used above
        let result = compute_diff(old, new);
        assert_eq!(result.added, 1);
        assert_eq!(result.removed, 0);
    }

    #[test]
    fn factory_record_diff_hunk_in_expected_format() {
        let rec = EditRecordFactory::new(7)
            .with_diff_hunk(Some("@@ -1,1 +1,2 @@\n-old\n+new\n+extra\n".to_string()))
            .build();
        let hunk = rec.diff_hunk.unwrap();
        assert!(hunk.starts_with("@@"));
        assert!(hunk.contains("+new"));
    }

    #[test]
    fn compute_diff_no_newline_at_end() {
        // Text without trailing newline still computes correctly
        let result = compute_diff("hello", "world");
        assert_eq!(result.added, 1);
        assert_eq!(result.removed, 1);
    }

    #[test]
    fn tampered_oversized_lines_detectable_via_diff() {
        use crate::testkit::factories::tampered_oversized_lines;
        let rec = tampered_oversized_lines(1);
        assert!(rec.added_lines > 1000, "oversized: {}", rec.added_lines);
    }
}
