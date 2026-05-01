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

    // ---- proptest: write_atomic is torn-write-free under concurrency ----

    use proptest::collection::vec;
    use proptest::prelude::{ProptestConfig, prop_assert, proptest};
    use std::collections::BTreeSet;
    use std::thread;

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 16,
            ..ProptestConfig::default()
        })]

        /// Many threads call `write_atomic` against the same target with
        /// distinct payloads. The final on-disk content must be exactly
        /// one of the per-thread payloads — never a torn or mixed write.
        #[test]
        fn prop_concurrent_writes_never_torn(
            payloads in vec(vec(0_u8..=255, 1..1024), 2..6),
        ) {
            let dir = tempfile::tempdir().unwrap();
            let target = dir.path().join("contents.bin");

            thread::scope(|scope| {
                for p in &payloads {
                    let target = &target;
                    scope.spawn(move || {
                        write_atomic(target, p).expect("write_atomic ok");
                    });
                }
            });

            let final_bytes = std::fs::read(&target).expect("file exists");
            // The final bytes must equal exactly one of the inputs.
            let inputs: BTreeSet<Vec<u8>> = payloads.into_iter().collect();
            prop_assert!(
                inputs.contains(&final_bytes),
                "final file is neither of the inputs: {} bytes",
                final_bytes.len(),
            );
        }
    }
}
