use serde::{Deserialize, Serialize};

/// Codex's own agent state as reported in its statusline. `Ready` is
/// ambiguous on its own — it means "finished" only if `Working` was seen
/// first, otherwise it means "hasn't started yet". Callers are expected to
/// track that themselves (see `RuntimeInfo::seen_working`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum AgentState {
    Starting,
    Ready,
    Working,
}

impl AgentState {
    fn parse(s: &str) -> Option<AgentState> {
        match s {
            "Starting" => Some(AgentState::Starting),
            "Ready" => Some(AgentState::Ready),
            "Working" => Some(AgentState::Working),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StatuslineInfo {
    pub cwd: String,
    pub model: String,
    pub profile: String,
    pub state: AgentState,
    pub context: String,
    pub session_id: String,
}

const AWAITING_APPROVAL_MARKER: &str = "Please enter to confirm or esc to cancel";

/// The statusline is a single line, fields separated by ` · `:
/// `cwd · model · profile · state · context · sessionId`. Scan captured
/// pane text bottom-up for the most recent line matching that shape,
/// tolerating the statusline being briefly absent (e.g. right after
/// launch, before Codex has drawn it).
pub fn parse_statusline(pane_text: &str) -> Option<StatuslineInfo> {
    for line in pane_text.lines().rev() {
        let fields: Vec<&str> = line.split(" · ").map(str::trim).collect();
        if fields.len() != 6 {
            continue;
        }
        if let Some(state) = AgentState::parse(fields[3]) {
            return Some(StatuslineInfo {
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

/// Independent of statusline parsing: Codex shows `Awaiting approval` as a
/// trailing prompt line rather than a statusline state.
pub fn detect_awaiting_approval(pane_text: &str) -> bool {
    pane_text.contains(AWAITING_APPROVAL_MARKER)
}
