mod agent;
mod job;
mod logging;
mod pickup;
mod poll;
mod runtime;
mod scanner;
mod status_writer;
mod tmux;
mod validator;
mod watcher;

use std::path::PathBuf;

use clap::Parser;

/// aide — filesystem-driven agent orchestrator watcher.
#[derive(Parser)]
#[command(name = "aide")]
struct Cli {
    /// Path to the workspace root to watch for `aide.yml` job specs.
    workspace: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    watcher::run(cli.workspace)
}
