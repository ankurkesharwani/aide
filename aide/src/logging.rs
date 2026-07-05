use chrono::Local;

/// Console-only structured-ish logging for job lifecycle events. Kept as
/// one small helper rather than pulling in a logging framework — the
/// watcher has exactly one log sink (stdout) and a fixed set of event
/// kinds.
fn line(level: &str, job_id: &str, message: &str) {
    let ts = Local::now().format("%Y-%m-%d %H:%M:%S");
    println!("[{ts}] {level:<5} job={job_id} {message}");
}

pub fn queued(job_id: &str) {
    line("INFO", job_id, "queued");
}

pub fn scheduled(job_id: &str, window: &str) {
    line("INFO", job_id, &format!("scheduled window={window}"));
}

pub fn failed_to_schedule(job_id: &str, reason: &str) {
    line("WARN", job_id, &format!("failed to schedule: {reason}"));
}

pub fn running(job_id: &str, window: &str) {
    line("INFO", job_id, &format!("running window={window}"));
}

pub fn awaiting_approval(job_id: &str) {
    line("INFO", job_id, "awaiting approval");
}

pub fn done(job_id: &str) {
    line("INFO", job_id, "done");
}

pub fn success(job_id: &str) {
    line("INFO", job_id, "success");
}

pub fn failure(job_id: &str) {
    line("WARN", job_id, "failure");
}

pub fn lost(job_id: &str, reason: &str) {
    line("ERROR", job_id, &format!("lost/unreachable: {reason}"));
}

pub fn error(job_id: &str, message: &str) {
    line("ERROR", job_id, message);
}
