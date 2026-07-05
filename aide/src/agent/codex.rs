use super::{AgentState, AgentStatus, AgentStrategy, shell_quote};
use crate::job::AgentConfig;

const AWAITING_APPROVAL_MARKER: &str = "Please enter to confirm or esc to cancel";

fn parse_state(s: &str) -> Option<AgentState> {
    match s {
        "Starting" => Some(AgentState::Starting),
        "Ready" => Some(AgentState::Ready),
        "Working" => Some(AgentState::Working),
        _ => None,
    }
}

/// The Codex CLI backend: invoked as `codex <arguments...> <prompt>` — the
/// interactive TUI accepts an optional trailing `[PROMPT]` positional
/// argument to start the session with (`codex --help`), which this uses
/// instead of pasting the prompt into the pane after launch, avoiding a
/// race against the TUI being ready to receive input. Reports its state
/// via a ` · `-separated statusline and an "awaiting approval" marker line
/// for approval prompts (see `docs/spec.md`).
pub struct CodexAgent;

impl AgentStrategy for CodexAgent {
    fn binary(&self) -> &'static str {
        "codex"
    }

    fn build_command(&self, config: &AgentConfig, prompt: &str) -> String {
        let mut cmd = String::from(self.binary());
        for arg in &config.arguments {
            cmd.push(' ');
            cmd.push_str(arg);
        }
        cmd.push(' ');
        cmd.push_str(&shell_quote(prompt));
        cmd
    }

    fn process_matches(&self, cmdline_lower: &str) -> bool {
        cmdline_lower.contains("codex")
    }

    /// The statusline is a single line, fields separated by ` · `:
    /// `cwd · model · profile · state · context · sessionId`. Scan captured
    /// pane text bottom-up for the most recent line matching that shape,
    /// tolerating the statusline being briefly absent (e.g. right after
    /// launch, before Codex has drawn it).
    fn parse_status(&self, pane_text: &str) -> Option<AgentStatus> {
        for line in pane_text.lines().rev() {
            let fields: Vec<&str> = line.split(" · ").map(str::trim).collect();
            if fields.len() != 6 {
                continue;
            }
            if let Some(state) = parse_state(fields[3]) {
                return Some(AgentStatus {
                    cwd: fields[0].to_string(),
                    model: fields[1].to_string(),
                    profile: fields[2].to_string(),
                    state,
                    context: fields[4].to_string(),
                    session_id: fields[5].to_string(),
                });
            }
        }
        None
    }

    fn detect_awaiting_approval(&self, pane_text: &str) -> bool {
        pane_text.contains(AWAITING_APPROVAL_MARKER)
    }
}
