use std::path::{Path, PathBuf};

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

/// Name of the scratch folder an agent may create inside a job's directory
/// (see the "Workspace" section of `build_system_prompt`). Purely a prompt
/// convention — the watcher itself never reads or writes it.
const WORKSPACE_DIR_NAME: &str = "workspace";

/// Absolute form of `job_dir`. The agent's cwd is `job.root`, which is
/// generally a different directory than the job's own directory, so any
/// path we tell it to read/write there must be spelled out in full rather
/// than left relative.
fn absolute(job_dir: &Path) -> PathBuf {
    job_dir
        .canonicalize()
        .unwrap_or_else(|_| job_dir.to_path_buf())
}

/// Concatenates the job's `dirs`/`git` context, the workspace-folder
/// convention, and the `.temp`-reporting instruction into a system prompt
/// preamble for the job's agent.
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
        let mut s = String::from(
            "## Available directories\nYou have access to these directories. \
             Use them exactly as the task instructions below say to — this \
             list only tells you what's available, not what to do with it.\n",
        );
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
        let mut s = String::from(
            "## Available git repos\nYou have access to these git \
             repositories, to be used as the task instructions below say to. \
             If the task requires making changes to one of them, do the work \
             in a new worktree rather than directly on the primary checkout \
             or on `master`: use the `worktree` path shown for that repo \
             below if one is given, otherwise create one yourself, and \
             branch it from `master` with a new branch named for this task.\n",
        );
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

    sections.push(format!(
        "## Workspace\nYou may create a `{WORKSPACE_DIR_NAME}/` folder inside \
         `{}` (the same directory as this job's aide.yml) and freely create, \
         read, update, or delete any file inside it for your own working \
         notes or intermediate files. If this task calls for producing an \
         output document, write it into that folder using the filename \
         pattern `output-*.md` (e.g. `output-summary.md`) — output files \
         must be Markdown only.",
        absolute(job_dir).display()
    ));

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
    temp::path_for(&absolute(job_dir)).display().to_string()
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
