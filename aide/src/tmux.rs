use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

fn run(args: &[&str]) -> Result<String> {
    let output = Command::new("tmux")
        .args(args)
        .output()
        .with_context(|| format!("failed to run `tmux {}`", args.join(" ")))?;
    if !output.status.success() {
        bail!(
            "`tmux {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// True if this process is running inside a tmux session.
pub fn in_tmux() -> bool {
    std::env::var("TMUX").is_ok()
}

/// The name of the tmux session this process is attached to. Every job
/// window the watcher creates lives in this same session.
pub fn current_session() -> Result<String> {
    Ok(run(&["display-message", "-p", "#S"])?.trim().to_string())
}

/// Create a new window in `session`, named `window`, with its starting
/// working directory set to `cwd`, running `shell_command`.
pub fn new_window(session: &str, window: &str, cwd: &Path, shell_command: &str) -> Result<()> {
    run(&[
        "new-window",
        "-t",
        session,
        "-n",
        window,
        "-c",
        &cwd.to_string_lossy(),
        shell_command,
    ])?;
    Ok(())
}

/// Whether `session:window` currently exists.
pub fn window_exists(session: &str, window: &str) -> bool {
    let target = format!("{session}:{window}");
    Command::new("tmux")
        .args(["list-windows", "-t", &target])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// The PID of `session:window`'s pane-root process, or `None` if the
/// window/session doesn't exist.
fn pane_pid(session: &str, window: &str) -> Option<u32> {
    let target = format!("{session}:{window}");
    let output = Command::new("tmux")
        .args(["list-panes", "-t", &target, "-F", "#{pane_pid}"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()?
        .trim()
        .parse()
        .ok()
}

fn cmdline_matches(pid: u32, matches: &impl Fn(&str) -> bool) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/cmdline"))
        .map(|s| matches(&s.to_lowercase()))
        .unwrap_or(false)
}

fn child_pids(pid: u32) -> Vec<u32> {
    let mut children = Vec::new();
    let Ok(entries) = std::fs::read_dir(format!("/proc/{pid}/task")) else {
        return children;
    };
    for entry in entries.flatten() {
        if let Ok(text) = std::fs::read_to_string(entry.path().join("children")) {
            children.extend(text.split_whitespace().filter_map(|s| s.parse::<u32>().ok()));
        }
    }
    children
}

/// Whether the job's agent backend (as opposed to a bare shell) is running
/// in `session:window`, per `matches` (see `AgentStrategy::process_matches`).
/// Walks the pane's whole process tree rather than just its immediate
/// foreground command — agents transiently shell out to tool subprocesses
/// (git, bash, ...) while working, and checking only the foreground command
/// would misreport those moments as "lost".
pub fn pane_runs_process(session: &str, window: &str, matches: impl Fn(&str) -> bool) -> bool {
    let Some(root_pid) = pane_pid(session, window) else {
        return false;
    };
    let mut stack = vec![root_pid];
    let mut seen = std::collections::HashSet::new();
    while let Some(pid) = stack.pop() {
        if !seen.insert(pid) {
            continue;
        }
        if cmdline_matches(pid, &matches) {
            return true;
        }
        stack.extend(child_pids(pid));
    }
    false
}

/// Capture the visible+scrollback text of `session:window`'s active pane.
pub fn capture_pane(session: &str, window: &str) -> Result<String> {
    let target = format!("{session}:{window}");
    run(&["capture-pane", "-p", "-t", &target])
}

/// Feed multi-line `text` into `session:window` as if the user pasted it
/// and pressed Enter. Uses a tmux paste buffer instead of `send-keys`
/// directly so embedded newlines/special characters in the prompt don't
/// need shell-style escaping.
pub fn send_text(session: &str, window: &str, text: &str) -> Result<()> {
    let target = format!("{session}:{window}");
    let buffer_name = format!("aide-{}", std::process::id());

    let mut tmp = std::env::temp_dir();
    tmp.push(format!("{buffer_name}.txt"));
    std::fs::write(&tmp, text)?;

    run(&["load-buffer", "-b", &buffer_name, &tmp.to_string_lossy()])?;
    let paste_result = run(&["paste-buffer", "-b", &buffer_name, "-t", &target]);
    let _ = run(&["delete-buffer", "-b", &buffer_name]);
    let _ = std::fs::remove_file(&tmp);
    paste_result?;

    run(&["send-keys", "-t", &target, "Enter"])?;
    Ok(())
}
