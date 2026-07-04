use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;

use crate::job::{AideJob, JobStatus};
use crate::logging;
use crate::runtime::RuntimeInfo;
use crate::scanner::DiscoveredJob;
use crate::status_writer;
use crate::statusline::AgentState;
use crate::tmux;

/// Concatenates the job's `dirs`/`git` context and resolved `model`
/// settings into a system prompt preamble for Codex.
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

    if let Some(codex) = &job.model.codex {
        let mut s = format!("## Model\ncodex model: {}", codex.name);
        if let Some(thinking) = &codex.thinking {
            s.push_str(&format!(", thinking: {thinking}"));
        }
        if let Some(speed) = &codex.speed {
            s.push_str(&format!(", speed: {speed}"));
        }
        sections.push(s);
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

/// Builds the shell command used to launch Codex with the job's resolved
/// model parameters.
fn codex_command(job: &AideJob) -> String {
    let mut cmd = String::from("codex");
    if let Some(codex) = &job.model.codex {
        cmd.push_str(&format!(" -m {}", codex.name));
        if let Some(thinking) = &codex.thinking {
            cmd.push_str(&format!(
                " -c model_reasoning_effort={}",
                thinking.to_lowercase()
            ));
        }
        if let Some(speed) = &codex.speed {
            cmd.push_str(&format!(" -c model_speed={}", speed.to_lowercase()));
        }
    }
    cmd
}

/// Picks up an eligible, validated, non-colliding `READY` job: builds and
/// sends its prompt, opens its tmux window, writes the initial
/// `runtime.yml`, makes `aide.yml` read-only, and flips `status` to
/// `RUNNING`.
pub fn pickup(session: &str, discovered: &DiscoveredJob) -> Result<()> {
    let job = &discovered.job;
    let root = Path::new(&job.root);

    let prompt = assemble_prompt(job, &discovered.dir)?;
    let command = codex_command(job);

    tmux::new_window(session, &job.window, root, &command)
        .with_context(|| format!("failed to open tmux window '{}'", job.window))?;
    tmux::send_text(session, &job.window, &prompt)
        .with_context(|| format!("failed to feed prompt into window '{}'", job.window))?;

    let runtime = RuntimeInfo {
        window: job.window.clone(),
        session_id: None,
        model: job.model.codex.as_ref().map(|c| c.name.clone()),
        thinking: job.model.codex.as_ref().and_then(|c| c.thinking.clone()),
        speed: job.model.codex.as_ref().and_then(|c| c.speed.clone()),
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
