use std::io::IsTerminal;

use chrono::Local;

/// Console-only structured-ish logging for job lifecycle events. Kept as
/// one small helper rather than pulling in a logging framework — the
/// watcher has exactly one log sink (stdout) and a fixed set of event
/// kinds.
const RESET: &str = "\x1b[0m";
const DIM: &str = "\x1b[2m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";
const BOLD_RED: &str = "\x1b[1;31m";
const BOLD_GREEN: &str = "\x1b[1;32m";

/// Colors are only worth emitting when stdout is an actual terminal —
/// redirected to a file or piped elsewhere, raw escape codes would just be
/// noise in the log.
fn colors_enabled() -> bool {
    std::io::stdout().is_terminal()
}

fn line(level: &str, color: &str, job_id: &str, message: &str) {
    let ts = Local::now().format("%Y-%m-%d %H:%M:%S");
    if colors_enabled() {
        println!("[{ts}] {color}{level:<5}{RESET} job={job_id} {color}{message}{RESET}");
    } else {
        println!("[{ts}] {level:<5} job={job_id} {message}");
    }
}

pub fn queued(job_id: &str) {
    line("INFO", DIM, job_id, "queued");
}

pub fn scheduled(job_id: &str, window: &str) {
    line("INFO", CYAN, job_id, &format!("scheduled window={window}"));
}

pub fn failed_to_schedule(job_id: &str, reason: &str) {
    line("WARN", YELLOW, job_id, &format!("failed to schedule: {reason}"));
}

pub fn running(job_id: &str, window: &str) {
    line("INFO", CYAN, job_id, &format!("running window={window}"));
}

pub fn awaiting_approval(job_id: &str) {
    line("INFO", YELLOW, job_id, "awaiting approval");
}

pub fn done(job_id: &str) {
    line("INFO", GREEN, job_id, "done");
}

pub fn success(job_id: &str) {
    line("INFO", BOLD_GREEN, job_id, "success");
}

pub fn failure(job_id: &str) {
    line("WARN", BOLD_RED, job_id, "failure");
}

pub fn lost(job_id: &str, reason: &str) {
    line("ERROR", RED, job_id, &format!("lost/unreachable: {reason}"));
}

pub fn error(job_id: &str, message: &str) {
    line("ERROR", RED, job_id, message);
}
