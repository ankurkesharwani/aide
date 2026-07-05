use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use toml_edit::{DocumentMut, Item, Table, value};

use super::{AgentState, AgentStatus, AgentStrategy, shell_quote};
use crate::job::AgentConfig;

const AWAITING_APPROVAL_MARKER: &str = "Please enter to confirm or esc to cancel";

/// `$CODEX_HOME/config.toml`, defaulting to `~/.codex/config.toml` — same
/// resolution Codex itself uses.
fn config_path() -> PathBuf {
    let home = std::env::var("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_default();
            Path::new(&home).join(".codex")
        });
    home.join("config.toml")
}

/// Marks each of `paths` `trust_level = "trusted"` under
/// `[projects."<path>"]` in Codex's own config, so launching Codex there
/// doesn't block on its one-time "do you trust the contents of this
/// directory?" onboarding prompt — confirmed no CLI flag suppresses this
/// (tried `-a never` and `--dangerously-bypass-approvals-and-sandbox`), so
/// pre-approving it here is the only way to keep an unattended job from
/// hanging on it forever. This is safe to do unconditionally because these
/// are exactly the directories the *user* already named in the job spec —
/// equivalent to them having pressed "yes" themselves the first time.
///
/// Edits the file surgically via `toml_edit` rather than round-tripping
/// through a full config struct, so unrelated settings/formatting the user
/// already has are left untouched. Idempotent: already-trusted entries are
/// left alone, and the file is only rewritten if something actually
/// changed.
fn trust_paths(paths: &[&Path]) -> Result<()> {
    let path = config_path();
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc: DocumentMut = text
        .parse()
        .with_context(|| format!("failed to parse {}", path.display()))?;

    let projects = doc.entry("projects").or_insert_with(|| {
        let mut t = Table::new();
        t.set_implicit(true); // no bare `[projects]` header, matches Codex's own style
        Item::Table(t)
    });
    let projects_table = projects
        .as_table_mut()
        .context("`projects` in codex config is not a table")?;

    let mut changed = false;
    for p in paths {
        let key = p.display().to_string();
        let already_trusted = projects_table
            .get(&key)
            .and_then(Item::as_table)
            .and_then(|t| t.get("trust_level"))
            .and_then(Item::as_str)
            == Some("trusted");
        if already_trusted {
            continue;
        }
        let mut entry = Table::new();
        entry.insert("trust_level", value("trusted"));
        projects_table.insert(&key, Item::Table(entry));
        changed = true;
    }

    if changed {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        std::fs::write(&path, doc.to_string())
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

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

    fn prepare(&self, paths: &[&Path]) -> Result<()> {
        trust_paths(paths)
    }
}
