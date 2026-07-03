use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "aitrack",
    version,
    about = "Hardened AI coding edit telemetry CLI"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Install hooks into AI coding tools
    Init(InitArgs),
    /// Remove hooks from AI coding tools
    Remove(RemoveArgs),
    /// Hook callback: reads stdin JSON and records an edit event
    Capture(CaptureArgs),
    /// Show recent local records
    Inspect(InspectArgs),
    /// Show record stats grouped by token
    Stats,
    /// Show status (token, pending count, native hooks, registered agents)
    Status,
    /// Delete local records
    Clean(CleanArgs),
    /// Send a heartbeat to the server (forced, ignores throttle)
    Heartbeat,
    /// Scan and sync local scalar usage rollups
    Usage(UsageArgs),
    /// Hook callback: reads stdin JSON and records a user prompt
    PromptCapture(PromptCaptureArgs),
    /// Check for and install the latest release (verifies ed25519 signature)
    Update,
}

#[derive(Args)]
pub struct InitArgs {
    #[arg(long)]
    pub claude: bool,
    #[arg(long)]
    pub codex: bool,
    #[arg(long)]
    pub cursor: bool,
    #[arg(long = "tool", value_name = "name")]
    pub tool: Vec<String>,
    #[arg(long)]
    pub api_url: Option<String>,
    /// Combined credential string: "<token>-<hmac_secret>"
    #[arg(long)]
    pub credential: Option<String>,
}

#[derive(Args)]
pub struct RemoveArgs {
    #[arg(long)]
    pub claude: bool,
    #[arg(long)]
    pub codex: bool,
    #[arg(long)]
    pub cursor: bool,
    #[arg(long = "tool", value_name = "name")]
    pub tool: Vec<String>,
}

#[derive(Args)]
pub struct CaptureArgs {
    /// Source tool name.
    #[arg(short, long, default_value = "claude")]
    pub tool: String,
    #[arg(long)]
    pub api_url: Option<String>,
    /// Combined credential string: "<token>-<hmac_secret>"
    #[arg(long)]
    pub credential: Option<String>,
}

#[derive(Args)]
pub struct InspectArgs {
    #[arg(long, default_value_t = 20)]
    pub limit: i64,
    #[arg(long)]
    pub pending: bool,
    #[arg(long)]
    pub current_token: bool,
}

#[derive(Args)]
pub struct CleanArgs {
    #[arg(long)]
    pub all: bool,
    #[arg(long)]
    pub force: bool,
}

#[derive(Args)]
pub struct UsageArgs {
    #[command(subcommand)]
    pub command: UsageCommand,
}

#[derive(Subcommand)]
pub enum UsageCommand {
    /// Scan local agent usage sources into usage.sqlite without uploading
    Scan(UsageScanArgs),
    /// Scan, roll up, enqueue, and upload scalar usage data
    Sync(UsageSyncArgs),
    /// Probe local source structure safely and print a redacted JSON schema report
    Probe(UsageProbeArgs),
    /// Show local usage ledger status
    Status,
}

#[derive(Args, Clone)]
pub struct UsageScanArgs {
    /// Restrict scan to one or more agent keys; omit to scan every supported local source
    #[arg(long = "tool", value_name = "name")]
    pub tool: Vec<String>,
    /// Include records on or after YYYY-MM-DD
    #[arg(long)]
    pub since: Option<String>,
    /// Include records on or before YYYY-MM-DD
    #[arg(long)]
    pub until: Option<String>,
}

#[derive(Args, Clone)]
pub struct UsageSyncArgs {
    /// Restrict scan to one or more agent keys; omit to scan every supported local source
    #[arg(long = "tool", value_name = "name")]
    pub tool: Vec<String>,
    /// Include records on or after YYYY-MM-DD
    #[arg(long)]
    pub since: Option<String>,
    /// Include records on or before YYYY-MM-DD
    #[arg(long)]
    pub until: Option<String>,
    #[arg(long)]
    pub api_url: Option<String>,
    /// Combined credential string: "<token>-<hmac_secret>"
    #[arg(long)]
    pub credential: Option<String>,
}

#[derive(Args, Clone)]
pub struct UsageProbeArgs {
    /// Restrict probe to one or more agent keys; omit to probe schema-pending defaults
    #[arg(long = "tool", value_name = "name")]
    pub tool: Vec<String>,
    /// Include files modified on or after YYYY-MM-DD
    #[arg(long)]
    pub since: Option<String>,
    /// Include files modified on or before YYYY-MM-DD
    #[arg(long)]
    pub until: Option<String>,
    /// Maximum source files inspected per agent
    #[arg(long, default_value_t = 20)]
    pub max_files: usize,
    /// Maximum bytes read from one JSON/JSONL/SQLite source
    #[arg(long, default_value_t = 1_048_576)]
    pub max_bytes: u64,
    /// Maximum JSON/JSONL records or SQLite rows counted per source/table
    #[arg(long, default_value_t = 200)]
    pub max_records: usize,
}

#[derive(Args)]
pub struct PromptCaptureArgs {
    #[arg(short, long, default_value = "claude")]
    pub tool: String,
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command, UsageCommand};
    use clap::Parser;

    #[test]
    fn agent_init_accepts_repeatable_tool_flags() {
        let cli = Cli::parse_from(["aitrack", "init", "--tool", "trae", "--tool", "qwen"]);

        let Command::Init(args) = cli.command else {
            panic!("expected init command");
        };

        assert_eq!(args.tool, vec!["trae".to_string(), "qwen".to_string()]);
    }

    #[test]
    fn agent_remove_accepts_legacy_flags_and_tool_flags() {
        let cli = Cli::parse_from(["aitrack", "remove", "--claude", "--tool", "warp"]);

        let Command::Remove(args) = cli.command else {
            panic!("expected remove command");
        };

        assert!(args.claude);
        assert_eq!(args.tool, vec!["warp".to_string()]);
    }

    #[test]
    fn usage_scan_accepts_repeatable_tool_flags() {
        let cli = Cli::parse_from([
            "aitrack", "usage", "scan", "--tool", "codex", "--tool", "cursor",
        ]);

        let Command::Usage(args) = cli.command else {
            panic!("expected usage command");
        };
        let UsageCommand::Scan(scan) = args.command else {
            panic!("expected usage scan");
        };
        assert_eq!(scan.tool, vec!["codex".to_string(), "cursor".to_string()]);
    }

    #[test]
    fn usage_probe_accepts_limits_and_octofriend_alias() {
        let cli = Cli::parse_from([
            "aitrack",
            "usage",
            "probe",
            "--tool",
            "octofriend",
            "--max-files",
            "3",
            "--max-bytes",
            "4096",
            "--max-records",
            "7",
        ]);

        let Command::Usage(args) = cli.command else {
            panic!("expected usage command");
        };
        let UsageCommand::Probe(probe) = args.command else {
            panic!("expected usage probe");
        };
        assert_eq!(probe.tool, vec!["octofriend".to_string()]);
        assert_eq!(probe.max_files, 3);
        assert_eq!(probe.max_bytes, 4096);
        assert_eq!(probe.max_records, 7);
    }
}
