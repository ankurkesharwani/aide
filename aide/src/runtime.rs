use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::statusline::AgentState;

/// `runtime.yml` — written and owned entirely by the watcher once a job is
/// picked up. Holds everything discovered/assigned during execution that
/// doesn't belong in the (post-pickup, read-only) `aide.yml`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuntimeInfo {
    pub window: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub thinking: Option<String>,
    #[serde(default)]
    pub speed: Option<String>,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub state: Option<AgentState>,
    #[serde(default)]
    pub context: Option<String>,
    /// Whether a `Working` state has been observed yet, so a later `Ready`
    /// can be told apart from the pre-start idle `Ready`.
    #[serde(default)]
    pub seen_working: bool,
    #[serde(default)]
    pub awaiting_approval: bool,
    /// Set when the watcher can no longer find the job's tmux window/session,
    /// or the window no longer runs Codex. Visibility-only: never fed back
    /// into `aide.yml`'s `status`.
    #[serde(default)]
    pub lost: bool,
    pub updated_at: DateTime<Utc>,
}

impl RuntimeInfo {
    pub fn path_for(job_dir: &Path) -> std::path::PathBuf {
        job_dir.join("runtime.yml")
    }

    pub fn load(job_dir: &Path) -> anyhow::Result<RuntimeInfo> {
        let text = std::fs::read_to_string(Self::path_for(job_dir))?;
        Ok(serde_yaml::from_str(&text)?)
    }

    pub fn save(&self, job_dir: &Path) -> anyhow::Result<()> {
        let text = serde_yaml::to_string(self)?;
        std::fs::write(Self::path_for(job_dir), text)?;
        Ok(())
    }
}
