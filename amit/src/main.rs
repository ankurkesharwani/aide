mod agent;
mod job;
mod logging;
mod pickup;
mod poll;
mod runtime;
mod scanner;
mod status_writer;
mod temp;
mod tmux;
mod validator;
mod watcher;

use std::path::PathBuf;

use clap::Parser;

/// amit — filesystem-driven agent orchestrator watcher.
#[derive(Parser)]
#[command(name = "amit")]
struct Cli {
    /// Path to the workspace root to watch for `task.yml` job specs.
    workspace: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    watcher::run(cli.workspace)
}
