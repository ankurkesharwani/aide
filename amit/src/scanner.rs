use std::path::{Path, PathBuf};

use crate::job::AmitJob;

/// A discovered `task.yml`, alongside the directory it lives in (jobs keep
/// their `task.yml`, prompt file, and eventually `runtime.yml` side by
/// side in one directory).
pub struct DiscoveredJob {
    pub dir: PathBuf,
    pub spec_path: PathBuf,
    pub job: AmitJob,
}

/// A job spec that failed to parse, kept around so callers can log it.
pub struct UnparseableJob {
    pub spec_path: PathBuf,
    pub error: anyhow::Error,
}

pub struct ScanResult {
    pub jobs: Vec<DiscoveredJob>,
    pub failed: Vec<UnparseableJob>,
}

/// Recursively glob `workspace` for `task.yml` files and parse each one.
/// Files that fail to parse are collected separately rather than aborting
/// the scan — one bad job spec shouldn't blind the watcher to every other
/// job in the workspace.
pub fn scan_workspace(workspace: &Path) -> anyhow::Result<ScanResult> {
    let pattern = format!("{}/**/task.yml", workspace.display());
    let mut jobs = Vec::new();
    let mut failed = Vec::new();

    for entry in glob::glob(&pattern)? {
        let spec_path = entry?;
        match AmitJob::load(&spec_path) {
            Ok(job) => {
                let dir = spec_path
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| workspace.to_path_buf());
                jobs.push(DiscoveredJob {
                    dir,
                    spec_path,
                    job,
                });
            }
            Err(error) => failed.push(UnparseableJob { spec_path, error }),
        }
    }

    Ok(ScanResult { jobs, failed })
}
