use chrono::{DateTime, Utc};

use crate::job::AmitJob;

/// Checks a parsed job against the `task.yml` schema: required fields
/// present, enum-valued fields hold one of their allowed values. Pure
/// function — it has no opinion on what the watcher does with the result.
pub fn validate(job: &AmitJob) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if job.title.trim().is_empty() {
        errors.push("title is required".to_string());
    }
    if job.id.trim().is_empty() {
        errors.push("id is required".to_string());
    }
    if job.window.trim().is_empty() {
        errors.push("window is required".to_string());
    }
    if job.root.trim().is_empty() {
        errors.push("root is required".to_string());
    }
    if job.prompt_file.trim().is_empty() {
        errors.push("prompt-file is required".to_string());
    }

    if job.status().is_none() {
        errors.push(format!(
            "status must be one of DRAFT, READY, RUNNING, DONE, SUCCESS, FAILURE, got '{}'",
            job.status
        ));
    }

    if let Some(execute_after) = &job.execute_after
        && execute_after.parse::<DateTime<Utc>>().is_err()
    {
        errors.push(format!(
            "executeAfter must be an RFC3339 timestamp, got '{execute_after}'"
        ));
    }

    for dir in &job.dirs {
        if dir.name.trim().is_empty() {
            errors.push("dirs[].name is required".to_string());
        }
        if dir.dir.trim().is_empty() {
            errors.push("dirs[].dir is required".to_string());
        }
    }

    for git in &job.git {
        if git.name.trim().is_empty() {
            errors.push("git[].name is required".to_string());
        }
        if git.dir.trim().is_empty() {
            errors.push("git[].dir is required".to_string());
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
