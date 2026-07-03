use aitrack::cli::Cli;
use clap::Parser;

#[tokio::main]
async fn main() {
    // prompt-capture is invoked silently by hooks — skip banner to avoid
    // polluting hook stdout with decoration.
    // update also skips the banner so progress output is clean.
    let mut args = std::env::args().skip(1);
    let first_arg = args.next();
    let second_arg = args.next();
    let skip_banner = first_arg
        .as_deref()
        .map(|a| {
            a == "prompt-capture"
                || a == "update"
                || (a == "usage" && second_arg.as_deref() == Some("probe"))
        })
        .unwrap_or(false);
    if !skip_banner {
        aitrack::print_banner();
    }

    let cli = Cli::parse();
    if let Err(e) = aitrack::run(cli).await {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
