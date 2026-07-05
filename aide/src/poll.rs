use anyhow::Result;
use chrono::Utc;

use crate::agent::AgentState;
use crate::job::JobStatus;
use crate::logging;
use crate::runtime::RuntimeInfo;
use crate::scanner::DiscoveredJob;
use crate::status_writer;
use crate::temp;
use crate::tmux;

/// One poll of a job whose `aide.yml` says `RUNNING`. Doubles as both the
/// steady-state 15s statusline poll and the watcher's startup
/// reconciliation check — there's no separate "is this job still alive"
/// code path; every poll re-derives it from the tmux window's current
/// state, per the spec's resilience model (no state kept beyond disk).
pub fn poll_running_job(session: &str, discovered: &DiscoveredJob) -> Result<()> {
    let job_id = &discovered.job.id;
    let mut runtime = RuntimeInfo::load(&discovered.dir)?;

    let Some((kind, _config)) = discovered.job.backend() else {
        // A RUNNING job should always have an agent backend — pickup
        // requires one — but guard rather than panic if aide.yml was
        // hand-edited after pickup.
        return Ok(());
    };
    let strategy = kind.strategy();

    if !tmux::window_exists(session, &runtime.window) {
        mark_lost(
            session,
            discovered,
            &mut runtime,
            "tmux window no longer exists",
        )?;
        return Ok(());
    }

    if !tmux::pane_runs_process(session, &runtime.window, |cmdline| {
        strategy.process_matches(cmdline)
    }) {
        mark_lost(
            session,
            discovered,
            &mut runtime,
            "window is no longer running its agent",
        )?;
        return Ok(());
    }

    if runtime.lost {
        // Recovered from a previous "lost" reading without user intervention
        // (e.g. transient tmux hiccup); clear the flag now that it's alive again.
        runtime.lost = false;
    }

    let pane_text = tmux::capture_pane(session, &runtime.window)?;

    let was_awaiting_approval = runtime.awaiting_approval;
    runtime.awaiting_approval = strategy.detect_awaiting_approval(&pane_text);
    if runtime.awaiting_approval && !was_awaiting_approval {
        logging::awaiting_approval(job_id);
    }

    if let Some(info) = strategy.parse_status(&pane_text) {
        runtime.cwd = Some(info.cwd);
        runtime.model = Some(info.model);
        runtime.profile = Some(info.profile);
        runtime.context = Some(info.context);
        runtime.session_id = Some(info.session_id);

        if info.state == AgentState::Working {
            runtime.seen_working = true;
        }

        // `seen_working` is the primary signal (a `Ready` seen right after
        // `Starting`, before any `Working`, just means the agent hasn't
        // begun yet). But on a coarse poll interval a fast task can run
        // through its entire `Starting -> Working -> Ready` lifecycle
        // between two polls, so the watcher never samples `Working` at
        // all — `seen_working` would then wrongly stay false forever. A
        // `.temp` outcome is independent, stronger evidence: the agent
        // only writes it as its very last action, so its presence means
        // real work happened even if polling missed the `Working` state.
        let outcome_reported = temp::read_outcome(&discovered.dir).is_some();
        let completed =
            info.state == AgentState::Ready && (runtime.seen_working || outcome_reported);
        runtime.state = Some(info.state);
        runtime.updated_at = Utc::now();
        runtime.save(&discovered.dir)?;

        if completed {
            // The agent's own SUCCESS/FAILURE judgment call, self-reported via
            // the `outcome` key of the `.temp` file (see `crate::temp`), takes
            // precedence; a job that finished without reporting one just
            // settles on DONE.
            let final_status = temp::read_outcome(&discovered.dir).unwrap_or(JobStatus::Done);
            status_writer::set_status(&discovered.spec_path, final_status)?;
            match final_status {
                JobStatus::Success => logging::success(job_id),
                JobStatus::Failure => logging::failure(job_id),
                _ => logging::done(job_id),
            }
        }
    } else {
        // Statusline briefly absent (e.g. right after launch); still persist
        // whatever else changed (awaiting-approval, lost recovery).
        runtime.updated_at = Utc::now();
        runtime.save(&discovered.dir)?;
    }

    Ok(())
}

fn mark_lost(
    session: &str,
    discovered: &DiscoveredJob,
    runtime: &mut RuntimeInfo,
    reason: &str,
) -> Result<()> {
    let _ = session;
    let was_lost = runtime.lost;
    runtime.lost = true;
    runtime.updated_at = Utc::now();
    runtime.save(&discovered.dir)?;
    if !was_lost {
        logging::lost(&discovered.job.id, reason);
    }
    Ok(())
}
