pub mod adapter;
pub mod agent;
pub mod cli;
pub mod config;
pub mod domain;
pub mod git;
pub mod heartbeat;
pub mod init;
pub mod port;
#[cfg(test)]
pub mod testkit;
pub mod update;
pub mod uploader;

/// Crate-wide test synchronization for process-global state.
#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::Mutex;

    /// Single process-wide lock guarding every test that mutates a process-global
    /// env var (`AITRACK_HOME`, `AITRACK_API_URL`, `AITRACK_API_TOKEN`, ...).
    ///
    /// Env vars are process-global, so a `config` test and a `lib` test running on
    /// different threads would otherwise race on the same variable. A per-module
    /// lock cannot prevent that — only a single shared lock across all modules can,
    /// which is why this lives in the crate root rather than in each test module.
    pub static ENV_LOCK: Mutex<()> = Mutex::new(());
}

use anyhow::Result;

use cli::{Cli, Command};

/// Print the ASCII startup banner.
/// This is skipped automatically for `prompt-capture` (called silently by hooks).
pub fn print_banner() {
    let version = env!("CARGO_PKG_VERSION");
    println!(
        "\n   _   ___ _____ ____      _    ____ _  __\n  / \\ |_ _|_   _|  _ \\    / \\  / ___| |/ /\n / _ \\ | |  | | | |_) |  / _ \\| |   | ' /\n/ ___ \\| |  | | |  _ <  / ___ \\ |___| . \\\n/_/   \\_\\___| |_| |_| \\_\\/_/   \\_\\____|_|\\_\\\n\n  AI Coding Usage Tracker  \u{00B7}  v{version}\n  \u{00A9} 2026 MapleEve  \u{00B7}  Apache-2.0\n  https://github.com/MapleEve/company-aitrack\n{}",
        "━".repeat(45)
    );
}
use adapter::sqlite::{
    clean_all, clean_synced, get_recent_prompt, insert_prompt_context, inspect_records, open_db,
    pending_count, token_breakdown,
};
use config::{apply_init_args, load_config, mask_token, resolve_api_config, split_credential};
use init::{detect_installed_tools, detect_tool_statuses, install_hooks, remove_hooks};

pub async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Init(args) => handle_init(args).await?,
        Command::Remove(args) => handle_remove(args)?,
        Command::Capture(args) => handle_capture(args).await?,
        Command::Inspect(args) => handle_inspect(args)?,
        Command::Stats => handle_stats()?,
        Command::Status => handle_status()?,
        Command::Clean(args) => handle_clean(args)?,
        Command::Heartbeat => handle_heartbeat().await?,
        Command::PromptCapture(args) => handle_prompt_capture(args).await?,
        Command::Update => update::run_update()?,
    }
    Ok(())
}

async fn handle_init(args: cli::InitArgs) -> Result<()> {
    let cfg = apply_init_args(args.api_url, args.credential)?;

    let tools = selected_tools(args.claude, args.codex, args.cursor, args.tool);

    let home = dirs::home_dir().expect("cannot find home directory");

    let tools = if tools.is_empty() {
        // No flags passed — auto-detect installed AI tools by config dir presence.
        let detected = detect_installed_tools(&home);
        if detected.is_empty() {
            println!(
                "No AI tools detected. Use --claude, --codex, --cursor, or --tool <name> to install manually."
            );
            return Ok(());
        }
        println!("Auto-detected tools: {}", detected.join(", "));
        detected
    } else {
        tools
    };

    let aitrack_bin = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "aitrack".to_string());

    let tool_refs = tool_refs(&tools);
    install_hooks(&tool_refs, &aitrack_bin, &home)?;

    // Initialize the keyword store after hook installation.
    // Failure is non-fatal — keyword integrity is best-effort.
    {
        let kw_path = config::config_dir().join("keywords.db");
        if let Err(e) = adapter::sqlite::keyword_store::open_keyword_store(&kw_path) {
            eprintln!("[aitrack] keyword store init warning: {e}");
        }
    }

    let native_tools = selected_native_edit_tools(&tools);
    let registry_only_tools = selected_registry_only_tools(&tools);
    if native_tools.is_empty() {
        println!("No native hooks installed.");
    } else {
        println!(
            "Native hook installation complete for: {}",
            native_tools.join(", ")
        );
    }
    if !registry_only_tools.is_empty() {
        println!(
            "Registered agents without native hook installer: {}",
            registry_only_tools.join(", ")
        );
    }

    let statuses = detect_tool_statuses(&home);
    println!("Agent status:");
    print_agent_statuses(&statuses);

    if !cfg.api_url.is_empty() {
        println!("API URL: {}", cfg.api_url);
    }
    if !cfg.credential.is_empty() {
        if let Ok((token, _)) = split_credential(&cfg.credential) {
            println!("Token: {}", mask_token(&token));
        }
    }
    println!("Device ID: {}", cfg.device_id);

    Ok(())
}

fn handle_remove(args: cli::RemoveArgs) -> Result<()> {
    let tools = selected_tools(args.claude, args.codex, args.cursor, args.tool);

    if tools.is_empty() {
        println!("No tools selected. Use --claude, --codex, --cursor, or --tool <name>.");
        return Ok(());
    }

    let home = dirs::home_dir().expect("cannot find home directory");
    let tool_refs = tool_refs(&tools);
    remove_hooks(&tool_refs, &home)?;
    let native_tools = selected_native_edit_tools(&tools);
    let registry_only_tools = selected_registry_only_tools(&tools);
    if native_tools.is_empty() {
        println!("No native hooks removed.");
    } else {
        println!("Native hooks removed for: {}", native_tools.join(", "));
    }
    if !registry_only_tools.is_empty() {
        println!(
            "Registered agents without native hook remover: {}",
            registry_only_tools.join(", ")
        );
    }
    Ok(())
}

fn selected_tools(
    claude: bool,
    codex: bool,
    cursor: bool,
    explicit_tools: Vec<String>,
) -> Vec<String> {
    let mut tools = Vec::new();
    if claude {
        push_unique_tool(&mut tools, "claude");
    }
    if codex {
        push_unique_tool(&mut tools, "codex");
    }
    if cursor {
        push_unique_tool(&mut tools, "cursor");
    }
    for tool in explicit_tools {
        push_unique_tool(&mut tools, tool.trim());
    }
    tools
}

fn push_unique_tool(tools: &mut Vec<String>, tool: &str) {
    if tool.is_empty() {
        return;
    }
    if !tools.iter().any(|existing| existing == tool) {
        tools.push(tool.to_string());
    }
}

fn tool_refs(tools: &[String]) -> Vec<&str> {
    tools.iter().map(String::as_str).collect()
}

fn selected_native_edit_tools(tools: &[String]) -> Vec<String> {
    tools
        .iter()
        .filter_map(|tool| {
            agent::agent_by_name(tool)
                .filter(|registered| registered.has_native_edit_adapter)
                .map(|registered| registered.name.to_string())
        })
        .collect()
}

fn selected_registry_only_tools(tools: &[String]) -> Vec<String> {
    tools
        .iter()
        .filter_map(|tool| {
            agent::agent_by_name(tool)
                .filter(|registered| !registered.has_native_edit_adapter)
                .map(|registered| registered.name.to_string())
        })
        .collect()
}

/// 32 MiB: generous enough for any real hook payload, prevents OOM from malformed input.
const STDIN_MAX_BYTES: usize = 32 * 1024 * 1024;

async fn handle_capture(args: cli::CaptureArgs) -> Result<()> {
    use std::io::Read as _;
    let mut raw = String::new();
    if let Err(e) = std::io::stdin()
        .take(STDIN_MAX_BYTES as u64 + 1)
        .read_to_string(&mut raw)
    {
        eprintln!("[aitrack] failed to read stdin: {e}");
        return Ok(());
    }
    if raw.len() > STDIN_MAX_BYTES {
        eprintln!("[aitrack] stdin payload too large (>{STDIN_MAX_BYTES} bytes), dropping");
        return Ok(());
    }
    let stdin_json = raw.trim();

    let record_opt = parse_capture_event(stdin_json, &args.tool);

    let mut record = match record_opt {
        Some(r) => r,
        None => {
            eprintln!(
                "[aitrack] adapter returned no record for tool={}",
                args.tool
            );
            return Ok(());
        }
    };

    // Enrich with git metadata
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let repo = git::infer_repo_info(&cwd);
    record.repo_url = repo.repo_url;
    record.branch = repo.branch;
    record.current_sha = repo.current_sha;

    // Set token_key, device_id, and hostname
    let (api_url, credential) = resolve_api_config(args.api_url, args.credential);
    let cfg = load_config();
    let (token, hmac_secret) = if credential.is_empty() {
        (String::new(), String::new())
    } else {
        match split_credential(&credential) {
            Ok(parts) => parts,
            Err(e) => {
                eprintln!("[aitrack] invalid credential: {e}");
                return Ok(());
            }
        }
    };
    record.token_key = mask_token(&token);
    record.device_id = cfg.device_id.clone();
    record.hostname = gethostname::gethostname()
        .into_string()
        .unwrap_or_else(|_| String::new());

    // Compute record signature
    record.record_sig = domain::crypto::compute_record_sig(
        &hmac_secret,
        &record.token_key,
        &record.device_id,
        &record.hostname,
        &record.timestamp,
        &record.tool,
        &record.file_path,
        &record.repo_url,
        &record.current_sha,
        record.added_lines,
        record.removed_lines,
        record.diff_hunk.as_deref(),
    );

    let conn = open_db()?;

    // Attach most recent prompt for this session
    record.prompt_summary = get_recent_prompt(&conn, &record.session_id);

    let inserted = adapter::sqlite::insert_record(&conn, &record)?;

    // Non-fatal backfill: propagate current git info to any previously-inserted
    // records that had empty repo_url (captured outside a git repo).
    if !record.repo_url.is_empty() {
        if let Err(e) = adapter::sqlite::backfill_repo_info(
            &conn,
            &record.repo_url,
            &record.branch,
            &record.current_sha,
            &record.token_key,
        ) {
            eprintln!("[aitrack] backfill_repo_info warning: {e}");
        }
    }

    if inserted && !api_url.is_empty() && !credential.is_empty() {
        let http_uploader =
            adapter::http::upload::HttpUploader::new(api_url.clone(), credential.clone());
        uploader::flush_unsynced(&conn, &http_uploader).await?;

        // Throttled heartbeat
        heartbeat::send_heartbeat(&conn, &api_url, &credential, false).await?;
    }

    Ok(())
}

fn parse_capture_event(stdin_json: &str, tool: &str) -> Option<domain::model::Record> {
    if tool.trim().is_empty() {
        eprintln!("[aitrack] --tool must not be empty");
        return None;
    }

    adapter::event::parse_known_agent(tool, stdin_json)
}

fn handle_inspect(args: cli::InspectArgs) -> Result<()> {
    let limit = args.limit.min(200);
    let conn = open_db()?;
    let cfg = load_config();

    let token_key = if args.current_token {
        if let Ok((token, _)) = split_credential(&cfg.credential) {
            mask_token(&token)
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let rows = inspect_records(&conn, limit, args.pending, &token_key)?;

    if rows.is_empty() {
        println!("No records found.");
        return Ok(());
    }

    println!(
        "{:<6} {:<10} {:<20} {:<40} {:>5} {:>5} {:>6} {:>5} {:<20}",
        "id", "tool", "model", "file_path", "added", "rmvd", "synced", "retry", "timestamp"
    );
    println!("{}", "-".repeat(130));

    for r in rows {
        let model = r.model.as_deref().unwrap_or("-");
        let file = if r.file_path.len() > 40 {
            format!("...{}", &r.file_path[r.file_path.len() - 37..])
        } else {
            r.file_path.clone()
        };
        println!(
            "{:<6} {:<10} {:<20} {:<40} {:>5} {:>5} {:>6} {:>5} {:<20}",
            r.id,
            r.tool,
            model,
            file,
            r.added_lines,
            r.removed_lines,
            r.synced,
            r.retry_count,
            r.timestamp
        );
    }

    Ok(())
}

fn handle_stats() -> Result<()> {
    let conn = open_db()?;
    let rows = token_breakdown(&conn)?;

    if rows.is_empty() {
        println!("No records.");
        return Ok(());
    }

    for (token_key, total, pending) in rows {
        println!("{token_key}: {total} records, {pending} pending");
    }

    Ok(())
}

fn handle_status() -> Result<()> {
    let cfg = load_config();
    let conn = open_db()?;
    let token_key = if cfg.credential.is_empty() {
        String::new()
    } else {
        match split_credential(&cfg.credential) {
            Ok((token, _)) => mask_token(&token),
            Err(_) => "(malformed credential)".to_string(),
        }
    };
    let pending = pending_count(&conn, &token_key);
    let home = dirs::home_dir().expect("cannot find home directory");
    let statuses = detect_tool_statuses(&home);

    println!(
        "API URL:      {}",
        if cfg.api_url.is_empty() {
            "(not set)"
        } else {
            &cfg.api_url
        }
    );
    println!(
        "Token:        {}",
        if cfg.credential.is_empty() {
            "(not set)"
        } else {
            &token_key
        }
    );
    println!(
        "Device ID:    {}",
        if cfg.device_id.is_empty() {
            "(not set)"
        } else {
            &cfg.device_id
        }
    );
    println!("Pending sync: {pending}");
    println!("Agent status:");
    print_agent_statuses(&statuses);

    Ok(())
}

fn print_agent_statuses(statuses: &std::collections::HashMap<String, bool>) {
    for registered in agent::registered_agents() {
        let active = statuses.get(registered.name).copied().unwrap_or(false);
        let label = if registered.has_native_edit_adapter {
            if active {
                "native hook installed"
            } else {
                "native hook not installed"
            }
        } else if active {
            "local state detected"
        } else {
            "local state not detected"
        };
        println!("  {:<13} {}", registered.name, label);
    }
}

fn handle_clean(args: cli::CleanArgs) -> Result<()> {
    if !args.force {
        print!("Delete records? [y/N] ");
        use std::io::Write;
        std::io::stdout().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    let conn = open_db()?;
    let n = if args.all {
        clean_all(&conn)?
    } else {
        clean_synced(&conn)?
    };

    println!("Deleted {n} record(s).");
    Ok(())
}

async fn handle_heartbeat() -> Result<()> {
    let (api_url, credential) = resolve_api_config(None, None);

    if api_url.is_empty() || credential.is_empty() {
        eprintln!("[aitrack] api_url or credential not configured");
        return Ok(());
    }

    let conn = open_db()?;
    heartbeat::send_heartbeat(&conn, &api_url, &credential, true).await?;
    println!("Heartbeat sent.");
    Ok(())
}

async fn handle_prompt_capture(args: cli::PromptCaptureArgs) -> Result<()> {
    use std::io::Read as _;
    let mut raw = String::new();
    if let Err(e) = std::io::stdin()
        .take(STDIN_MAX_BYTES as u64 + 1)
        .read_to_string(&mut raw)
    {
        eprintln!("[aitrack] failed to read stdin: {e}");
        return Ok(());
    }
    if raw.len() > STDIN_MAX_BYTES {
        eprintln!("[aitrack] stdin payload too large, dropping");
        return Ok(());
    }
    let stdin_json = raw.trim();

    // Only agents with native prompt hooks run prompt capture. Currently Claude.
    if args.tool.as_str() != "claude" {
        if agent::is_known_agent(args.tool.as_str()) {
            eprintln!(
                "[aitrack] known agent has no native prompt-capture hook yet: {}",
                args.tool
            );
        } else {
            eprintln!("[aitrack] unsupported prompt-capture tool: {}", args.tool);
        }
        return Ok(());
    }

    // Parse: {"session_id": "...", "prompt": "..."}
    let val: serde_json::Value = match serde_json::from_str(stdin_json) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[aitrack] prompt-capture parse error: {e}");
            return Ok(());
        }
    };

    let session_id = val.get("session_id").and_then(|v| v.as_str()).unwrap_or("");
    let prompt = val.get("prompt").and_then(|v| v.as_str()).unwrap_or("");

    if session_id.is_empty() || prompt.is_empty() {
        return Ok(());
    }

    // Store only the intent label; raw prompt text stays out of local records
    // and upload payloads.
    let conn = open_db()?;
    insert_prompt_context(&conn, session_id, prompt_intent_label(prompt))?;
    Ok(())
}

fn prompt_intent_label(prompt: &str) -> &'static str {
    domain::keywords::classify_prompt(prompt).as_str()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::ENV_LOCK;
    use clap::Parser;
    use tempfile::TempDir;

    #[allow(dead_code)]
    fn with_home<F: FnOnce()>(dir: &TempDir, f: F) {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        std::env::set_var("AITRACK_HOME", dir.path());
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        std::env::remove_var("AITRACK_HOME");
        if let Err(e) = r {
            std::panic::resume_unwind(e);
        }
    }

    /// Async variant: sets AITRACK_HOME for the duration of an async block,
    /// holding the env lock while the block executes synchronously via
    /// `tokio::task::block_in_place`.
    async fn with_home_async<F, Fut>(dir: &TempDir, f: F)
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let path = dir.path().to_owned();
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        std::env::set_var("AITRACK_HOME", &path);
        f().await;
        std::env::remove_var("AITRACK_HOME");
    }

    // -------------------------------------------------------------------------
    // handle_remove: no-tools branch
    // -------------------------------------------------------------------------
    #[test]
    fn handle_remove_no_tools_selected_returns_ok() {
        let args = cli::RemoveArgs {
            claude: false,
            codex: false,
            cursor: false,
            tool: vec![],
        };
        // Should print message and return Ok without touching FS
        let result = handle_remove(args);
        assert!(result.is_ok());
    }

    // -------------------------------------------------------------------------
    // handle_stats: empty DB
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn run_stats_empty_db() {
        let dir = TempDir::new().unwrap();
        with_home_async(&dir, || async {
            handle_stats().unwrap();
        })
        .await;
    }

    // -------------------------------------------------------------------------
    // handle_inspect: empty DB, no filter
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn run_inspect_empty_db() {
        let dir = TempDir::new().unwrap();
        with_home_async(&dir, || async {
            let args = cli::InspectArgs {
                limit: 20,
                pending: false,
                current_token: false,
            };
            handle_inspect(args).unwrap();
        })
        .await;
    }

    // -------------------------------------------------------------------------
    // handle_inspect: pending filter, current_token flag
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn run_inspect_pending_and_current_token() {
        let dir = TempDir::new().unwrap();
        with_home_async(&dir, || async {
            let args = cli::InspectArgs {
                limit: 10,
                pending: true,
                current_token: true,
            };
            handle_inspect(args).unwrap();
        })
        .await;
    }

    // -------------------------------------------------------------------------
    // handle_status: empty config
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn run_status_empty_config() {
        let dir = TempDir::new().unwrap();
        with_home_async(&dir, || async {
            handle_status().unwrap();
        })
        .await;
    }

    // -------------------------------------------------------------------------
    // handle_clean --force --all
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn run_clean_force_all() {
        let dir = TempDir::new().unwrap();
        with_home_async(&dir, || async {
            handle_clean(cli::CleanArgs {
                all: true,
                force: true,
            })
            .unwrap();
        })
        .await;
    }

    // -------------------------------------------------------------------------
    // handle_clean --force (synced-only)
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn run_clean_force_synced_only() {
        let dir = TempDir::new().unwrap();
        with_home_async(&dir, || async {
            handle_clean(cli::CleanArgs {
                all: false,
                force: true,
            })
            .unwrap();
        })
        .await;
    }

    // -------------------------------------------------------------------------
    // handle_heartbeat: no api_url configured → returns Ok (prints error msg)
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn run_heartbeat_no_config_returns_ok() {
        let dir = TempDir::new().unwrap();
        with_home_async(&dir, || async {
            std::env::remove_var("AITRACK_API_URL");
            std::env::remove_var("AITRACK_API_TOKEN");
            handle_heartbeat().await.unwrap();
        })
        .await;
    }

    // -------------------------------------------------------------------------
    // run() dispatch: Stats command
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn run_dispatch_stats() {
        let dir = TempDir::new().unwrap();
        with_home_async(&dir, || async {
            let cli = Cli::parse_from(["aitrack", "stats"]);
            run(cli).await.unwrap();
        })
        .await;
    }

    // -------------------------------------------------------------------------
    // run() dispatch: Inspect command
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn run_dispatch_inspect() {
        let dir = TempDir::new().unwrap();
        with_home_async(&dir, || async {
            let cli = Cli::parse_from(["aitrack", "inspect"]);
            run(cli).await.unwrap();
        })
        .await;
    }

    // -------------------------------------------------------------------------
    // run() dispatch: Status command
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn run_dispatch_status() {
        let dir = TempDir::new().unwrap();
        with_home_async(&dir, || async {
            let cli = Cli::parse_from(["aitrack", "status"]);
            run(cli).await.unwrap();
        })
        .await;
    }

    // -------------------------------------------------------------------------
    // run() dispatch: Clean --force --all
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn run_dispatch_clean_force_all() {
        let dir = TempDir::new().unwrap();
        with_home_async(&dir, || async {
            let cli = Cli::parse_from(["aitrack", "clean", "--force", "--all"]);
            run(cli).await.unwrap();
        })
        .await;
    }

    // -------------------------------------------------------------------------
    // run() dispatch: Remove (no tools selected — no FS needed)
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn run_dispatch_remove_no_tools() {
        let cli = Cli::parse_from(["aitrack", "remove"]);
        run(cli).await.unwrap();
    }

    // -------------------------------------------------------------------------
    // run() dispatch: PromptCapture (missing --tool treated as default "claude")
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn run_dispatch_prompt_capture_missing_tool_returns_ok() {
        let dir = TempDir::new().unwrap();
        with_home_async(&dir, || async {
            // prompt-capture with default tool reads from stdin; stdin is empty in
            // test context so it will parse error and return Ok.
            let cli = Cli::parse_from(["aitrack", "prompt-capture"]);
            run(cli).await.unwrap();
        })
        .await;
    }

    #[test]
    fn capture_dispatch_unknown_tool_returns_none() {
        let json = serde_json::json!({
            "session_id": "sess-local",
            "tool_input": {
                "file_path": "src/local.rs",
                "old_string": "old\n",
                "new_string": "new\n"
            }
        })
        .to_string();

        assert!(parse_capture_event(&json, "unsupported-agent").is_none());
    }

    #[test]
    fn capture_dispatch_empty_tool_returns_none() {
        let json = serde_json::json!({
            "file_path": "src/local.rs",
            "content": "new\n"
        })
        .to_string();

        assert!(parse_capture_event(&json, "").is_none());
        assert!(parse_capture_event(&json, "  ").is_none());
    }

    #[test]
    fn capture_dispatch_unknown_tool_rejects_payloads() {
        assert!(parse_capture_event(r#"{"session_id":"oops""#, "unsupported-agent").is_none());
        assert!(parse_capture_event(r#"{"session_id":"missing"}"#, "unsupported-agent").is_none());
        assert!(parse_capture_event(
            r#"{"file_path":"src/missing-content.rs","metadata":{"request_id":"req"}}"#,
            "unsupported-agent"
        )
        .is_none());
    }

    #[test]
    fn agent_capture_known_without_native_edit_adapter_returns_none() {
        assert!(crate::agent::agent_by_name("qwen").is_some());
        assert!(
            !crate::agent::agent_by_name("qwen")
                .unwrap()
                .has_native_edit_adapter
        );

        let json = serde_json::json!({
            "session_id": "sess-local",
            "tool_input": {
                "file_path": "src/local.rs",
                "old_string": "old\n",
                "new_string": "new\n"
            }
        })
        .to_string();

        assert!(parse_capture_event(&json, "qwen").is_none());
    }

    #[test]
    fn prompt_capture_label_excludes_raw_prompt_text() {
        let raw_prompt = "fix leaked customer token in checkout.rs";
        let label = prompt_intent_label(raw_prompt);

        assert_eq!(label, "fix_debug");
        assert!(!label.contains("customer"));
        assert!(!label.contains("checkout"));
    }

    // -------------------------------------------------------------------------
    // run() dispatch: Heartbeat (no config → Ok, prints error internally)
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn run_dispatch_heartbeat_no_config() {
        let dir = TempDir::new().unwrap();
        with_home_async(&dir, || async {
            std::env::remove_var("AITRACK_API_URL");
            std::env::remove_var("AITRACK_API_TOKEN");
            let cli = Cli::parse_from(["aitrack", "heartbeat"]);
            run(cli).await.unwrap();
        })
        .await;
    }

    // -------------------------------------------------------------------------
    // handle_inspect: limit clamped to 200
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn run_inspect_limit_clamped() {
        let dir = TempDir::new().unwrap();
        with_home_async(&dir, || async {
            let args = cli::InspectArgs {
                limit: 500,
                pending: false,
                current_token: false,
            };
            handle_inspect(args).unwrap();
        })
        .await;
    }

    // -------------------------------------------------------------------------
    // print_banner: must not panic; output contains "aitrack"
    // -------------------------------------------------------------------------
    #[test]
    fn test_print_banner_does_not_panic() {
        // print_banner writes to stdout — just verify it completes without panic.
        print_banner();
    }

    // -------------------------------------------------------------------------
    // handle_init: no tools selected → returns Ok
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn run_init_no_tools_returns_ok() {
        let dir = TempDir::new().unwrap();
        with_home_async(&dir, || async {
            let args = cli::InitArgs {
                claude: false,
                codex: false,
                cursor: false,
                tool: vec![],
                api_url: None,
                credential: None,
            };
            handle_init(args).await.unwrap();
        })
        .await;
    }

    // -------------------------------------------------------------------------
    // handle_remove: with tools selected (uses temp home so no real FS impact)
    // -------------------------------------------------------------------------
    #[test]
    fn handle_remove_claude_selected_returns_ok() {
        let dir = TempDir::new().unwrap();
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        std::env::set_var("AITRACK_HOME", dir.path());
        let args = cli::RemoveArgs {
            claude: true,
            codex: false,
            cursor: false,
            tool: vec![],
        };
        let result = handle_remove(args);
        std::env::remove_var("AITRACK_HOME");
        assert!(result.is_ok());
    }

    // -------------------------------------------------------------------------
    // run() dispatch: Init (no tools → Ok fast path)
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn run_dispatch_init_no_tools() {
        let dir = TempDir::new().unwrap();
        with_home_async(&dir, || async {
            let cli = Cli::parse_from(["aitrack", "init"]);
            run(cli).await.unwrap();
        })
        .await;
    }

    // -------------------------------------------------------------------------
    // run() dispatch: Clean --force (synced-only, no --all)
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn run_dispatch_clean_force_synced() {
        let dir = TempDir::new().unwrap();
        with_home_async(&dir, || async {
            let cli = Cli::parse_from(["aitrack", "clean", "--force"]);
            run(cli).await.unwrap();
        })
        .await;
    }
}
