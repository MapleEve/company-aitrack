use clap::Parser;
use aitrack::cli::Cli;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(e) = aitrack::run(cli).await {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
