use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;

use crate::agent::AgentState;
use crate::job::{AideJob, JobStatus};
use crate::logging;
use crate::runtime::RuntimeInfo;
use crate::scanner::DiscoveredJob;
use crate::status_writer;
use crate::temp;
use crate::tmux;

/// Concatenates the job's `dirs`/`git` context and the `.temp`-reporting
/// instruction into a system prompt preamble for the job's agent. `job_dir`
/// is used for that instruction only — the agent's cwd is `job.root`,
/// which is generally a different directory, so the `.temp` file's path
/// must be spelled out in full.
fn build_system_prompt(job: &AideJob, job_dir: &Path) -> String {
    let mut sections = Vec::new();

    sections.push(format!("# Task: {}", job.title));

    sections.push(
        "## Operating context\nYou are running unattended in a background \
         tmux session as part of an automated job queue. No one is watching \
         this session in real time — if you stop to ask a question or wait \
         for approval, the task may sit idle indefinitely before anyone \
         notices. Make the best autonomous judgment call you can rather than \
         waiting for input; prefer a safe, reversible action over blocking \
         when you're unsure."
            .to_string(),
    );

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

    let temp_path = outcome_temp_path(job_dir);
    sections.push(format!(
        "## Reporting back to the watcher\nWrite a YAML file to `{temp_path}` \
         (create it if it doesn't exist) to communicate structured \
         information back to the watcher. Currently the only key it looks \
         for is `outcome`: when you are finished, decide whether this task \
         was a SUCCESS or a FAILURE and set it accordingly, e.g.:\n```\n\
         outcome: SUCCESS\n```\nThis is the only way the watcher learns the \
         outcome of your work — it can't infer that from your process state \
         alone. If you don't write `outcome`, the task is simply recorded \
         as done, with no success/failure judgment, and anything depending \
         on it will not proceed."
    ));

    sections.join("\n\n")
}

fn outcome_temp_path(job_dir: &Path) -> String {
    let job_dir_abs = job_dir
        .canonicalize()
        .unwrap_or_else(|_| job_dir.to_path_buf());
    temp::path_for(&job_dir_abs).display().to_string()
}

/// Embeds the contents of `prompt-file` after the system prompt, followed
/// by a trailing reminder of the outcome-reporting instruction. The
/// reminder is deliberately repeated at the very end, immediately before
/// the agent starts working, rather than relying solely on the system
/// preamble — an instruction given once, before a long task, is exactly
/// the kind of thing a model can let slip by the time it wraps up.
fn assemble_prompt(job: &AideJob, job_dir: &Path) -> Result<String> {
    let prompt_path = job_dir.join(&job.prompt_file);
    let prompt_body = std::fs::read_to_string(&prompt_path)
        .with_context(|| format!("failed to read prompt-file {}", prompt_path.display()))?;
    Ok(format!(
        "{}\n\n---\n\n{}\n\n---\n\nReminder: before you stop, write your \
         outcome (`SUCCESS` or `FAILURE`) to `{}` as described above.",
        build_system_prompt(job, job_dir),
        prompt_body,
        outcome_temp_path(job_dir)
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
