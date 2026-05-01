//! Atomic file write primitive: write to a tempfile in the same directory,
//! `fsync`, then atomic-rename over the target.
//!
//! Used by [`State`](crate::State) and [`active_txt`](crate::active_txt).

use std::io::Write;
use std::path::Path;

use crate::Error;

/// Write `contents` to `path` atomically.
///
/// Steps:
/// 1. Create a tempfile in the same directory as `path` (so the rename
///    is in-filesystem and therefore atomic).
/// 2. Write all bytes.
/// 3. `fsync` the tempfile.
/// 4. `persist` (rename) over the target.
///
/// On success, the file at `path` either has the old contents (if a
/// crash happened before rename) or the new contents (if after). It is
/// never half-written.
pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), Error> {
    let parent = path.parent().ok_or_else(|| Error::Io {
        operation: "write_atomic: target has no parent".to_owned(),
        path: path.to_path_buf(),
        source: std::io::Error::other("no parent dir"),
    })?;

    std::fs::create_dir_all(parent).map_err(|source| Error::Io {
        operation: "create parent dir".to_owned(),
        path: parent.to_path_buf(),
        source,
    })?;

    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|source| Error::Io {
        operation: "create tempfile".to_owned(),
        path: parent.to_path_buf(),
        source,
    })?;

    tmp.write_all(contents).map_err(|source| Error::Io {
        operation: "write tempfile".to_owned(),
        path: tmp.path().to_path_buf(),
        source,
    })?;
    tmp.as_file().sync_all().map_err(|source| Error::Io {
        operation: "fsync tempfile".to_owned(),
        path: tmp.path().to_path_buf(),
        source,
    })?;

    tmp.persist(path).map_err(|e| Error::Io {
        operation: "persist tempfile".to_owned(),
        path: path.to_path_buf(),
        source: e.error,
    })?;

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
    use super::write_atomic;

    #[test]
    fn writes_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        write_atomic(&path, b"hello").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"hello");
    }

    #[test]
    fn overwrites_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        std::fs::write(&path, b"old").unwrap();
        write_atomic(&path, b"new").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
    }

    #[test]
    fn creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("deep").join("test.txt");
        write_atomic(&path, b"deep").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"deep");
    }
}
