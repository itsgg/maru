//! `active.txt` — single-line file with the currently-active profile.
//!
//! GENESIS §10: "one line: <profile-name>; empty = no active". Reads are
//! lock-free; writes go through the same atomic-rename + advisory-lock
//! path as `state.toml`.

use std::path::Path;

use maru_core::ProfileName;

use crate::{Error, atomic, lock};

/// Read the active profile name from `<maru_home>/active.txt`.
///
/// Returns `Ok(None)` if the file does not exist or its first non-empty
/// line is empty/whitespace, OR if the content does not parse as a valid
/// [`ProfileName`] (resolution should silently fall through to the next
/// source per GENESIS §10 — corrupted active.txt should not exit-1 the
/// shim).
///
/// # Errors
///
/// Returns [`Error::Io`] on read errors other than `NotFound`.
pub fn read(maru_home: &Path) -> Result<Option<ProfileName>, Error> {
    let path = maru_home.join("active.txt");
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(Error::Io {
                operation: "read active.txt".to_owned(),
                path,
                source,
            });
        }
    };
    let text = String::from_utf8_lossy(&bytes);
    Ok(parse_first_line(&text))
}

/// Write the active profile name. `None` writes an empty file (the
/// canonical "no active profile" representation).
///
/// # Errors
///
/// Returns [`Error::Io`] on lock or write failure.
pub fn write(maru_home: &Path, name: Option<&ProfileName>) -> Result<(), Error> {
    lock::with_write_lock(maru_home, || {
        let path = maru_home.join("active.txt");
        let contents = name.map_or(String::new(), |n| format!("{}\n", n.as_str()));
        atomic::write_atomic(&path, contents.as_bytes())
    })
}

fn parse_first_line(text: &str) -> Option<ProfileName> {
    let line = text.lines().next()?.trim();
    if line.is_empty() {
        return None;
    }
    ProfileName::new(line).ok()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "tests"
)]
mod tests {
    use super::{read, write};
    use maru_core::ProfileName;

    #[test]
    fn returns_none_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read(dir.path()).unwrap().is_none());
    }

    #[test]
    fn returns_none_when_empty() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("active.txt"), "").unwrap();
        assert!(read(dir.path()).unwrap().is_none());
    }

    #[test]
    fn returns_none_when_whitespace_only() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("active.txt"), "   \n\n").unwrap();
        assert!(read(dir.path()).unwrap().is_none());
    }

    #[test]
    fn returns_first_line_as_profile_name() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("active.txt"), "work\nignored\n").unwrap();
        assert_eq!(read(dir.path()).unwrap().unwrap().as_str(), "work");
    }

    #[test]
    fn ignores_invalid_name() {
        // Garbage in active.txt should fall through to next resolution
        // source rather than crash the shim.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("active.txt"), "-bad name\n").unwrap();
        assert!(read(dir.path()).unwrap().is_none());
    }

    #[test]
    fn write_then_read_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let name = ProfileName::new("personal").unwrap();
        write(dir.path(), Some(&name)).unwrap();
        assert_eq!(read(dir.path()).unwrap().unwrap(), name);
    }

    #[test]
    fn write_none_clears() {
        let dir = tempfile::tempdir().unwrap();
        let name = ProfileName::new("work").unwrap();
        write(dir.path(), Some(&name)).unwrap();
        write(dir.path(), None).unwrap();
        assert!(read(dir.path()).unwrap().is_none());
    }
}
