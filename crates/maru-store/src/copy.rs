//! Filtered profile-dir copy.
//!
//! Walks a source dir and copies every regular file to a parallel
//! location under the destination, except files that match the
//! [`crate::deny_list`] for the supplied harness.
//!
//! Used by `maru profile clone` and `maru profile import-existing`.

use std::path::{Path, PathBuf};

use maru_core::HarnessId;

use crate::{Error, deny_list};

/// Stats returned by [`copy_filtered`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CopyStats {
    /// Number of files copied.
    pub copied: usize,
    /// Number of files excluded by the deny-list.
    pub excluded: usize,
    /// Files that were excluded (relative to source root). Useful for
    /// surfacing in CLI output.
    pub excluded_paths: Vec<PathBuf>,
}

/// Recursively copy `src` to `dst`, excluding files that match the
/// per-harness deny-list. Symlinks are resolved (their target is
/// copied; symlinks are not preserved).
///
/// `dst` is created (`mkdir -p`) if missing. Existing files at `dst` are
/// overwritten. The intent is that callers pass a fresh destination.
///
/// # Errors
///
/// Returns [`Error::Io`] for any I/O failure walking `src` or writing to
/// `dst`.
pub fn copy_filtered(src: &Path, dst: &Path, harness: HarnessId) -> Result<CopyStats, Error> {
    let mut stats = CopyStats::default();
    if !src.exists() {
        return Ok(stats);
    }
    walk(src, src, dst, harness, &mut stats)?;
    Ok(stats)
}

fn walk(
    root: &Path,
    src: &Path,
    dst: &Path,
    harness: HarnessId,
    stats: &mut CopyStats,
) -> Result<(), Error> {
    let metadata = std::fs::metadata(src).map_err(|source| Error::Io {
        operation: "stat copy source".to_owned(),
        path: src.to_path_buf(),
        source,
    })?;

    if metadata.is_dir() {
        std::fs::create_dir_all(dst).map_err(|source| Error::Io {
            operation: "create destination dir".to_owned(),
            path: dst.to_path_buf(),
            source,
        })?;

        let entries = std::fs::read_dir(src).map_err(|source| Error::Io {
            operation: "read source dir".to_owned(),
            path: src.to_path_buf(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| Error::Io {
                operation: "read dir entry".to_owned(),
                path: src.to_path_buf(),
                source,
            })?;
            let entry_src = entry.path();
            let name = entry.file_name();
            let entry_dst = dst.join(&name);
            walk(root, &entry_src, &entry_dst, harness, stats)?;
        }
        return Ok(());
    }

    if metadata.is_file() {
        let relative = src.strip_prefix(root).unwrap_or(src);
        if deny_list::is_excluded(harness, relative) {
            stats.excluded = stats.excluded.saturating_add(1);
            stats.excluded_paths.push(relative.to_path_buf());
            return Ok(());
        }
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Error::Io {
                operation: "create dst parent".to_owned(),
                path: parent.to_path_buf(),
                source,
            })?;
        }
        std::fs::copy(src, dst).map_err(|source| Error::Io {
            operation: "copy file".to_owned(),
            path: src.to_path_buf(),
            source,
        })?;
        stats.copied = stats.copied.saturating_add(1);
        return Ok(());
    }

    // Symlinks / sockets / fifos: ignore.
    Ok(())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]
mod tests {
    use super::copy_filtered;
    use maru_core::HarnessId;

    #[test]
    fn copies_benign_files_excludes_credentials() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();

        std::fs::write(src.path().join("settings.json"), b"{}").unwrap();
        std::fs::write(src.path().join(".credentials.json"), b"SECRET").unwrap();
        std::fs::create_dir_all(src.path().join("projects")).unwrap();
        std::fs::write(src.path().join("projects").join("notes.md"), b"hello").unwrap();

        let stats = copy_filtered(src.path(), dst.path(), HarnessId::Claude).unwrap();

        assert_eq!(stats.copied, 2);
        assert_eq!(stats.excluded, 1);
        assert!(
            stats
                .excluded_paths
                .iter()
                .any(|p| p.ends_with(".credentials.json"))
        );

        assert!(dst.path().join("settings.json").exists());
        assert!(dst.path().join("projects/notes.md").exists());
        assert!(!dst.path().join(".credentials.json").exists());
    }

    #[test]
    fn missing_src_is_noop() {
        let dst = tempfile::tempdir().unwrap();
        let stats = copy_filtered(
            std::path::Path::new("/no/such/source/path"),
            dst.path(),
            HarnessId::Claude,
        )
        .unwrap();
        assert_eq!(stats.copied, 0);
        assert_eq!(stats.excluded, 0);
    }

    #[test]
    fn keychain_files_universally_excluded() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("MyKeychain.dat"), b"k").unwrap();
        std::fs::write(src.path().join("settings.json"), b"{}").unwrap();
        let stats = copy_filtered(src.path(), dst.path(), HarnessId::Codex).unwrap();
        assert_eq!(stats.excluded, 1);
        assert!(!dst.path().join("MyKeychain.dat").exists());
        assert!(dst.path().join("settings.json").exists());
    }
}
