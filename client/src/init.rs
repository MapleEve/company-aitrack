use anyhow::{Context, Result};
use serde_json::Value;
use std::fs;
use std::path::Path;

const COMMENT_MARKER: &str = "# aitrack";

pub fn install_hooks(tools: &[&str], aitrack_bin: &str, home: &Path) -> Result<()> {
    for tool in tools {
        match *tool {
            "claude" => {
                install_claude_hook(&home.join(".claude").join("settings.json"), aitrack_bin)?
            }
            "codex" => install_codex_hook(&home.join(".codex").join("config.toml"), aitrack_bin)?,
            "cursor" => install_cursor_hook(&home.join(".cursor").join("hooks.json"), aitrack_bin)?,
            _ => {}
        }
    }
    Ok(())
}

pub fn remove_hooks(tools: &[&str], home: &Path) -> Result<()> {
    for tool in tools {
        match *tool {
            "claude" => remove_claude_hook(&home.join(".claude").join("settings.json"))?,
            "codex" => remove_codex_hook(&home.join(".codex").join("config.toml"))?,
            "cursor" => remove_cursor_hook(&home.join(".cursor").join("hooks.json"))?,
            _ => {}
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Claude Code: ~/.claude/settings.json
// ---------------------------------------------------------------------------

pub fn has_claude_hook(path: &Path) -> bool {
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(val) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    val["hooks"]["PostToolUse"]
        .as_array()
        .map(|arr| {
            arr.iter().any(|entry| {
                entry["hooks"]
                    .as_array()
                    .map(|hooks| {
                        hooks.iter().any(|h| {
                            h["command"]
                                .as_str()
                                .map(|c| c.contains("aitrack"))
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

pub fn has_claude_prompt_hook(path: &Path) -> bool {
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(val) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    val["hooks"]["UserPromptSubmit"]
        .as_array()
        .map(|arr| {
            arr.iter().any(|entry| {
                entry["hooks"]
                    .as_array()
                    .map(|hooks| {
                        hooks.iter().any(|h| {
                            h["command"]
                                .as_str()
                                .map(|c| c.contains("aitrack prompt-capture"))
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

pub fn install_claude_hook(path: &Path, aitrack_bin: &str) -> Result<()> {
    if has_claude_hook(path) && has_claude_prompt_hook(path) {
        return Ok(());
    }

    // Warn if another tool has already registered a PostToolUse hook.
    if path.exists() && check_claude_third_party_conflict(path) {
        eprintln!(
            "[aitrack] warning: ~/.claude/settings.json already contains a PostToolUse hook from another tool; aitrack will be added alongside it"
        );
    }

    let mut val = if path.exists() {
        let text = fs::read_to_string(path).context("read settings.json")?;
        serde_json::from_str::<Value>(&text).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        serde_json::json!({})
    };

    if !has_claude_hook(path) {
        let new_entry = serde_json::json!({
            "matcher": "apply_patch|Edit|Write",
            "hooks": [
                {
                    "type": "command",
                    "command": format!("{aitrack_bin} capture --tool claude"),
                    "timeout": 10
                }
            ]
        });

        let hooks = val
            .as_object_mut()
            .unwrap()
            .entry("hooks")
            .or_insert_with(|| serde_json::json!({}));
        let post_tool_use = hooks
            .as_object_mut()
            .unwrap()
            .entry("PostToolUse")
            .or_insert_with(|| serde_json::json!([]));

        if let Some(arr) = post_tool_use.as_array_mut() {
            arr.push(new_entry);
        }
    }

    if !has_claude_prompt_hook(path) {
        let prompt_entry = serde_json::json!({
            "hooks": [
                {
                    "type": "command",
                    "command": format!("{aitrack_bin} prompt-capture --tool claude"),
                    "timeout": 10
                }
            ]
        });

        let hooks = val
            .as_object_mut()
            .unwrap()
            .entry("hooks")
            .or_insert_with(|| serde_json::json!({}));
        let user_prompt_submit = hooks
            .as_object_mut()
            .unwrap()
            .entry("UserPromptSubmit")
            .or_insert_with(|| serde_json::json!([]));

        if let Some(arr) = user_prompt_submit.as_array_mut() {
            arr.push(prompt_entry);
        }
    }

    let text = serde_json::to_string_pretty(&val)?;
    fs::write(path, text).context("write settings.json")?;
    Ok(())
}

pub fn remove_claude_hook(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let text = fs::read_to_string(path)?;
    let mut val: Value = serde_json::from_str(&text).unwrap_or_else(|_| serde_json::json!({}));

    if let Some(arr) = val["hooks"]["PostToolUse"].as_array_mut() {
        arr.retain(|entry| {
            !entry["hooks"]
                .as_array()
                .map(|hooks| {
                    hooks.iter().any(|h| {
                        h["command"]
                            .as_str()
                            .map(|c| c.contains("aitrack"))
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false)
        });
        // Clean up empty PostToolUse array
        if arr.is_empty() {
            if let Some(hooks) = val["hooks"].as_object_mut() {
                hooks.remove("PostToolUse");
            }
        }
    }

    if let Some(arr) = val["hooks"]["UserPromptSubmit"].as_array_mut() {
        arr.retain(|entry| {
            !entry["hooks"]
                .as_array()
                .map(|hooks| {
                    hooks.iter().any(|h| {
                        h["command"]
                            .as_str()
                            .map(|c| c.contains("aitrack prompt-capture"))
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false)
        });
        // Clean up empty UserPromptSubmit array
        if arr.is_empty() {
            if let Some(hooks) = val["hooks"].as_object_mut() {
                hooks.remove("UserPromptSubmit");
            }
        }
    }

    let text = serde_json::to_string_pretty(&val)?;
    fs::write(path, text)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Codex CLI: ~/.codex/config.toml
// ---------------------------------------------------------------------------

pub fn has_codex_hook(path: &Path) -> bool {
    fs::read_to_string(path)
        .map(|text| text.contains(COMMENT_MARKER))
        .unwrap_or(false)
}

pub fn install_codex_hook(path: &Path, aitrack_bin: &str) -> Result<()> {
    if has_codex_hook(path) {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Escape backslashes and double-quotes so the binary path embeds safely in a
    // TOML double-quoted string (relevant on Windows or unusual install paths).
    let bin_escaped = aitrack_bin.replace('\\', "\\\\").replace('"', "\\\"");
    let snippet = format!(
        "\n{COMMENT_MARKER}\n[[hooks.PostToolUse]]\nmatcher = \"apply_patch|Edit|Write\"\n\n[[hooks.PostToolUse.hooks]]\ntype = \"command\"\ncommand = \"{bin_escaped} capture --tool codex\"\ntimeout = 10\n"
    );

    let existing = if path.exists() {
        fs::read_to_string(path)?
    } else {
        String::new()
    };

    fs::write(path, format!("{existing}{snippet}")).context("write codex config.toml")?;
    Ok(())
}

pub fn remove_codex_hook(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let text = fs::read_to_string(path)?;
    let cleaned = remove_codex_block(&text);
    fs::write(path, cleaned)?;
    Ok(())
}

fn remove_codex_block(text: &str) -> String {
    // Remove the block starting with COMMENT_MARKER until the next blank line pair
    let marker = COMMENT_MARKER;
    if let Some(start) = text.find(marker) {
        let before = &text[..start];
        let after = &text[start..];

        // Find end: look for end of the [[hooks.PostToolUse.hooks]] block
        // We'll find the next occurrence of a non-related section or end of string
        let lines_after: Vec<&str> = after.lines().collect();
        let mut end_line = lines_after.len();

        // Skip the comment marker block: everything from the marker until
        // we see a line that isn't part of this block
        let mut in_block = false;
        let mut _last_block_line = 0;

        for (i, line) in lines_after.iter().enumerate() {
            if line.trim() == marker.trim() {
                in_block = true;
                _last_block_line = i;
                continue;
            }
            if in_block {
                if line.starts_with("[[hooks.PostToolUse")
                    || line.starts_with("matcher")
                    || line.starts_with("type")
                    || line.starts_with("command")
                    || line.starts_with("timeout")
                    || line.trim().is_empty()
                {
                    _last_block_line = i;
                } else {
                    end_line = i;
                    break;
                }
            }
        }

        let remaining: Vec<&str> = lines_after[end_line..].to_vec();
        let before_trimmed = before.trim_end_matches('\n');
        let remaining_text = remaining.join("\n");

        if remaining_text.trim().is_empty() {
            format!("{before_trimmed}\n")
        } else {
            format!("{before_trimmed}\n{remaining_text}\n")
        }
    } else {
        text.to_string()
    }
}

// ---------------------------------------------------------------------------
// Cursor: ~/.cursor/hooks.json
// ---------------------------------------------------------------------------

pub fn has_cursor_hook(path: &Path) -> bool {
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(val) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    // Check either registration point — both are installed together
    let check_array = |key: &str| -> bool {
        val["hooks"][key]
            .as_array()
            .map(|arr| {
                arr.iter().any(|entry| {
                    entry["command"]
                        .as_str()
                        .map(|c| c.contains("aitrack"))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    };
    check_array("afterFileEdit") || check_array("postToolUse")
}

pub fn install_cursor_hook(path: &Path, aitrack_bin: &str) -> Result<()> {
    if has_cursor_hook(path) {
        return Ok(());
    }

    let mut val = if path.exists() {
        let text = fs::read_to_string(path)?;
        serde_json::from_str::<Value>(&text).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        serde_json::json!({})
    };

    let new_entry = serde_json::json!({
        "command": format!("{aitrack_bin} capture --tool cursor"),
        "matcher": "Write",
        "timeout": 10
    });

    // Ensure hooks object exists
    if val.as_object_mut().unwrap().get("hooks").is_none() {
        val["hooks"] = serde_json::json!({});
    }

    // Register in postToolUse (catches tool-use events)
    {
        let post_tool_use = val["hooks"]
            .as_object_mut()
            .unwrap()
            .entry("postToolUse")
            .or_insert_with(|| serde_json::json!([]));
        if let Some(arr) = post_tool_use.as_array_mut() {
            arr.push(new_entry.clone());
        }
    }

    // Register in afterFileEdit (catches file-edit events)
    {
        let after_file_edit = val["hooks"]
            .as_object_mut()
            .unwrap()
            .entry("afterFileEdit")
            .or_insert_with(|| serde_json::json!([]));
        if let Some(arr) = after_file_edit.as_array_mut() {
            arr.push(new_entry);
        }
    }

    let text = serde_json::to_string_pretty(&val)?;
    fs::write(path, text)?;
    Ok(())
}

pub fn remove_cursor_hook(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let text = fs::read_to_string(path)?;
    let mut val: Value = serde_json::from_str(&text).unwrap_or_else(|_| serde_json::json!({}));

    if let Some(arr) = val["hooks"]["afterFileEdit"].as_array_mut() {
        arr.retain(|entry| {
            !entry["command"]
                .as_str()
                .map(|c| c.contains("aitrack"))
                .unwrap_or(false)
        });
        if arr.is_empty() {
            if let Some(hooks) = val["hooks"].as_object_mut() {
                hooks.remove("afterFileEdit");
            }
        }
    }

    if let Some(arr) = val["hooks"]["postToolUse"].as_array_mut() {
        arr.retain(|entry| {
            !entry["command"]
                .as_str()
                .map(|c| c.contains("aitrack"))
                .unwrap_or(false)
        });
        if arr.is_empty() {
            if let Some(hooks) = val["hooks"].as_object_mut() {
                hooks.remove("postToolUse");
            }
        }
    }

    let text = serde_json::to_string_pretty(&val)?;
    fs::write(path, text)?;
    Ok(())
}

pub fn detect_tool_statuses(home: &Path) -> (bool, bool, bool) {
    (
        has_claude_hook(&home.join(".claude").join("settings.json")),
        has_codex_hook(&home.join(".codex").join("config.toml")),
        has_cursor_hook(&home.join(".cursor").join("hooks.json")),
    )
}

/// Detect which AI coding tools appear to be installed on this machine.
///
/// Uses presence of the tool's config *directory* as the signal — more reliable
/// than checking for the hook itself (which may not be installed yet).
///
/// Returns a list of tool name strings that can be passed to `install_hooks`.
pub fn detect_installed_tools(home: &Path) -> Vec<&'static str> {
    let mut tools = Vec::new();
    if home.join(".claude").exists() {
        tools.push("claude");
    }
    if home.join(".codex").exists() {
        tools.push("codex");
    }
    if home.join(".cursor").exists() {
        tools.push("cursor");
    }
    tools
}

/// Returns `true` if `settings.json` already contains a PostToolUse hook command
/// that does NOT belong to aitrack.
///
/// Used to warn the user that a third-party tool has registered a conflicting
/// hook before aitrack installs its own entry.
pub fn check_claude_third_party_conflict(path: &Path) -> bool {
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(val) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    val["hooks"]["PostToolUse"]
        .as_array()
        .map(|arr| {
            arr.iter().any(|entry| {
                entry["hooks"]
                    .as_array()
                    .map(|hooks| {
                        hooks.iter().any(|h| {
                            h["command"]
                                .as_str()
                                .map(|c| !c.contains("aitrack"))
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_home() -> TempDir {
        TempDir::new().unwrap()
    }

    // ---------------------------------------------------------------------------
    // Claude hook tests
    // ---------------------------------------------------------------------------

    #[test]
    fn claude_install_creates_hook_entry() {
        let home = setup_home();
        let path = home.path().join(".claude").join("settings.json");
        install_claude_hook(&path, "/usr/local/bin/aitrack").unwrap();
        assert!(has_claude_hook(&path), "hook should be installed");
    }

    #[test]
    fn claude_install_is_idempotent() {
        let home = setup_home();
        let path = home.path().join(".claude").join("settings.json");
        install_claude_hook(&path, "/usr/local/bin/aitrack").unwrap();
        install_claude_hook(&path, "/usr/local/bin/aitrack").unwrap();
        // Count entries — should be exactly 1
        let text = std::fs::read_to_string(&path).unwrap();
        let count = text.matches("aitrack").count();
        // Two occurrences: one in command, one potentially in content-type
        // At minimum there should only be one hook entry
        let val: serde_json::Value = serde_json::from_str(&text).unwrap();
        let arr = val["hooks"]["PostToolUse"].as_array().unwrap();
        assert_eq!(arr.len(), 1, "idempotent install: only 1 entry");
        let _ = count;
    }

    #[test]
    fn claude_remove_cleans_hook() {
        let home = setup_home();
        let path = home.path().join(".claude").join("settings.json");
        install_claude_hook(&path, "/usr/local/bin/aitrack").unwrap();
        assert!(has_claude_hook(&path));
        remove_claude_hook(&path).unwrap();
        assert!(!has_claude_hook(&path), "hook should be removed");
    }

    #[test]
    fn claude_remove_cleans_empty_post_tool_use() {
        let home = setup_home();
        let path = home.path().join(".claude").join("settings.json");
        install_claude_hook(&path, "/usr/local/bin/aitrack").unwrap();
        remove_claude_hook(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let val: serde_json::Value = serde_json::from_str(&text).unwrap();
        // PostToolUse key should be removed when empty
        assert!(
            val["hooks"]["PostToolUse"].is_null(),
            "empty PostToolUse should be removed"
        );
    }

    #[test]
    fn claude_hook_absent_when_file_missing() {
        let home = setup_home();
        let path = home.path().join(".claude").join("settings.json");
        assert!(!has_claude_hook(&path));
    }

    #[test]
    fn claude_install_on_existing_settings_merges() {
        let home = setup_home();
        let path = home.path().join(".claude").join("settings.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        // Pre-existing settings with unrelated content
        std::fs::write(&path, r#"{"other_key": true}"#).unwrap();
        install_claude_hook(&path, "/usr/local/bin/aitrack").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let val: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            val["other_key"],
            serde_json::Value::Bool(true),
            "existing keys preserved"
        );
        assert!(has_claude_hook(&path));
    }

    #[test]
    fn claude_remove_nonexistent_file_is_noop() {
        let home = setup_home();
        let path = home.path().join(".claude").join("settings.json");
        // Should not error
        remove_claude_hook(&path).unwrap();
    }

    // ---------------------------------------------------------------------------
    // Claude prompt hook tests
    // ---------------------------------------------------------------------------

    #[test]
    fn claude_prompt_hook_install_and_detect() {
        let home = setup_home();
        let path = home.path().join(".claude").join("settings.json");
        install_claude_hook(&path, "/usr/local/bin/aitrack").unwrap();
        assert!(
            has_claude_prompt_hook(&path),
            "prompt hook should be installed"
        );
        let text = std::fs::read_to_string(&path).unwrap();
        let val: serde_json::Value = serde_json::from_str(&text).unwrap();
        let arr = val["hooks"]["UserPromptSubmit"].as_array().unwrap();
        assert_eq!(arr.len(), 1, "exactly 1 UserPromptSubmit entry");
        let cmd = arr[0]["hooks"][0]["command"].as_str().unwrap();
        assert!(
            cmd.contains("prompt-capture"),
            "command should contain prompt-capture"
        );
    }

    #[test]
    fn claude_prompt_hook_install_is_idempotent() {
        let home = setup_home();
        let path = home.path().join(".claude").join("settings.json");
        install_claude_hook(&path, "/usr/local/bin/aitrack").unwrap();
        install_claude_hook(&path, "/usr/local/bin/aitrack").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let val: serde_json::Value = serde_json::from_str(&text).unwrap();
        let arr = val["hooks"]["UserPromptSubmit"].as_array().unwrap();
        assert_eq!(arr.len(), 1, "idempotent: only 1 UserPromptSubmit entry");
    }

    #[test]
    fn claude_remove_cleans_prompt_hook_too() {
        let home = setup_home();
        let path = home.path().join(".claude").join("settings.json");
        install_claude_hook(&path, "/usr/local/bin/aitrack").unwrap();
        assert!(has_claude_prompt_hook(&path));
        remove_claude_hook(&path).unwrap();
        assert!(
            !has_claude_prompt_hook(&path),
            "prompt hook should be removed"
        );
        let text = std::fs::read_to_string(&path).unwrap();
        let val: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(
            val["hooks"]["UserPromptSubmit"].is_null(),
            "empty UserPromptSubmit should be removed"
        );
    }

    // ---------------------------------------------------------------------------
    // Codex hook tests
    // ---------------------------------------------------------------------------

    #[test]
    fn codex_install_creates_hook_block() {
        let home = setup_home();
        let path = home.path().join(".codex").join("config.toml");
        install_codex_hook(&path, "/usr/local/bin/aitrack").unwrap();
        assert!(has_codex_hook(&path));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("aitrack"));
        assert!(text.contains("PostToolUse"));
    }

    #[test]
    fn codex_install_is_idempotent() {
        let home = setup_home();
        let path = home.path().join(".codex").join("config.toml");
        install_codex_hook(&path, "/usr/local/bin/aitrack").unwrap();
        install_codex_hook(&path, "/usr/local/bin/aitrack").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        // "# aitrack" marker should appear exactly once
        assert_eq!(text.matches("# aitrack").count(), 1);
    }

    #[test]
    fn codex_remove_cleans_hook() {
        let home = setup_home();
        let path = home.path().join(".codex").join("config.toml");
        install_codex_hook(&path, "/usr/local/bin/aitrack").unwrap();
        assert!(has_codex_hook(&path));
        remove_codex_hook(&path).unwrap();
        assert!(!has_codex_hook(&path));
    }

    #[test]
    fn codex_hook_absent_when_file_missing() {
        let home = setup_home();
        let path = home.path().join(".codex").join("config.toml");
        assert!(!has_codex_hook(&path));
    }

    #[test]
    fn codex_install_appends_to_existing_config() {
        let home = setup_home();
        let path = home.path().join(".codex").join("config.toml");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "[settings]\nsome_key = true\n").unwrap();
        install_codex_hook(&path, "/usr/local/bin/aitrack").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("some_key = true"),
            "existing config preserved"
        );
        assert!(has_codex_hook(&path));
    }

    #[test]
    fn codex_remove_nonexistent_file_is_noop() {
        let home = setup_home();
        let path = home.path().join(".codex").join("config.toml");
        remove_codex_hook(&path).unwrap();
    }

    // ---------------------------------------------------------------------------
    // Cursor hook tests
    // ---------------------------------------------------------------------------

    #[test]
    fn cursor_install_creates_hook_entry() {
        let home = setup_home();
        let path = home.path().join(".cursor").join("hooks.json");
        install_cursor_hook(&path, "/usr/local/bin/aitrack").unwrap();
        assert!(has_cursor_hook(&path));
        // Verify both registration points are populated
        let text = std::fs::read_to_string(&path).unwrap();
        let val: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(
            val["hooks"]["postToolUse"].as_array().is_some(),
            "postToolUse array should exist"
        );
        assert!(
            val["hooks"]["afterFileEdit"].as_array().is_some(),
            "afterFileEdit array should exist"
        );
        // Verify matcher and timeout fields are present
        let entry = &val["hooks"]["afterFileEdit"][0];
        assert_eq!(entry["matcher"], "Write", "matcher field should be Write");
        assert_eq!(entry["timeout"], 10, "timeout field should be 10");
        let entry = &val["hooks"]["postToolUse"][0];
        assert_eq!(entry["matcher"], "Write", "matcher field should be Write");
        assert_eq!(entry["timeout"], 10, "timeout field should be 10");
    }

    #[test]
    fn cursor_install_is_idempotent() {
        let home = setup_home();
        let path = home.path().join(".cursor").join("hooks.json");
        install_cursor_hook(&path, "/usr/local/bin/aitrack").unwrap();
        install_cursor_hook(&path, "/usr/local/bin/aitrack").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let val: serde_json::Value = serde_json::from_str(&text).unwrap();
        let arr = val["hooks"]["afterFileEdit"].as_array().unwrap();
        assert_eq!(arr.len(), 1, "idempotent: only 1 afterFileEdit hook entry");
        let arr = val["hooks"]["postToolUse"].as_array().unwrap();
        assert_eq!(arr.len(), 1, "idempotent: only 1 postToolUse hook entry");
    }

    #[test]
    fn cursor_remove_cleans_hook() {
        let home = setup_home();
        let path = home.path().join(".cursor").join("hooks.json");
        install_cursor_hook(&path, "/usr/local/bin/aitrack").unwrap();
        assert!(has_cursor_hook(&path));
        remove_cursor_hook(&path).unwrap();
        assert!(!has_cursor_hook(&path));
    }

    #[test]
    fn cursor_remove_cleans_empty_after_file_edit() {
        let home = setup_home();
        let path = home.path().join(".cursor").join("hooks.json");
        install_cursor_hook(&path, "/usr/local/bin/aitrack").unwrap();
        remove_cursor_hook(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let val: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(
            val["hooks"]["afterFileEdit"].is_null(),
            "empty afterFileEdit should be removed"
        );
        assert!(
            val["hooks"]["postToolUse"].is_null(),
            "empty postToolUse should be removed"
        );
    }

    #[test]
    fn cursor_hook_absent_when_file_missing() {
        let home = setup_home();
        let path = home.path().join(".cursor").join("hooks.json");
        assert!(!has_cursor_hook(&path));
    }

    #[test]
    fn cursor_remove_nonexistent_file_is_noop() {
        let home = setup_home();
        let path = home.path().join(".cursor").join("hooks.json");
        remove_cursor_hook(&path).unwrap();
    }

    // ---------------------------------------------------------------------------
    // detect_tool_statuses
    // ---------------------------------------------------------------------------

    #[test]
    fn detect_tool_statuses_all_false_when_none_installed() {
        let home = setup_home();
        let (claude, codex, cursor) = detect_tool_statuses(home.path());
        assert!(!claude);
        assert!(!codex);
        assert!(!cursor);
    }

    #[test]
    fn detect_tool_statuses_reflects_installed_tools() {
        let home = setup_home();
        let claude_path = home.path().join(".claude").join("settings.json");
        install_claude_hook(&claude_path, "/usr/local/bin/aitrack").unwrap();

        let (claude, codex, cursor) = detect_tool_statuses(home.path());
        assert!(claude, "claude should be detected");
        assert!(!codex);
        assert!(!cursor);
    }

    // ---------------------------------------------------------------------------
    // install_hooks / remove_hooks orchestration
    // ---------------------------------------------------------------------------

    #[test]
    fn install_hooks_multiple_tools() {
        let home = setup_home();
        install_hooks(&["claude", "cursor"], "/usr/local/bin/aitrack", home.path()).unwrap();
        let (claude, _codex, cursor) = detect_tool_statuses(home.path());
        assert!(claude);
        assert!(cursor);
    }

    #[test]
    fn remove_hooks_multiple_tools() {
        let home = setup_home();
        install_hooks(
            &["claude", "codex", "cursor"],
            "/usr/local/bin/aitrack",
            home.path(),
        )
        .unwrap();
        remove_hooks(&["claude", "codex", "cursor"], home.path()).unwrap();
        let (claude, codex, cursor) = detect_tool_statuses(home.path());
        assert!(!claude);
        assert!(!codex);
        assert!(!cursor);
    }
}
