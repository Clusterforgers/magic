//! The `mtg-cli` binary: parses the command line, runs it, prints the summary and sets the exit code.

use std::process::ExitCode;

use clap::Parser;
use mtg_cli::args::Cli;
use mtg_cli::progress::Progress;

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match mtg_cli::run(cli, Progress::stderr()).await {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
