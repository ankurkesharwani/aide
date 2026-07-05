//! Abstracts everything about an agent CLI backend that differs from one
//! to the next: how to invoke it, how to describe its resolved config in
//! the system prompt, how to recognize its process in a tmux pane, and how
//! to read its status back out of captured pane text. Codex is the only
//! implementation for now (see `docs/spec.md`); `claude`, `gemini`, etc.
//! are expected to join as sibling `AgentKind` variants later.

mod codex;

pub use codex::CodexAgent;

use crate::job::AgentConfig;

/// Which agent CLI backend a job targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentKind {
    Codex,
}

impl AgentKind {
    /// The strategy implementing this backend's behavior.
    pub fn strategy(self) -> Box<dyn AgentStrategy> {
        match self {
            AgentKind::Codex => Box::new(CodexAgent),
        }
    }

    /// Name recorded in `runtime.yml` and logs.
    pub fn name(self) -> &'static str {
        match self {
            AgentKind::Codex => "codex",
        }
    }
}

/// Common statusline-derived agent state. `Ready` is ambiguous on its own
/// — it means "finished" only if `Working` was seen first, otherwise it
/// means "hasn't started yet". Callers track that themselves (see
/// `RuntimeInfo::seen_working`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum AgentState {
    Starting,
    Ready,
    Working,
}

/// A backend's status as read out of a captured tmux pane.
#[derive(Debug, Clone)]
pub struct AgentStatus {
    pub cwd: String,
    pub model: String,
    pub profile: String,
    pub state: AgentState,
    pub context: String,
    pub session_id: String,
}

/// How the watcher invokes and reads status from one agent CLI backend.
pub trait AgentStrategy {
    /// The CLI binary this backend invokes.
    fn binary(&self) -> &'static str;

    /// Build the shell command line used to launch this agent with the
    /// job's resolved `arguments`.
    fn build_command(&self, config: &AgentConfig) -> String;

    /// Fragment describing the resolved backend/arguments, appended to the
    /// job's system prompt.
    fn describe(&self, config: &AgentConfig) -> String;

    /// Whether a process's (lowercased) `/proc/<pid>/cmdline` indicates
    /// this agent is running, as opposed to a bare shell.
    fn process_matches(&self, cmdline_lower: &str) -> bool;

    /// Parse this backend's status from captured tmux pane text.
    fn parse_status(&self, pane_text: &str) -> Option<AgentStatus>;

    /// Independently detect an "awaiting approval" prompt in pane text.
    fn detect_awaiting_approval(&self, pane_text: &str) -> bool;
}
