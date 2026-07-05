use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;

use crate::agent::AgentState;
use crate::job::{AideJob, JobStatus};
use crate::logging;
use crate::runtime::RuntimeInfo;
use crate::scanner::DiscoveredJob;
use crate::status_writer;
use crate::tmux;

/// Concatenates the job's `dirs`/`git` context and resolved `agent`
/// settings into a system prompt preamble for the job's agent.
fn build_system_prompt(job: &AideJob) -> String {
    let mut sections = Vec::new();

    sections.push(format!("# Task: {}", job.title));

    if !job.dirs.is_empty() {
        let mut s = String::from("## Available directories\n");
        for d in &job.dirs {
            s.push_str(&format!("- {} ({})", d.name, d.dir));
            if let Some(desc) = &d.description {
                s.push_str(&format!(" — {desc}"));
            }
            s.push('\n');
        }
        sections.push(s);
    }

    if !job.git.is_empty() {
        let mut s = String::from("## Available git repos\n");
        for g in &job.git {
            s.push_str(&format!("- {} ({})", g.name, g.dir));
            if let Some(desc) = &g.description {
                s.push_str(&format!(" — {desc}"));
            }
            if let Some(worktree) = &g.worktree {
                s.push_str(&format!(" [worktree: {worktree}]"));
            }
            s.push('\n');
        }
        sections.push(s);
    }

    if let Some((kind, config)) = job.backend() {
        sections.push(kind.strategy().describe(config));
    }

    sections.join("\n\n")
}

/// Embeds the contents of `prompt-file` after the system prompt.
fn assemble_prompt(job: &AideJob, job_dir: &Path) -> Result<String> {
    let prompt_path = job_dir.join(&job.prompt_file);
    let prompt_body = std::fs::read_to_string(&prompt_path)
        .with_context(|| format!("failed to read prompt-file {}", prompt_path.display()))?;
    Ok(format!(
        "{}\n\n---\n\n{}",
        build_system_prompt(job),
        prompt_body
    ))
}

/// Picks up an eligible, validated, non-colliding `READY` job: builds and
/// sends its prompt, opens its tmux window, writes the initial
/// `runtime.yml`, makes `aide.yml` read-only, and flips `status` to
/// `RUNNING`.
pub fn pickup(session: &str, discovered: &DiscoveredJob) -> Result<()> {
    let job = &discovered.job;
    let root = Path::new(&job.root);

    let (kind, config) = job
        .backend()
        .context("job has no agent backend configured under `agent`")?;
    let strategy = kind.strategy();

    let prompt = assemble_prompt(job, &discovered.dir)?;
    let command = strategy.build_command(config);

    tmux::new_window(session, &job.window, root, &command)
        .with_context(|| format!("failed to open tmux window '{}'", job.window))?;
    tmux::send_text(session, &job.window, &prompt)
        .with_context(|| format!("failed to feed prompt into window '{}'", job.window))?;

    let runtime = RuntimeInfo {
        window: job.window.clone(),
        agent: Some(kind.name().to_string()),
        session_id: None,
        model: None,
        profile: None,
        cwd: Some(job.root.clone()),
        state: Some(AgentState::Starting),
        context: None,
        seen_working: false,
        awaiting_approval: false,
        lost: false,
        updated_at: Utc::now(),
    };
    runtime.save(&discovered.dir)?;

    status_writer::make_read_only(&discovered.spec_path)?;
    status_writer::set_status(&discovered.spec_path, JobStatus::Running)?;

    logging::scheduled(&job.id, &job.window);
    logging::running(&job.id, &job.window);
    Ok(())
}
