//! Pre-destructive `state.toml` snapshots.
//!
//! GENESIS §10: "Before any destructive op (`delete`, `import overwriting`),
//! `state.toml` is snapshotted under `backups/` with an ISO-8601 timestamp."

use std::path::{Path, PathBuf};

use crate::{Error, time::now_iso8601};

/// Snapshot the current `state.toml` into
/// `<maru_home>/backups/<iso-8601>/state.toml`.
///
/// Returns the absolute path of the snapshot directory. If `state.toml`
/// does not exist, this is a no-op and returns `Ok(None)`.
///
/// # Errors
///
/// Returns [`Error::Io`] for any I/O failure.
pub fn snapshot_state(maru_home: &Path) -> Result<Option<PathBuf>, Error> {
    let src = maru_home.join("state.toml");
    if !src.exists() {
        return Ok(None);
    }

    // ISO-8601 with `:` is fine on Unix, problematic on Windows. Replace
    // `:` with `-` for cross-platform safety.
    let stamp = now_iso8601().replace(':', "-");
    let dest_dir = maru_home.join("backups").join(&stamp);
    std::fs::create_dir_all(&dest_dir).map_err(|source| Error::Io {
        operation: "create snapshot dir".to_owned(),
        path: dest_dir.clone(),
        source,
    })?;

    let dest = dest_dir.join("state.toml");
    std::fs::copy(&src, &dest).map_err(|source| Error::Io {
        operation: "copy state.toml to snapshot".to_owned(),
        path: dest.clone(),
        source,
    })?;

    Ok(Some(dest_dir))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]
mod tests {
    use super::snapshot_state;

    #[test]
    fn snapshots_existing_state() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("state.toml"),
            "schema_version = 1\n[profiles]\n",
        )
        .unwrap();
        let snap = snapshot_state(dir.path()).unwrap().unwrap();
        assert!(snap.starts_with(dir.path().join("backups")));
        assert!(snap.join("state.toml").exists());
        let copied = std::fs::read_to_string(snap.join("state.toml")).unwrap();
        assert!(copied.contains("schema_version = 1"));
    }

    #[test]
    fn no_op_when_state_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(snapshot_state(dir.path()).unwrap().is_none());
        assert!(!dir.path().join("backups").exists());
    }

    #[test]
    fn each_snapshot_in_unique_dir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("state.toml"),
            "schema_version = 1\n[profiles]\n",
        )
        .unwrap();
        let s1 = snapshot_state(dir.path()).unwrap().unwrap();
        // Sleep so ISO-8601 second-resolution stamp differs.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let s2 = snapshot_state(dir.path()).unwrap().unwrap();
        assert_ne!(s1, s2);
    }
}
