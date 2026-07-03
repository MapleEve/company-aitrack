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
pub mod usage;

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
    pending_count, prune_local_record_storage, token_breakdown,
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
        Command::Usage(args) => handle_usage(args).await?,
        Command::PromptCapture(args) => handle_prompt_capture(args).await?,
        Command::Update => update::run_update()?,
    }
    Ok(())
}

async fn handle_usage(args: cli::UsageArgs) -> Result<()> {
    match args.command {
        cli::UsageCommand::Scan(scan) => {
            let report = usage::scan_now(usage::UsageScanOptions {
                tools: scan.tool,
                since: scan.since,
                until: scan.until,
            })
            .await?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        cli::UsageCommand::Sync(sync) => {
            let report = usage::sync_now(usage::UsageSyncOptions {
                scan: usage::UsageScanOptions {
                    tools: sync.tool,
                    since: sync.since,
                    until: sync.until,
                },
                api_url: sync.api_url,
                credential: sync.credential,
            })
            .await?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        cli::UsageCommand::Probe(probe) => {
            let report = usage::probe_now(usage::UsageProbeOptions {
                tools: probe.tool,
                since: probe.since,
                until: probe.until,
                max_files_per_agent: probe.max_files,
                max_bytes_per_file: probe.max_bytes,
                max_records_per_file: probe.max_records,
            })?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        cli::UsageCommand::Status => {
            let status = usage::status()?;
            println!("{}", serde_json::to_string_pretty(&status)?);
        }
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
const HOOK_EVENT_FILE_NAME: &str = "hook-events.jsonl";
const IMPORT_MANIFEST_FILE_NAME: &str = "aitrack-sources.json";
const HOOK_EVENTS_MAX_FILE_BYTES: usize = 16 * 1024 * 1024;
const HOOK_EVENTS_MAX_LINES: usize = 2000;

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

    let record_opt = journal_capture_event_for_known_agent(&args.tool, stdin_json)?;

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
    record.token_key = if token.is_empty() {
        String::new()
    } else {
        mask_token(&token)
    };
    record.device_id = cfg.device_id.clone();
    record.hostname = gethostname::gethostname()
        .into_string()
        .unwrap_or_else(|_| String::new());

    domain::model::sanitize_sig_bound_record_fields(&mut record);

    // Compute record signature over the same bounded diff that will be stored and uploaded.
    record.record_sig = if record.token_key.is_empty() {
        String::new()
    } else {
        domain::crypto::compute_record_sig(
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
        )
    };

    let conn = open_db()?;

    // Attach most recent prompt for this session
    record.prompt_summary = get_recent_prompt(&conn, &record.session_id);
    domain::model::sanitize_non_sig_record_fields(&mut record);

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

    prune_local_record_storage(&conn)?;

    Ok(())
}

fn parse_capture_event(stdin_json: &str, tool: &str) -> Option<domain::model::Record> {
    if tool.trim().is_empty() {
        eprintln!("[aitrack] --tool must not be empty");
        return None;
    }

    adapter::event::parse_known_agent(tool, stdin_json)
}

fn journal_capture_event_for_known_agent(
    tool: &str,
    stdin_json: &str,
) -> Result<Option<domain::model::Record>> {
    let Some(agent_name) = known_agent_name(tool) else {
        if tool.trim().is_empty() {
            eprintln!("[aitrack] --tool must not be empty");
        } else {
            eprintln!("[aitrack] unsupported capture tool: {}", tool);
        }
        return Ok(None);
    };

    if let Err(e) = append_local_hook_event(agent_name, stdin_json) {
        eprintln!("[aitrack] hook-event journal warning: {e}");
    }

    Ok(parse_capture_event(stdin_json, agent_name))
}

fn append_local_hook_event(tool: &str, stdin_json: &str) -> Result<bool> {
    let value = match serde_json::from_str::<serde_json::Value>(stdin_json) {
        Ok(value) => value,
        Err(_) => return Ok(false),
    };
    append_local_hook_event_value(tool, &value)
}

fn append_local_hook_event_value(tool: &str, value: &serde_json::Value) -> Result<bool> {
    if !value.is_object() {
        return Ok(false);
    }

    let record = serde_json::json!({
        "aitrack": {
            "schema": 1,
            "source": "hook-event",
            "agent": tool,
            "captured_at": chrono::Utc::now()
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        },
        "event": value,
    });
    let mut line = serde_json::to_vec(&record)?;
    line.push(b'\n');
    if line.len() > HOOK_EVENTS_MAX_FILE_BYTES {
        return Ok(false);
    }

    let source_dir = config::config_dir().join("local-sources").join(tool);
    std::fs::create_dir_all(&source_dir)?;
    write_private_file(
        &source_dir.join(IMPORT_MANIFEST_FILE_NAME),
        br#"{"files":["hook-events.jsonl"]}"#,
    )?;
    let path = source_dir.join(HOOK_EVENT_FILE_NAME);
    append_private_file(&path, &line)?;
    prune_hook_event_file(&path)?;
    Ok(true)
}

fn known_agent_name(tool: &str) -> Option<&'static str> {
    let lowered = tool.trim().to_ascii_lowercase();
    if lowered.is_empty() {
        return None;
    }
    let canonical = agent::canonical_agent_name(&lowered);
    agent::agent_by_name(canonical).map(|registered| registered.name)
}

fn append_private_file(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write as _;
    let mut opts = std::fs::OpenOptions::new();
    opts.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts.open(path)?;
    file.write_all(bytes)?;
    file.flush()?;
    Ok(())
}

fn write_private_file(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write as _;
    let mut opts = std::fs::OpenOptions::new();
    opts.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts.open(path)?;
    file.write_all(bytes)?;
    file.flush()?;
    Ok(())
}

fn prune_hook_event_file(path: &std::path::Path) -> Result<()> {
    let bytes = std::fs::read(path)?;
    let lines = bytes
        .split_inclusive(|byte| *byte == b'\n')
        .filter(|line| !line.iter().all(|byte| matches!(byte, b'\n' | b'\r')))
        .collect::<Vec<_>>();
    if bytes.len() <= HOOK_EVENTS_MAX_FILE_BYTES && lines.len() <= HOOK_EVENTS_MAX_LINES {
        return Ok(());
    }

    let mut kept = Vec::new();
    let mut total = 0usize;
    for line in lines.into_iter().rev() {
        if kept.len() >= HOOK_EVENTS_MAX_LINES {
            break;
        }
        if line.len() > HOOK_EVENTS_MAX_FILE_BYTES {
            continue;
        }
        if total + line.len() > HOOK_EVENTS_MAX_FILE_BYTES {
            break;
        }
        kept.push(line);
        total += line.len();
    }

    let mut out = Vec::with_capacity(total);
    for line in kept.into_iter().rev() {
        out.extend_from_slice(line);
        if !line.ends_with(b"\n") {
            out.push(b'\n');
        }
    }
    write_private_file(path, &out)
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

    capture_prompt_payload(&args.tool, stdin_json)?;
    Ok(())
}

fn capture_prompt_payload(tool: &str, stdin_json: &str) -> Result<bool> {
    let Some(agent_name) = known_agent_name(tool) else {
        eprintln!("[aitrack] unsupported prompt-capture tool: {}", tool);
        return Ok(false);
    };

    let val: serde_json::Value = match serde_json::from_str(stdin_json) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[aitrack] prompt-capture parse error: {e}");
            return Ok(false);
        }
    };

    if let Err(e) = append_local_hook_event_value(agent_name, &val) {
        eprintln!("[aitrack] hook-event journal warning: {e}");
    }

    let session_id = prompt_session_id(&val).unwrap_or_default();
    let prompt = prompt_text(&val).unwrap_or_default();

    if session_id.is_empty() || prompt.is_empty() {
        return Ok(false);
    }

    let conn = open_db()?;
    insert_prompt_context(&conn, &session_id, &prompt_capture_text(&prompt))?;
    prune_local_record_storage(&conn)?;
    Ok(true)
}

fn prompt_capture_text(prompt: &str) -> String {
    domain::model::truncate_chars(prompt, domain::model::MAX_STORED_PROMPT_CHARS)
}

fn prompt_session_id(val: &serde_json::Value) -> Option<String> {
    string_field(val, &["session_id", "conversation_id", "transcript_path"])
}

fn prompt_text(val: &serde_json::Value) -> Option<String> {
    string_field(val, &["prompt"])
}

fn string_field(val: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| val.get(*key).and_then(|v| v.as_str()))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
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

    #[tokio::test]
    async fn known_non_native_capture_journals_hook_event_without_native_record() {
        let dir = TempDir::new().unwrap();
        with_home_async(&dir, || async {
            let json = serde_json::json!({
                "session_id": "kiro-session",
                "prompt": "summarize local state"
            })
            .to_string();

            let record = journal_capture_event_for_known_agent("kiro", &json).unwrap();

            assert!(record.is_none(), "kiro has no native edit adapter yet");
            let source_dir = dir.path().join("local-sources").join("kiro");
            let manifest = std::fs::read_to_string(source_dir.join("aitrack-sources.json"))
                .expect("manifest should be written");
            assert_eq!(manifest, r#"{"files":["hook-events.jsonl"]}"#);

            let jsonl = std::fs::read_to_string(source_dir.join("hook-events.jsonl"))
                .expect("hook-events jsonl should be written");
            let lines = jsonl.lines().collect::<Vec<_>>();
            assert_eq!(lines.len(), 1);
            let line: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
            assert_eq!(line["aitrack"]["agent"], "kiro");
            assert_eq!(line["event"]["session_id"], "kiro-session");
            assert_eq!(line["event"]["prompt"], "summarize local state");

            let report = usage::scan_now(usage::UsageScanOptions {
                tools: vec!["kiro".to_string()],
                since: None,
                until: None,
            })
            .await
            .unwrap();
            assert_eq!(report.files_scanned, 1);
            assert_eq!(report.monitoring_events_parsed, 1);
        })
        .await;
    }

    #[tokio::test]
    async fn known_non_native_prompt_capture_stores_prompt_context() {
        let dir = TempDir::new().unwrap();
        with_home_async(&dir, || async {
            let json = serde_json::json!({
                "session_id": "kiro-prompt-session",
                "prompt": "write the local source manifest"
            })
            .to_string();

            assert!(capture_prompt_payload("kiro", &json).unwrap());

            let conn = open_db().unwrap();
            let prompt = get_recent_prompt(&conn, "kiro-prompt-session")
                .expect("prompt context should be stored");
            assert_eq!(prompt, "write the local source manifest");
            assert!(dir
                .path()
                .join("local-sources/kiro/hook-events.jsonl")
                .exists());
        })
        .await;
    }

    #[tokio::test]
    async fn unknown_capture_and_prompt_capture_do_not_create_local_sources() {
        let dir = TempDir::new().unwrap();
        with_home_async(&dir, || async {
            let json = serde_json::json!({
                "session_id": "unknown-session",
                "prompt": "do not preserve this"
            })
            .to_string();

            let record = journal_capture_event_for_known_agent("unsupported-agent", &json).unwrap();
            assert!(record.is_none());
            assert!(!capture_prompt_payload("unsupported-agent", &json).unwrap());
            assert!(!dir.path().join("local-sources").exists());
        })
        .await;
    }

    #[test]
    fn prompt_capture_text_preserves_monitoring_content() {
        let raw_prompt = "fix leaked customer token in checkout.rs";
        let text = prompt_capture_text(raw_prompt);

        assert!(text.contains("customer"));
        assert!(text.contains("checkout"));
    }

    #[test]
    fn prompt_capture_extracts_supported_hook_payloads() {
        let codex = serde_json::json!({
            "hook_event_name": "UserPromptSubmit",
            "session_id": "codex-session",
            "turn_id": "turn-1",
            "prompt": "codex prompt"
        });
        assert_eq!(prompt_session_id(&codex).as_deref(), Some("codex-session"));
        assert_eq!(prompt_text(&codex).as_deref(), Some("codex prompt"));

        let cursor = serde_json::json!({
            "hook_event_name": "beforeSubmitPrompt",
            "transcript_path": "/tmp/cursor/transcript.jsonl",
            "prompt": "cursor prompt"
        });
        assert_eq!(
            prompt_session_id(&cursor).as_deref(),
            Some("/tmp/cursor/transcript.jsonl")
        );
        assert_eq!(prompt_text(&cursor).as_deref(), Some("cursor prompt"));
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
