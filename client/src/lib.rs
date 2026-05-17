pub mod adapters;
pub mod cli;
pub mod config;
pub mod crypto;
pub mod db;
pub mod diff;
pub mod git;
pub mod heartbeat;
pub mod init;
pub mod uploader;
#[cfg(test)]
pub mod testkit;

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
use std::io::Read;

use cli::{Cli, Command};
use config::{apply_init_args, load_config, mask_token, resolve_api_config};
use db::{clean_all, clean_synced, inspect_records, open_db, pending_count, token_breakdown};
use init::{detect_tool_statuses, install_hooks, remove_hooks};

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
    }
    Ok(())
}

async fn handle_init(args: cli::InitArgs) -> Result<()> {
    let cfg = apply_init_args(args.api_url, args.api_token, args.hmac_secret)?;

    let tools: Vec<&str> = {
        let mut t = Vec::new();
        if args.claude {
            t.push("claude");
        }
        if args.codex {
            t.push("codex");
        }
        if args.cursor {
            t.push("cursor");
        }
        t
    };

    if tools.is_empty() {
        println!("No tools selected. Use --claude, --codex, or --cursor.");
        return Ok(());
    }

    let home = dirs::home_dir().expect("cannot find home directory");
    let aitrack_bin = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "aitrack".to_string());

    install_hooks(&tools, &aitrack_bin, &home)?;

    let (claude, codex, cursor) = detect_tool_statuses(&home);
    println!("Hook installation complete:");
    println!("  claude: {}", if claude { "installed" } else { "not installed" });
    println!("  codex:  {}", if codex { "installed" } else { "not installed" });
    println!("  cursor: {}", if cursor { "installed" } else { "not installed" });

    if !cfg.api_url.is_empty() {
        println!("API URL: {}", cfg.api_url);
    }
    if !cfg.token.is_empty() {
        println!("Token: {}", mask_token(&cfg.token));
    }
    println!("Device ID: {}", cfg.device_id);

    Ok(())
}

fn handle_remove(args: cli::RemoveArgs) -> Result<()> {
    let tools: Vec<&str> = {
        let mut t = Vec::new();
        if args.claude {
            t.push("claude");
        }
        if args.codex {
            t.push("codex");
        }
        if args.cursor {
            t.push("cursor");
        }
        t
    };

    if tools.is_empty() {
        println!("No tools selected. Use --claude, --codex, or --cursor.");
        return Ok(());
    }

    let home = dirs::home_dir().expect("cannot find home directory");
    remove_hooks(&tools, &home)?;
    println!("Hooks removed for: {}", tools.join(", "));
    Ok(())
}

async fn handle_capture(args: cli::CaptureArgs) -> Result<()> {
    let mut raw = String::new();
    std::io::stdin()
        .read_to_string(&mut raw)
        .expect("read stdin");
    let stdin_json = raw.trim();

    let record_opt = match args.tool.as_str() {
        "claude" => adapters::parse_claude(stdin_json),
        "codex" => adapters::parse_codex(stdin_json),
        "cursor" => adapters::parse_cursor(stdin_json),
        other => {
            eprintln!("[aitrack] unknown tool: {other}");
            return Ok(());
        }
    };

    let mut record = match record_opt {
        Some(r) => r,
        None => {
            eprintln!("[aitrack] adapter returned no record for tool={}", args.tool);
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
    let (api_url, token) = resolve_api_config(args.api_url, args.api_token);
    let cfg = load_config();
    record.token_key = mask_token(&token);
    record.device_id = cfg.device_id.clone();
    record.hostname = gethostname::gethostname()
        .into_string()
        .unwrap_or_else(|_| String::new());

    // Compute record signature
    record.record_sig = crypto::compute_record_sig(
        &cfg.hmac_secret,
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
    let inserted = db::insert_record(&conn, &record)?;

    if inserted && !api_url.is_empty() && !token.is_empty() {
        uploader::flush_unsynced(&conn, &api_url, &token).await?;

        // Throttled heartbeat
        heartbeat::send_heartbeat(&conn, &api_url, &token, false).await?;
    }

    Ok(())
}

fn handle_inspect(args: cli::InspectArgs) -> Result<()> {
    let limit = args.limit.min(200);
    let conn = open_db()?;
    let cfg = load_config();

    let token_key = if args.current_token {
        mask_token(&cfg.token)
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
            r.id, r.tool, model, file, r.added_lines, r.removed_lines, r.synced, r.retry_count, r.timestamp
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
    let token_key = mask_token(&cfg.token);
    let pending = pending_count(&conn, &token_key);
    let home = dirs::home_dir().expect("cannot find home directory");
    let (claude, codex, cursor) = detect_tool_statuses(&home);

    println!("API URL:      {}", if cfg.api_url.is_empty() { "(not set)" } else { &cfg.api_url });
    println!("Token:        {}", if cfg.token.is_empty() { "(not set)" } else { &token_key });
    println!("Device ID:    {}", if cfg.device_id.is_empty() { "(not set)" } else { &cfg.device_id });
    println!("Pending sync: {pending}");
    println!(
        "Hooks:        claude={} codex={} cursor={}",
        claude, codex, cursor
    );

    Ok(())
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
    let (api_url, token) = resolve_api_config(None, None);

    if api_url.is_empty() || token.is_empty() {
        eprintln!("[aitrack] api_url or token not configured");
        return Ok(());
    }

    let conn = open_db()?;
    heartbeat::send_heartbeat(&conn, &api_url, &token, true).await?;
    println!("Heartbeat sent.");
    Ok(())
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
        }).await;
    }

    // -------------------------------------------------------------------------
    // handle_inspect: empty DB, no filter
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn run_inspect_empty_db() {
        let dir = TempDir::new().unwrap();
        with_home_async(&dir, || async {
            let args = cli::InspectArgs { limit: 20, pending: false, current_token: false };
            handle_inspect(args).unwrap();
        }).await;
    }

    // -------------------------------------------------------------------------
    // handle_inspect: pending filter, current_token flag
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn run_inspect_pending_and_current_token() {
        let dir = TempDir::new().unwrap();
        with_home_async(&dir, || async {
            let args = cli::InspectArgs { limit: 10, pending: true, current_token: true };
            handle_inspect(args).unwrap();
        }).await;
    }

    // -------------------------------------------------------------------------
    // handle_status: empty config
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn run_status_empty_config() {
        let dir = TempDir::new().unwrap();
        with_home_async(&dir, || async {
            handle_status().unwrap();
        }).await;
    }

    // -------------------------------------------------------------------------
    // handle_clean --force --all
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn run_clean_force_all() {
        let dir = TempDir::new().unwrap();
        with_home_async(&dir, || async {
            handle_clean(cli::CleanArgs { all: true, force: true }).unwrap();
        }).await;
    }

    // -------------------------------------------------------------------------
    // handle_clean --force (synced-only)
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn run_clean_force_synced_only() {
        let dir = TempDir::new().unwrap();
        with_home_async(&dir, || async {
            handle_clean(cli::CleanArgs { all: false, force: true }).unwrap();
        }).await;
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
        }).await;
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
        }).await;
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
        }).await;
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
        }).await;
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
        }).await;
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
        }).await;
    }

    // -------------------------------------------------------------------------
    // handle_inspect: limit clamped to 200
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn run_inspect_limit_clamped() {
        let dir = TempDir::new().unwrap();
        with_home_async(&dir, || async {
            let args = cli::InspectArgs { limit: 500, pending: false, current_token: false };
            handle_inspect(args).unwrap();
        }).await;
    }
}
