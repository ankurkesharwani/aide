use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::job::JobStatus;

/// Filename of the agent-writable scratch file inside a job's directory,
/// sibling to `task.yml`/`runtime.yml`/the prompt file. A simple key/value
/// YAML document with a small set of defined keys the agent may fill in to
/// communicate back to the watcher — currently just `outcome`, with more
/// expected to be added here as the need arises. Written by the agent per
/// the instruction appended to its system prompt (see
/// `pickup::build_system_prompt`); the watcher only ever reads it, so
/// there's no write-write race with `task.yml` (watcher-owned, read-only
/// post-pickup) or `runtime.yml` (rewritten wholesale on every poll).
pub const FILE_NAME: &str = ".temp";

pub fn path_for(job_dir: &Path) -> PathBuf {
    job_dir.join(FILE_NAME)
}

/// The `.temp` file's defined keys. Unknown keys are ignored rather than
/// rejected, so an agent writing a key we don't understand yet (or a stale
/// key from a prior schema) doesn't blow up parsing of the ones we do.
#[derive(Debug, Clone, Default, Deserialize)]
struct TempFile {
    #[serde(default)]
    outcome: Option<String>,
}

fn load(job_dir: &Path) -> Option<TempFile> {
    let text = std::fs::read_to_string(path_for(job_dir)).ok()?;
    serde_yaml::from_str(&text).ok()
}

/// Reads the agent's self-reported outcome from the `outcome` key of the
/// `.temp` file, if present. Tolerates surrounding whitespace/casing in the
/// value since it's free-form agent output. `None` if the file is missing,
/// unreadable, malformed YAML, or `outcome` is absent/unrecognized.
pub fn read_outcome(job_dir: &Path) -> Option<JobStatus> {
    let outcome = load(job_dir)?.outcome?;
    match outcome.trim().to_uppercase().as_str() {
        "SUCCESS" => Some(JobStatus::Success),
        "FAILURE" => Some(JobStatus::Failure),
        _ => None,
    }
}
