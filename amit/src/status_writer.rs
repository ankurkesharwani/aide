use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde_yaml::Value;

use crate::job::JobStatus;

const READ_ONLY: u32 = 0o444;
const OWNER_WRITABLE: u32 = 0o644;

/// Called once, right after pickup: from this point on the watcher is the
/// only writer to `task.yml`, and `status` is the only field it will ever
/// touch. Everyone else (including the user's editor) only gets read
/// access.
pub fn make_read_only(spec_path: &Path) -> Result<()> {
    fs::set_permissions(spec_path, fs::Permissions::from_mode(READ_ONLY))
        .with_context(|| format!("failed to chmod {} read-only", spec_path.display()))
}

/// Rewrite just the `status` field of `spec_path`, leaving every other key
/// untouched. Bypasses the read-only permissions set by
/// [`make_read_only`] by briefly restoring write access to itself.
pub fn set_status(spec_path: &Path, status: JobStatus) -> Result<()> {
    let original_mode = fs::metadata(spec_path)?.permissions().mode();

    fs::set_permissions(spec_path, fs::Permissions::from_mode(OWNER_WRITABLE))?;
    let result = (|| -> Result<()> {
        let text = fs::read_to_string(spec_path)?;
        let mut doc: Value = serde_yaml::from_str(&text)?;
        let Value::Mapping(map) = &mut doc else {
            bail!("{} is not a YAML mapping", spec_path.display());
        };
        map.insert(
            Value::String("status".to_string()),
            Value::String(status.to_string()),
        );
        let updated = serde_yaml::to_string(&doc)?;
        fs::write(spec_path, updated)?;
        Ok(())
    })();

    // Always restore permissions, even if the write above failed.
    fs::set_permissions(spec_path, fs::Permissions::from_mode(original_mode))?;
    result
}
