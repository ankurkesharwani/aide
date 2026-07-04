use std::fmt;
use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

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
    pub dependency: Vec<String>,
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
    pub model: ModelConfig,
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

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ModelConfig {
    #[serde(default)]
    pub codex: Option<CodexModel>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CodexModel {
    pub name: String,
    #[serde(default)]
    pub thinking: Option<String>,
    #[serde(default)]
    pub speed: Option<String>,
}

/// The set of allowed values for `status`. Kept separate from `AideJob`
/// (which stores `status` as a raw `String`) so an on-disk value outside
/// this set is a validation failure rather than a parse failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Draft,
    Ready,
    Running,
    Done,
}

impl FromStr for JobStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "DRAFT" => Ok(JobStatus::Draft),
            "READY" => Ok(JobStatus::Ready),
            "RUNNING" => Ok(JobStatus::Running),
            "DONE" => Ok(JobStatus::Done),
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
        };
        f.write_str(s)
    }
}

impl AideJob {
    pub fn status(&self) -> Option<JobStatus> {
        self.status.parse().ok()
    }

    pub fn load(path: &Path) -> anyhow::Result<AideJob> {
        let text = std::fs::read_to_string(path)?;
        let job: AideJob = serde_yaml::from_str(&text)?;
        Ok(job)
    }
}
