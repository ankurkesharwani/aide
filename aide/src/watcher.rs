use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use anyhow::Result;
use chrono::Utc;

use crate::job::JobStatus;
use crate::logging;
use crate::pickup;
use crate::poll;
use crate::scanner::{self, DiscoveredJob};
use crate::validator;

pub struct Watcher {
    workspace: PathBuf,
    session: String,
    /// Log-dedup only (not scheduling state): job ids we've already logged
    /// as "queued" so we don't repeat it every poll while it waits on
    /// dependencies/executeAfter.
    logged_queued: HashSet<String>,
}

impl Watcher {
    pub fn new(workspace: PathBuf, session: String) -> Self {
        Watcher {
            workspace,
            session,
            logged_queued: HashSet::new(),
        }
    }

    pub fn tick(&mut self) {
        let scan = match scanner::scan_workspace(&self.workspace) {
            Ok(scan) => scan,
            Err(e) => {
                logging::error("-", &format!("workspace scan failed: {e:#}"));
                return;
            }
        };

        for failed in &scan.failed {
            logging::failed_to_schedule(
                &failed.spec_path.display().to_string(),
                &format!("failed to parse: {:#}", failed.error),
            );
        }

        // Validate every discovered job up front; only validated jobs
        // participate in the uniqueness check and dependency resolution.
        let mut valid: Vec<&DiscoveredJob> = Vec::new();
        for discovered in &scan.jobs {
            match validator::validate(&discovered.job) {
                Ok(()) => valid.push(discovered),
                Err(errors) => {
                    logging::failed_to_schedule(
                        &discovered.job.id,
                        &format!("invalid schema: {}", errors.join("; ")),
                    );
                }
            }
        }

        // Uniqueness gate across the whole workspace.
        let mut id_counts: HashMap<&str, u32> = HashMap::new();
        let mut window_counts: HashMap<&str, u32> = HashMap::new();
        for discovered in &valid {
            *id_counts.entry(discovered.job.id.as_str()).or_default() += 1;
            *window_counts
                .entry(discovered.job.window.as_str())
                .or_default() += 1;
        }

        let mut unique: Vec<&DiscoveredJob> = Vec::new();
        for discovered in valid {
            let id_dup = id_counts
                .get(discovered.job.id.as_str())
                .copied()
                .unwrap_or(0)
                > 1;
            let window_dup = window_counts
                .get(discovered.job.window.as_str())
                .copied()
                .unwrap_or(0)
                > 1;
            if id_dup || window_dup {
                let mut reasons = Vec::new();
                if id_dup {
                    reasons.push(format!("duplicate id '{}'", discovered.job.id));
                }
                if window_dup {
                    reasons.push(format!("duplicate window '{}'", discovered.job.window));
                }
                logging::failed_to_schedule(&discovered.job.id, &reasons.join("; "));
            } else {
                unique.push(discovered);
            }
        }

        // Status of every validated, non-colliding job, keyed by id — used
        // to resolve `dependencies` lists.
        let statuses: HashMap<&str, JobStatus> = unique
            .iter()
            .filter_map(|d| d.job.status().map(|s| (d.job.id.as_str(), s)))
            .collect();

        let now = Utc::now();
        for discovered in &unique {
            let job = &discovered.job;
            let Some(status) = job.status() else { continue };

            match status {
                JobStatus::Running => {
                    if let Err(e) = poll::poll_running_job(&self.session, discovered) {
                        logging::error(&job.id, &format!("poll failed: {e:#}"));
                    }
                }
                JobStatus::Ready => {
                    // Only a dependency's own agent-reported SUCCESS
                    // satisfies it — DONE (no outcome reported) or FAILURE
                    // leaves the dependent queued indefinitely, surfacing
                    // as a visible "queued" job rather than proceeding on
                    // an unknown or failed upstream result.
                    let deps_done = job
                        .dependencies
                        .iter()
                        .all(|dep_id| statuses.get(dep_id.as_str()) == Some(&JobStatus::Success));
                    let time_ok = match &job.execute_after {
                        Some(ts) => ts
                            .parse::<chrono::DateTime<Utc>>()
                            .map(|t| t <= now)
                            .unwrap_or(false),
                        None => true,
                    };

                    if deps_done && time_ok {
                        self.logged_queued.remove(&job.id);
                        if let Err(e) = pickup::pickup(&self.session, discovered) {
                            logging::failed_to_schedule(&job.id, &format!("{e:#}"));
                        }
                    } else if self.logged_queued.insert(job.id.clone()) {
                        logging::queued(&job.id);
                    }
                }
                JobStatus::Draft | JobStatus::Done | JobStatus::Success | JobStatus::Failure => {}
            }
        }
    }
}

pub fn run(workspace: PathBuf) -> Result<()> {
    if !crate::tmux::in_tmux() {
        anyhow::bail!("aide watcher must be run from inside a tmux session");
    }
    let session = crate::tmux::current_session()?;

    let mut watcher = Watcher::new(workspace, session);
    loop {
        watcher.tick();
        std::thread::sleep(std::time::Duration::from_secs(15));
    }
}
