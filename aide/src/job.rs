use std::fmt;
use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::agent::AgentKind;

/// Raw representation of an `aide.yml` job spec. Fields that the schema
/// draft (`docs/aide.yml`) documents as enums are kept as plain `String`s
/// here so a job with an invalid value still parses — the validator is
/// what decides whether an enum value is acceptable, not serde.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AideJob {
    pub title: String,
    pub id: String,
    pub window: String,
    pub status: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(
        rename = "executeAfter",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub execute_after: Option<String>,
    pub root: String,
    #[serde(default)]
    pub dirs: Vec<DirEntry>,
    #[serde(default)]
    pub git: Vec<GitEntry>,
    #[serde(default)]
    pub agent: AgentBackends,
    #[serde(rename = "prompt-file")]
    pub prompt_file: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DirEntry {
    pub name: String,
    pub dir: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GitEntry {
    pub name: String,
    pub dir: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub worktree: Option<String>,
}

/// The `agent:` block of `aide.yml`. `codex` is the only backend for now;
/// `claude`, `gemini`, etc. are expected to join as sibling keys later
/// (see `docs/spec.md`).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AgentBackends {
    #[serde(default)]
    pub codex: Option<AgentConfig>,
}

/// An agent backend's resolved config: the command-line arguments used to
/// invoke it. Shared across backends — how those arguments are assembled
/// into a launch command and described in the system prompt is up to each
/// backend's `AgentStrategy` (see `crate::agent`).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AgentConfig {
    #[serde(default)]
    pub arguments: Vec<String>,
}

/// The set of allowed values for `status`. Kept separate from `AideJob`
/// (which stores `status` as a raw `String`) so an on-disk value outside
/// this set is a validation failure rather than a parse failure.
///
/// `Success`/`Failure` are the agent's own judgment call, self-reported via
/// the `outcome` sentinel file (see `crate::outcome`) — the watcher can't
/// infer them from process state alone. `Done` remains what the watcher
/// itself observes (the agent stopped working) and is what a job settles
/// on if it never reports an outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Draft,
    Ready,
    Running,
    Done,
    Success,
    Failure,
}

impl FromStr for JobStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "DRAFT" => Ok(JobStatus::Draft),
            "READY" => Ok(JobStatus::Ready),
            "RUNNING" => Ok(JobStatus::Running),
            "DONE" => Ok(JobStatus::Done),
            "SUCCESS" => Ok(JobStatus::Success),
            "FAILURE" => Ok(JobStatus::Failure),
            _ => Err(()),
        }
    }
}

impl fmt::Display for JobStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            JobStatus::Draft => "DRAFT",
            JobStatus::Ready => "READY",
            JobStatus::Running => "RUNNING",
            JobStatus::Done => "DONE",
            JobStatus::Success => "SUCCESS",
            JobStatus::Failure => "FAILURE",
        };
        f.write_str(s)
    }
}

impl AideJob {
    pub fn status(&self) -> Option<JobStatus> {
        self.status.parse().ok()
    }

    /// The agent backend this job targets, and its resolved config.
    /// `None` if no backend is configured under `agent` — the caller
    /// decides whether that's an error (e.g. pickup can't launch without
    /// one, but a still-`DRAFT` job may not have picked one yet).
    pub fn backend(&self) -> Option<(AgentKind, &AgentConfig)> {
        if let Some(config) = &self.agent.codex {
            return Some((AgentKind::Codex, config));
        }
        None
    }

    pub fn load(path: &Path) -> anyhow::Result<AideJob> {
        let text = std::fs::read_to_string(path)?;
        let job: AideJob = serde_yaml::from_str(&text)?;
        Ok(job)
    }
}
